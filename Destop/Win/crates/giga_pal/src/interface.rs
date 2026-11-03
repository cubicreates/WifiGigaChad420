use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::{ IpAddr, Ipv4Addr, Ipv6Addr };
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PalError {
    #[error("OS API call failed: {0}")]
    OsError(String),
    #[error("Interface '{0}' has no routable IPv4 address")]
    NoIpv4Address(String),
    #[error("Socket binding failed on interface '{interface}': {source}")]
    SocketBindError {
        interface: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Bound TCP connection timed out after {0} ms")]
    ConnectTimeout(u64),
}

/// Unique hardware/OS identifier for a network adapter (e.g., Windows Adapter GUID or interface name).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct InterfaceId(pub String);

impl fmt::Display for InterfaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Real-time 802.11 wireless link telemetry extracted from OS WLAN APIs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WifiTelemetry {
    pub ssid: Option<String>,
    pub bssid: Option<String>,
    pub rssi_dbm: i32,
    pub signal_quality_pct: u32,
    pub rx_phy_rate_bps: u64,
    pub tx_phy_rate_bps: u64,
}

/// Physical or logical medium classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InterfaceType {
    Ethernet,
    WiFi(Option<WifiTelemetry>),
    Cellular { carrier_or_desc: String },
    UsbTethering,
    VirtualOrLoopback,
    Unknown(u32),
}

impl InterfaceType {
    pub fn short_label(&self) -> &'static str {
        match self {
            InterfaceType::Ethernet => "Ethernet",
            InterfaceType::WiFi(_) => "Wi-Fi",
            InterfaceType::Cellular { .. } => "Cellular",
            InterfaceType::UsbTethering => "USB-Tether",
            InterfaceType::VirtualOrLoopback => "Virtual/Loopback",
            InterfaceType::Unknown(_) => "Unknown",
        }
    }
}

/// The 7-state network health state machine states managed by the Intelligent Network Resolver (INR).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterfaceState {
    Discovered,
    Testing,
    Active,
    Degraded,
    Excluded,
    Offline,
    Recovering,
}

impl fmt::Display for InterfaceState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            InterfaceState::Discovered => "DISCOVERED",
            InterfaceState::Testing => "TESTING",
            InterfaceState::Active => "ACTIVE",
            InterfaceState::Degraded => "DEGRADED",
            InterfaceState::Excluded => "EXCLUDED",
            InterfaceState::Offline => "OFFLINE",
            InterfaceState::Recovering => "RECOVERING",
        };
        write!(f, "{}", s)
    }
}

/// Point-in-time hardware and Layer 2 / Layer 3 snapshot of an OS network interface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceSnapshot {
    pub id: InterfaceId,
    pub if_index: u32,
    pub friendly_name: String,
    pub description: String,
    pub mac_address: String,
    pub if_type: InterfaceType,
    pub state: InterfaceState,
    pub is_up: bool,
    pub is_virtual_or_loopback: bool,
    pub is_metered: bool,
    pub ipv4_addresses: Vec<Ipv4Addr>,
    pub ipv6_addresses: Vec<Ipv6Addr>,
    pub gateways: Vec<IpAddr>,
    pub dns_servers: Vec<IpAddr>,
    pub tx_link_speed_bps: u64,
    pub rx_link_speed_bps: u64,
}

impl InterfaceSnapshot {
    /// Returns the primary routable IPv4 address of this interface, if available.
    pub fn primary_ipv4(&self) -> Option<Ipv4Addr> {
        self.ipv4_addresses
            .iter()
            .copied()
            .find(|ip| !ip.is_loopback() && !ip.is_link_local() && !ip.is_unspecified())
    }

    /// Returns the primary IPv4 default gateway of this interface, if configured.
    pub fn primary_ipv4_gateway(&self) -> Option<Ipv4Addr> {
        self.gateways.iter().find_map(|gw| match gw {
            IpAddr::V4(v4) if !v4.is_unspecified() => Some(*v4),
            _ => None,
        })
    }

    /// Returns true if this adapter is a candidate for multi-interface WAN aggregation
    /// (Up, not a virtual switch/loopback, has a routable IPv4 and a default gateway).
    pub fn is_wan_eligible(&self) -> bool {
        self.is_up
            && !self.is_virtual_or_loopback
            && self.primary_ipv4().is_some()
            && self.primary_ipv4_gateway().is_some()
    }
}

#[async_trait::async_trait]
pub trait PlatformNetworkManager: Send + Sync {
    /// Enumerates all network interfaces on the host (both physical and virtual).
    fn enumerate_interfaces(&self) -> Result<Vec<InterfaceSnapshot>, PalError>;

    /// Enumerates only physical, WAN-eligible interfaces that are currently UP with a valid IP and gateway.
    fn enumerate_wan_candidates(&self) -> Result<Vec<InterfaceSnapshot>, PalError> {
        let all = self.enumerate_interfaces()?;
        Ok(all.into_iter().filter(|i| i.is_wan_eligible()).collect())
    }
}

