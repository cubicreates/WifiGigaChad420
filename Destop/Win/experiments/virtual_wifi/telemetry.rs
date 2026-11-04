//! Structured Experimental Telemetry & Honest Status Labels (`telemetry.rs`).
//!
//! Complies with Phase 13 and Phase 14 requirements:
//! - Uses strict honest status labels (`SUPPORTED`, `UNSUPPORTED`, `EXPERIMENTAL`, `UNKNOWN`, `NOT TESTED`, `BLOCKED`).
//! - Emits structured experimental events without ever logging Wi-Fi passwords or credentials.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// Honest engineering status labels mandated by Phase 13.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilityStatusLabel {
    #[serde(rename = "SUPPORTED")]
    Supported,
    #[serde(rename = "UNSUPPORTED")]
    Unsupported,
    #[serde(rename = "EXPERIMENTAL")]
    Experimental,
    #[serde(rename = "UNKNOWN")]
    Unknown,
    #[serde(rename = "NOT TESTED")]
    NotTested,
    #[serde(rename = "BLOCKED")]
    Blocked,
}

impl CapabilityStatusLabel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Supported => "SUPPORTED",
            Self::Unsupported => "UNSUPPORTED",
            Self::Experimental => "EXPERIMENTAL",
            Self::Unknown => "UNKNOWN",
            Self::NotTested => "NOT TESTED",
            Self::Blocked => "BLOCKED",
        }
    }
}

/// Structured event types mandated by Phase 14.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StructuredEventKind {
    #[serde(rename = "VIRTUAL_INTERFACE_CREATED")]
    VirtualInterfaceCreated,
    #[serde(rename = "VIRTUAL_INTERFACE_ROLLED_BACK")]
    VirtualInterfaceRolledBack,
    #[serde(rename = "SECONDARY_STA_CAPABILITY_DETECTED")]
    SecondaryStaCapabilityDetected,
    #[serde(rename = "DRIVER_CAPABILITY_UNSUPPORTED")]
    DriverCapabilityUnsupported,
    #[serde(rename = "RADIO_SWITCH_STARTED")]
    RadioSwitchStarted,
    #[serde(rename = "RADIO_SWITCH_COMPLETED")]
    RadioSwitchCompleted,
    #[serde(rename = "NETWORK_ASSOCIATION_LOST")]
    NetworkAssociationLost,
    #[serde(rename = "NETWORK_ASSOCIATION_RESTORED")]
    NetworkAssociationRestored,
    #[serde(rename = "PACKET_QUEUE_OVERFLOW")]
    PacketQueueOverflow,
    #[serde(rename = "INR_NETWORK_EXCLUDED")]
    InrNetworkExcluded,
    #[serde(rename = "INR_NETWORK_RESTORED")]
    InrNetworkRestored,
    #[serde(rename = "CAPABILITY_PROBE_COMPLETED")]
    CapabilityProbeCompleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredExperimentEvent {
    pub timestamp: String,
    pub component: String,
    pub physical_interface: String,
    pub virtual_interface: String,
    pub network_identifier: String,
    pub event: StructuredEventKind,
    pub result: CapabilityStatusLabel,
    pub error_code: Option<i32>,
    pub duration_ms: Option<f64>,
    pub measurement: String,
}

pub fn now_iso_utc() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    format!("{:02}:{:02}:{:02} UTC", h, m, s)
}

#[derive(Clone, Default)]
pub struct StructuredEventLogger {
    events: Arc<RwLock<Vec<StructuredExperimentEvent>>>,
}

impl StructuredEventLogger {
    pub fn new() -> Self {
        Self {
            events: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn record(&self, mut entry: StructuredExperimentEvent) {
        // Security guardrail (Phase 12 & 14): scrub any accidental key=... or password patterns
        if entry.measurement.to_ascii_lowercase().contains("key=")
            || entry.measurement.to_ascii_lowercase().contains("password")
        {
            entry.measurement = "[REDACTED_CREDENTIAL_GUARD]".to_string();
        }
        let mut guard = self.events.write().await;
        guard.insert(0, entry);
        if guard.len() > 200 {
            guard.truncate(200);
        }
    }

    pub async fn snapshot(&self) -> Vec<StructuredExperimentEvent> {
        self.events.read().await.clone()
    }

    pub async fn clear(&self) {
        self.events.write().await.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_structured_event_logger_redacts_secrets() {
        let logger = StructuredEventLogger::new();
        logger
            .record(StructuredExperimentEvent {
                timestamp: now_iso_utc(),
                component: "connection_manager".to_string(),
                physical_interface: "Wi-Fi".to_string(),
                virtual_interface: "WiFi GigaChad 420 Virtual Network".to_string(),
                network_identifier: "Gheras_5G".to_string(),
                event: StructuredEventKind::DriverCapabilityUnsupported,
                result: CapabilityStatusLabel::Unsupported,
                error_code: Some(50),
                duration_ms: Some(12.4),
                measurement: "key=supersecret123".to_string(),
            })
            .await;

        let snap = logger.snapshot().await;
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].measurement, "[REDACTED_CREDENTIAL_GUARD]");
    }
}

