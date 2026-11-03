//! System-Wide Local SOCKS5 (`127.0.0.1:1080`) & HTTP CONNECT/Forward Proxy (`127.0.0.1:1081`)
//! with Happy Eyeballs v3 Multi-Interface Connection Hedging and Parallel DNS Racing.

use giga_inr::InrEngine;
use giga_pal::{connect_bound_tcp_v4, BoundConnectResult, InterfaceSnapshot};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Configuration for the local system-wide proxy gateway.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyGatewayConfig {
    pub socks5_bind_addr: SocketAddr,
    pub http_bind_addr: SocketAddr,
    pub happy_eyeballs_hedge_delay_ms: u64,
    pub connect_timeout_ms: u64,
}

impl Default for ProxyGatewayConfig {
    fn default() -> Self {
        Self {
            socks5_bind_addr: "127.0.0.1:1080".parse().expect("valid socket addr"),
            http_bind_addr: "127.0.0.1:1081".parse().expect("valid socket addr"),
            happy_eyeballs_hedge_delay_ms: 150,
            connect_timeout_ms: 5000,
        }
    }
}

/// Live atomic telemetry counters for the local proxy gateway.
#[derive(Default)]
pub struct ProxyTelemetryCounters {
    pub active_connections: AtomicU64,
    pub total_connections: AtomicU64,
    pub hedged_secondary_wins: AtomicU64,
    pub total_rx_bytes: AtomicU64,
    pub total_tx_bytes: AtomicU64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyTelemetrySnapshot {
    pub socks5_addr: String,
    pub http_addr: String,
    pub active_connections: u64,
    pub total_connections: u64,
    pub hedged_secondary_wins: u64,
    pub total_rx_bytes: u64,
    pub total_tx_bytes: u64,
}

#[derive(Clone)]
pub struct SystemProxyGateway {
    config: ProxyGatewayConfig,
    inr: InrEngine,
    counters: Arc<ProxyTelemetryCounters>,
}

impl SystemProxyGateway {
    pub fn new(config: ProxyGatewayConfig, inr: InrEngine) -> Self {
        Self {
            config,
            inr,
            counters: Arc::new(ProxyTelemetryCounters::default()),
        }
    }

    pub fn snapshot(&self) -> ProxyTelemetrySnapshot {
        ProxyTelemetrySnapshot {
            socks5_addr: self.config.socks5_bind_addr.to_string(),
            http_addr: self.config.http_bind_addr.to_string(),
            active_connections: self.counters.active_connections.load(Ordering::Relaxed),
            total_connections: self.counters.total_connections.load(Ordering::Relaxed),
            hedged_secondary_wins: self.counters.hedged_secondary_wins.load(Ordering::Relaxed),
            total_rx_bytes: self.counters.total_rx_bytes.load(Ordering::Relaxed),
            total_tx_bytes: self.counters.total_tx_bytes.load(Ordering::Relaxed),
        }
    }

    /// Spawns both the SOCKS5 (`127.0.0.1:1080`) and HTTP CONNECT (`127.0.0.1:1081`) proxy listeners.
    pub async fn run(&self) -> std::io::Result<()> {
        let socks_listener = TcpListener::bind(self.config.socks5_bind_addr).await?;
        let http_listener = TcpListener::bind(self.config.http_bind_addr).await?;

        let proxy_for_socks = self.clone();
        let socks_task = tokio::spawn(async move {
            loop {
                let Ok((client, _)) = socks_listener.accept().await else {
                    continue;
                };
                let gw = proxy_for_socks.clone();
                tokio::spawn(async move {
                    let _ = gw.handle_socks5_client(client).await;
                });
            }
        });

        let proxy_for_http = self.clone();
        let http_task = tokio::spawn(async move {
            loop {
                let Ok((client, _)) = http_listener.accept().await else {
                    continue;
                };
                let gw = proxy_for_http.clone();
                tokio::spawn(async move {
                    let _ = gw.handle_http_proxy_client(client).await;
                });
            }
        });

        let _ = tokio::join!(socks_task, http_task);
        Ok(())
    }

    async fn handle_socks5_client(&self, mut client: TcpStream) -> std::io::Result<()> {
        // 1. Greeting: VER(1) | NMETHODS(1) | METHODS(1..255)
        let mut hdr = [0u8; 2];
        client.read_exact(&mut hdr).await?;
        if hdr[0] != 0x05 {
            return Ok(());
        }
        let nmethods = hdr[1] as usize;
        let mut methods = vec![0u8; nmethods];
        client.read_exact(&mut methods).await?;

        // Reply: VER=5, METHOD=0 (No authentication)
        client.write_all(&[0x05, 0x00]).await?;

        // 2. Request: VER(1) | CMD(1) | RSV(1) | ATYP(1) | DST.ADDR | DST.PORT(2)
        let mut req = [0u8; 4];
        client.read_exact(&mut req).await?;
        if req[1] != 0x01 {
            // Only CONNECT (0x01) is supported
            let _ = client
                .write_all(&[0x05, 0x07, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await;
            return Ok(());
        }

        let host = match req[3] {
            0x01 => {
                let mut ip = [0u8; 4];
                client.read_exact(&mut ip).await?;
                Ipv4Addr::from(ip).to_string()
            }
            0x03 => {
                let mut len = [0u8; 1];
                client.read_exact(&mut len).await?;
                let mut domain = vec![0u8; len[0] as usize];
                client.read_exact(&mut domain).await?;
                String::from_utf8_lossy(&domain).to_string()
            }
            _ => {
                let _ = client
                    .write_all(&[0x05, 0x08, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                    .await;
                return Ok(());
            }
        };

        let mut port_buf = [0u8; 2];
        client.read_exact(&mut port_buf).await?;
        let port = u16::from_be_bytes(port_buf);

        // 3. Connect outbound via Happy Eyeballs v3 Bound Interface Hedging
        match self.connect_hedged_outbound(&host, port).await {
            Ok((mut upstream, info, won_by_secondary)) => {
                if won_by_secondary {
                    self.counters
                        .hedged_secondary_wins
                        .fetch_add(1, Ordering::Relaxed);
                }
                // Reply success
                client
                    .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                    .await?;
                self.pipe_bidirectional(&mut client, &mut upstream, info.if_index)
                    .await;
            }
            Err(_) => {
                let _ = client
                    .write_all(&[0x05, 0x04, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                    .await;
            }
        }
        Ok(())
    }

    async fn handle_http_proxy_client(&self, mut client: TcpStream) -> std::io::Result<()> {
        let mut buf = [0u8; 4096];
        let n = client.read(&mut buf).await?;
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
        let target = parts[1];

        if method.eq_ignore_ascii_case("CONNECT") {
            let (host, port) = parse_host_port(target, 443);
            if let Ok((mut upstream, info, won_by_secondary)) =
                self.connect_hedged_outbound(&host, port).await
            {
                if won_by_secondary {
                    self.counters
                        .hedged_secondary_wins
                        .fetch_add(1, Ordering::Relaxed);
                }
                client
                    .write_all(b"HTTP/1.1 200 Connection Established\r\nProxy-Agent: WiFiGigaChad420/0.1\r\n\r\n")
                    .await?;
                self.pipe_bidirectional(&mut client, &mut upstream, info.if_index)
                    .await;
            } else {
                let _ = client.write_all(b"HTTP/1.1 502 Bad Gateway\r\n\r\n").await;
            }
        } else {
            // Standard HTTP forward proxy request (e.g., GET http://example.com/path HTTP/1.1)
            let stripped = target
                .strip_prefix("http://")
                .unwrap_or(target);
            let host_part = stripped.split('/').next().unwrap_or(stripped);
            let (host, port) = parse_host_port(host_part, 80);

            if let Ok((mut upstream, info, won_by_secondary)) =
                self.connect_hedged_outbound(&host, port).await
            {
                if won_by_secondary {
                    self.counters
                        .hedged_secondary_wins
                        .fetch_add(1, Ordering::Relaxed);
                }
                upstream.write_all(&buf[..n]).await?;
                self.pipe_bidirectional(&mut client, &mut upstream, info.if_index)
                    .await;
            } else {
                let _ = client.write_all(b"HTTP/1.1 502 Bad Gateway\r\n\r\n").await;
            }
        }

        Ok(())
    }

    async fn pipe_bidirectional(
        &self,
        client: &mut TcpStream,
        upstream: &mut TcpStream,
        if_index: u32,
    ) {
        self.counters
            .active_connections
            .fetch_add(1, Ordering::Relaxed);
        self.counters
            .total_connections
            .fetch_add(1, Ordering::Relaxed);

        if let Ok((tx_bytes, rx_bytes)) = tokio::io::copy_bidirectional(client, upstream).await {
            self.counters
                .total_tx_bytes
                .fetch_add(tx_bytes, Ordering::Relaxed);
            self.counters
                .total_rx_bytes
                .fetch_add(rx_bytes, Ordering::Relaxed);
            self.inr
                .record_transfer_sample(if_index, rx_bytes, tx_bytes, None, true)
                .await;
        }

        self.counters
            .active_connections
            .fetch_sub(1, Ordering::Relaxed);
    }

    /// Resolves `host` via the Parallel DNS Race Resolver and connects a bound TCP socket using
    /// Happy Eyeballs v3 hedging (`150 ms` fallback to Secondary interface).
    pub async fn connect_hedged_outbound(
        &self,
        host: &str,
        port: u16,
    ) -> Result<(TcpStream, BoundConnectResult, bool), String> {
        let mut ranked = self.inr.ranked_healthy_interfaces().await;
        if ranked.is_empty() {
            let _ = self.inr.refresh_and_probe(Duration::from_millis(2000)).await;
            ranked = self.inr.ranked_healthy_interfaces().await;
        }
        if ranked.is_empty() {
            return Err("No healthy WAN interfaces available".to_string());
        }

        let raw_ifaces: Vec<InterfaceSnapshot> = ranked.iter().map(|(i, _)| i.clone()).collect();
        let dns_res = self
            .inr
            .dns()
            .resolve_v4(host, &raw_ifaces, Duration::from_millis(2500))
            .await?;

        let target_ip = *dns_res
            .resolved_ips
            .first()
            .ok_or_else(|| "No IPv4 address returned".to_string())?;
        let remote_addr = SocketAddr::new(IpAddr::V4(target_ip), port);

        let timeout = Duration::from_millis(self.config.connect_timeout_ms);
        let hedge_delay = Duration::from_millis(self.config.happy_eyeballs_hedge_delay_ms);

        let primary = ranked[0].0.clone();
        if ranked.len() == 1 {
            let (stream, res) = connect_bound_tcp_v4(&primary, remote_addr, timeout)
                .await
                .map_err(|e| e.to_string())?;
            self.inr
                .record_transfer_sample(primary.if_index, 0, 0, Some(res.handshake_rtt_ms), true)
                .await;
            return Ok((stream, res, false));
        }

        let secondary = ranked[1].0.clone();
        let p_clone = primary.clone();
        let mut primary_fut =
            Box::pin(async move { connect_bound_tcp_v4(&p_clone, remote_addr, timeout).await });

        // Wait up to `hedge_delay` (150ms) for Primary before launching Secondary race
        tokio::select! {
            res1 = &mut primary_fut => {
                if let Ok((stream, info)) = res1 {
                    self.inr
                        .record_transfer_sample(primary.if_index, 0, 0, Some(info.handshake_rtt_ms), true)
                        .await;
                    return Ok((stream, info, false));
                }
                // Primary failed quickly; immediately try Secondary
                let (stream, info) = connect_bound_tcp_v4(&secondary, remote_addr, timeout)
                    .await
                    .map_err(|e| e.to_string())?;
                self.inr
                    .record_transfer_sample(secondary.if_index, 0, 0, Some(info.handshake_rtt_ms), true)
                    .await;
                Ok((stream, info, true))
            }
            _ = tokio::time::sleep(hedge_delay) => {
                // Primary is slow (>150ms); race Primary against Secondary concurrently!
                let s_clone = secondary.clone();
                let secondary_fut = connect_bound_tcp_v4(&s_clone, remote_addr, timeout);
                tokio::pin!(secondary_fut);

                tokio::select! {
                    res1 = &mut primary_fut => {
                        if let Ok((stream, info)) = res1 {
                            self.inr
                                .record_transfer_sample(primary.if_index, 0, 0, Some(info.handshake_rtt_ms), true)
                                .await;
                            return Ok((stream, info, false));
                        }
                        let (stream, info) = secondary_fut.await.map_err(|e| e.to_string())?;
                        self.inr
                            .record_transfer_sample(secondary.if_index, 0, 0, Some(info.handshake_rtt_ms), true)
                            .await;
                        Ok((stream, info, true))
                    }
                    res2 = &mut secondary_fut => {
                        if let Ok((stream, info)) = res2 {
                            self.inr
                                .record_transfer_sample(secondary.if_index, 0, 0, Some(info.handshake_rtt_ms), true)
                                .await;
                            return Ok((stream, info, true));
                        }
                        let (stream, info) = primary_fut.await.map_err(|e| e.to_string())?;
                        self.inr
                            .record_transfer_sample(primary.if_index, 0, 0, Some(info.handshake_rtt_ms), true)
                            .await;
                        Ok((stream, info, false))
                    }
                }
            }
        }
    }
}

fn parse_host_port(input: &str, default_port: u16) -> (String, u16) {
    if let Some((h, p)) = input.rsplit_once(':') {
        if let Ok(port) = p.parse::<u16>() {
            return (h.to_string(), port);
        }
    }
    (input.to_string(), default_port)
}
