use anyhow::Result;
use clap::{Parser, Subcommand};
use giga_inr::{HealthScoreSnapshot, InrEngine};
use giga_mesh::MeshBridgeEngine;
use giga_pal::{
    apply_windows_multi_connector_optimizations, connect_bound_tcp_v4, create_platform_manager,
    verify_pool_independence, InterfaceSnapshot, InterfaceType,
};
use giga_transport::{MultiPathDownloader, ProxyGatewayConfig, SystemProxyGateway};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(
    name = "giga-cli",
    version,
    about = "WiFi GigaChad 420 — Windows Multi-Interface Network Aggregator & Resolver CLI"
)]
struct Cli {
    /// Emit output in machine-readable JSON format
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Discover and inspect all network interfaces, IP/Gateway config, and 802.11 Wi-Fi telemetry
    Interfaces {
        /// Show all interfaces including loopback, virtual, and disconnected adapters
        #[arg(long)]
        all: bool,
    },
    /// Probe all WAN-eligible interfaces using bound sockets (IP_UNICAST_IF) to verify gateway & WAN IP independence
    VerifyWan {
        /// Probe timeout in milliseconds per interface
        #[arg(long, default_value_t = 3500)]
        timeout_ms: u64,
    },
    /// Test strict pre-connect socket binding (IP_UNICAST_IF + bind) on a specific interface index
    BindTest {
        /// OS Interface Index (if_index) to bind the outbound TCP socket to
        #[arg(short, long)]
        if_index: u32,

        /// Remote target address (IPv4:Port) to connect to
        #[arg(short, long, default_value = "8.8.8.8:53")]
        target: SocketAddr,

        /// Connection timeout in milliseconds
        #[arg(long, default_value_t = 3500)]
        timeout_ms: u64,
    },
    /// Race parallel DNS A-record queries across all active interfaces (UDP + TCP :53)
    DnsRace {
        /// Hostname to resolve (e.g., github.com, cloudflare.com)
        hostname: String,

        /// Timeout in milliseconds
        #[arg(long, default_value_t = 3000)]
        timeout_ms: u64,
    },
    /// Download a file using Multi-Path HTTP/HTTPS Byte-Range Striping, Kalman Scheduling & Work-Stealing
    Download {
        /// HTTP or HTTPS URL to download
        url: String,

        /// Output file path on disk
        #[arg(short, long)]
        output: PathBuf,

        /// Concurrent bound socket workers per active network interface
        #[arg(short, long, default_value_t = 4)]
        workers_per_if: usize,
    },
    /// Start the System-Wide Local SOCKS5 (127.0.0.1:1080) & HTTP CONNECT (127.0.0.1:1081) Proxy with Happy Eyeballs v3 Hedging
    Proxy {
        /// SOCKS5 bind address
        #[arg(long, default_value = "127.0.0.1:1080")]
        socks5_addr: SocketAddr,

        /// HTTP CONNECT / forward proxy bind address
        #[arg(long, default_value = "127.0.0.1:1081")]
        http_addr: SocketAddr,

        /// Happy Eyeballs v3 connection hedge delay in milliseconds
        #[arg(long, default_value_t = 150)]
        hedge_delay_ms: u64,
    },
    /// Broadcast a Cross-Device Phone <-> Laptop Wi-Fi Mesh beacon (UDP :42069) and scan for peers
    MeshScan {
        /// Listen duration in milliseconds
        #[arg(long, default_value_t = 2000)]
        listen_ms: u64,
    },
    /// Request Windows UAC Administrator access and apply OS Multi-WiFi / Multi-Connector modifications
    OsOptimize {
        /// Prompt for Windows UAC elevation if not already running as Administrator
        #[arg(long, default_value_t = true)]
        uac: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Interfaces { all: false }) {
        Commands::Interfaces { all } => cmd_interfaces(all, cli.json)?,
        Commands::VerifyWan { timeout_ms } => {
            cmd_verify_wan(Duration::from_millis(timeout_ms), cli.json).await?
        }
        Commands::BindTest {
            if_index,
            target,
            timeout_ms,
        } => cmd_bind_test(if_index, target, Duration::from_millis(timeout_ms), cli.json).await?,
        Commands::DnsRace {
            hostname,
            timeout_ms,
        } => cmd_dns_race(&hostname, Duration::from_millis(timeout_ms), cli.json).await?,
        Commands::Download {
            url,
            output,
            workers_per_if,
        } => cmd_download(&url, &output, workers_per_if, cli.json).await?,
        Commands::Proxy {
            socks5_addr,
            http_addr,
            hedge_delay_ms,
        } => cmd_proxy(socks5_addr, http_addr, hedge_delay_ms).await?,
        Commands::MeshScan { listen_ms } => {
            cmd_mesh_scan(Duration::from_millis(listen_ms), cli.json).await?
        }
        Commands::OsOptimize { uac } => cmd_os_optimize(uac, cli.json)?,
    }

    Ok(())
}

fn cmd_interfaces(show_all: bool, json_output: bool) -> Result<()> {
    let pal = create_platform_manager();
    let interfaces = if show_all {
        pal.enumerate_interfaces()?
    } else {
        let wan = pal.enumerate_wan_candidates()?;
        if wan.is_empty() {
            pal.enumerate_interfaces()?
        } else {
            wan
        }
    };

    if json_output {
        println!("{}", serde_json::to_string_pretty(&interfaces)?);
        return Ok(());
    }

    println!("================================================================================");
    println!(" WIFI GIGACHAD 420 — PAL INTERFACE DISCOVERY & 802.11 TELEMETRY (WINDOWS)");
    println!("================================================================================");
    println!(
        " Discovered {} interface(s) ({})",
        interfaces.len(),
        if show_all {
            "showing all adapters"
        } else {
            "showing active WAN-capable adapters; use --all for virtual/disconnected"
        }
    );
    println!("--------------------------------------------------------------------------------");

    for iface in &interfaces {
        print_interface_card(iface);
    }

    Ok(())
}

fn print_interface_card(iface: &InterfaceSnapshot) {
    let wan_badge = if iface.is_wan_eligible() {
        "WAN-ELIGIBLE"
    } else {
        "NON-WAN / STANDBY"
    };

    println!(
        "[if_index: {:>2}] {} ({}) — [{}]",
        iface.if_index, iface.friendly_name, iface.description, wan_badge
    );
    println!("  Adapter ID / GUID : {}", iface.id);
    println!(
        "  Medium & Status   : {} | State: {} | OperStatusUp: {} | Virtual/Loopback: {} | Metered: {}",
        iface.if_type.short_label(),
        iface.state,
        iface.is_up,
        iface.is_virtual_or_loopback,
        iface.is_metered
    );
    println!("  MAC Address       : {}", iface.mac_address);
    println!(
        "  Link Speed        : Tx {} Mbps / Rx {} Mbps",
        iface.tx_link_speed_bps / 1_000_000,
        iface.rx_link_speed_bps / 1_000_000
    );

    let ipv4_list: Vec<String> = iface.ipv4_addresses.iter().map(|ip| ip.to_string()).collect();
    let gw_list: Vec<String> = iface.gateways.iter().map(|ip| ip.to_string()).collect();
    let dns_list: Vec<String> = iface.dns_servers.iter().map(|ip| ip.to_string()).collect();

    println!(
        "  IPv4 Addresses    : {}",
        if ipv4_list.is_empty() {
            "None".to_string()
        } else {
            ipv4_list.join(", ")
        }
    );
    println!(
        "  Default Gateways  : {}",
        if gw_list.is_empty() {
            "None".to_string()
        } else {
            gw_list.join(", ")
        }
    );
    println!(
        "  DNS Servers       : {}",
        if dns_list.is_empty() {
            "None".to_string()
        } else {
            dns_list.join(", ")
        }
    );

    if let InterfaceType::WiFi(Some(wifi)) = &iface.if_type {
        println!(
            "  802.11 Wi-Fi Info : SSID: '{}' | BSSID: {} | Signal: {}% | RSSI: {} dBm",
            wifi.ssid.as_deref().unwrap_or("Unknown"),
            wifi.bssid.as_deref().unwrap_or("Unknown"),
            wifi.signal_quality_pct,
            wifi.rssi_dbm
        );
        println!(
            "  802.11 PHY Rate   : Rx {:.1} Mbps / Tx {:.1} Mbps",
            wifi.rx_phy_rate_bps as f64 / 1_000_000.0,
            wifi.tx_phy_rate_bps as f64 / 1_000_000.0
        );
    }
    println!("--------------------------------------------------------------------------------");
}

async fn cmd_verify_wan(timeout: Duration, json_output: bool) -> Result<()> {
    let pal = create_platform_manager();
    let wan_interfaces = pal.enumerate_wan_candidates()?;
    if wan_interfaces.is_empty() {
        anyhow::bail!("No active WAN-eligible network interfaces found on this machine.");
    }

    let reports = verify_pool_independence(&wan_interfaces, timeout).await;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&reports)?);
        return Ok(());
    }

    println!("================================================================================");
    println!(" WIFI GIGACHAD 420 — BOUND SOCKET PROBE & WAN INDEPENDENCE VERIFICATION");
    println!("================================================================================");
    for report in &reports {
        let iface = wan_interfaces
            .iter()
            .find(|i| i.if_index == report.if_index);
        let rssi = iface.and_then(|i| match &i.if_type {
            InterfaceType::WiFi(Some(w)) => Some(w.rssi_dbm),
            _ => None,
        });

        println!(
            "[if_index: {:>2}] {} (Local IPv4: {}, Gateway: {})",
            report.if_index,
            report.interface_name,
            report
                .local_ip
                .map(|ip| ip.to_string())
                .unwrap_or_else(|| "None".to_string()),
            report
                .gateway_ip
                .map(|g| g.to_string())
                .unwrap_or_else(|| "None".to_string())
        );
        println!(
            "  Bound Socket Probe: Anycast TCP Handshake RTT: {}",
            report
                .anycast_tcp_rtt_ms
                .map(|r| format!("{:.2} ms", r))
                .unwrap_or_else(|| "TIMEOUT".to_string())
        );
        println!(
            "  External WAN IP   : {}",
            report.public_wan_ip.as_deref().unwrap_or("Unavailable")
        );
        println!("  Independence Mode : {:?}", report.status);

        if let Some(rtt) = report.anycast_tcp_rtt_ms {
            let chs = HealthScoreSnapshot::compute(
                report.if_index,
                report.interface_name.clone(),
                rtt,
                1.5,
                0.0,
                rssi,
            );
            println!(
                "  Initial INR Score : CHS = {:.1} / 100.0 ({:?})",
                chs.score, chs.state
            );
        }
        println!("--------------------------------------------------------------------------------");
    }

    Ok(())
}

async fn cmd_bind_test(
    if_index: u32,
    target: SocketAddr,
    timeout: Duration,
    json_output: bool,
) -> Result<()> {
    let pal = create_platform_manager();
    let interfaces = pal.enumerate_interfaces()?;
    let iface = interfaces
        .iter()
        .find(|i| i.if_index == if_index)
        .ok_or_else(|| anyhow::anyhow!("No network interface found with if_index = {}", if_index))?;

    let (_stream, res) = connect_bound_tcp_v4(iface, target, timeout).await?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&res)?);
        return Ok(());
    }

    println!("================================================================================");
    println!(" STRICT INTERFACE SOCKET BINDING TEST (IP_UNICAST_IF + SO_BIND)");
    println!("================================================================================");
    println!(
        "  Interface      : [if_index: {}] {} ({})",
        iface.if_index, iface.friendly_name, iface.description
    );
    println!("  Bound Local IP : {}:{}", res.local_ip, res.local_port);
    println!("  Connected Peer : {}", res.remote_addr);
    println!("  Handshake RTT  : {:.2} ms", res.handshake_rtt_ms);
    println!("  Status         : SUCCESS (Kernel strong host routing enforced via IP_UNICAST_IF)");
    println!("================================================================================");

    Ok(())
}

async fn cmd_dns_race(hostname: &str, timeout: Duration, json_output: bool) -> Result<()> {
    let pal = create_platform_manager();
    let wan_interfaces = pal.enumerate_wan_candidates()?;
    let inr = InrEngine::new();
    let res = inr
        .dns()
        .resolve_v4(hostname, &wan_interfaces, timeout)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&res)?);
        return Ok(());
    }

    let ip_strings: Vec<String> = res.resolved_ips.iter().map(|ip| ip.to_string()).collect();
    println!("================================================================================");
    println!(" PARALLEL MULTI-INTERFACE DNS RACE RESOLVER");
    println!("================================================================================");
    println!("  Target Hostname   : {}", res.hostname);
    println!("  Resolved IPv4s    : {}", ip_strings.join(", "));
    println!(
        "  Winning Interface : [if_index: {:?}] {}",
        res.winning_if_index, res.winning_interface
    );
    println!("  Winning Transport : {}", res.winning_resolver);
    println!("  Race Latency      : {:.2} ms (Cached: {})", res.resolution_time_ms, res.from_cache);
    println!("================================================================================");
    Ok(())
}

async fn cmd_download(
    url: &str,
    output: &PathBuf,
    workers_per_if: usize,
    json_output: bool,
) -> Result<()> {
    let inr = InrEngine::new();
    let _ = inr.refresh_and_probe(Duration::from_millis(2500)).await;
    let downloader = MultiPathDownloader::new(inr);

    if !json_output {
        println!("================================================================================");
        println!(" MULTI-PATH HTTP/HTTPS BYTE-RANGE DOWNLOADER (KALMAN + WORK-STEALING)");
        println!("================================================================================");
        println!("  Source URL     : {}", url);
        println!("  Output File    : {}", output.display());
        println!("  Workers / NIC  : {}", workers_per_if);
        println!("--------------------------------------------------------------------------------");
    }

    let summary = downloader
        .download_url(url, output, workers_per_if)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&summary)?);
        return Ok(());
    }

    println!("  Final Status   : {:?}", summary.state);
    println!(
        "  Bytes Written  : {} / {} bytes ({:.1}%)",
        summary.downloaded_bytes, summary.total_bytes, summary.progress_pct
    );
    println!(
        "  Average Speed  : {:.2} Mbps ({:.2} seconds)",
        summary.aggregate_speed_mbps, summary.elapsed_sec
    );
    println!(
        "  Chunk Stats    : {} total | {} completed | {} work-stolen",
        summary.total_chunks, summary.completed_chunks, summary.work_stolen_chunks
    );
    for (if_idx, bytes) in &summary.per_interface_bytes {
        println!("  [if_index: {:>2}] : {} bytes contributed", if_idx, bytes);
    }
    if let Some(sha) = &summary.sha256_hex {
        println!("  SHA-256 Digest : {}", sha);
    }
    println!("================================================================================");

    Ok(())
}

async fn cmd_proxy(
    socks5_addr: SocketAddr,
    http_addr: SocketAddr,
    hedge_delay_ms: u64,
) -> Result<()> {
    let inr = InrEngine::new();
    let initial = inr.refresh_and_probe(Duration::from_millis(2500)).await;

    let cfg = ProxyGatewayConfig {
        socks5_bind_addr: socks5_addr,
        http_bind_addr: http_addr,
        happy_eyeballs_hedge_delay_ms: hedge_delay_ms,
        connect_timeout_ms: 5000,
    };
    let gateway = SystemProxyGateway::new(cfg, inr.clone());

    println!("================================================================================");
    println!(" WIFI GIGACHAD 420 — SYSTEM-WIDE LOCAL SOCKS5 & HTTP CONNECT PROXY");
    println!("================================================================================");
    println!("  SOCKS5 Endpoint : {}", socks5_addr);
    println!("  HTTP Endpoint   : {}", http_addr);
    println!("  Hedge Delay     : {} ms (Happy Eyeballs v3 Multi-NIC Failover)", hedge_delay_ms);
    println!("  Active WAN Pool : {} interface(s)", initial.len());
    for snap in &initial {
        println!(
            "    * [if_index: {}] {} — CHS: {:.1}/100 ({:?}), RTT: {:.1} ms, WAN: {}",
            snap.if_index,
            snap.interface_name,
            snap.score,
            snap.state,
            snap.ewma_rtt_ms,
            snap.public_wan_ip.as_deref().unwrap_or("N/A")
        );
    }
    println!("--------------------------------------------------------------------------------");
    println!("  Proxy is live. Press Ctrl+C to stop.");

    // Background INR health refresh loop every 5 seconds
    let inr_bg = inr.clone();
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(5));
        loop {
            ticker.tick().await;
            let _ = inr_bg.refresh_and_probe(Duration::from_millis(2000)).await;
        }
    });

    gateway.run().await?;
    Ok(())
}

async fn cmd_mesh_scan(listen_duration: Duration, json_output: bool) -> Result<()> {
    let mesh = MeshBridgeEngine::new("Windows-GigaChad-Laptop");
    let pin = mesh.pairing_pin().await;
    let peers = mesh.broadcast_and_scan(listen_duration).await;

    if json_output {
        let payload = serde_json::json!({
            "local_peer_id": mesh.local_peer_id(),
            "local_device_name": mesh.local_device_name(),
            "pairing_pin": pin,
            "discovered_peers": peers,
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    println!("================================================================================");
    println!(" WIFI GIGACHAD 420 — CROSS-DEVICE PHONE <-> LAPTOP WI-FI MESH SCANNER");
    println!("================================================================================");
    println!("  Local Peer ID   : {}", mesh.local_peer_id());
    println!("  Local Device    : {}", mesh.local_device_name());
    println!("  6-Digit Pair PIN: {}", pin);
    println!("  Discovery Port  : UDP :42069 | Tunnel Port: TCP :42070");
    println!("--------------------------------------------------------------------------------");
    if peers.is_empty() {
        println!("  No remote GigaChad Phone/Laptop peers responded on the local Wi-Fi subnet yet.");
    } else {
        for p in &peers {
            println!(
                "  * {} ({}) @ {}:{} — Uplink: {} ({} Mbps) | Paired: {}",
                p.device_name,
                p.platform,
                p.lan_ip,
                p.tunnel_port,
                p.uplink_medium,
                p.advertised_bandwidth_mbps,
                p.paired
            );
        }
    }
    println!("================================================================================");

    Ok(())
}

fn cmd_os_optimize(prompt_uac: bool, json_output: bool) -> Result<()> {
    let pal = create_platform_manager();
    let wan = pal.enumerate_wan_candidates().unwrap_or_default();
    let report = apply_windows_multi_connector_optimizations(&wan, prompt_uac);

    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    println!("================================================================================");
    println!(" WIFI GIGACHAD 420 — WINDOWS OS MULTI-WIFI & MULTI-CONNECTOR OPTIMIZER");
    println!("================================================================================");
    println!("  Elevated Administrator : {}", report.is_elevated_admin);
    println!("  System Proxy Enabled   : {}", report.system_proxy_enabled);
    println!("--------------------------------------------------------------------------------");
    for step in &report.steps {
        println!(
            "  [{}] {}",
            if step.applied { "APPLIED" } else { "PENDING" },
            step.title
        );
        println!("    Command : {}", step.command_desc);
        println!("    Status  : {}", step.detail);
    }
    println!("================================================================================");
    Ok(())
}
