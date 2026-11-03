pub mod independence;
pub mod interface;
pub mod socket;

#[cfg(windows)]
pub mod windows;

pub use independence::{IndependenceReport, WanIndependenceStatus, verify_pool_independence};
pub use interface::{
    InterfaceId, InterfaceSnapshot, InterfaceState, InterfaceType, PalError,
    PlatformNetworkManager, WifiTelemetry,
};
pub use socket::{
    BoundConnectResult, connect_bound_tcp_v4, create_bound_tcp_socket_v4,
    create_bound_udp_socket_v4,
};

/// Returns the default platform-specific network manager for the current OS.
pub fn create_platform_manager() -> Box<dyn PlatformNetworkManager> {
    #[cfg(windows)]
    {
        Box::new(windows::WindowsNetworkManager::new())
    }
    #[cfg(not(windows))]
    {
        unimplemented!("Non-Windows PAL implementation is scheduled for Milestone 6 (Linux/macOS)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enumerate_interfaces_returns_adapters() {
        let mgr = create_platform_manager();
        let interfaces = mgr
            .enumerate_interfaces()
            .expect("should enumerate interfaces on Windows");
        assert!(
            !interfaces.is_empty(),
            "Expected at least one network interface on host"
        );
    }

    #[tokio::test]
    async fn test_bound_socket_creation_on_wan_candidates() {
        let mgr = create_platform_manager();
        let candidates = mgr
            .enumerate_wan_candidates()
            .expect("should enumerate WAN candidates");
        for iface in &candidates {
            let sock = create_bound_tcp_socket_v4(iface);
            assert!(
                sock.is_ok(),
                "Failed to create IP_UNICAST_IF bound socket on {}: {:?}",
                iface.friendly_name,
                sock.err()
            );
        }
    }
}
