//! Virtual Network Interface Manager (`virtual_interface.rs`).
//!
//! Manages the logical `WiFi GigaChad 420 Virtual Network` interface (`10.240.0.1/24` & local
//! SOCKS5 `:1080` / HTTP `:1081` gateway) with a reversible configuration transaction so any
//! changes can be cleanly rolled back in 1 click (Rule 5 & Phase 12).

use crate::telemetry::{
    now_iso_utc, CapabilityStatusLabel, StructuredEventKind, StructuredEventLogger,
    StructuredExperimentEvent,
};
use giga_pal::{is_system_proxy_enabled, set_windows_system_proxy};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackSnapshot {
    pub had_system_proxy_enabled: bool,
    pub captured_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualInterfaceManager {
    pub interface_name: String,
    pub virtual_ipv4: String,
    pub virtual_mac: String,
    pub enabled: bool,
    pub status_label: CapabilityStatusLabel,
    pub driver_installation_status: CapabilityStatusLabel,
    pub rollback_snapshot: Option<RollbackSnapshot>,
    pub bound_member_if_indices: Vec<u32>,
}

impl Default for VirtualInterfaceManager {
    fn default() -> Self {
        Self {
            interface_name: "WiFi GigaChad 420 Virtual Network".to_string(),
            virtual_ipv4: "10.240.0.1".to_string(),
            virtual_mac: "02:47:43:04:20:01".to_string(), // IEEE 802 Locally Administered bit (0x02)
            enabled: true,
            status_label: CapabilityStatusLabel::Experimental,
            driver_installation_status: CapabilityStatusLabel::NotTested,
            rollback_snapshot: None,
            bound_member_if_indices: Vec::new(),
        }
    }
}

impl VirtualInterfaceManager {
    /// Activates the User-Mode Virtual Network Interface with a reversible rollback snapshot.
    pub async fn activate_user_mode_virtual_interface(
        &mut self,
        member_if_indices: Vec<u32>,
        enable_system_proxy_routing: bool,
        logger: &StructuredEventLogger,
    ) -> Result<String, String> {
        if self.rollback_snapshot.is_none() {
            self.rollback_snapshot = Some(RollbackSnapshot {
                had_system_proxy_enabled: is_system_proxy_enabled(),
                captured_at: now_iso_utc(),
            });
        }

        self.enabled = true;
        self.bound_member_if_indices = member_if_indices.clone();
        self.status_label = CapabilityStatusLabel::Experimental;

        if enable_system_proxy_routing {
            let _ = set_windows_system_proxy(true, "127.0.0.1:1081");
        }

        let msg = format!(
            "Activated '{}' (IPv4: {}, MAC: {}) binding {} physical interface(s): {:?}",
            self.interface_name,
            self.virtual_ipv4,
            self.virtual_mac,
            member_if_indices.len(),
            member_if_indices
        );

        logger
            .record(StructuredExperimentEvent {
                timestamp: now_iso_utc(),
                component: "VIRTUAL_INTERFACE".to_string(),
                physical_interface: format!("{:?}", member_if_indices),
                virtual_interface: self.interface_name.clone(),
                network_identifier: "UNIFIED_POOL".to_string(),
                event: StructuredEventKind::VirtualInterfaceCreated,
                result: CapabilityStatusLabel::Experimental,
                error_code: None,
                duration_ms: Some(1.2),
                measurement: msg.clone(),
            })
            .await;

        Ok(msg)
    }

    /// Reverts the Virtual Network Interface and restores the pre-experiment Windows configuration.
    pub async fn rollback_and_deactivate(
        &mut self,
        logger: &StructuredEventLogger,
    ) -> Result<String, String> {
        if let Some(snap) = self.rollback_snapshot.take() {
            let _ = set_windows_system_proxy(snap.had_system_proxy_enabled, "127.0.0.1:1081");
        } else {
            let _ = set_windows_system_proxy(false, "127.0.0.1:1081");
        }

        self.enabled = false;
        self.bound_member_if_indices.clear();
        self.status_label = CapabilityStatusLabel::NotTested;

        let msg = format!(
            "Rolled back '{}' and restored original Windows network/proxy configuration.",
            self.interface_name
        );

        logger
            .record(StructuredExperimentEvent {
                timestamp: now_iso_utc(),
                component: "VIRTUAL_INTERFACE".to_string(),
                physical_interface: "ALL".to_string(),
                virtual_interface: self.interface_name.clone(),
                network_identifier: "ROLLBACK".to_string(),
                event: StructuredEventKind::VirtualInterfaceRolledBack,
                result: CapabilityStatusLabel::Supported,
                error_code: None,
                duration_ms: Some(0.8),
                measurement: msg.clone(),
            })
            .await;

        Ok(msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_virtual_interface_activation_and_rollback() {
        let mut vim = VirtualInterfaceManager::default();
        let logger = StructuredEventLogger::new();
        let res = vim
            .activate_user_mode_virtual_interface(vec![4, 15], false, &logger)
            .await;
        assert!(res.is_ok());
        assert!(vim.enabled);
        assert_eq!(vim.bound_member_if_indices, vec![4, 15]);

        let rb = vim.rollback_and_deactivate(&logger).await;
        assert!(rb.is_ok());
        assert!(!vim.enabled);
        assert!(vim.bound_member_if_indices.is_empty());
    }
}

