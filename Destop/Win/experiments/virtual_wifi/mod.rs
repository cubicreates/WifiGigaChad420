//! Phase Four & Six: Experimental Virtual Wi-Fi, Capability Detection & Radio Time-Sharing Subsystem
//! (`Destop/Win/experiments/virtual_wifi/mod.rs`).

pub mod adapter_manager;
pub mod capability_detection;
pub mod connection_manager;
pub mod diagnostics;
pub mod network_resolver;
pub mod packet_mapper;
pub mod radio_scheduler;
pub mod telemetry;
pub mod virtual_interface;

pub use diagnostics::{ExperimentalVirtualWifiEngine, ExperimentalVirtualWifiTelemetry};
pub use telemetry::{CapabilityStatusLabel, StructuredEventKind, StructuredExperimentEvent};

