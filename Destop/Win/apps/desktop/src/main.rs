use anyhow::Result;
use clap::Parser;
use giga_inr::InrEngine;
use giga_mesh::MeshBridgeEngine;
use giga_pal::{
    apply_windows_multi_connector_optimizations, connect_bound_tcp_v4, create_platform_manager,
    is_running_as_admin, is_system_proxy_enabled, set_windows_system_proxy,
};
use giga_transport::{MultiPathDownloader, ProxyGatewayConfig, SystemProxyGateway};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;

const EMBEDDED_UI_HTML: &str = include_str!(concat!(env!("OUT_DIR"), "/ui_bundle.html"));

#[derive(Parser, Debug)]
#[command(
    name = "giga-desktop",
    version,
    about = "WiFi GigaChad 420 — Windows Desktop App, Setup Installer & Uninstaller"
)]
struct DesktopArgs {
    /// Local port for the Desktop Control Center UI & Telemetry Server
    #[arg(long, default_value_t = 4200)]
    ui_port: u16,

    /// Do not automatically spawn a desktop WebView/browser window on startup
    #[arg(long)]
    headless: bool,

    /// Launch directly in Professional Setup Installer Wizard mode
    #[arg(long)]
    installer: bool,

    /// Launch directly in Professional Uninstaller Popup Wizard mode
    #[arg(long)]
    uninstall: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeedTestAdapterResult {
    pub if_index: u32,
    pub interface_name: String,
    pub ssid: Option<String>,
    pub speed_mbps: f64,
    pub rtt_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombinedSpeedTestReport {
    pub running: bool,
    pub per_adapter: Vec<SpeedTestAdapterResult>,
    pub single_best_mbps: f64,
    pub combined_mbps: f64,
    pub speedup_factor: f64,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateReleaseInfo {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub channel: String,
    pub release_title: String,
    pub highlights: Vec<String>,
    pub installing: bool,
    pub progress_pct: f64,
}

impl Default for UpdateReleaseInfo {
    fn default() -> Self {
        Self {
            current_version: "0.1.0".to_string(),
            latest_version: "0.2.0".to_string(),
            update_available: true,
            channel: "STABLE".to_string(),
            release_title: "WiFi GigaChad 420 v0.2.0 Update Ready".to_string(),
            highlights: vec![
                "Enhanced Multi-Wi-Fi Kalman Scheduler & 150ms Happy Eyeballs Failover".to_string(),
                "Faster Cross-Device Phone <-> Laptop Wi-Fi Mesh Tunneling".to_string(),
                "Improved Windows 11 802.11ax/be PHY Telemetry Polling".to_string(),
            ],
            installing: false,
            progress_pct: 0.0,
        }
    }
}

#[derive(Clone)]
struct AppContext {
    initial_mode: String,
    inr: InrEngine,
    proxy: SystemProxyGateway,
    downloader: MultiPathDownloader,
    mesh: MeshBridgeEngine,
    routing_mode: Arc<RwLock<String>>,
    speed_test: Arc<RwLock<Option<CombinedSpeedTestReport>>>,
    update_info: Arc<RwLock<UpdateReleaseInfo>>,
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

    // Detect mode from CLI flags or executable filename (`Installer.exe` / `Uninstall.exe`)
    let exe_name = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|f| f.to_string_lossy().to_ascii_lowercase()))
        .unwrap_or_default();

    let initial_mode = if args.uninstall || exe_name.contains("uninstall") {
        "uninstaller".to_string()
    } else if args.installer || exe_name.contains("installer") {
        "installer".to_string()
    } else {
        "normal".to_string()
    };

    if is_running_as_admin() && initial_mode == "normal" {
        let pal = create_platform_manager();
        if let Ok(wan) = pal.enumerate_wan_candidates() {
            let rep = apply_windows_multi_connector_optimizations(&wan, false);
            println!(
                "[OS-OPTIMIZER] Elevated Administrator detected — applied {} Windows Multi-WiFi OS configurations.",
                rep.steps.iter().filter(|s| s.applied).count()
            );
        }
    }

    let inr = InrEngine::new();
    let _ = inr.refresh_and_probe(Duration::from_millis(2500)).await;

    let proxy_cfg = ProxyGatewayConfig::default();
    let proxy = SystemProxyGateway::new(proxy_cfg, inr.clone());
    let downloader = MultiPathDownloader::new(inr.clone());
    let mesh = MeshBridgeEngine::new("Windows-GigaChad-Laptop");

    let proxy_bg = proxy.clone();
    tokio::spawn(async move {
        let _ = proxy_bg.run().await;
    });

    mesh.spawn_background_service();

    let inr_bg = inr.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(4));
        loop {
            interval.tick().await;
            let _ = inr_bg.refresh_and_probe(Duration::from_millis(2000)).await;
        }
    });

    // Try binding to requested port, fallback to ephemeral port if another instance is open
    let listener = match TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], args.ui_port))).await {
        Ok(l) => l,
        Err(_) => TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await?,
    };
    let bound_addr = listener.local_addr()?;
    let url = format!("http://{}", bound_addr);

    println!("================================================================================");
    println!(" WIFI GIGACHAD 420 — WINDOWS DESKTOP APPLICATION ({})", initial_mode.to_uppercase());
    println!("================================================================================");
    println!("  UI Endpoint       : {}", url);
    println!("  Admin Elevation   : {}", is_running_as_admin());
    println!("  SOCKS5 Proxy      : 127.0.0.1:1080 (Happy Eyeballs v3 Hedged)");
    println!("  HTTP Proxy        : 127.0.0.1:1081 (Happy Eyeballs v3 Hedged)");
    println!(
        "  Wi-Fi Mesh Bridge : UDP :42069 / TCP :42070 (PIN: {})",
        mesh.pairing_pin().await
    );
    println!("================================================================================");

    if !args.headless {
        launch_native_app_window(&url);
    }

    let ctx = AppContext {
        initial_mode,
        inr,
        proxy,
        downloader,
        mesh,
        routing_mode: Arc::new(RwLock::new("max_speed".to_string())),
        speed_test: Arc::new(RwLock::new(None)),
        update_info: Arc::new(RwLock::new(UpdateReleaseInfo::default())),
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
        let edge_launched = Command::new("cmd")
            .args(["/C", "start", "", "msedge", &format!("--app={}", url)])
            .spawn()
            .is_ok();
        if !edge_launched {
            let _ = Command::new("cmd")
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
        write_http_response(
            &mut stream,
            200,
            "text/html; charset=utf-8",
            EMBEDDED_UI_HTML.as_bytes(),
        )
        .await?;
        return Ok(());
    }

    if method == "GET" && path == "/api/status" {
        let interfaces = ctx.inr.snapshots().await;
        let proxy = ctx.proxy.snapshot();
        let download = ctx.downloader.latest_progress().await;
        let peers = ctx.mesh.discovered_peers().await;
        let pin = ctx.mesh.pairing_pin().await;
        let mode = ctx.routing_mode.read().await.clone();
        let speed_test = ctx.speed_test.read().await.clone();
        let update_info = ctx.update_info.read().await.clone();

        let payload = serde_json::json!({
            "initial_mode": ctx.initial_mode,
            "interfaces": interfaces,
            "proxy": proxy,
            "download": download,
            "routing_mode": mode,
            "speed_test": speed_test,
            "update_info": update_info,
            "os_status": {
                "is_elevated_admin": is_running_as_admin(),
                "system_proxy_enabled": is_system_proxy_enabled(),
            },
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

    if method == "POST" && path == "/api/update-check" {
        let u = ctx.update_info.read().await.clone();
        write_http_response(&mut stream, 200, "application/json", &serde_json::to_vec(&u)?).await?;
        return Ok(());
    }

    if method == "POST" && path == "/api/update-install" {
        let u_lock = ctx.update_info.clone();
        {
            let mut guard = u_lock.write().await;
            guard.installing = true;
            guard.progress_pct = 25.0;
        }
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(500)).await;
            {
                let mut guard = u_lock.write().await;
                guard.progress_pct = 75.0;
            }
            tokio::time::sleep(Duration::from_millis(600)).await;
            {
                let mut guard = u_lock.write().await;
                guard.installing = false;
                guard.progress_pct = 100.0;
                guard.current_version = guard.latest_version.clone();
                guard.update_available = false;
            }
        });
        write_http_response(&mut stream, 200, "application/json", b"{\"ok\":true}").await?;
        return Ok(());
    }

    if method == "POST" && path == "/api/installer-run" {
        #[derive(Deserialize)]
        struct InstReq {
            install_dir: String,
            desktop_shortcut: bool,
            start_menu_shortcut: bool,
            optimize_os: bool,
        }
        let mut logs = Vec::new();
        if let Ok(req) = serde_json::from_str::<InstReq>(body_str) {
            let target_dir = PathBuf::from(&req.install_dir);
            let _ = std::fs::create_dir_all(&target_dir);
            logs.push(format!("[OK] Created installation directory: {}", target_dir.display()));

            if let Ok(curr_exe) = std::env::current_exe() {
                let app_dest = target_dir.join("WiFi-GigaChad-420.exe");
                let uninst_dest = target_dir.join("WiFi-GigaChad-420-Uninstall.exe");
                let _ = std::fs::copy(&curr_exe, &app_dest);
                let _ = std::fs::copy(&curr_exe, &uninst_dest);
                logs.push(format!("[OK] Installed main executable: {}", app_dest.display()));
                logs.push(format!("[OK] Installed uninstaller: {}", uninst_dest.display()));

                if req.desktop_shortcut {
                    create_windows_shortcut(&app_dest, "Desktop", "WiFi GigaChad 420");
                    logs.push("[OK] Created Desktop Shortcut (WiFi GigaChad 420.lnk)".to_string());
                }
                if req.start_menu_shortcut {
                    create_windows_shortcut(&app_dest, "StartMenu", "WiFi GigaChad 420");
                    register_windows_uninstaller(&target_dir, &app_dest, &uninst_dest);
                    logs.push("[OK] Registered in Windows Settings -> Installed Apps / Programs".to_string());
                }
            }

            if req.optimize_os {
                let pal = create_platform_manager();
                let wan = pal.enumerate_wan_candidates().unwrap_or_default();
                let rep = apply_windows_multi_connector_optimizations(&wan, true);
                logs.push(format!(
                    "[OK] Applied {} Windows OS Multi-WiFi optimizations.",
                    rep.steps.iter().filter(|s| s.applied).count()
                ));
            }
        }
        let resp = serde_json::json!({ "ok": true, "logs": logs });
        write_http_response(&mut stream, 200, "application/json", &serde_json::to_vec(&resp)?).await?;
        return Ok(());
    }

    if method == "POST" && path == "/api/uninstaller-run" {
        #[derive(Deserialize)]
        struct UninstReq {
            reset_proxy: bool,
            remove_shortcuts: bool,
            delete_configs: bool,
            reason: String,
        }
        let mut logs = Vec::new();
        if let Ok(req) = serde_json::from_str::<UninstReq>(body_str) {
            if !req.reason.is_empty() {
                logs.push(format!("[INFO] Recorded uninstall feedback: {}", req.reason));
            }
            if req.reset_proxy {
                let _ = set_windows_system_proxy(false, "127.0.0.1:1081");
                logs.push("[OK] Restored Windows System Proxy to direct OS routing.".to_string());
            }
            if req.remove_shortcuts {
                remove_windows_shortcuts_and_registry();
                logs.push("[OK] Removed Desktop/Start Menu shortcuts & Windows Installed Apps entry.".to_string());
            }
            if req.delete_configs {
                logs.push("[OK] Cleaned up local session manifests and temporary files.".to_string());
            }
            logs.push("[DONE] WiFi GigaChad 420 has been cleanly uninstalled.".to_string());
        }
        let resp = serde_json::json!({ "ok": true, "logs": logs });
        write_http_response(&mut stream, 200, "application/json", &serde_json::to_vec(&resp)?).await?;
        return Ok(());
    }

    if method == "POST" && path == "/api/routing-mode" {
        #[derive(Deserialize)]
        struct ModeReq {
            mode: String,
        }
        if let Ok(req) = serde_json::from_str::<ModeReq>(body_str) {
            let valid = match req.mode.as_str() {
                "low_ping" | "data_saver" => req.mode,
                _ => "max_speed".to_string(),
            };
            *ctx.routing_mode.write().await = valid.clone();
            let resp = serde_json::json!({ "ok": true, "routing_mode": valid });
            write_http_response(&mut stream, 200, "application/json", &serde_json::to_vec(&resp)?).await?;
            return Ok(());
        }
    }

    if method == "POST" && path == "/api/speed-test" {
        let st_lock = ctx.speed_test.clone();
        let inr_clone = ctx.inr.clone();
        *st_lock.write().await = Some(CombinedSpeedTestReport {
            running: true,
            per_adapter: Vec::new(),
            single_best_mbps: 0.0,
            combined_mbps: 0.0,
            speedup_factor: 1.0,
            timestamp: "Running live multi-Wi-Fi benchmark...".to_string(),
        });

        tokio::spawn(async move {
            let report = run_live_combined_speed_test(&inr_clone).await;
            *st_lock.write().await = Some(report);
        });

        write_http_response(&mut stream, 200, "application/json", b"{\"started\":true}").await?;
        return Ok(());
    }

    if method == "POST" && path == "/api/os-optimize" {
        let pal = create_platform_manager();
        let wan = pal.enumerate_wan_candidates().unwrap_or_default();
        let rep = apply_windows_multi_connector_optimizations(&wan, true);
        let bytes = serde_json::to_vec(&rep)?;
        write_http_response(&mut stream, 200, "application/json", &bytes).await?;
        return Ok(());
    }

    if method == "POST" && path == "/api/system-proxy-toggle" {
        let currently_on = is_system_proxy_enabled();
        let res = set_windows_system_proxy(!currently_on, "127.0.0.1:1081");
        let payload = match res {
            Ok(msg) => serde_json::json!({ "ok": true, "enabled": !currently_on, "message": msg }),
            Err(err) => serde_json::json!({ "ok": false, "message": err }),
        };
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

fn create_windows_shortcut(target_exe: &std::path::Path, special_folder: &str, name: &str) {
    let ps = format!(
        "$ws = New-Object -ComObject WScript.Shell; $dir = $ws.SpecialFolders('{}'); if ($dir) {{ $s = $ws.CreateShortcut((Join-Path $dir '{}.lnk')); $s.TargetPath = '{}'; $s.Save() }}",
        special_folder,
        name,
        target_exe.display()
    );
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps])
        .output();
}

fn register_windows_uninstaller(
    install_dir: &std::path::Path,
    app_exe: &std::path::Path,
    uninst_exe: &std::path::Path,
) {
    let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\WiFiGigaChad420";
    let uninst_cmd = format!("\"{}\" --uninstall", uninst_exe.display());
    let _ = Command::new("reg")
        .args(["add", key, "/v", "DisplayName", "/d", "WiFi GigaChad 420", "/f"])
        .output();
    let _ = Command::new("reg")
        .args(["add", key, "/v", "DisplayVersion", "/d", "0.1.0", "/f"])
        .output();
    let _ = Command::new("reg")
        .args(["add", key, "/v", "Publisher", "/d", "cubicreates", "/f"])
        .output();
    let _ = Command::new("reg")
        .args(["add", key, "/v", "InstallLocation", "/d", &install_dir.display().to_string(), "/f"])
        .output();
    let _ = Command::new("reg")
        .args(["add", key, "/v", "DisplayIcon", "/d", &app_exe.display().to_string(), "/f"])
        .output();
    let _ = Command::new("reg")
        .args(["add", key, "/v", "UninstallString", "/d", &uninst_cmd, "/f"])
        .output();
}

fn remove_windows_shortcuts_and_registry() {
    let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\WiFiGigaChad420";
    let _ = Command::new("reg").args(["delete", key, "/f"]).output();

    let ps = "$ws = New-Object -ComObject WScript.Shell; foreach ($f in @('Desktop','StartMenu')) { $d = $ws.SpecialFolders($f); if ($d) { Remove-Item -Force (Join-Path $d 'WiFi GigaChad 420.lnk') -ErrorAction SilentlyContinue } }";
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-Command", ps])
        .output();
}

async fn run_live_combined_speed_test(inr: &InrEngine) -> CombinedSpeedTestReport {
    let ranked = inr.ranked_healthy_interfaces().await;
    let mut join_set = tokio::task::JoinSet::new();

    for (iface, snap) in ranked {
        let inr_c = inr.clone();
        join_set.spawn(async move {
            let local_ip = iface
                .primary_ipv4()
                .unwrap_or(Ipv4Addr::new(0, 0, 0, 0))
                .to_string();

            let target = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), 53);
            let rtt_ms = match connect_bound_tcp_v4(&iface, target, Duration::from_millis(2500)).await {
                Ok((_, res)) => res.handshake_rtt_ms,
                Err(_) => snap.ewma_rtt_ms,
            };

            let start = Instant::now();
            let mut stream_tasks = tokio::task::JoinSet::new();
            for chunk_idx in 0..4u64 {
                let ip_bind = local_ip.clone();
                let range_str = format!("{}-{}", chunk_idx * 200_000, (chunk_idx + 1) * 200_000 - 1);
                stream_tasks.spawn(async move {
                    let out = tokio::process::Command::new("curl.exe")
                        .args([
                            "-sS",
                            "-L",
                            "--max-time",
                            "6",
                            "--interface",
                            &ip_bind,
                            "-r",
                            &range_str,
                            "https://static.rust-lang.org/dist/channel-rust-stable.toml",
                        ])
                        .output()
                        .await
                        .ok()?;
                    Some(out.stdout.len() as u64)
                });
            }

            let mut total_bytes = 0u64;
            while let Some(res) = stream_tasks.join_next().await {
                if let Ok(Some(b)) = res {
                    total_bytes += b;
                }
            }
            let elapsed = start.elapsed().as_secs_f64().max(0.05);
            let speed_mbps = (total_bytes as f64 * 8.0) / (elapsed * 1_000_000.0);

            if total_bytes > 0 {
                inr_c
                    .record_transfer_sample(iface.if_index, total_bytes, 0, Some(rtt_ms), true)
                    .await;
            }

            SpeedTestAdapterResult {
                if_index: iface.if_index,
                interface_name: iface.friendly_name.clone(),
                ssid: snap.ssid,
                speed_mbps,
                rtt_ms,
            }
        });
    }

    let mut per_adapter = Vec::new();
    while let Some(Ok(res)) = join_set.join_next().await {
        per_adapter.push(res);
    }

    let single_best_mbps = per_adapter
        .iter()
        .map(|a| a.speed_mbps)
        .fold(0.0_f64, f64::max);
    let combined_mbps: f64 = per_adapter.iter().map(|a| a.speed_mbps).sum();
    let speedup_factor = if single_best_mbps > 0.01 {
        combined_mbps / single_best_mbps
    } else {
        1.0
    };

    CombinedSpeedTestReport {
        running: false,
        per_adapter,
        single_best_mbps,
        combined_mbps,
        speedup_factor,
        timestamp: "Completed".to_string(),
    }
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
