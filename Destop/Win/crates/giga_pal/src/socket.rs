use crate::interface::{InterfaceSnapshot, PalError};
use serde::{Deserialize, Serialize};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::{Duration, Instant};
use tokio::net::{TcpSocket, TcpStream, UdpSocket};

/// Result of a bound TCP connection handshake test on a specific network interface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundConnectResult {
    pub interface_name: String,
    pub if_index: u32,
    pub local_ip: Ipv4Addr,
    pub local_port: u16,
    pub remote_addr: SocketAddr,
    pub handshake_rtt_ms: f64,
}

/// Creates a non-blocking IPv4 TCP socket explicitly bound to the given network interface
/// BEFORE `connect()` is issued.
///
/// On Windows, this performs two strict bindings:
/// 1. `setsockopt(IPPROTO_IP, IP_UNICAST_IF, if_index.to_be())` to force the Windows TCP/IP
///    stack to route outbound SYN packets through this interface's adapter index regardless of
///    default route metrics (Strong Host / Interface-specific routing).
/// 2. `bind(local_ipv4:0)` so the source IP address of the TCP 4-tuple matches the interface.
pub fn create_bound_tcp_socket_v4(iface: &InterfaceSnapshot) -> Result<TcpSocket, PalError> {
    let local_ip = iface
        .primary_ipv4()
        .ok_or_else(|| PalError::NoIpv4Address(iface.friendly_name.clone()))?;

    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP)).map_err(|e| {
        PalError::SocketBindError {
            interface: iface.friendly_name.clone(),
            source: e,
        }
    })?;

    socket
        .set_nonblocking(true)
        .map_err(|e| PalError::SocketBindError {
            interface: iface.friendly_name.clone(),
            source: e,
        })?;

    socket
        .set_nodelay(true)
        .map_err(|e| PalError::SocketBindError {
            interface: iface.friendly_name.clone(),
            source: e,
        })?;

    // Platform-specific strong interface index binding before connect()
    #[cfg(windows)]
    bind_windows_unicast_if(&socket, iface.if_index).map_err(|e| PalError::SocketBindError {
        interface: iface.friendly_name.clone(),
        source: e,
    })?;

    // Bind source IP to the interface's primary IPv4 address (ephemeral port 0)
    let bind_addr = SocketAddrV4::new(local_ip, 0);
    socket
        .bind(&SockAddr::from(bind_addr))
        .map_err(|e| PalError::SocketBindError {
            interface: iface.friendly_name.clone(),
            source: e,
        })?;

    let std_stream: std::net::TcpStream = socket.into();
    Ok(TcpSocket::from_std_stream(std_stream))
}

/// Creates a non-blocking IPv4 UDP socket explicitly bound to the given network interface
/// (`IP_UNICAST_IF` + `bind(local_ipv4:port)`) for interface-specific DNS racing and LAN mesh discovery.
pub fn create_bound_udp_socket_v4(
    iface: &InterfaceSnapshot,
    local_port: u16,
) -> Result<UdpSocket, PalError> {
    let local_ip = iface
        .primary_ipv4()
        .ok_or_else(|| PalError::NoIpv4Address(iface.friendly_name.clone()))?;

    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).map_err(|e| {
        PalError::SocketBindError {
            interface: iface.friendly_name.clone(),
            source: e,
        }
    })?;

    socket
        .set_nonblocking(true)
        .map_err(|e| PalError::SocketBindError {
            interface: iface.friendly_name.clone(),
            source: e,
        })?;

    let _ = socket.set_reuse_address(true);
    let _ = socket.set_broadcast(true);

    #[cfg(windows)]
    bind_windows_unicast_if(&socket, iface.if_index).map_err(|e| PalError::SocketBindError {
        interface: iface.friendly_name.clone(),
        source: e,
    })?;

    let bind_addr = SocketAddrV4::new(local_ip, local_port);
    socket
        .bind(&SockAddr::from(bind_addr))
        .map_err(|e| PalError::SocketBindError {
            interface: iface.friendly_name.clone(),
            source: e,
        })?;

    let std_udp: std::net::UdpSocket = socket.into();
    UdpSocket::from_std(std_udp).map_err(|e| PalError::SocketBindError {
        interface: iface.friendly_name.clone(),
        source: e,
    })
}

#[cfg(windows)]
fn bind_windows_unicast_if(socket: &Socket, if_index: u32) -> std::io::Result<()> {
    use std::os::windows::io::AsRawSocket;
    use windows_sys::Win32::Networking::WinSock::{IP_UNICAST_IF, IPPROTO_IP, setsockopt};

    if if_index == 0 {
        return Ok(());
    }

    // Per Microsoft WinSock documentation, IP_UNICAST_IF for IPv4 expects the 32-bit
    // interface index in network byte order (big-endian).
    let if_index_be: u32 = if_index.to_be();
    let raw_sock = socket.as_raw_socket() as usize;

    let ret = unsafe {
        setsockopt(
            raw_sock,
            IPPROTO_IP as i32,
            IP_UNICAST_IF as i32,
            &if_index_be as *const u32 as *const u8,
            std::mem::size_of::<u32>() as i32,
        )
    };

    if ret != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// Connects a bound IPv4 TCP socket to `remote_addr` strictly through `iface`, returning
/// the connected `TcpStream` and handshake telemetry (including measured SYN -> SYN-ACK RTT).
pub async fn connect_bound_tcp_v4(
    iface: &InterfaceSnapshot,
    remote_addr: SocketAddr,
    timeout: Duration,
) -> Result<(TcpStream, BoundConnectResult), PalError> {
    let local_ip = iface
        .primary_ipv4()
        .ok_or_else(|| PalError::NoIpv4Address(iface.friendly_name.clone()))?;

    let tcp_socket = create_bound_tcp_socket_v4(iface)?;
    let start = Instant::now();

    let stream = tokio::time::timeout(timeout, tcp_socket.connect(remote_addr))
        .await
        .map_err(|_| PalError::ConnectTimeout(timeout.as_millis() as u64))?
        .map_err(|e| PalError::SocketBindError {
            interface: iface.friendly_name.clone(),
            source: e,
        })?;

    let rtt_ms = start.elapsed().as_secs_f64() * 1000.0;
    let local_port = stream
        .local_addr()
        .map(|a| a.port())
        .unwrap_or(0);

    let result = BoundConnectResult {
        interface_name: iface.friendly_name.clone(),
        if_index: iface.if_index,
        local_ip,
        local_port,
        remote_addr,
        handshake_rtt_ms: rtt_ms,
    };

    Ok((stream, result))
}

/// Helper to test whether an interface is a virtual or loopback adapter by name/description.
pub fn is_virtual_adapter_heuristic(friendly_name: &str, description: &str) -> bool {
    let combined = format!(
        "{} {}",
        friendly_name.to_ascii_lowercase(),
        description.to_ascii_lowercase()
    );
    let virtual_keywords = [
        "vethernet",
        "hyper-v",
        "wsl",
        "vmware",
        "virtualbox",
        "vbox",
        "docker",
        "loopback",
        "pseudo-interface",
        "tap-windows",
        "wintun",
        "wireguard",
        "tailscale",
        "zerotier",
        "npcap",
        "bluetooth",
        "wan miniport",
        "wi-fi direct",
        "virtual adapter",
        "teredo",
        "isatap",
    ];
    virtual_keywords.iter().any(|kw| combined.contains(kw))
}

/// Resolves an IPv4 socket address from a raw IP string and port.
pub fn ipv4_sockaddr(ip: Ipv4Addr, port: u16) -> SocketAddr {
    SocketAddr::new(IpAddr::V4(ip), port)
}

