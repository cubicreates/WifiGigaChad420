//! Physical & Virtual Adapter Discovery Manager (`adapter_manager.rs`).
//!
//! Strictly separates (Rule 2):
//! - Physical radio adapters (`MediaTek Wi-Fi 6 MT7921`, USB Wi-Fi dongles, Ethernet, USB RNDIS)
//! - Windows Wi-Fi Direct virtual ports (`Microsoft Wi-Fi Direct Virtual Adapter` `VWIFIMP_WFD`)
//! - Logical Virtual Interface (`WiFi GigaChad 420 Virtual Network`)

use giga_pal::{create_platform_manager, InterfaceSnapshot, InterfaceType};
use serde::{Deserialize, Serialize};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn hidden_cmd(prog: &str) -> Command {
    let mut c = Command::new(prog);
    #[cfg(windows)]
    {
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdapterCategory {
    PhysicalWifiRadio,
    PhysicalEthernetOrUsbTether,
    WifiDirectVirtualPort,
    LogicalVirtualNetwork,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredAdapterInfo {
    pub if_index: u32,
    pub name: String,
    pub description: String,
    pub category: AdapterCategory,
    pub is_connected: bool,
    pub connected_ssid: Option<String>,
    pub channel: Option<u32>,
    pub band: Option<String>,
    pub local_ipv4: Option<String>,
    pub mac_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterTopologySnapshot {
    pub physical_wifi_radios: usize,
    pub actual_connected_wifi_networks: usize,
    pub active_wired_or_usb_tethers: usize,
    pub wifi_direct_virtual_ports: usize,
    pub logical_virtual_interfaces: usize,
    pub current_active_channel: Option<u32>,
    pub current_active_network: Option<String>,
    pub adapters: Vec<DiscoveredAdapterInfo>,
}

pub fn discover_adapter_topology(virtual_interface_enabled: bool) -> AdapterTopologySnapshot {
    let pal = create_platform_manager();
    let all_ifaces: Vec<InterfaceSnapshot> = pal.enumerate_interfaces().unwrap_or_default();

    // Parse channel & band from `netsh wlan show interfaces`
    let wlan_if_raw = hidden_cmd("netsh")
        .args(["wlan", "show", "interfaces"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let (active_channel, active_band, active_ssid) = parse_wlan_interface_channel(&wlan_if_raw);

    let mut adapters = Vec::new();
    let mut physical_wifi_radios = 0usize;
    let mut actual_connected_wifi_networks = 0usize;
    let mut active_wired_or_usb_tethers = 0usize;
    let mut wifi_direct_virtual_ports = 0usize;

    for iface in &all_ifaces {
        let desc_lower = iface.description.to_ascii_lowercase();
        if desc_lower.contains("wi-fi direct virtual adapter") {
            wifi_direct_virtual_ports += 1;
            adapters.push(DiscoveredAdapterInfo {
                if_index: iface.if_index,
                name: iface.friendly_name.clone(),
                description: iface.description.clone(),
                category: AdapterCategory::WifiDirectVirtualPort,
                is_connected: iface.is_up,
                connected_ssid: None,
                channel: None,
                band: None,
                local_ipv4: iface.primary_ipv4().map(|ip| ip.to_string()),
                mac_address: iface.mac_address.clone(),
            });
            continue;
        }

        if iface.is_virtual_or_loopback
            || desc_lower.contains("wan miniport")
            || desc_lower.contains("hyper-v")
            || desc_lower.contains("teredo")
            || desc_lower.contains("kernel debug")
        {
            continue;
        }

        let wifi_ssid_from_pal = match &iface.if_type {
            InterfaceType::WiFi(Some(w)) => w.ssid.clone(),
            _ => None,
        };
        let is_wifi_medium = matches!(iface.if_type, InterfaceType::WiFi(_))
            || desc_lower.contains("wi-fi")
            || desc_lower.contains("wireless")
            || desc_lower.contains("802.11");

        if is_wifi_medium {
            physical_wifi_radios += 1;
            let ssid = wifi_ssid_from_pal.or_else(|| active_ssid.clone());
            let is_conn = iface.is_up && ssid.is_some();
            if is_conn {
                actual_connected_wifi_networks += 1;
            }
            adapters.push(DiscoveredAdapterInfo {
                if_index: iface.if_index,
                name: iface.friendly_name.clone(),
                description: iface.description.clone(),
                category: AdapterCategory::PhysicalWifiRadio,
                is_connected: is_conn,
                connected_ssid: ssid,
                channel: active_channel,
                band: active_band.clone(),
                local_ipv4: iface.primary_ipv4().map(|ip| ip.to_string()),
                mac_address: iface.mac_address.clone(),
            });
        } else {
            if iface.is_up && iface.primary_ipv4().is_some() {
                active_wired_or_usb_tethers += 1;
            }
            adapters.push(DiscoveredAdapterInfo {
                if_index: iface.if_index,
                name: iface.friendly_name.clone(),
                description: iface.description.clone(),
                category: AdapterCategory::PhysicalEthernetOrUsbTether,
                is_connected: iface.is_up && iface.primary_ipv4().is_some(),
                connected_ssid: None,
                channel: None,
                band: None,
                local_ipv4: iface.primary_ipv4().map(|ip| ip.to_string()),
                mac_address: iface.mac_address.clone(),
            });
        }
    }

    let logical_virtual_interfaces = if virtual_interface_enabled { 1 } else { 0 };
    if virtual_interface_enabled {
        adapters.insert(
            0,
            DiscoveredAdapterInfo {
                if_index: 42000,
                name: "WiFi GigaChad 420 Virtual Network".to_string(),
                description: "WiFi GigaChad 420 Virtual Network Interface (User-Mode Logical Aggregator)".to_string(),
                category: AdapterCategory::LogicalVirtualNetwork,
                is_connected: actual_connected_wifi_networks > 0 || active_wired_or_usb_tethers > 0,
                connected_ssid: active_ssid.clone(),
                channel: active_channel,
                band: active_band.clone(),
                local_ipv4: Some("10.240.0.1".to_string()),
                mac_address: "02:47:43:04:20:01".to_string(),
            },
        );
    }

    AdapterTopologySnapshot {
        physical_wifi_radios,
        actual_connected_wifi_networks,
        active_wired_or_usb_tethers,
        wifi_direct_virtual_ports,
        logical_virtual_interfaces,
        current_active_channel: active_channel,
        current_active_network: active_ssid,
        adapters,
    }
}

pub fn parse_wlan_interface_channel(raw: &str) -> (Option<u32>, Option<String>, Option<String>) {
    let mut channel = None;
    let mut band = None;
    let mut ssid = None;
    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some((k, v)) = trimmed.split_once(':') {
            let key = k.trim();
            let val = v.trim();
            match key {
                "Channel" => channel = val.parse::<u32>().ok(),
                "Band" => band = Some(val.to_string()),
                "SSID" if !val.is_empty() => ssid = Some(val.to_string()),
                _ => {}
            }
        }
    }
    (channel, band, ssid)
}
