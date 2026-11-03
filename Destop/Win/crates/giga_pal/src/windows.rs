use crate::interface::{
    InterfaceId, InterfaceSnapshot, InterfaceState, InterfaceType, PalError,
    PlatformNetworkManager, WifiTelemetry,
};
use crate::socket::is_virtual_adapter_heuristic;
use std::collections::HashMap;
use std::ffi::{CStr, OsString};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::os::windows::ffi::OsStringExt;
use std::ptr;
use windows_sys::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_SUCCESS, HANDLE};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GAA_FLAG_INCLUDE_GATEWAYS, GAA_FLAG_INCLUDE_PREFIX, GetAdaptersAddresses,
    IF_TYPE_ETHERNET_CSMACD, IF_TYPE_IEEE80211, IF_TYPE_SOFTWARE_LOOPBACK, IF_TYPE_TUNNEL,
    IP_ADAPTER_ADDRESSES_LH,
};
use windows_sys::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows_sys::Win32::NetworkManagement::WiFi::{
    WLAN_CONNECTION_ATTRIBUTES, WLAN_INTERFACE_INFO_LIST, WlanCloseHandle, WlanEnumInterfaces,
    WlanFreeMemory, WlanOpenHandle, WlanQueryInterface, wlan_intf_opcode_current_connection,
    wlan_intf_opcode_rssi, wlan_interface_state_connected,
};
use windows_sys::Win32::Networking::WinSock::{
    AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR, SOCKADDR_IN, SOCKADDR_IN6,
};

const IF_TYPE_WWANPP: u32 = 243;
const IF_TYPE_WWANPP2: u32 = 244;

pub struct WindowsNetworkManager;

impl WindowsNetworkManager {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsNetworkManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformNetworkManager for WindowsNetworkManager {
    fn enumerate_interfaces(&self) -> Result<Vec<InterfaceSnapshot>, PalError> {
        let wifi_map = query_all_wifi_telemetry();
        let mut buf_len: u32 = 32 * 1024;
        let mut buffer: Vec<u8> = vec![0u8; buf_len as usize];

        let flags = GAA_FLAG_INCLUDE_PREFIX | GAA_FLAG_INCLUDE_GATEWAYS;

        loop {
            let ret = unsafe {
                GetAdaptersAddresses(
                    AF_UNSPEC as u32,
                    flags,
                    ptr::null(),
                    buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH,
                    &mut buf_len,
                )
            };

            if ret == ERROR_BUFFER_OVERFLOW {
                buffer.resize(buf_len as usize, 0);
                continue;
            } else if ret != ERROR_SUCCESS {
                return Err(PalError::OsError(format!(
                    "GetAdaptersAddresses failed with Win32 error code {}",
                    ret
                )));
            }
            break;
        }

        let mut snapshots = Vec::new();
        let mut current_ptr = buffer.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;

        while !current_ptr.is_null() {
            let adapter = unsafe { &*current_ptr };

            let adapter_guid = unsafe { cstr_to_string(adapter.AdapterName) };
            let friendly_name = unsafe { pwstr_to_string(adapter.FriendlyName) };
            let description = unsafe { pwstr_to_string(adapter.Description) };

            let mac_len = (adapter.PhysicalAddressLength as usize).min(adapter.PhysicalAddress.len());
            let mac_address = if mac_len > 0 {
                adapter.PhysicalAddress[..mac_len]
                    .iter()
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<_>>()
                    .join(":")
            } else {
                String::from("00:00:00:00:00:00")
            };

            let if_index = unsafe { adapter.Anonymous1.Anonymous.IfIndex };
            let is_up = adapter.OperStatus == IfOperStatusUp;

            // Collect Unicast IPv4 and IPv6 addresses
            let mut ipv4_addresses = Vec::new();
            let mut ipv6_addresses = Vec::new();
            let mut unicast_ptr = adapter.FirstUnicastAddress;
            while !unicast_ptr.is_null() {
                let unicast = unsafe { &*unicast_ptr };
                if let Some(ip) = unsafe { sockaddr_to_ipaddr(unicast.Address.lpSockaddr) } {
                    match ip {
                        IpAddr::V4(v4) => ipv4_addresses.push(v4),
                        IpAddr::V6(v6) => ipv6_addresses.push(v6),
                    }
                }
                unicast_ptr = unicast.Next;
            }

            // Collect Gateway addresses
            let mut gateways = Vec::new();
            let mut gw_ptr = adapter.FirstGatewayAddress;
            while !gw_ptr.is_null() {
                let gw = unsafe { &*gw_ptr };
                if let Some(ip) = unsafe { sockaddr_to_ipaddr(gw.Address.lpSockaddr) } {
                    gateways.push(ip);
                }
                gw_ptr = gw.Next;
            }

            // Collect DNS Server addresses
            let mut dns_servers = Vec::new();
            let mut dns_ptr = adapter.FirstDnsServerAddress;
            while !dns_ptr.is_null() {
                let dns = unsafe { &*dns_ptr };
                if let Some(ip) = unsafe { sockaddr_to_ipaddr(dns.Address.lpSockaddr) } {
                    dns_servers.push(ip);
                }
                dns_ptr = dns.Next;
            }

            let is_virtual_or_loopback = adapter.IfType == IF_TYPE_SOFTWARE_LOOPBACK
                || adapter.IfType == IF_TYPE_TUNNEL
                || is_virtual_adapter_heuristic(&friendly_name, &description);

            let normalized_guid = normalize_guid_key(&adapter_guid);
            let desc_lower = description.to_ascii_lowercase();

            let if_type = if is_virtual_or_loopback {
                InterfaceType::VirtualOrLoopback
            } else if adapter.IfType == IF_TYPE_IEEE80211 {
                let telemetry = wifi_map.get(&normalized_guid).cloned();
                InterfaceType::WiFi(telemetry)
            } else if adapter.IfType == IF_TYPE_WWANPP || adapter.IfType == IF_TYPE_WWANPP2 {
                InterfaceType::Cellular {
                    carrier_or_desc: description.clone(),
                }
            } else if adapter.IfType == IF_TYPE_ETHERNET_CSMACD {
                if desc_lower.contains("rndis")
                    || desc_lower.contains("ndis")
                    || desc_lower.contains("usb")
                    || desc_lower.contains("apple mobile")
                    || desc_lower.contains("android")
                {
                    InterfaceType::UsbTethering
                } else {
                    InterfaceType::Ethernet
                }
            } else {
                InterfaceType::Unknown(adapter.IfType)
            };

            let is_metered = matches!(
                if_type,
                InterfaceType::Cellular { .. } | InterfaceType::UsbTethering
            );

            let tx_speed = if adapter.TransmitLinkSpeed == u64::MAX {
                0
            } else {
                adapter.TransmitLinkSpeed
            };
            let rx_speed = if adapter.ReceiveLinkSpeed == u64::MAX {
                0
            } else {
                adapter.ReceiveLinkSpeed
            };

            let state = if !is_up {
                InterfaceState::Offline
            } else if !is_virtual_or_loopback && !ipv4_addresses.is_empty() && !gateways.is_empty() {
                InterfaceState::Discovered
            } else {
                InterfaceState::Excluded
            };

            snapshots.push(InterfaceSnapshot {
                id: InterfaceId(adapter_guid),
                if_index,
                friendly_name,
                description,
                mac_address,
                if_type,
                state,
                is_up,
                is_virtual_or_loopback,
                is_metered,
                ipv4_addresses,
                ipv6_addresses,
                gateways,
                dns_servers,
                tx_link_speed_bps: tx_speed,
                rx_link_speed_bps: rx_speed,
            });

            current_ptr = adapter.Next;
        }

        // Sort so WAN-eligible physical adapters appear first
        snapshots.sort_by_key(|s| (!s.is_wan_eligible(), !s.is_up, s.if_index));
        Ok(snapshots)
    }
}

/// Queries Windows Native Wifi (`wlanapi.dll`) for all wireless interfaces, returning a map
/// from normalized GUID string -> `WifiTelemetry` (SSID, BSSID, RSSI dBm, Signal Quality %, PHY speeds).
fn query_all_wifi_telemetry() -> HashMap<String, WifiTelemetry> {
    let mut map = HashMap::new();
    let mut negotiated_version: u32 = 0;
    let mut client_handle: HANDLE = ptr::null_mut();

    let status = unsafe { WlanOpenHandle(2, ptr::null(), &mut negotiated_version, &mut client_handle) };
    if status != ERROR_SUCCESS || client_handle.is_null() {
        return map;
    }

    let mut iface_list_ptr: *mut WLAN_INTERFACE_INFO_LIST = ptr::null_mut();
    let enum_status =
        unsafe { WlanEnumInterfaces(client_handle, ptr::null(), &mut iface_list_ptr) };

    if enum_status == ERROR_SUCCESS && !iface_list_ptr.is_null() {
        let count = unsafe { (*iface_list_ptr).dwNumberOfItems } as usize;
        let first_info_ptr = unsafe { (*iface_list_ptr).InterfaceInfo.as_ptr() };

        for i in 0..count {
            let info = unsafe { &*first_info_ptr.add(i) };
            if info.isState != wlan_interface_state_connected {
                continue;
            }

            let guid_str = format_guid(&info.InterfaceGuid);

            // 1. Query current connection attributes (SSID, BSSID, Signal Quality, Rx/Tx PHY rates)
            let mut conn_data_size: u32 = 0;
            let mut conn_data_ptr: *mut core::ffi::c_void = ptr::null_mut();
            let mut opcode_value_type: i32 = 0;

            let q_conn = unsafe {
                WlanQueryInterface(
                    client_handle,
                    &info.InterfaceGuid,
                    wlan_intf_opcode_current_connection,
                    ptr::null(),
                    &mut conn_data_size,
                    &mut conn_data_ptr,
                    &mut opcode_value_type,
                )
            };

            let mut ssid = None;
            let mut bssid = None;
            let mut signal_quality_pct = 0u32;
            let mut rx_phy_rate_bps = 0u64;
            let mut tx_phy_rate_bps = 0u64;

            if q_conn == ERROR_SUCCESS
                && !conn_data_ptr.is_null()
                && (conn_data_size as usize) >= std::mem::size_of::<WLAN_CONNECTION_ATTRIBUTES>()
            {
                let attrs = unsafe { &*(conn_data_ptr as *const WLAN_CONNECTION_ATTRIBUTES) };
                let dot11_ssid = &attrs.wlanAssociationAttributes.dot11Ssid;
                let ssid_len = (dot11_ssid.uSSIDLength as usize).min(dot11_ssid.ucSSID.len());
                if ssid_len > 0 {
                    ssid = Some(
                        String::from_utf8_lossy(&dot11_ssid.ucSSID[..ssid_len]).into_owned(),
                    );
                }

                let bssid_bytes = &attrs.wlanAssociationAttributes.dot11Bssid;
                bssid = Some(
                    bssid_bytes
                        .iter()
                        .map(|b| format!("{:02X}", b))
                        .collect::<Vec<_>>()
                        .join(":"),
                );

                signal_quality_pct = attrs.wlanAssociationAttributes.wlanSignalQuality.min(100);
                // ulRxRate and ulTxRate in WLAN_ASSOCIATION_ATTRIBUTES are reported in kbps
                rx_phy_rate_bps = (attrs.wlanAssociationAttributes.ulRxRate as u64) * 1000;
                tx_phy_rate_bps = (attrs.wlanAssociationAttributes.ulTxRate as u64) * 1000;

                unsafe { WlanFreeMemory(conn_data_ptr) };
            }

            // 2. Query raw RSSI in dBm via wlan_intf_opcode_rssi (with linear fallback from signal_quality_pct)
            let mut rssi_data_size: u32 = 0;
            let mut rssi_data_ptr: *mut core::ffi::c_void = ptr::null_mut();
            let q_rssi = unsafe {
                WlanQueryInterface(
                    client_handle,
                    &info.InterfaceGuid,
                    wlan_intf_opcode_rssi,
                    ptr::null(),
                    &mut rssi_data_size,
                    &mut rssi_data_ptr,
                    &mut opcode_value_type,
                )
            };

            let rssi_dbm = if q_rssi == ERROR_SUCCESS
                && !rssi_data_ptr.is_null()
                && (rssi_data_size as usize) >= std::mem::size_of::<i32>()
            {
                let val = unsafe { *(rssi_data_ptr as *const i32) };
                unsafe { WlanFreeMemory(rssi_data_ptr) };
                val
            } else {
                // Standard Windows NDIS interpolation: 0% = -100 dBm, 100% = -50 dBm
                -100 + (signal_quality_pct as i32 / 2)
            };

            map.insert(
                guid_str,
                WifiTelemetry {
                    ssid,
                    bssid,
                    rssi_dbm,
                    signal_quality_pct,
                    rx_phy_rate_bps,
                    tx_phy_rate_bps,
                },
            );
        }

        unsafe { WlanFreeMemory(iface_list_ptr as *const core::ffi::c_void) };
    }

    unsafe { WlanCloseHandle(client_handle, ptr::null()) };
    map
}

fn format_guid(g: &windows_sys::core::GUID) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        g.data1,
        g.data2,
        g.data3,
        g.data4[0],
        g.data4[1],
        g.data4[2],
        g.data4[3],
        g.data4[4],
        g.data4[5],
        g.data4[6],
        g.data4[7]
    )
}

fn normalize_guid_key(s: &str) -> String {
    s.trim_matches(|c| c == '{' || c == '}').to_ascii_lowercase()
}

unsafe fn cstr_to_string(ptr: *mut u8) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr as *const i8) }
        .to_string_lossy()
        .into_owned()
}

unsafe fn pwstr_to_string(ptr: *mut u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        OsString::from_wide(slice).to_string_lossy().into_owned()
    }
}

unsafe fn sockaddr_to_ipaddr(sa: *mut SOCKADDR) -> Option<IpAddr> {
    if sa.is_null() {
        return None;
    }
    let family = unsafe { (*sa).sa_family };
    if family == AF_INET {
        let sin = unsafe { &*(sa as *const SOCKADDR_IN) };
        let bytes = unsafe { sin.sin_addr.S_un.S_un_b };
        Some(IpAddr::V4(Ipv4Addr::new(
            bytes.s_b1, bytes.s_b2, bytes.s_b3, bytes.s_b4,
        )))
    } else if family == AF_INET6 {
        let sin6 = unsafe { &*(sa as *const SOCKADDR_IN6) };
        let bytes = unsafe { sin6.sin6_addr.u.Byte };
        Some(IpAddr::V6(Ipv6Addr::from(bytes)))
    } else {
        None
    }
}

