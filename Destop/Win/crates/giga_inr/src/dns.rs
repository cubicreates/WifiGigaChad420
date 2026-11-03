//! Zero-Latency Multi-Interface Parallel DNS Race Resolver (`DnsRaceResolver`).
//!
//! Fires concurrent RFC 1035 DNS A-record queries over interface-bound UDP (`:53`) and TCP (`:53`)
//! sockets across all active network interfaces, returning the fastest valid answer and caching it.

use giga_pal::{connect_bound_tcp_v4, create_bound_udp_socket_v4, InterfaceSnapshot};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsRaceResult {
    pub hostname: String,
    pub resolved_ips: Vec<Ipv4Addr>,
    pub winning_if_index: Option<u32>,
    pub winning_interface: String,
    pub winning_resolver: String,
    pub resolution_time_ms: f64,
    pub from_cache: bool,
}

#[derive(Debug, Clone)]
struct CachedDnsEntry {
    ips: Vec<Ipv4Addr>,
    winning_if_index: Option<u32>,
    winning_interface: String,
    winning_resolver: String,
    expires_at: Instant,
}

#[derive(Clone, Default)]
pub struct DnsRaceResolver {
    cache: Arc<RwLock<HashMap<String, CachedDnsEntry>>>,
}

impl DnsRaceResolver {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Resolves `hostname` to one or more `Ipv4Addr` by racing bound DNS queries across all
    /// provided `interfaces` in parallel.
    pub async fn resolve_v4(
        &self,
        hostname: &str,
        interfaces: &[InterfaceSnapshot],
        timeout: Duration,
    ) -> Result<DnsRaceResult, String> {
        let clean_host = hostname.trim().trim_end_matches('.').to_ascii_lowercase();

        // 1. Direct IPv4 literal check
        if let Ok(ip) = clean_host.parse::<Ipv4Addr>() {
            return Ok(DnsRaceResult {
                hostname: clean_host,
                resolved_ips: vec![ip],
                winning_if_index: interfaces.first().map(|i| i.if_index),
                winning_interface: "Literal-IP".to_string(),
                winning_resolver: "Literal".to_string(),
                resolution_time_ms: 0.0,
                from_cache: true,
            });
        }

        if clean_host == "localhost" {
            return Ok(DnsRaceResult {
                hostname: clean_host,
                resolved_ips: vec![Ipv4Addr::new(127, 0, 0, 1)],
                winning_if_index: None,
                winning_interface: "Loopback".to_string(),
                winning_resolver: "Localhost".to_string(),
                resolution_time_ms: 0.0,
                from_cache: true,
            });
        }

        // 2. Check fast in-memory TTL cache
        {
            let guard = self.cache.read().await;
            if let Some(entry) = guard.get(&clean_host) {
                if Instant::now() < entry.expires_at {
                    return Ok(DnsRaceResult {
                        hostname: clean_host,
                        resolved_ips: entry.ips.clone(),
                        winning_if_index: entry.winning_if_index,
                        winning_interface: entry.winning_interface.clone(),
                        winning_resolver: entry.winning_resolver.clone(),
                        resolution_time_ms: 0.05,
                        from_cache: true,
                    });
                }
            }
        }

        let start = Instant::now();
        let query_packet = build_dns_a_query(&clean_host, 0x1337)?;

        let mut tasks = tokio::task::JoinSet::new();

        for iface in interfaces.iter().filter(|i| i.is_wan_eligible()) {
            let mut resolvers = vec![
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), 53),
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 53),
            ];
            for dns in &iface.dns_servers {
                if let IpAddr::V4(v4) = dns {
                    let addr = SocketAddr::new(IpAddr::V4(*v4), 53);
                    if !resolvers.contains(&addr) {
                        resolvers.insert(0, addr);
                    }
                }
            }

            for resolver_addr in resolvers.into_iter().take(3) {
                let iface_clone = iface.clone();
                let pkt = query_packet.clone();
                tasks.spawn(async move {
                    // Race UDP and TCP on this interface + resolver
                    if let Some(ips) =
                        query_dns_udp_bound(&iface_clone, resolver_addr, &pkt, timeout).await
                    {
                        return Some((
                            ips,
                            iface_clone.if_index,
                            iface_clone.friendly_name.clone(),
                            format!("UDP/{}", resolver_addr),
                        ));
                    }
                    if let Some(ips) =
                        query_dns_tcp_bound(&iface_clone, resolver_addr, &pkt, timeout).await
                    {
                        return Some((
                            ips,
                            iface_clone.if_index,
                            iface_clone.friendly_name.clone(),
                            format!("TCP/{}", resolver_addr),
                        ));
                    }
                    None
                });
            }
        }

        while let Some(res) = tasks.join_next().await {
            if let Ok(Some((ips, if_idx, if_name, resolver_label))) = res {
                if !ips.is_empty() {
                    tasks.abort_all();
                    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
                    let mut guard = self.cache.write().await;
                    guard.insert(
                        clean_host.clone(),
                        CachedDnsEntry {
                            ips: ips.clone(),
                            winning_if_index: Some(if_idx),
                            winning_interface: if_name.clone(),
                            winning_resolver: resolver_label.clone(),
                            expires_at: Instant::now() + Duration::from_secs(180),
                        },
                    );
                    return Ok(DnsRaceResult {
                        hostname: clean_host,
                        resolved_ips: ips,
                        winning_if_index: Some(if_idx),
                        winning_interface: if_name,
                        winning_resolver: resolver_label,
                        resolution_time_ms: elapsed_ms,
                        from_cache: false,
                    });
                }
            }
        }

        // Fallback to OS resolver if all raw bound sockets were filtered
        if let Ok(addrs) = tokio::net::lookup_host((clean_host.as_str(), 80)).await {
            let ips: Vec<Ipv4Addr> = addrs
                .filter_map(|sa| match sa.ip() {
                    IpAddr::V4(v4) => Some(v4),
                    _ => None,
                })
                .collect();
            if !ips.is_empty() {
                let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
                return Ok(DnsRaceResult {
                    hostname: clean_host.clone(),
                    resolved_ips: ips,
                    winning_if_index: interfaces.first().map(|i| i.if_index),
                    winning_interface: "OS-Fallback".to_string(),
                    winning_resolver: "System-DNS".to_string(),
                    resolution_time_ms: elapsed_ms,
                    from_cache: false,
                });
            }
        }

        Err(format!(
            "DNS Race Resolver failed to resolve '{}' across all interfaces",
            clean_host
        ))
    }
}

fn build_dns_a_query(hostname: &str, tx_id: u16) -> Result<Vec<u8>, String> {
    let mut pkt = Vec::with_capacity(64);
    pkt.extend_from_slice(&tx_id.to_be_bytes());
    pkt.extend_from_slice(&[0x01, 0x00]); // Standard query, recursion desired
    pkt.extend_from_slice(&[0x00, 0x01]); // QDCOUNT = 1
    pkt.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00]); // AN/NS/AR = 0

    for label in hostname.split('.') {
        if label.is_empty() || label.len() > 63 {
            return Err(format!("Invalid DNS label in '{}'", hostname));
        }
        pkt.push(label.len() as u8);
        pkt.extend_from_slice(label.as_bytes());
    }
    pkt.push(0x00); // root label
    pkt.extend_from_slice(&[0x00, 0x01]); // QTYPE = A (1)
    pkt.extend_from_slice(&[0x00, 0x01]); // QCLASS = IN (1)
    Ok(pkt)
}

async fn query_dns_udp_bound(
    iface: &InterfaceSnapshot,
    resolver: SocketAddr,
    query: &[u8],
    timeout: Duration,
) -> Option<Vec<Ipv4Addr>> {
    let sock = create_bound_udp_socket_v4(iface, 0).ok()?;
    tokio::time::timeout(timeout, sock.send_to(query, resolver))
        .await
        .ok()?
        .ok()?;
    let mut buf = [0u8; 512];
    let (n, _) = tokio::time::timeout(timeout, sock.recv_from(&mut buf))
        .await
        .ok()?
        .ok()?;
    parse_dns_a_response(&buf[..n])
}

async fn query_dns_tcp_bound(
    iface: &InterfaceSnapshot,
    resolver: SocketAddr,
    query: &[u8],
    timeout: Duration,
) -> Option<Vec<Ipv4Addr>> {
    let (mut stream, _) = connect_bound_tcp_v4(iface, resolver, timeout).await.ok()?;
    let len_be = (query.len() as u16).to_be_bytes();
    let mut tcp_pkt = Vec::with_capacity(2 + query.len());
    tcp_pkt.extend_from_slice(&len_be);
    tcp_pkt.extend_from_slice(query);

    tokio::time::timeout(timeout, stream.write_all(&tcp_pkt))
        .await
        .ok()?
        .ok()?;
    let mut buf = [0u8; 512];
    let n = tokio::time::timeout(timeout, stream.read(&mut buf))
        .await
        .ok()?
        .ok()?;
    if n <= 2 {
        return None;
    }
    parse_dns_a_response(&buf[2..n])
}

fn parse_dns_a_response(msg: &[u8]) -> Option<Vec<Ipv4Addr>> {
    if msg.len() < 12 {
        return None;
    }
    let flags = u16::from_be_bytes([msg[2], msg[3]]);
    let rcode = flags & 0x000F;
    if rcode != 0 {
        return None;
    }
    let qdcount = u16::from_be_bytes([msg[4], msg[5]]) as usize;
    let ancount = u16::from_be_bytes([msg[6], msg[7]]) as usize;
    if ancount == 0 {
        return None;
    }

    let mut pos = 12;
    for _ in 0..qdcount {
        pos = skip_dns_name(msg, pos)?;
        pos += 4; // QTYPE + QCLASS
        if pos > msg.len() {
            return None;
        }
    }

    let mut ips = Vec::new();
    for _ in 0..ancount {
        pos = skip_dns_name(msg, pos)?;
        if pos + 10 > msg.len() {
            break;
        }
        let rtype = u16::from_be_bytes([msg[pos], msg[pos + 1]]);
        let rclass = u16::from_be_bytes([msg[pos + 2], msg[pos + 3]]);
        let rdlength = u16::from_be_bytes([msg[pos + 8], msg[pos + 9]]) as usize;
        pos += 10;
        if pos + rdlength > msg.len() {
            break;
        }
        if rtype == 1 && rclass == 1 && rdlength == 4 {
            let ip = Ipv4Addr::new(msg[pos], msg[pos + 1], msg[pos + 2], msg[pos + 3]);
            if !ip.is_unspecified() {
                ips.push(ip);
            }
        }
        pos += rdlength;
    }

    if ips.is_empty() {
        None
    } else {
        Some(ips)
    }
}

fn skip_dns_name(msg: &[u8], mut pos: usize) -> Option<usize> {
    loop {
        if pos >= msg.len() {
            return None;
        }
        let b = msg[pos];
        if b == 0 {
            return Some(pos + 1);
        }
        if (b & 0xC0) == 0xC0 {
            return if pos + 1 < msg.len() {
                Some(pos + 2)
            } else {
                None
            };
        }
        let label_len = (b & 0x3F) as usize;
        pos += 1 + label_len;
    }
}
