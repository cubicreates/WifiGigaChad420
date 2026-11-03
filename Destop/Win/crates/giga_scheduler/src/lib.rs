//! `giga_scheduler` — Multi-Path Kalman Bandwidth Estimator, Thompson Sampling Interface Selector,
//! Dynamic Byte-Range Chunk Splitter, and Work-Stealing Straggler Speculative Execution Engine.

use giga_inr::HealthScoreSnapshot;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Traffic priority class for QoS flow steering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrafficClass {
    /// Interactive / real-time flows (VoIP, gaming, SSH, DNS) -> lowest RTT + jitter path.
    RealTime,
    /// Web browsing / API calls -> highest Composite Health Score path with 150ms hedgers.
    Interactive,
    /// Bulk file transfers / streams -> striped across all healthy interfaces.
    BulkStriped,
}

impl TrafficClass {
    pub fn classify_port(port: u16) -> Self {
        match port {
            22 | 53 | 853 | 3478..=3481 | 19302..=19309 | 27015..=27050 => TrafficClass::RealTime,
            80 | 443 | 8080 | 8443 => TrafficClass::Interactive,
            _ => TrafficClass::Interactive,
        }
    }
}

/// 2-state Kalman Filter estimating an interface's instantaneous throughput `b_hat_bps`
/// and rate of change `db_dt_bps` along with estimation uncertainty `variance`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KalmanBandwidthEstimator {
    pub if_index: u32,
    pub b_hat_bps: f64,
    pub db_dt_bps: f64,
    pub variance: f64,
    pub samples: u64,
}

impl KalmanBandwidthEstimator {
    pub fn new(if_index: u32, initial_bps: f64) -> Self {
        Self {
            if_index,
            b_hat_bps: initial_bps.max(1_000_000.0),
            db_dt_bps: 0.0,
            variance: 25_000_000.0,
            samples: 0,
        }
    }

    /// Updates the Kalman state with a newly observed throughput measurement `measured_bps` over `dt_sec`.
    pub fn update(&mut self, measured_bps: f64, dt_sec: f64) {
        let dt = dt_sec.clamp(0.05, 5.0);
        // State prediction
        let predicted_b = (self.b_hat_bps + self.db_dt_bps * dt).max(100_000.0);
        let process_noise = 4_000_000.0 * dt;
        let predicted_var = (self.variance * 0.98) + process_noise;

        // Measurement update
        let measurement_noise = (measured_bps * 0.15).max(500_000.0);
        let innovation = measured_bps - predicted_b;
        let kalman_gain = predicted_var / (predicted_var + measurement_noise);

        self.b_hat_bps = (predicted_b + kalman_gain * innovation).max(50_000.0);
        self.db_dt_bps = 0.7 * self.db_dt_bps + 0.3 * (innovation / dt);
        self.variance = ((1.0 - kalman_gain) * predicted_var).clamp(100_000.0, 100_000_000.0);
        self.samples += 1;
    }

    /// Computes a deterministic Thompson-like upper confidence score combining Kalman bandwidth
    /// estimate, uncertainty bonus (exploration), and the interface's INR Composite Health Score.
    pub fn scheduling_weight(&self, chs_score: f64, step_seed: u64) -> f64 {
        // Pseudo-normal exploration bonus derived from step_seed so recovering interfaces get probed
        let hash = ((step_seed.wrapping_mul(6364136223846793005).wrapping_add(self.if_index as u64))
            >> 32) as f64
            / (u32::MAX as f64);
        let exploration_z = (hash - 0.5) * 1.2;
        let sampled_bw = (self.b_hat_bps + exploration_z * self.variance.sqrt()).max(100_000.0);
        sampled_bw * (chs_score / 100.0).powi(2)
    }
}

/// Lifecycle state of an individual byte-range chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChunkState {
    Queued,
    InFlight,
    Completed,
    Failed,
}

/// Byte-range work unit (`[start_byte..=end_byte]`, inclusive) assigned to a bound interface worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByteRangeChunk {
    pub chunk_id: u64,
    pub start_byte: u64,
    pub current_byte: u64,
    pub end_byte: u64,
    pub assigned_if_index: Option<u32>,
    pub state: ChunkState,
    pub retries: u32,
    pub stolen_from_chunk: Option<u64>,
}

impl ByteRangeChunk {
    pub fn total_len(&self) -> u64 {
        if self.end_byte >= self.start_byte {
            self.end_byte - self.start_byte + 1
        } else {
            0
        }
    }

    pub fn downloaded_bytes(&self) -> u64 {
        self.current_byte.saturating_sub(self.start_byte)
    }

    pub fn remaining_bytes(&self) -> u64 {
        if self.state == ChunkState::Completed {
            0
        } else if self.end_byte >= self.current_byte {
            self.end_byte - self.current_byte + 1
        } else {
            0
        }
    }

    pub fn is_empty(&self) -> bool {
        self.total_len() == 0
    }
}

/// Dynamic Multi-Path Scheduler & Work-Stealing Chunk Coordinator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiPathScheduler {
    pub total_bytes: u64,
    pub chunks: Vec<ByteRangeChunk>,
    pub estimators: HashMap<u32, KalmanBandwidthEstimator>,
    pub step_counter: u64,
}

impl MultiPathScheduler {
    /// Creates a new scheduler and partitions `total_bytes` into dynamic chunks sized between
    /// `512 KB` and `8 MB` depending on total payload size and active interface count.
    pub fn new(total_bytes: u64, active_if_count: usize) -> Self {
        let chunks = Self::plan_initial_chunks(total_bytes, active_if_count.max(1));
        Self {
            total_bytes,
            chunks,
            estimators: HashMap::new(),
            step_counter: 1,
        }
    }

    pub fn plan_initial_chunks(total_bytes: u64, active_if_count: usize) -> Vec<ByteRangeChunk> {
        if total_bytes == 0 {
            return Vec::new();
        }

        let target_chunks = (active_if_count * 8).clamp(4, 64) as u64;
        let raw_chunk_size = (total_bytes / target_chunks).clamp(256 * 1024, 8 * 1024 * 1024);

        let mut chunks = Vec::new();
        let mut offset = 0u64;
        let mut chunk_id = 0u64;

        while offset < total_bytes {
            let end = (offset + raw_chunk_size - 1).min(total_bytes - 1);
            chunks.push(ByteRangeChunk {
                chunk_id,
                start_byte: offset,
                current_byte: offset,
                end_byte: end,
                assigned_if_index: None,
                state: ChunkState::Queued,
                retries: 0,
                stolen_from_chunk: None,
            });
            offset = end + 1;
            chunk_id += 1;
        }

        chunks
    }

    /// Updates the Kalman bandwidth estimator for `if_index`.
    pub fn record_bandwidth(&mut self, if_index: u32, measured_bps: f64, dt_sec: f64) {
        let est = self
            .estimators
            .entry(if_index)
            .or_insert_with(|| KalmanBandwidthEstimator::new(if_index, measured_bps));
        est.update(measured_bps, dt_sec);
    }

    /// Selects the optimal interface for a new chunk or proxy connection using Thompson Sampling + INR CHS.
    pub fn select_best_interface(&mut self, health_snapshots: &[HealthScoreSnapshot]) -> Option<u32> {
        self.step_counter = self.step_counter.wrapping_add(1);
        let step = self.step_counter;

        health_snapshots
            .iter()
            .filter(|h| h.score >= 25.0)
            .max_by(|a, b| {
                let wa = self
                    .estimators
                    .get(&a.if_index)
                    .map(|e| e.scheduling_weight(a.score, step))
                    .unwrap_or(a.score * 100_000.0);
                let wb = self
                    .estimators
                    .get(&b.if_index)
                    .map(|e| e.scheduling_weight(b.score, step))
                    .unwrap_or(b.score * 100_000.0);
                wa.partial_cmp(&wb).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|h| h.if_index)
    }

    /// Claims the next `Queued` (or `Failed` with retries < 4) chunk for `if_index`.
    /// If no queued chunks remain, attempts **Work-Stealing Straggler Speculative Execution**
    /// by bisecting the largest remaining `InFlight` chunk!
    pub fn claim_or_steal_chunk(&mut self, if_index: u32) -> Option<ByteRangeChunk> {
        // 1. First look for any Queued or retryable Failed chunk
        if let Some(chunk) = self.chunks.iter_mut().find(|c| {
            c.state == ChunkState::Queued || (c.state == ChunkState::Failed && c.retries < 4)
        }) {
            chunk.state = ChunkState::InFlight;
            chunk.assigned_if_index = Some(if_index);
            return Some(chunk.clone());
        }

        // 2. Work-Stealing Straggler Mitigation: find the InFlight chunk with the most remaining bytes (>= 512 KB)
        const MIN_STEALABLE_BYTES: u64 = 512 * 1024;
        let next_id = self.chunks.len() as u64;

        let victim_idx = self
            .chunks
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                c.state == ChunkState::InFlight && c.remaining_bytes() >= MIN_STEALABLE_BYTES * 2
            })
            .max_by_key(|(_, c)| c.remaining_bytes())
            .map(|(idx, _)| idx)?;

        let victim = &mut self.chunks[victim_idx];
        let remaining = victim.remaining_bytes();
        let split_offset = victim.current_byte + (remaining / 2);
        let original_end = victim.end_byte;
        let victim_id = victim.chunk_id;

        // Truncate the slow victim chunk's upper bound
        victim.end_byte = split_offset - 1;

        // Create the stolen second-half chunk for the fast idle interface
        let stolen_chunk = ByteRangeChunk {
            chunk_id: next_id,
            start_byte: split_offset,
            current_byte: split_offset,
            end_byte: original_end,
            assigned_if_index: Some(if_index),
            state: ChunkState::InFlight,
            retries: 0,
            stolen_from_chunk: Some(victim_id),
        };

        self.chunks.push(stolen_chunk.clone());
        Some(stolen_chunk)
    }

    /// Advances the byte progress of `chunk_id` and returns the effective upper bound `end_byte`
    /// (allowing an in-flight worker to stop early if its second half was work-stolen).
    pub fn advance_chunk(&mut self, chunk_id: u64, delta_bytes: u64) -> Option<(u64, u64, bool)> {
        let chunk = self.chunks.iter_mut().find(|c| c.chunk_id == chunk_id)?;
        chunk.current_byte = (chunk.current_byte + delta_bytes).min(chunk.end_byte + 1);
        let completed = chunk.current_byte > chunk.end_byte;
        if completed {
            chunk.state = ChunkState::Completed;
        }
        Some((chunk.current_byte, chunk.end_byte, completed))
    }

    pub fn mark_chunk_failed(&mut self, chunk_id: u64) {
        if let Some(chunk) = self.chunks.iter_mut().find(|c| c.chunk_id == chunk_id) {
            if chunk.state != ChunkState::Completed {
                chunk.state = ChunkState::Failed;
                chunk.retries += 1;
                chunk.start_byte = chunk.current_byte;
            }
        }
    }

    pub fn is_all_completed(&self) -> bool {
        !self.chunks.is_empty() && self.chunks.iter().all(|c| c.state == ChunkState::Completed)
    }

    pub fn total_downloaded_bytes(&self) -> u64 {
        self.chunks.iter().map(|c| c.downloaded_bytes()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_splitting_and_work_stealing() {
        let mut sched = MultiPathScheduler::new(4 * 1024 * 1024, 2);
        assert!(!sched.chunks.is_empty());

        // Claim all queued chunks on interface 5
        let mut claimed = Vec::new();
        while let Some(c) = sched.claim_or_steal_chunk(5) {
            if c.stolen_from_chunk.is_some() {
                break;
            }
            claimed.push(c);
        }
        assert!(!claimed.is_empty());

        // Now all chunks are InFlight; if any chunk is >= 1 MB, work-stealing should bisect one
        let mut large_sched = MultiPathScheduler::new(16 * 1024 * 1024, 1);
        while let Some(c) = large_sched.claim_or_steal_chunk(5) {
            if large_sched.chunks.iter().all(|ch| ch.state == ChunkState::InFlight) {
                let stolen = large_sched.claim_or_steal_chunk(17);
                assert!(stolen.is_some(), "Expected work-stealing to bisect an in-flight chunk");
                assert_eq!(stolen.unwrap().assigned_if_index, Some(17));
                break;
            }
            let _ = c;
        }
    }
}
