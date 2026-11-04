//! Authorized Wireless Network Connection Manager (`connection_manager.rs`).
//!
//! Discovers visible 802.11 BSSIDs and saved user-authorized Windows WLAN profiles.
//! Never extracts Wi-Fi passwords (`key=clear` is strictly forbidden per Phase 12).

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisibleWifiNetwork {
    pub ssid: String,
    pub bssid: String,
    pub signal_pct: u32,
    pub channel: u32,
    pub band: String,
    pub radio_type: String,
    pub authentication: String,
    pub has_saved_profile: bool,
}

pub fn scan_visible_and_authorized_networks() -> (Vec<VisibleWifiNetwork>, Vec<String>) {
    let profiles_raw = hidden_cmd("netsh")
        .args(["wlan", "show", "profiles"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let saved_profiles = parse_saved_profiles(&profiles_raw);

    let networks_raw = hidden_cmd("netsh")
        .args(["wlan", "show", "networks", "mode=bssid"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let visible = parse_visible_networks(&networks_raw, &saved_profiles);
    (visible, saved_profiles)
}

pub fn parse_saved_profiles(raw: &str) -> Vec<String> {
    let mut profiles = Vec::new();
    for line in raw.lines() {
        if line.contains("All User Profile") || line.contains("Current User Profile") {
            if let Some((_, v)) = line.split_once(':') {
                let p = v.trim();
                if !p.is_empty() {
                    profiles.push(p.to_string());
                }
            }
        }
    }
    profiles
}

pub fn parse_visible_networks(raw: &str, saved_profiles: &[String]) -> Vec<VisibleWifiNetwork> {
    let mut list = Vec::new();
    let mut cur_ssid = String::new();
    let mut cur_auth = String::new();
    let mut cur_bssid = String::new();
    let mut cur_signal = 0u32;
    let mut cur_radio = String::new();
    let mut cur_band = String::new();
    let mut cur_channel = 0u32;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("SSID ") {
            if let Some((_, v)) = trimmed.split_once(':') {
                cur_ssid = v.trim().to_string();
            }
        } else if trimmed.starts_with("Authentication") {
            if let Some((_, v)) = trimmed.split_once(':') {
                cur_auth = v.trim().to_string();
            }
        } else if trimmed.starts_with("BSSID ") {
            if !cur_bssid.is_empty() && !cur_ssid.is_empty() {
                let has_prof = saved_profiles
                    .iter()
                    .any(|p| p.eq_ignore_ascii_case(&cur_ssid));
                list.push(VisibleWifiNetwork {
                    ssid: cur_ssid.clone(),
                    bssid: cur_bssid.clone(),
                    signal_pct: cur_signal,
                    channel: cur_channel,
                    band: cur_band.clone(),
                    radio_type: cur_radio.clone(),
                    authentication: cur_auth.clone(),
                    has_saved_profile: has_prof,
                });
            }
            if let Some((_, v)) = trimmed.split_once(':') {
                cur_bssid = v.trim().to_string();
            }
        } else if trimmed.starts_with("Signal") {
            if let Some((_, v)) = trimmed.split_once(':') {
                cur_signal = v.trim().trim_end_matches('%').parse().unwrap_or(0);
            }
        } else if trimmed.starts_with("Radio type") {
            if let Some((_, v)) = trimmed.split_once(':') {
                cur_radio = v.trim().to_string();
            }
        } else if trimmed.starts_with("Band") {
            if let Some((_, v)) = trimmed.split_once(':') {
                cur_band = v.trim().to_string();
            }
        } else if trimmed.starts_with("Channel") {
            if let Some((_, v)) = trimmed.split_once(':') {
                cur_channel = v.trim().parse().unwrap_or(0);
            }
        }
    }

    if !cur_bssid.is_empty() && !cur_ssid.is_empty() {
        let has_prof = saved_profiles
            .iter()
            .any(|p| p.eq_ignore_ascii_case(&cur_ssid));
        list.push(VisibleWifiNetwork {
            ssid: cur_ssid,
            bssid: cur_bssid,
            signal_pct: cur_signal,
            channel: cur_channel,
            band: cur_band,
            radio_type: cur_radio,
            authentication: cur_auth,
            has_saved_profile: has_prof,
        });
    }

    list
}

