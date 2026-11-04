//! Hysteresis-Controlled Intelligent Network Resolver (`network_resolver.rs`).
//!
//! Implements Phase 7.3 requirements:
//! - Evaluates reachability, DNS latency, RTT, packet loss, signal strength, and stability.
//! - Enforces a switching hysteresis deadband (`hysteresis_margin = 15.0` score points) and
//!   minimum dwell timer (`min_dwell_ms = 10_000`) to prevent oscillation from minor RSSI jitter.
//! - Excludes dead/unhealthy connections (`INR_NETWORK_EXCLUDED`) and re-admits recovered
//!   connections (`INR_NETWORK_RESTORED`).

use crate::telemetry::{
    now_iso_utc, CapabilityStatusLabel, StructuredEventKind, StructuredEventLogger,
    StructuredExperimentEvent,
};
use giga_inr::HealthScoreSnapshot;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HysteresisConfig {
    /// Minimum score improvement required to switch primary interface (prevents flapping)
    pub hysteresis_margin_points: f64,
    /// Minimum score below which an interface is excluded from the pool
    pub exclusion_threshold_score: f64,
    /// Score required for an excluded interface to be restored to the active pool
    pub recovery_threshold_score: f64,
    /// Minimum dwell time (ms) before switching primary path unless current path is excluded
    pub min_dwell_ms: u64,
}

impl Default for HysteresisConfig {
    fn default() -> Self {
        Self {
            hysteresis_margin_points: 15.0,
            exclusion_threshold_score: 30.0,
            recovery_threshold_score: 48.0,
            min_dwell_ms: 10_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HysteresisResolverState {
    pub config: HysteresisConfig,
    pub current_primary_if_index: Option<u32>,
    pub current_primary_score: f64,
    pub excluded_interfaces: HashSet<u32>,
    pub last_switch_elapsed_ms: u64,
    pub switch_count: u64,
}

impl Default for HysteresisResolverState {
    fn default() -> Self {
        Self {
            config: HysteresisConfig::default(),
            current_primary_if_index: None,
            current_primary_score: 0.0,
            excluded_interfaces: HashSet::new(),
            last_switch_elapsed_ms: 15_000,
            switch_count: 0,
        }
    }
}

impl HysteresisResolverState {
    /// Evaluates current interface snapshots with hysteresis deadband and updates exclusions.
    pub async fn evaluate(
        &mut self,
        snapshots: &[HealthScoreSnapshot],
        elapsed_since_last_eval_ms: u64,
        logger: Option<&StructuredEventLogger>,
    ) -> Option<u32> {
        self.last_switch_elapsed_ms = self
            .last_switch_elapsed_ms
            .saturating_add(elapsed_since_last_eval_ms);

        // 1. Check for newly unhealthy or recovered interfaces
        for snap in snapshots {
            let is_currently_excluded = self.excluded_interfaces.contains(&snap.if_index);
            if !is_currently_excluded
                && (snap.score < self.config.exclusion_threshold_score || snap.loss_rate > 0.65)
            {
                self.excluded_interfaces.insert(snap.if_index);
                if let Some(log) = logger {
                    log.record(StructuredExperimentEvent {
                        timestamp: now_iso_utc(),
                        component: "INR_HYSTERESIS".to_string(),
                        physical_interface: snap.interface_name.clone(),
                        virtual_interface: "WiFi GigaChad 420 Virtual Network".to_string(),
                        network_identifier: snap
                            .ssid
                            .clone()
                            .unwrap_or_else(|| snap.interface_name.clone()),
                        event: StructuredEventKind::InrNetworkExcluded,
                        result: CapabilityStatusLabel::Experimental,
                        error_code: None,
                        duration_ms: Some(snap.ewma_rtt_ms),
                        measurement: format!(
                            "Score {:.1} dropped below exclusion threshold {:.1} (Loss: {:.0}%)",
                            snap.score,
                            self.config.exclusion_threshold_score,
                            snap.loss_rate * 100.0
                        ),
                    })
                    .await;
                }
            } else if is_currently_excluded
                && snap.score >= self.config.recovery_threshold_score
                && snap.loss_rate < 0.25
            {
                self.excluded_interfaces.remove(&snap.if_index);
                if let Some(log) = logger {
                    log.record(StructuredExperimentEvent {
                        timestamp: now_iso_utc(),
                        component: "INR_HYSTERESIS".to_string(),
                        physical_interface: snap.interface_name.clone(),
                        virtual_interface: "WiFi GigaChad 420 Virtual Network".to_string(),
                        network_identifier: snap
                            .ssid
                            .clone()
                            .unwrap_or_else(|| snap.interface_name.clone()),
                        event: StructuredEventKind::InrNetworkRestored,
                        result: CapabilityStatusLabel::Supported,
                        error_code: None,
                        duration_ms: Some(snap.ewma_rtt_ms),
                        measurement: format!(
                            "Score {:.1} recovered above recovery threshold {:.1}",
                            snap.score, self.config.recovery_threshold_score
                        ),
                    })
                    .await;
                }
            }
        }

        // 2. Filter eligible candidates
        let eligible: Vec<&HealthScoreSnapshot> = snapshots
            .iter()
            .filter(|s| !self.excluded_interfaces.contains(&s.if_index))
            .collect();

        if eligible.is_empty() {
            self.current_primary_if_index = None;
            self.current_primary_score = 0.0;
            return None;
        }

        let best_candidate = eligible
            .iter()
            .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(std::cmp::Ordering::Equal))
            .copied()?;

        // 3. Apply Hysteresis Rule: keep current primary if still eligible and within deadband
        if let Some(cur_if) = self.current_primary_if_index {
            if let Some(cur_snap) = eligible.iter().find(|s| s.if_index == cur_if) {
                self.current_primary_score = cur_snap.score;
                let score_gain = best_candidate.score - cur_snap.score;
                if best_candidate.if_index != cur_if
                    && score_gain >= self.config.hysteresis_margin_points
                    && self.last_switch_elapsed_ms >= self.config.min_dwell_ms
                {
                    self.current_primary_if_index = Some(best_candidate.if_index);
                    self.current_primary_score = best_candidate.score;
                    self.last_switch_elapsed_ms = 0;
                    self.switch_count += 1;
                }
                return self.current_primary_if_index;
            }
        }

        // Current primary is None or was excluded -> select best candidate immediately
        self.current_primary_if_index = Some(best_candidate.if_index);
        self.current_primary_score = best_candidate.score;
        self.last_switch_elapsed_ms = 0;
        self.current_primary_if_index
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use giga_inr::InterfaceHealthState;
    use giga_pal::WanIndependenceStatus;
    use std::net::Ipv4Addr;

    fn mock_snap(if_index: u32, score: f64, loss: f64) -> HealthScoreSnapshot {
        HealthScoreSnapshot {
            if_index,
            interface_name: format!("Wi-Fi {}", if_index),
            description: "Test NIC".to_string(),
            medium: "Wifi".to_string(),
            local_ipv4: Some(Ipv4Addr::new(192, 168, 1, 10)),
            gateway_v4: Some(Ipv4Addr::new(192, 168, 1, 1)),
            public_wan_ip: None,
            independence: WanIndependenceStatus::IndependentWan,
            score,
            state: InterfaceHealthState::Healthy,
            ewma_rtt_ms: 20.0,
            min_rtt_ms: 15.0,
            jitter_ms: 2.0,
            loss_rate: loss,
            rssi_dbm: Some(-60),
            rssi_slope_dbm_per_sec: 0.0,
            signal_quality_pct: Some(80),
            ssid: Some(format!("SSID_{}", if_index)),
            rx_throughput_bps: 0.0,
            tx_throughput_bps: 0.0,
            total_rx_bytes: 0,
            total_tx_bytes: 0,
            consecutive_failures: 0,
            is_metered: false,
        }
    }

    #[tokio::test]
    async fn test_hysteresis_prevents_oscillation_on_small_fluctuations() {
        let mut resolver = HysteresisResolverState::default();
        let snaps1 = vec![mock_snap(1, 75.0, 0.0), mock_snap(2, 70.0, 0.0)];
        let selected1 = resolver.evaluate(&snaps1, 12_000, None).await;
        assert_eq!(selected1, Some(1));

        // Interface 2 improves slightly to 82.0 (only +7.0 points over Interface 1's 75.0, below 15.0 deadband)
        let snaps2 = vec![mock_snap(1, 75.0, 0.0), mock_snap(2, 82.0, 0.0)];
        let selected2 = resolver.evaluate(&snaps2, 12_000, None).await;
        assert_eq!(
            selected2,
            Some(1),
            "Should NOT switch when gain (+7.0) < hysteresis deadband (15.0)"
        );

        // Interface 2 jumps to 92.0 (+17.0 points >= 15.0 deadband) after dwell time
        let snaps3 = vec![mock_snap(1, 75.0, 0.0), mock_snap(2, 92.0, 0.0)];
        let selected3 = resolver.evaluate(&snaps3, 12_000, None).await;
        assert_eq!(
            selected3,
            Some(2),
            "Should switch when gain (+17.0) >= hysteresis deadband (15.0)"
        );
    }

    #[tokio::test]
    async fn test_exclusion_and_recovery_thresholds() {
        let mut resolver = HysteresisResolverState::default();
        let snaps1 = vec![mock_snap(1, 22.0, 0.70), mock_snap(2, 65.0, 0.0)];
        let selected = resolver.evaluate(&snaps1, 5_000, None).await;
        assert_eq!(selected, Some(2));
        assert!(resolver.excluded_interfaces.contains(&1));

        // Interface 1 recovers to 55.0 (>= 48.0 recovery threshold)
        let snaps2 = vec![mock_snap(1, 55.0, 0.0), mock_snap(2, 65.0, 0.0)];
        let _ = resolver.evaluate(&snaps2, 5_000, None).await;
        assert!(!resolver.excluded_interfaces.contains(&1));
    }
}
