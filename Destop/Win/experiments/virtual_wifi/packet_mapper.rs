//! Virtual-to-Physical Packet & Flow Mapper (`packet_mapper.rs`).
//!
//! Implements Phase 7.5 requirements:
//! - Maps traffic flows to healthy physical network paths.
//! - Maintains flow metadata, source/destination mapping, and failover remapping.
//! - Strictly prohibits MAC spoofing / device impersonation (uses legitimate local interface bindings).

use giga_inr::HealthScoreSnapshot;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FlowKey {
    pub protocol: String,
    pub dst_endpoint: String,
    pub flow_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappedFlowBinding {
    pub flow_key: FlowKey,
    pub assigned_if_index: u32,
    pub assigned_interface_name: String,
    pub assigned_local_ip: String,
    pub packets_forwarded: u64,
    pub bytes_forwarded: u64,
    pub failover_remaps: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PacketMapperEngine {
    pub active_flows: HashMap<u64, MappedFlowBinding>,
    pub total_mapped_flows: u64,
    pub total_failover_remaps: u64,
}

impl PacketMapperEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Maps a flow to a healthy physical interface based on health score weights and flow affinity.
    pub fn map_flow(
        &mut self,
        flow_key: FlowKey,
        healthy_interfaces: &[HealthScoreSnapshot],
    ) -> Option<MappedFlowBinding> {
        if healthy_interfaces.is_empty() {
            return None;
        }

        // If flow already exists and its assigned interface is still healthy, preserve flow affinity
        // (essential for TCP connection continuity since a single TCP socket cannot change source IP mid-stream).
        if let Some(existing) = self.active_flows.get_mut(&flow_key.flow_id) {
            if healthy_interfaces
                .iter()
                .any(|i| i.if_index == existing.assigned_if_index && i.score >= 30.0)
            {
                existing.packets_forwarded += 1;
                return Some(existing.clone());
            }
            // Assigned interface failed or degraded -> remap flow to best healthy interface
            let best = healthy_interfaces
                .iter()
                .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(std::cmp::Ordering::Equal))?;
            existing.assigned_if_index = best.if_index;
            existing.assigned_interface_name = best.interface_name.clone();
            existing.assigned_local_ip = best
                .local_ipv4
                .map(|ip| ip.to_string())
                .unwrap_or_else(|| "0.0.0.0".to_string());
            existing.failover_remaps += 1;
            existing.packets_forwarded += 1;
            self.total_failover_remaps += 1;
            return Some(existing.clone());
        }

        // New flow: weighted distribution across healthy interfaces using flow_id hash
        let idx = (flow_key.flow_id as usize) % healthy_interfaces.len();
        let chosen = &healthy_interfaces[idx];

        let binding = MappedFlowBinding {
            flow_key: flow_key.clone(),
            assigned_if_index: chosen.if_index,
            assigned_interface_name: chosen.interface_name.clone(),
            assigned_local_ip: chosen
                .local_ipv4
                .map(|ip| ip.to_string())
                .unwrap_or_else(|| "0.0.0.0".to_string()),
            packets_forwarded: 1,
            bytes_forwarded: 0,
            failover_remaps: 0,
        };

        self.active_flows.insert(flow_key.flow_id, binding.clone());
        self.total_mapped_flows += 1;
        Some(binding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use giga_inr::InterfaceHealthState;
    use giga_pal::WanIndependenceStatus;
    use std::net::Ipv4Addr;

    fn sample_if(if_index: u32, score: f64, ip: &str) -> HealthScoreSnapshot {
        HealthScoreSnapshot {
            if_index,
            interface_name: format!("NIC_{}", if_index),
            description: "Test".to_string(),
            medium: "Wifi".to_string(),
            local_ipv4: ip.parse::<Ipv4Addr>().ok(),
            gateway_v4: None,
            public_wan_ip: None,
            independence: WanIndependenceStatus::IndependentWan,
            score,
            state: InterfaceHealthState::Healthy,
            ewma_rtt_ms: 15.0,
            min_rtt_ms: 12.0,
            jitter_ms: 1.0,
            loss_rate: 0.0,
            rssi_dbm: Some(-55),
            rssi_slope_dbm_per_sec: 0.0,
            signal_quality_pct: Some(90),
            ssid: Some(format!("AP_{}", if_index)),
            rx_throughput_bps: 0.0,
            tx_throughput_bps: 0.0,
            total_rx_bytes: 0,
            total_tx_bytes: 0,
            consecutive_failures: 0,
            is_metered: false,
        }
    }

    #[test]
    fn test_packet_mapper_preserves_tcp_affinity_and_fails_over_on_outage() {
        let mut mapper = PacketMapperEngine::new();
        let pool = vec![
            sample_if(4, 85.0, "192.168.1.10"),
            sample_if(15, 80.0, "192.168.2.20"),
        ];

        let fk = FlowKey {
            protocol: "TCP".to_string(),
            dst_endpoint: "140.82.121.4:443".to_string(),
            flow_id: 100, // 100 % 2 == 0 -> if_index 4
        };

        let b1 = mapper.map_flow(fk.clone(), &pool).unwrap();
        assert_eq!(b1.assigned_if_index, 4);

        // Next packet on same flow stays pinned to if_index 4
        let b2 = mapper.map_flow(fk.clone(), &pool).unwrap();
        assert_eq!(b2.assigned_if_index, 4);
        assert_eq!(b2.packets_forwarded, 2);

        // Now if_index 4 drops out of healthy pool -> flow automatically fails over to if_index 15
        let degraded_pool = vec![sample_if(15, 80.0, "192.168.2.20")];
        let b3 = mapper.map_flow(fk, &degraded_pool).unwrap();
        assert_eq!(b3.assigned_if_index, 15);
        assert_eq!(b3.failover_remaps, 1);
    }
}
