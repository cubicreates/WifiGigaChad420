//! `giga_mesh` — Cross-Device Phone <-> Laptop Wi-Fi Mesh Discovery & Uplink Bridge.
//!
//! Enables a Windows laptop and an Android/iOS phone connected to the same Wi-Fi router (or
//! mobile hotspot) to discover each other via UDP LAN beacons (`:42069`), authenticate with a
//! 6-digit pairing PIN, and aggregate the phone's 5G/LTE cellular uplink over the local Wi-Fi mesh (`:42070`).

use giga_store::sha256_bytes_hex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::RwLock;

pub const MESH_DISCOVERY_UDP_PORT: u16 = 42069;
pub const MESH_TUNNEL_TCP_PORT: u16 = 42070;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshBeaconPacket {
    pub magic: String,
    pub peer_id: String,
    pub device_name: String,
    pub platform: String,
    pub uplink_medium: String,
    pub advertised_bandwidth_mbps: u32,
    pub battery_pct: u8,
    pub tunnel_port: u16,
    pub pairing_digest_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredMeshPeer {
    pub peer_id: String,
    pub device_name: String,
    pub platform: String,
    pub lan_ip: Ipv4Addr,
    pub tunnel_port: u16,
    pub uplink_medium: String,
    pub advertised_bandwidth_mbps: u32,
    pub battery_pct: u8,
    pub paired: bool,
    pub last_seen_ms_ago: u64,
}

struct PeerRecord {
    info: DiscoveredMeshPeer,
    last_seen: Instant,
}

#[derive(Clone)]
pub struct MeshBridgeEngine {
    local_peer_id: String,
    local_device_name: String,
    pairing_pin: Arc<RwLock<String>>,
    peers: Arc<RwLock<HashMap<String, PeerRecord>>>,
}

impl MeshBridgeEngine {
    pub fn new(device_name: impl Into<String>) -> Self {
        let now_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let peer_id = format!("GIGA-WIN-{:08X}", (now_ns & 0xFFFF_FFFF) as u32);
        let pin = format!("{:06}", ((now_ns >> 12) % 900_000 + 100_000) as u32);

        Self {
            local_peer_id: peer_id,
            local_device_name: device_name.into(),
            pairing_pin: Arc::new(RwLock::new(pin)),
            peers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn local_peer_id(&self) -> &str {
        &self.local_peer_id
    }

    pub fn local_device_name(&self) -> &str {
        &self.local_device_name
    }

    pub async fn pairing_pin(&self) -> String {
        self.pairing_pin.read().await.clone()
    }

    pub async fn set_pairing_pin(&self, new_pin: String) {
        *self.pairing_pin.write().await = new_pin;
    }

    pub async fn discovered_peers(&self) -> Vec<DiscoveredMeshPeer> {
        let now = Instant::now();
        let guard = self.peers.read().await;
        let mut out: Vec<DiscoveredMeshPeer> = guard
            .values()
            .filter(|r| now.duration_since(r.last_seen) < Duration::from_secs(30))
            .map(|r| {
                let mut item = r.info.clone();
                item.last_seen_ms_ago = now.duration_since(r.last_seen).as_millis() as u64;
                item
            })
            .collect();
        out.sort_by(|a, b| a.device_name.cmp(&b.device_name));
        out
    }

    /// Broadcasts a single LAN discovery beacon on UDP `:42069` and listens for any peer responses
    /// during `listen_window`.
    pub async fn broadcast_and_scan(&self, listen_window: Duration) -> Vec<DiscoveredMeshPeer> {
        if let Ok(sock) = UdpSocket::bind(("0.0.0.0", 0)).await {
            let _ = sock.set_broadcast(true);
            let pin = self.pairing_pin().await;
            let beacon = MeshBeaconPacket {
                magic: "GIGACHAD_MESH_V1".to_string(),
                peer_id: self.local_peer_id.clone(),
                device_name: self.local_device_name.clone(),
                platform: "Windows-Desktop".to_string(),
                uplink_medium: "Wi-Fi-6 + GbE".to_string(),
                advertised_bandwidth_mbps: 286,
                battery_pct: 100,
                tunnel_port: MESH_TUNNEL_TCP_PORT,
                pairing_digest_hex: compute_pairing_digest(&self.local_peer_id, &pin),
            };
            if let Ok(bytes) = serde_json::to_vec(&beacon) {
                let _ = sock
                    .send_to(&bytes, ("255.255.255.255", MESH_DISCOVERY_UDP_PORT))
                    .await;
            }

            let deadline = Instant::now() + listen_window;
            let mut buf = [0u8; 2048];
            while Instant::now() < deadline {
                let rem = deadline.saturating_duration_since(Instant::now());
                let Ok(Ok((n, src))) = tokio::time::timeout(rem, sock.recv_from(&mut buf)).await
                else {
                    break;
                };
                self.ingest_beacon_bytes(&buf[..n], src.ip()).await;
            }
        }
        self.discovered_peers().await
    }

    /// Starts the background UDP `:42069` beacon listener/broadcaster and TCP `:42070` mesh tunnel server.
    pub fn spawn_background_service(&self) {
        let engine_udp = self.clone();
        tokio::spawn(async move {
            let Ok(sock) = UdpSocket::bind(("0.0.0.0", MESH_DISCOVERY_UDP_PORT)).await else {
                return;
            };
            let _ = sock.set_broadcast(true);
            let mut buf = [0u8; 2048];
            let mut broadcast_tick = tokio::time::interval(Duration::from_secs(4));

            loop {
                tokio::select! {
                    _ = broadcast_tick.tick() => {
                        let pin = engine_udp.pairing_pin().await;
                        let beacon = MeshBeaconPacket {
                            magic: "GIGACHAD_MESH_V1".to_string(),
                            peer_id: engine_udp.local_peer_id.clone(),
                            device_name: engine_udp.local_device_name.clone(),
                            platform: "Windows-Desktop".to_string(),
                            uplink_medium: "Wi-Fi-6 + Multi-NIC".to_string(),
                            advertised_bandwidth_mbps: 286,
                            battery_pct: 100,
                            tunnel_port: MESH_TUNNEL_TCP_PORT,
                            pairing_digest_hex: compute_pairing_digest(&engine_udp.local_peer_id, &pin),
                        };
                        if let Ok(bytes) = serde_json::to_vec(&beacon) {
                            let _ = sock.send_to(&bytes, ("255.255.255.255", MESH_DISCOVERY_UDP_PORT)).await;
                        }
                    }
                    recv = sock.recv_from(&mut buf) => {
                        if let Ok((n, src)) = recv {
                            engine_udp.ingest_beacon_bytes(&buf[..n], src.ip()).await;
                        }
                    }
                }
            }
        });

        let engine_tcp = self.clone();
        tokio::spawn(async move {
            let Ok(listener) = TcpListener::bind(("0.0.0.0", MESH_TUNNEL_TCP_PORT)).await else {
                return;
            };
            loop {
                let Ok((mut client, _)) = listener.accept().await else {
                    continue;
                };
                let pin_check = engine_tcp.pairing_pin().await;
                tokio::spawn(async move {
                    // Mesh tunnel protocol: "CONNECT <host>:<port> PIN:<pin>\n"
                    let mut line_buf = [0u8; 512];
                    let Ok(n) = client.read(&mut line_buf).await else {
                        return;
                    };
                    let header = String::from_utf8_lossy(&line_buf[..n]);
                    let parts: Vec<&str> = header.trim().split_whitespace().collect();
                    if parts.len() >= 3 && parts[0] == "CONNECT" {
                        let target = parts[1];
                        let provided_pin = parts[2].strip_prefix("PIN:").unwrap_or("");
                        if provided_pin == pin_check {
                            if let Ok(mut upstream) = TcpStream::connect(target).await {
                                let _ = client.write_all(b"OK\n").await;
                                let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
                            }
                        } else {
                            let _ = client.write_all(b"ERR_INVALID_PIN\n").await;
                        }
                    }
                });
            }
        });
    }

    async fn ingest_beacon_bytes(&self, data: &[u8], src_ip: IpAddr) {
        let IpAddr::V4(v4) = src_ip else {
            return;
        };
        let Ok(pkt) = serde_json::from_slice::<MeshBeaconPacket>(data) else {
            return;
        };
        if pkt.magic != "GIGACHAD_MESH_V1" || pkt.peer_id == self.local_peer_id {
            return;
        }

        let local_pin = self.pairing_pin().await;
        let expected_digest = compute_pairing_digest(&pkt.peer_id, &local_pin);
        let paired = pkt.pairing_digest_hex == expected_digest;

        let peer = DiscoveredMeshPeer {
            peer_id: pkt.peer_id.clone(),
            device_name: pkt.device_name,
            platform: pkt.platform,
            lan_ip: v4,
            tunnel_port: pkt.tunnel_port,
            uplink_medium: pkt.uplink_medium,
            advertised_bandwidth_mbps: pkt.advertised_bandwidth_mbps,
            battery_pct: pkt.battery_pct,
            paired,
            last_seen_ms_ago: 0,
        };

        let mut guard = self.peers.write().await;
        guard.insert(
            pkt.peer_id,
            PeerRecord {
                info: peer,
                last_seen: Instant::now(),
            },
        );
    }

    /// Connects to a remote `target_host:target_port` through a paired Phone/Laptop mesh peer
    /// over the local Wi-Fi link (`peer.lan_ip:peer.tunnel_port`).
    pub async fn connect_through_peer(
        &self,
        peer: &DiscoveredMeshPeer,
        target_host: &str,
        target_port: u16,
        timeout: Duration,
    ) -> Result<TcpStream, String> {
        let peer_addr = SocketAddr::new(IpAddr::V4(peer.lan_ip), peer.tunnel_port);
        let mut stream = tokio::time::timeout(timeout, TcpStream::connect(peer_addr))
            .await
            .map_err(|_| "Mesh peer connection timed out".to_string())?
            .map_err(|e| e.to_string())?;

        let pin = self.pairing_pin().await;
        let cmd = format!("CONNECT {}:{} PIN:{}\n", target_host, target_port, pin);
        stream
            .write_all(cmd.as_bytes())
            .await
            .map_err(|e| e.to_string())?;

        let mut resp = [0u8; 32];
        let n = tokio::time::timeout(timeout, stream.read(&mut resp))
            .await
            .map_err(|_| "Mesh peer handshake timeout".to_string())?
            .map_err(|e| e.to_string())?;
        let status = String::from_utf8_lossy(&resp[..n]);
        if status.starts_with("OK") {
            Ok(stream)
        } else {
            Err(format!("Mesh peer rejected tunnel: {}", status.trim()))
        }
    }
}

pub fn compute_pairing_digest(peer_id: &str, pin: &str) -> String {
    let payload = format!("{}:GIGACHAD_PIN:{}", peer_id, pin);
    let full = sha256_bytes_hex(payload.as_bytes());
    full[..16].to_string()
}
