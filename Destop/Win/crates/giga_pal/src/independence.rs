use crate::interface::InterfaceSnapshot;
use crate::socket::connect_bound_tcp_v4;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Classification of whether two or more local interfaces actually traverse independent ISP WAN links
/// or converge onto the same residential/campus gateway router.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WanIndependenceStatus {
    /// Genuinely independent WAN path with a unique Public IP and distinct gateway.
    IndependentWan,
    /// Shares the exact same local gateway IP (e.g., Ethernet + Wi-Fi plugged into the same router).
    SharedLocalGateway { shared_with_interface: String },
    /// Different local gateways, but both egress through the same upstream Public WAN IP.
    SharedPublicWanIp {
        public_ip: String,
        shared_with_interface: String,
    },
    /// Probe could not reach the external reflection endpoint on this interface.
    UnreachableUpstream { error: String },
}

/// Complete independence and reachability diagnostic report for an individual interface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndependenceReport {
    pub interface_name: String,
    pub if_index: u32,
    pub local_ip: Option<Ipv4Addr>,
    pub gateway_ip: Option<Ipv4Addr>,
    pub public_wan_ip: Option<String>,
    pub anycast_tcp_rtt_ms: Option<f64>,
    pub status: WanIndependenceStatus,
}

/// Probes each WAN-eligible interface using explicitly bound sockets (`IP_UNICAST_IF` + `bind()`)
/// to:
/// 1. Measure TCP handshake RTT to anycast DNS (`1.1.1.1:53` or `8.8.8.8:53` / `8.8.8.8:443`).
/// 2. Discover the external Public WAN IP via a lightweight bound HTTP request (`http://checkip.amazonaws.com` / `api.ipify.org`).
/// 3. Detect whether multiple active interfaces share the same local gateway or public WAN IP.
pub async fn verify_pool_independence(
    candidates: &[InterfaceSnapshot],
    probe_timeout: Duration,
) -> Vec<IndependenceReport> {
    let mut reports = Vec::with_capacity(candidates.len());
    let mut gateway_owners: HashMap<Ipv4Addr, String> = HashMap::new();
    let mut public_ip_owners: HashMap<String, String> = HashMap::new();

    for iface in candidates {
        let local_ip = iface.primary_ipv4();
        let gateway_ip = iface.primary_ipv4_gateway();

        // 1. Measure bound TCP handshake RTT to Google/Cloudflare Anycast (8.8.8.8:53 / 1.1.1.1:53)
        let anycast_targets = [
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), 53),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 443),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 4, 4)), 53),
        ];

        let mut anycast_rtt = None;
        let mut last_err = String::from("No probe targets attempted");

        for target in anycast_targets {
            match connect_bound_tcp_v4(iface, target, probe_timeout).await {
                Ok((_stream, res)) => {
                    anycast_rtt = Some(res.handshake_rtt_ms);
                    break;
                }
                Err(e) => {
                    last_err = e.to_string();
                }
            }
        }

        // 2. Attempt bound HTTP Public IP reflection probe if upstream TCP succeeded
        let public_wan_ip = if anycast_rtt.is_some() {
            fetch_bound_public_ip(iface, probe_timeout).await
        } else {
            None
        };

        // 3. Evaluate gateway and public IP independence against earlier interfaces
        let status = if anycast_rtt.is_none() {
            WanIndependenceStatus::UnreachableUpstream { error: last_err }
        } else if let Some(gw) = gateway_ip {
            if let Some(prev_owner) = gateway_owners.get(&gw) {
                WanIndependenceStatus::SharedLocalGateway {
                    shared_with_interface: prev_owner.clone(),
                }
            } else {
                gateway_owners.insert(gw, iface.friendly_name.clone());
                if let Some(ref pub_ip) = public_wan_ip {
                    if let Some(prev_pub_owner) = public_ip_owners.get(pub_ip) {
                        WanIndependenceStatus::SharedPublicWanIp {
                            public_ip: pub_ip.clone(),
                            shared_with_interface: prev_pub_owner.clone(),
                        }
                    } else {
                        public_ip_owners.insert(pub_ip.clone(), iface.friendly_name.clone());
                        WanIndependenceStatus::IndependentWan
                    }
                } else {
                    WanIndependenceStatus::IndependentWan
                }
            }
        } else {
            WanIndependenceStatus::IndependentWan
        };

        reports.push(IndependenceReport {
            interface_name: iface.friendly_name.clone(),
            if_index: iface.if_index,
            local_ip,
            gateway_ip,
            public_wan_ip,
            anycast_tcp_rtt_ms: anycast_rtt,
            status,
        });
    }

    reports
}

/// Discovers the external Public WAN IP strictly over `iface` using:
/// 1. Bound DNS-over-TCP (`208.67.222.222:53` OpenDNS `myip.opendns.com` A-record reflection,
///    which traverses campus/enterprise firewalls that block plain HTTP port 80).
/// 2. Fallback bound HTTP/1.1 trace request (`104.16.132.229:80/cdn-cgi/trace` and `1.1.1.1:80`).
async fn fetch_bound_public_ip(iface: &InterfaceSnapshot, timeout: Duration) -> Option<String> {
    if let Some(ip) = fetch_bound_public_ip_dns_tcp(iface, timeout).await {
        return Some(ip);
    }

    let targets = [
        (
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(104, 16, 132, 229)), 80),
            "www.cloudflare.com",
        ),
        (
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 80),
            "1.1.1.1",
        ),
    ];

    for (target, host) in targets {
        let Ok((mut stream, _)) = connect_bound_tcp_v4(iface, target, timeout).await else {
            continue;
        };

        let req = format!(
            "GET /cdn-cgi/trace HTTP/1.1\r\nHost: {}\r\nUser-Agent: WiFiGigaChad420/0.1\r\nConnection: close\r\n\r\n",
            host
        );
        if tokio::time::timeout(timeout, stream.write_all(req.as_bytes()))
            .await
            .is_err()
        {
            continue;
        }

        let mut accumulated = Vec::with_capacity(2048);
        let mut chunk = [0u8; 1024];
        for _ in 0..4 {
            match tokio::time::timeout(Duration::from_millis(1200), stream.read(&mut chunk)).await {
                Ok(Ok(0)) => break,
                Ok(Ok(n)) => {
                    accumulated.extend_from_slice(&chunk[..n]);
                    let text = String::from_utf8_lossy(&accumulated);
                    for line in text.lines() {
                        if let Some(ip_val) = line.strip_prefix("ip=") {
                            let trimmed = ip_val.trim();
                            if !trimmed.is_empty() {
                                return Some(trimmed.to_string());
                            }
                        }
                    }
                }
                _ => break,
            }
        }
    }
    None
}

/// Queries `myip.opendns.com` (Type A, Class IN) over a bound TCP socket to OpenDNS (`208.67.222.222:53`
/// or `208.67.220.220:53`) and extracts the reflected 4-byte IPv4 RDATA.
async fn fetch_bound_public_ip_dns_tcp(
    iface: &InterfaceSnapshot,
    timeout: Duration,
) -> Option<String> {
    let resolvers = [
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(208, 67, 222, 222)), 53),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(208, 67, 220, 220)), 53),
    ];

    // RFC 1035 DNS query for "myip.opendns.com" (Type A = 1, Class IN = 1) prefixed with 2-byte TCP length (34 bytes = 0x0022)
    const DNS_TCP_QUERY: [u8; 36] = [
        0x00, 0x22, // TCP message length: 34 bytes
        0x42, 0x42, // Transaction ID
        0x01, 0x00, // Flags: Standard query, recursion desired
        0x00, 0x01, // Questions: 1
        0x00, 0x00, // Answer RRs: 0
        0x00, 0x00, // Authority RRs: 0
        0x00, 0x00, // Additional RRs: 0
        0x04, b'm', b'y', b'i', b'p', 0x07, b'o', b'p', b'e', b'n', b'd', b'n', b's', 0x03, b'c',
        b'o', b'm', 0x00, // QNAME: myip.opendns.com.
        0x00, 0x01, // QTYPE: A (1)
        0x00, 0x01, // QCLASS: IN (1)
    ];

    for resolver in resolvers {
        let Ok((mut stream, _)) = connect_bound_tcp_v4(iface, resolver, timeout).await else {
            continue;
        };
        if tokio::time::timeout(timeout, stream.write_all(&DNS_TCP_QUERY))
            .await
            .is_err()
        {
            continue;
        }
        let mut resp = [0u8; 128];
        let Ok(Ok(n)) = tokio::time::timeout(timeout, stream.read(&mut resp)).await else {
            continue;
        };
        // Minimum valid DNS-over-TCP response with 1 A-record answer is >= 46 bytes, ending in RDLENGTH(0x00, 0x04) + 4 IPv4 bytes
        if n >= 46 && resp[n - 6] == 0x00 && resp[n - 5] == 0x04 {
            let ip = Ipv4Addr::new(resp[n - 4], resp[n - 3], resp[n - 2], resp[n - 1]);
            if !ip.is_unspecified() && !ip.is_private() {
                return Some(ip.to_string());
            }
        }
    }
    None
}

