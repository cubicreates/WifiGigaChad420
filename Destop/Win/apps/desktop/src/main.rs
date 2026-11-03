use anyhow::Result;
use clap::Parser;
use giga_inr::InrEngine;
use giga_mesh::MeshBridgeEngine;
use giga_pal::create_platform_manager;
use giga_transport::{MultiPathDownloader, ProxyGatewayConfig, SystemProxyGateway};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const EMBEDDED_UI_HTML: &str = include_str!("ui.html");

#[derive(Parser, Debug)]
#[command(
    name = "giga-desktop",
    version,
    about = "WiFi GigaChad 420 — Windows Desktop Control Center & Background Daemon"
)]
struct DesktopArgs {
    /// Local port for the Desktop Control Center UI & Telemetry Server
    #[arg(long, default_value_t = 4200)]
    ui_port: u16,

    /// Do not automatically spawn a desktop WebView/browser window on startup
    #[arg(long)]
    headless: bool,
}

#[derive(Clone)]
struct AppContext {
    inr: InrEngine,
    proxy: SystemProxyGateway,
    downloader: MultiPathDownloader,
    mesh: MeshBridgeEngine,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = DesktopArgs::parse();

    let inr = InrEngine::new();
    let _ = inr.refresh_and_probe(Duration::from_millis(2500)).await;

    let proxy_cfg = ProxyGatewayConfig::default();
    let proxy = SystemProxyGateway::new(proxy_cfg, inr.clone());
    let downloader = MultiPathDownloader::new(inr.clone());
    let mesh = MeshBridgeEngine::new("Windows-GigaChad-Laptop");

    // 1. Spawn background SOCKS5 (:1080) & HTTP CONNECT (:1081) System Proxy
    let proxy_bg = proxy.clone();
    tokio::spawn(async move {
        let _ = proxy_bg.run().await;
    });

    // 2. Spawn background Cross-Device Phone <-> Laptop Wi-Fi Mesh Discovery (:42069) & Tunnel (:42070)
    mesh.spawn_background_service();

    // 3. Spawn periodic INR active probe & 802.11 telemetry refresh loop
    let inr_bg = inr.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(4));
        loop {
            interval.tick().await;
            let _ = inr_bg.refresh_and_probe(Duration::from_millis(2000)).await;
        }
    });

    let ui_addr = SocketAddr::from(([127, 0, 0, 1], args.ui_port));
    let listener = TcpListener::bind(ui_addr).await?;
    let url = format!("http://{}", ui_addr);

    println!("================================================================================");
    println!(" WIFI GIGACHAD 420 — WINDOWS DESKTOP CONTROL CENTER & DAEMON");
    println!("================================================================================");
    println!("  Control Center UI : {}", url);
    println!("  SOCKS5 Proxy      : 127.0.0.1:1080 (Happy Eyeballs v3 Hedged)");
    println!("  HTTP Proxy        : 127.0.0.1:1081 (Happy Eyeballs v3 Hedged)");
    println!("  Wi-Fi Mesh Bridge : UDP :42069 / TCP :42070 (PIN: {})", mesh.pairing_pin().await);
    println!("================================================================================");

    if !args.headless {
        launch_native_app_window(&url);
    }

    let ctx = AppContext {
        inr,
        proxy,
        downloader,
        mesh,
    };

    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };
        let ctx_clone = ctx.clone();
        tokio::spawn(async move {
            let _ = handle_ui_http_connection(stream, ctx_clone).await;
        });
    }
}

fn launch_native_app_window(url: &str) {
    #[cfg(windows)]
    {
        // Try launching Microsoft Edge in native chromeless App-Window mode first, fallback to system browser
        let edge_launched = std::process::Command::new("cmd")
            .args(["/C", "start", "", "msedge", &format!("--app={}", url)])
            .spawn()
            .is_ok();
        if !edge_launched {
            let _ = std::process::Command::new("cmd")
                .args(["/C", "start", "", url])
                .spawn();
        }
    }
}

async fn handle_ui_http_connection(mut stream: TcpStream, ctx: AppContext) -> Result<()> {
    let mut buf = [0u8; 8192];
    let n = stream.read(&mut buf).await?;
    if n == 0 {
        return Ok(());
    }

    let req_str = String::from_utf8_lossy(&buf[..n]);
    let Some(first_line) = req_str.lines().next() else {
        return Ok(());
    };
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() < 2 {
        return Ok(());
    }

    let method = parts[0];
    let path = parts[1];
    let body_str = req_str.split("\r\n\r\n").nth(1).unwrap_or("");

    if method == "GET" && (path == "/" || path == "/index.html") {
        write_http_response(&mut stream, 200, "text/html; charset=utf-8", EMBEDDED_UI_HTML.as_bytes()).await?;
        return Ok(());
    }

    if method == "GET" && path == "/api/status" {
        let interfaces = ctx.inr.snapshots().await;
        let proxy = ctx.proxy.snapshot();
        let download = ctx.downloader.latest_progress().await;
        let peers = ctx.mesh.discovered_peers().await;
        let pin = ctx.mesh.pairing_pin().await;

        let payload = serde_json::json!({
            "interfaces": interfaces,
            "proxy": proxy,
            "download": download,
            "mesh": {
                "local_peer_id": ctx.mesh.local_peer_id(),
                "local_device_name": ctx.mesh.local_device_name(),
                "pairing_pin": pin,
                "peers": peers,
            }
        });
        let bytes = serde_json::to_vec(&payload)?;
        write_http_response(&mut stream, 200, "application/json", &bytes).await?;
        return Ok(());
    }

    if method == "POST" && path == "/api/probe" {
        let snaps = ctx.inr.refresh_and_probe(Duration::from_millis(2500)).await;
        let bytes = serde_json::to_vec(&snaps)?;
        write_http_response(&mut stream, 200, "application/json", &bytes).await?;
        return Ok(());
    }

    if method == "GET" && path.starts_with("/api/dns-race") {
        let host = path
            .split("host=")
            .nth(1)
            .unwrap_or("github.com")
            .split('&')
            .next()
            .unwrap_or("github.com");
        let decoded_host = host.replace("%3A", ":").replace("%2F", "/");
        let pal = create_platform_manager();
        let ifaces = pal.enumerate_wan_candidates().unwrap_or_default();

        let payload = match ctx
            .inr
            .dns()
            .resolve_v4(&decoded_host, &ifaces, Duration::from_millis(2500))
            .await
        {
            Ok(res) => serde_json::to_vec(&res)?,
            Err(e) => serde_json::to_vec(&serde_json::json!({ "error": e }))?,
        };
        write_http_response(&mut stream, 200, "application/json", &payload).await?;
        return Ok(());
    }

    if method == "POST" && path == "/api/download" {
        #[derive(serde::Deserialize)]
        struct DlReq {
            url: String,
            output: String,
            workers_per_if: Option<usize>,
        }
        if let Ok(req) = serde_json::from_str::<DlReq>(body_str) {
            let dl = ctx.downloader.clone();
            let out_path = PathBuf::from(&req.output);
            let workers = req.workers_per_if.unwrap_or(4);
            tokio::spawn(async move {
                let _ = dl.download_url(&req.url, &out_path, workers).await;
            });
            write_http_response(
                &mut stream,
                200,
                "application/json",
                b"{\"started\":true}",
            )
            .await?;
            return Ok(());
        }
    }

    if method == "POST" && path == "/api/mesh-scan" {
        let peers = ctx.mesh.broadcast_and_scan(Duration::from_millis(800)).await;
        let bytes = serde_json::to_vec(&peers)?;
        write_http_response(&mut stream, 200, "application/json", &bytes).await?;
        return Ok(());
    }

    if method == "POST" && path == "/api/mesh-pin" {
        #[derive(serde::Deserialize)]
        struct PinReq {
            pin: String,
        }
        if let Ok(req) = serde_json::from_str::<PinReq>(body_str) {
            ctx.mesh.set_pairing_pin(req.pin).await;
        }
        write_http_response(&mut stream, 200, "application/json", b"{\"ok\":true}").await?;
        return Ok(());
    }

    write_http_response(&mut stream, 404, "text/plain", b"Not Found").await?;
    Ok(())
}

async fn write_http_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<()> {
    let status_text = match status {
        200 => "OK",
        404 => "Not Found",
        _ => "Error",
    };
    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        status,
        status_text,
        content_type,
        body.len()
    );
    stream.write_all(header.as_bytes()).await?;
    stream.write_all(body).await?;
    Ok(())
}
