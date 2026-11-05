//! `giga_inr` — Intelligent Network Resolver (INR) Engine, Composite Health Score (`CHS`),
//! Predictive Brownout Detection, and Parallel DNS Race Resolver.

pub mod dns;

pub use dns::{DnsRaceResolver, DnsRaceResult};

use giga_pal::{
    connect_bound_tcp_v4, create_platform_manager, verify_pool_independence, IndependenceReport,
    InterfaceSnapshot, InterfaceType, WanIndependenceStatus,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Health state of a managed network interface in the INR 7-state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterfaceHealthState {
    Standby,
    Healthy,
    Degraded,
    FailingOver,
    Dead,
    Recovering,
}

/// Real-time Composite Health Score (`CHS(i) ∈ [0, 100]`) & telemetry snapshot for an interface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthScoreSnapshot {
    pub if_index: u32,
    pub interface_name: String,
    pub description: String,
    pub medium: String,
    pub local_ipv4: Option<Ipv4Addr>,
    pub gateway_v4: Option<Ipv4Addr>,
    pub public_wan_ip: Option<String>,
    pub independence: WanIndependenceStatus,
    pub score: f64,
    pub state: InterfaceHealthState,
    pub ewma_rtt_ms: f64,
    pub min_rtt_ms: f64,
    pub jitter_ms: f64,
    pub loss_rate: f64,
    pub rssi_dbm: Option<i32>,
    pub rssi_slope_dbm_per_sec: f64,
    pub signal_quality_pct: Option<u32>,
    pub ssid: Option<String>,
    pub rx_throughput_bps: f64,
    pub tx_throughput_bps: f64,
    pub total_rx_bytes: u64,
    pub total_tx_bytes: u64,
    pub consecutive_failures: u32,
    pub is_metered: bool,
}

impl HealthScoreSnapshot {
    /// Compute baseline health score from RTT, jitter, packet loss, and optional Wi-Fi RSSI.
    pub fn compute(
        if_index: u32,
        interface_name: String,
        ewma_rtt_ms: f64,
        jitter_ms: f64,
        loss_rate: f64,
        rssi_dbm: Option<i32>,
    ) -> Self {
        let score = compute_chs_formula(ewma_rtt_ms, jitter_ms, loss_rate, rssi_dbm, 0.0, 50_000_000.0);
        let state = classify_chs_state(score, 0.0, ewma_rtt_ms, ewma_rtt_ms, 0);

        Self {
            if_index,
            interface_name,
            description: String::new(),
            medium: "Auto".to_string(),
            local_ipv4: None,
            gateway_v4: None,
            public_wan_ip: None,
            independence: WanIndependenceStatus::IndependentWan,
            score,
            state,
            ewma_rtt_ms,
            min_rtt_ms: ewma_rtt_ms,
            jitter_ms,
            loss_rate,
            rssi_dbm,
            rssi_slope_dbm_per_sec: 0.0,
            signal_quality_pct: None,
            ssid: None,
            rx_throughput_bps: 0.0,
            tx_throughput_bps: 0.0,
            total_rx_bytes: 0,
            total_tx_bytes: 0,
            consecutive_failures: 0,
            is_metered: false,
        }
    }
}

/// Computes the analytical Composite Health Score `CHS(i) ∈ [0, 100]` per Section 21.3 of the spec:
/// `CHS = 0.25*S_rtt + 0.20*S_jitter + 0.30*S_loss + 0.15*S_bw + 0.10*S_rf`
pub fn compute_chs_formula(
    ewma_rtt_ms: f64,
    jitter_ms: f64,
    loss_rate: f64,
    rssi_dbm: Option<i32>,
    rssi_slope_dbm_per_sec: f64,
    capacity_bps: f64,
) -> f64 {
    let s_rtt = 100.0 * (-(ewma_rtt_ms - 10.0).max(0.0) / 120.0).exp();
    let s_jitter = 100.0 * (-jitter_ms.max(0.0) / 30.0).exp();
    let s_loss = 100.0 * (1.0 - loss_rate.clamp(0.0, 1.0)).powi(8);
    let s_bw = (100.0 * (1.0 + capacity_bps.max(0.0) / 1_000_000.0).ln() / (1.0 + 100.0_f64).ln())
        .clamp(15.0, 100.0);
    let mut s_rf = match rssi_dbm {
        Some(rssi) => ((rssi as f64 + 90.0) / 40.0 * 100.0).clamp(0.0, 100.0),
        None => 100.0,
    };

    // Penalize rapid RF fade (predictive brownout)
    if rssi_slope_dbm_per_sec < -4.0 {
        s_rf = (s_rf + rssi_slope_dbm_per_sec * 6.0).clamp(0.0, 100.0);
    }

    (0.25 * s_rtt + 0.20 * s_jitter + 0.30 * s_loss + 0.15 * s_bw + 0.10 * s_rf).clamp(0.0, 100.0)
}

pub fn classify_chs_state(
    score: f64,
    rssi_slope_dbm_per_sec: f64,
    ewma_rtt_ms: f64,
    min_rtt_ms: f64,
    consecutive_failures: u32,
) -> InterfaceHealthState {
    if consecutive_failures >= 3 || score < 25.0 {
        InterfaceHealthState::Dead
    } else if rssi_slope_dbm_per_sec < -6.0
        || (min_rtt_ms > 1.0 && ewma_rtt_ms > 3.2 * min_rtt_ms && ewma_rtt_ms > 350.0)
        || score < 40.0
    {
        InterfaceHealthState::FailingOver
    } else if score < 68.0 {
        InterfaceHealthState::Degraded
    } else {
        InterfaceHealthState::Healthy
    }
}

/// Internal per-interface state maintained by the INR Engine.
struct TrackedInterface {
    pub raw: InterfaceSnapshot,
    pub snapshot: HealthScoreSnapshot,
    pub last_rssi_time: Instant,
    pub last_bytes_time: Instant,
    pub bytes_window_rx: u64,
    pub bytes_window_tx: u64,
    pub last_hw_octets: Option<(u64, u64)>,
    pub last_hw_time: Instant,
}

/// The live Intelligent Network Resolver (INR) Engine.
#[derive(Clone)]
pub struct InrEngine {
    tracked: Arc<RwLock<HashMap<u32, TrackedInterface>>>,
    dns_resolver: DnsRaceResolver,
    secondary_ssid: Arc<RwLock<Option<String>>>,
    tertiary_ssid: Arc<RwLock<Option<String>>>,
}

impl Default for InrEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl InrEngine {
    pub fn new() -> Self {
        Self {
            tracked: Arc::new(RwLock::new(HashMap::new())),
            dns_resolver: DnsRaceResolver::new(),
            secondary_ssid: Arc::new(RwLock::new(None)),
            tertiary_ssid: Arc::new(RwLock::new(None)),
        }
    }

    pub fn dns(&self) -> &DnsRaceResolver {
        &self.dns_resolver
    }

    /// Polls real hardware octets for all interfaces directly from Windows kernel NDIS via GetIfEntry2
    pub async fn update_live_hardware_throughput(&self) {
        let mut guard = self.tracked.write().await;
        let now = Instant::now();
        for (if_index, tracked) in guard.iter_mut() {
            if let Some((in_octets, out_octets)) = giga_pal::query_interface_octets(*if_index) {
                tracked.snapshot.total_rx_bytes = in_octets;
                tracked.snapshot.total_tx_bytes = out_octets;

                if let Some((prev_in, prev_out)) = tracked.last_hw_octets {
                    let dt = now.duration_since(tracked.last_hw_time).as_secs_f64();
                    if dt >= 0.5 {
                        let delta_in = in_octets.saturating_sub(prev_in);
                        let delta_out = out_octets.saturating_sub(prev_out);
                        tracked.snapshot.rx_throughput_bps = (delta_in as f64 * 8.0) / dt;
                        tracked.snapshot.tx_throughput_bps = (delta_out as f64 * 8.0) / dt;
                        tracked.last_hw_octets = Some((in_octets, out_octets));
                        tracked.last_hw_time = now;
                    }
                } else {
                    tracked.last_hw_octets = Some((in_octets, out_octets));
                    tracked.last_hw_time = now;
                }
            }
        }
    }

    pub async fn set_secondary_wifi_ssid(&self, ssid: String) {
        let mut lock = self.secondary_ssid.write().await;
        *lock = Some(ssid);
    }

    pub async fn set_tertiary_wifi_ssid(&self, ssid: String) {
        let mut lock = self.tertiary_ssid.write().await;
        *lock = Some(ssid);
    }

    /// Refreshes hardware interfaces from `giga_pal` and runs an initial independence/RTT check
    /// for newly discovered WAN interfaces.
    pub async fn refresh_and_probe(&self, probe_timeout: Duration) -> Vec<HealthScoreSnapshot> {
        let pal = create_platform_manager();
        let Ok(all_interfaces) = pal.enumerate_interfaces() else {
            return self.snapshots().await;
        };

        let wan_candidates: Vec<InterfaceSnapshot> = all_interfaces
            .iter()
            .filter(|i| {
                i.is_up
                    && !i.is_virtual_or_loopback
                    && (i.primary_ipv4().is_some() || matches!(i.if_type, InterfaceType::WiFi(_)))
            })
            .cloned()
            .collect();

        // Check if we need full WAN independence discovery (for newly seen interfaces)
        let need_independence = {
            let guard = self.tracked.read().await;
            wan_candidates
                .iter()
                .any(|c| !guard.contains_key(&c.if_index))
        };

        let mut ind_map: HashMap<u32, IndependenceReport> = HashMap::new();
        if need_independence && !wan_candidates.is_empty() {
            for rep in verify_pool_independence(&wan_candidates, probe_timeout).await {
                ind_map.insert(rep.if_index, rep);
            }
        }

        let now = Instant::now();
        for iface in &wan_candidates {
            // Fast TCP RTT probe if not just probed by verify_pool_independence
            let probe_rtt = if let Some(rep) = ind_map.get(&iface.if_index) {
                rep.anycast_tcp_rtt_ms
            } else {
                fast_tcp_rtt_probe(iface, probe_timeout).await
            };

            let mut guard = self.tracked.write().await;
            let (rssi, sig_pct, ssid) = match &iface.if_type {
                InterfaceType::WiFi(Some(w)) => (Some(w.rssi_dbm), Some(w.signal_quality_pct), w.ssid.clone()),
                _ => (None, None, None),
            };

            if let Some(existing) = guard.get_mut(&iface.if_index) {
                // Compute RSSI derivative d(RSSI)/dt
                let dt_rssi = now.duration_since(existing.last_rssi_time).as_secs_f64().max(0.5);
                let rssi_slope = match (rssi, existing.snapshot.rssi_dbm) {
                    (Some(curr), Some(prev)) => (curr - prev) as f64 / dt_rssi,
                    _ => 0.0,
                };
                existing.last_rssi_time = now;

                // Update throughput rates
                let dt_bw = now.duration_since(existing.last_bytes_time).as_secs_f64();
                if dt_bw >= 0.4 {
                    existing.snapshot.rx_throughput_bps =
                        (existing.bytes_window_rx as f64 * 8.0) / dt_bw;
                    existing.snapshot.tx_throughput_bps =
                        (existing.bytes_window_tx as f64 * 8.0) / dt_bw;
                    existing.bytes_window_rx = 0;
                    existing.bytes_window_tx = 0;
                    existing.last_bytes_time = now;
                }

                if let Some(rtt) = probe_rtt {
                    let jitter_sample = (rtt - existing.snapshot.ewma_rtt_ms).abs();
                    existing.snapshot.ewma_rtt_ms =
                        0.75 * existing.snapshot.ewma_rtt_ms + 0.25 * rtt;
                    existing.snapshot.jitter_ms =
                        0.80 * existing.snapshot.jitter_ms + 0.20 * jitter_sample;
                    existing.snapshot.min_rtt_ms = existing.snapshot.min_rtt_ms.min(rtt);
                    existing.snapshot.loss_rate = (existing.snapshot.loss_rate * 0.7).max(0.0);
                    existing.snapshot.consecutive_failures = 0;
                } else {
                    existing.snapshot.consecutive_failures += 1;
                    existing.snapshot.loss_rate =
                        (existing.snapshot.loss_rate * 0.7 + 0.3).clamp(0.0, 1.0);
                }

                existing.snapshot.rssi_dbm = rssi;
                existing.snapshot.rssi_slope_dbm_per_sec = rssi_slope;
                existing.snapshot.signal_quality_pct = sig_pct;
                existing.snapshot.ssid = ssid;
                existing.snapshot.local_ipv4 = iface.primary_ipv4();
                existing.snapshot.gateway_v4 = iface.primary_ipv4_gateway();

                let link_cap = (iface.rx_link_speed_bps as f64)
                    .max(existing.snapshot.rx_throughput_bps)
                    .max(10_000_000.0);

                existing.snapshot.score = compute_chs_formula(
                    existing.snapshot.ewma_rtt_ms,
                    existing.snapshot.jitter_ms,
                    existing.snapshot.loss_rate,
                    existing.snapshot.rssi_dbm,
                    existing.snapshot.rssi_slope_dbm_per_sec,
                    link_cap,
                );
                existing.snapshot.state = classify_chs_state(
                    existing.snapshot.score,
                    existing.snapshot.rssi_slope_dbm_per_sec,
                    existing.snapshot.ewma_rtt_ms,
                    existing.snapshot.min_rtt_ms,
                    existing.snapshot.consecutive_failures,
                );
                existing.raw = iface.clone();
            } else {
                let rtt = probe_rtt.unwrap_or(150.0);
                let loss = if probe_rtt.is_some() { 0.0 } else { 0.5 };
                let failures = if probe_rtt.is_some() { 0 } else { 1 };
                let link_cap = (iface.rx_link_speed_bps as f64).max(25_000_000.0);
                let score = compute_chs_formula(rtt, 2.0, loss, rssi, 0.0, link_cap);
                let state = classify_chs_state(score, 0.0, rtt, rtt, failures);
                let ind_rep = ind_map.get(&iface.if_index);

                let snap = HealthScoreSnapshot {
                    if_index: iface.if_index,
                    interface_name: iface.friendly_name.clone(),
                    description: iface.description.clone(),
                    medium: iface.if_type.short_label().to_string(),
                    local_ipv4: iface.primary_ipv4(),
                    gateway_v4: iface.primary_ipv4_gateway(),
                    public_wan_ip: ind_rep.and_then(|r| r.public_wan_ip.clone()),
                    independence: ind_rep
                        .map(|r| r.status.clone())
                        .unwrap_or(WanIndependenceStatus::IndependentWan),
                    score,
                    state,
                    ewma_rtt_ms: rtt,
                    min_rtt_ms: rtt,
                    jitter_ms: 2.0,
                    loss_rate: loss,
                    rssi_dbm: rssi,
                    rssi_slope_dbm_per_sec: 0.0,
                    signal_quality_pct: sig_pct,
                    ssid,
                    rx_throughput_bps: 0.0,
                    tx_throughput_bps: 0.0,
                    total_rx_bytes: 0,
                    total_tx_bytes: 0,
                    consecutive_failures: failures,
                    is_metered: iface.is_metered,
                };

                guard.insert(
                    iface.if_index,
                    TrackedInterface {
                        raw: iface.clone(),
                        snapshot: snap,
                        last_rssi_time: now,
                        last_bytes_time: now,
                        bytes_window_rx: 0,
                        bytes_window_tx: 0,
                        last_hw_octets: None,
                        last_hw_time: now,
                    },
                );
            }
        }

        // Retain only currently active interfaces (unplugged or disconnected adapters are pruned)
        {
            let mut guard = self.tracked.write().await;
            let current_indices: std::collections::HashSet<u32> = wan_candidates.iter().map(|i| i.if_index).collect();
            guard.retain(|idx, _| current_indices.contains(idx));
        }

        // Update real hardware throughput directly from Windows kernel NDIS counters
        self.update_live_hardware_throughput().await;

        self.snapshots().await
    }

    /// Passive telemetry hook called by `giga_transport` whenever bytes or connection RTTs are observed.
    pub async fn record_transfer_sample(
        &self,
        if_index: u32,
        rx_bytes: u64,
        tx_bytes: u64,
        rtt_sample_ms: Option<f64>,
        success: bool,
    ) {
        let mut guard = self.tracked.write().await;
        let Some(entry) = guard.get_mut(&if_index) else {
            return;
        };

        entry.snapshot.total_rx_bytes += rx_bytes;
        entry.snapshot.total_tx_bytes += tx_bytes;
        entry.bytes_window_rx += rx_bytes;
        entry.bytes_window_tx += tx_bytes;

        let now = Instant::now();
        let dt = now.duration_since(entry.last_bytes_time).as_secs_f64();
        if dt >= 0.25 {
            entry.snapshot.rx_throughput_bps = (entry.bytes_window_rx as f64 * 8.0) / dt;
            entry.snapshot.tx_throughput_bps = (entry.bytes_window_tx as f64 * 8.0) / dt;
            entry.bytes_window_rx = 0;
            entry.bytes_window_tx = 0;
            entry.last_bytes_time = now;
        }

        if let Some(rtt) = rtt_sample_ms {
            let jitter = (rtt - entry.snapshot.ewma_rtt_ms).abs();
            entry.snapshot.ewma_rtt_ms = 0.8 * entry.snapshot.ewma_rtt_ms + 0.2 * rtt;
            entry.snapshot.jitter_ms = 0.85 * entry.snapshot.jitter_ms + 0.15 * jitter;
            entry.snapshot.min_rtt_ms = entry.snapshot.min_rtt_ms.min(rtt);
        }

        if success {
            entry.snapshot.consecutive_failures = 0;
            entry.snapshot.loss_rate = (entry.snapshot.loss_rate * 0.85).max(0.0);
        } else {
            entry.snapshot.consecutive_failures += 1;
            entry.snapshot.loss_rate = (entry.snapshot.loss_rate * 0.8 + 0.2).clamp(0.0, 1.0);
        }

        let cap = (entry.raw.rx_link_speed_bps as f64)
            .max(entry.snapshot.rx_throughput_bps)
            .max(10_000_000.0);
        entry.snapshot.score = compute_chs_formula(
            entry.snapshot.ewma_rtt_ms,
            entry.snapshot.jitter_ms,
            entry.snapshot.loss_rate,
            entry.snapshot.rssi_dbm,
            entry.snapshot.rssi_slope_dbm_per_sec,
            cap,
        );
        entry.snapshot.state = classify_chs_state(
            entry.snapshot.score,
            entry.snapshot.rssi_slope_dbm_per_sec,
            entry.snapshot.ewma_rtt_ms,
            entry.snapshot.min_rtt_ms,
            entry.snapshot.consecutive_failures,
        );
    }

    /// Returns all tracked interface health snapshots sorted by highest Composite Health Score (`score`).
    pub async fn snapshots(&self) -> Vec<HealthScoreSnapshot> {
        let guard = self.tracked.read().await;
        let mut list: Vec<HealthScoreSnapshot> =
            guard.values().map(|t| t.snapshot.clone()).collect();
        list.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        list
    }

    /// Returns active `InterfaceSnapshot`s ordered from highest `CHS` to lowest.
    pub async fn ranked_healthy_interfaces(&self) -> Vec<(InterfaceSnapshot, HealthScoreSnapshot)> {
        let guard = self.tracked.read().await;
        let mut list: Vec<(InterfaceSnapshot, HealthScoreSnapshot)> = guard
            .values()
            .filter(|t| t.snapshot.state != InterfaceHealthState::Dead)
            .map(|t| (t.raw.clone(), t.snapshot.clone()))
            .collect();
        list.sort_by(|a, b| {
            b.1.score
                .partial_cmp(&a.1.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        list
    }
}

async fn fast_tcp_rtt_probe(iface: &InterfaceSnapshot, timeout: Duration) -> Option<f64> {
    let targets = [
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), 53),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 443),
    ];
    for t in targets {
        if let Ok((_, res)) = connect_bound_tcp_v4(iface, t, timeout).await {
            return Some(res.handshake_rtt_ms);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_inr_discovers_real_hardware_interfaces() {
        let inr = InrEngine::new();
        let snaps = inr.refresh_and_probe(Duration::from_millis(500)).await;
        assert!(
            !snaps.is_empty(),
            "Expected at least one active network interface on host, found {}",
            snaps.len()
        );
        for s in &snaps {
            assert!(s.score >= 0.0, "Score should be non-negative");
            assert!(s.ewma_rtt_ms >= 0.0, "RTT should be non-negative");
        }
    }

    #[tokio::test]
    async fn test_live_hardware_throughput_polling() {
        let inr = InrEngine::new();
        let _ = inr.refresh_and_probe(Duration::from_millis(500)).await;
        inr.update_live_hardware_throughput().await;
        let snaps = inr.snapshots().await;
        for s in &snaps {
            assert!(s.total_rx_bytes > 0 || s.rx_throughput_bps >= 0.0);
        }
    }
}
