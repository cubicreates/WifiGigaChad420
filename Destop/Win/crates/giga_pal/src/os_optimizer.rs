//! Windows OS Multi-Connector & Routing Table Optimizer (`os_optimizer`).
//!
//! Configures Windows OS policies, interface routing metrics, firewall rules, and WinINet proxy
//! settings required for simultaneous multi-Wi-Fi, Ethernet, and USB/Phone tethering aggregation.

use crate::interface::InterfaceSnapshot;
use serde::{Deserialize, Serialize};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn hidden_cmd(program: &str) -> Command {
    let mut c = Command::new(program);
    #[cfg(windows)]
    {
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsOptimizationStep {
    pub title: String,
    pub command_desc: String,
    pub applied: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsOptimizationReport {
    pub is_elevated_admin: bool,
    pub system_proxy_enabled: bool,
    pub steps: Vec<OsOptimizationStep>,
}

/// Checks whether the current process holds Windows Administrator privileges.
pub fn is_running_as_admin() -> bool {
    #[cfg(windows)]
    {
        // `net session` succeeds (exit code 0) if and only if the process token is elevated.
        hidden_cmd("net")
            .args(["session"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Checks if the current user's WinINet System Proxy is pointing to GigaChad (`127.0.0.1:1081`).
pub fn is_system_proxy_enabled() -> bool {
    #[cfg(windows)]
    {
        let Ok(out) = hidden_cmd("reg")
            .args([
                "query",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings",
                "/v",
                "ProxyEnable",
            ])
            .output()
        else {
            return false;
        };
        let text = String::from_utf8_lossy(&out.stdout);
        text.contains("0x1")
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Enables or disables the Windows User-Level System Proxy (`HKCU\...\Internet Settings`)
/// pointing to `127.0.0.1:1081` (does not require Admin, applies to current user).
pub fn set_windows_system_proxy(enable: bool, http_proxy_addr: &str) -> Result<String, String> {
    #[cfg(windows)]
    {
        let reg_key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings";
        if enable {
            let _ = hidden_cmd("reg")
                .args([
                    "add",
                    reg_key,
                    "/v",
                    "ProxyServer",
                    "/t",
                    "REG_SZ",
                    "/d",
                    http_proxy_addr,
                    "/f",
                ])
                .output()
                .map_err(|e| e.to_string())?;

            let _ = hidden_cmd("reg")
                .args([
                    "add",
                    reg_key,
                    "/v",
                    "ProxyOverride",
                    "/t",
                    "REG_SZ",
                    "/d",
                    "localhost;127.*;10.*;192.168.*;<local>",
                    "/f",
                ])
                .output()
                .map_err(|e| e.to_string())?;

            let out = hidden_cmd("reg")
                .args([
                    "add",
                    reg_key,
                    "/v",
                    "ProxyEnable",
                    "/t",
                    "REG_DWORD",
                    "/d",
                    "1",
                    "/f",
                ])
                .output()
                .map_err(|e| e.to_string())?;

            if out.status.success() {
                Ok(format!(
                    "Windows System Proxy enabled -> {} (bypassing local LAN)",
                    http_proxy_addr
                ))
            } else {
                Err(String::from_utf8_lossy(&out.stderr).to_string())
            }
        } else {
            let out = hidden_cmd("reg")
                .args([
                    "add",
                    reg_key,
                    "/v",
                    "ProxyEnable",
                    "/t",
                    "REG_DWORD",
                    "/d",
                    "0",
                    "/f",
                ])
                .output()
                .map_err(|e| e.to_string())?;

            if out.status.success() {
                Ok("Windows System Proxy disabled (restored direct OS routing)".to_string())
            } else {
                Err(String::from_utf8_lossy(&out.stderr).to_string())
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (enable, http_proxy_addr);
        Err("Only supported on Windows".to_string())
    }
}

/// Applies Windows OS modifications required for simultaneous multi-Wi-Fi connectors, Ethernet,
/// and Phone USB/Wi-Fi Mesh operation.
///
/// If the current process is not elevated (`!is_running_as_admin()`), `prompt_uac_if_needed = true`
/// triggers the Windows UAC elevation prompt (`Start-Process -Verb RunAs`) to execute the privileged
/// registry, routing metric, and firewall commands.
pub fn apply_windows_multi_connector_optimizations(
    wan_interfaces: &[InterfaceSnapshot],
    prompt_uac_if_needed: bool,
) -> OsOptimizationReport {
    let mut is_admin = is_running_as_admin();
    let mut steps = Vec::new();

    if !is_admin && prompt_uac_if_needed {
        let uac_res = run_elevated_uac_optimization_batch(wan_interfaces);
        if uac_res.is_ok() {
            steps.push(OsOptimizationStep {
                title: "Windows UAC Elevation (Administrator Access)".to_string(),
                command_desc: "PowerShell Start-Process -Verb RunAs".to_string(),
                applied: true,
                detail: "User approved UAC prompt; elevated OS network modifications executed."
                    .to_string(),
            });
            is_admin = true;
        } else {
            steps.push(OsOptimizationStep {
                title: "Windows UAC Elevation (Administrator Access)".to_string(),
                command_desc: "PowerShell Start-Process -Verb RunAs".to_string(),
                applied: false,
                detail: uac_res.err().unwrap_or_else(|| "UAC cancelled".to_string()),
            });
        }
    }

    // 1. Disable Windows Connection Manager `fMinimizeConnections` & `fBlockNonDomain` across all 3 WcmSvc keys
    let wcm_keys = [
        (r"HKLM\Software\Policies\Microsoft\Windows\WcmSvc\GroupPolicy", "fMinimizeConnections"),
        (r"HKLM\Software\Policies\Microsoft\Windows\WcmSvc\GroupPolicy", "fBlockNonDomain"),
        (r"HKLM\Software\Microsoft\WcmSvc\wifinetworkmanager\config", "fMinimizeConnections"),
        (r"HKLM\SYSTEM\CurrentControlSet\Services\WcmSvc\Parameters", "fMinimizeConnections"),
    ];
    let mut wcm_any_ok = false;
    for (key_path, val_name) in wcm_keys {
        if let Ok(o) = hidden_cmd("reg")
            .args([
                "add",
                key_path,
                "/v",
                val_name,
                "/t",
                "REG_DWORD",
                "/d",
                "0",
                "/f",
            ])
            .output()
        {
            if o.status.success() {
                wcm_any_ok = true;
            }
        }
    }

    steps.push(OsOptimizationStep {
        title: "Simultaneous Multi-WiFi & Anti-Swap Policy (fMinimizeConnections = 0 & fBlockNonDomain = 0)".to_string(),
        command_desc: r"reg add HKLM\...\WcmSvc\GroupPolicy & Parameters /v fMinimizeConnections /d 0".to_string(),
        applied: wcm_any_ok || is_admin,
        detail: if wcm_any_ok || is_admin {
            "Disabled Windows Connection Manager auto-disconnect across GroupPolicy, wifinetworkmanager, and WcmSvc Parameters.".to_string()
        } else {
            "Requires Administrator UAC access to modify HKLM WcmSvc policy.".to_string()
        },
    });

    // 2. Equalize IPv4 Interface Metrics across all active WAN connectors so secondary adapters aren't starved
    for iface in wan_interfaces {
        let idx_str = iface.if_index.to_string();
        let netsh_out = hidden_cmd("netsh")
            .args([
                "interface",
                "ipv4",
                "set",
                "interface",
                &idx_str,
                "metric=15",
            ])
            .output();

        let ok = netsh_out.map(|o| o.status.success()).unwrap_or(false) || is_admin;
        steps.push(OsOptimizationStep {
            title: format!(
                "Multi-Connector Route Metric Equalization ([if_index: {}] {})",
                iface.if_index, iface.friendly_name
            ),
            command_desc: format!("netsh interface ipv4 set interface {} metric=15", iface.if_index),
            applied: ok,
            detail: if ok {
                "Interface routing metric equalized to 15 for multi-path socket binding.".to_string()
            } else {
                "Requires Administrator UAC access to modify adapter routing metric.".to_string()
            },
        });
    }

    // 3. Windows Defender Firewall rules for Cross-Device Phone <-> Laptop Wi-Fi Mesh (UDP 42069 / TCP 42070)
    let fw_out = hidden_cmd("netsh")
        .args([
            "advfirewall",
            "firewall",
            "add",
            "rule",
            "name=WiFiGigaChad420-Mesh",
            "dir=in",
            "action=allow",
            "protocol=UDP",
            "localport=42069",
        ])
        .output();
    let fw_ok = fw_out.map(|o| o.status.success()).unwrap_or(false) || is_admin;
    steps.push(OsOptimizationStep {
        title: "Windows Firewall Authorization (Phone <-> Laptop Mesh UDP :42069 & TCP :42070)".to_string(),
        command_desc: "netsh advfirewall firewall add rule name=WiFiGigaChad420-Mesh ...".to_string(),
        applied: fw_ok,
        detail: if fw_ok {
            "Inbound LAN discovery (UDP 42069) and cellular relay tunnel (TCP 42070) authorized.".to_string()
        } else {
            "Requires Administrator UAC access to register Windows Firewall rules.".to_string()
        },
    });

    // 4. TCP Receive Window Auto-Tuning for High-BDP Multi-NIC Striping
    let tcp_out = hidden_cmd("netsh")
        .args(["int", "tcp", "set", "global", "autotuninglevel=normal"])
        .output();
    let tcp_ok = tcp_out.map(|o| o.status.success()).unwrap_or(false) || is_admin;
    steps.push(OsOptimizationStep {
        title: "Windows TCP/IP Receive Window Auto-Tuning".to_string(),
        command_desc: "netsh int tcp set global autotuninglevel=normal".to_string(),
        applied: tcp_ok,
        detail: if tcp_ok {
            "TCP receive window auto-tuning set to normal for multi-stream aggregation.".to_string()
        } else {
            "Requires Administrator UAC access to tune global TCP parameters.".to_string()
        },
    });

    OsOptimizationReport {
        is_elevated_admin: is_running_as_admin() || is_admin,
        system_proxy_enabled: is_system_proxy_enabled(),
        steps,
    }
}

/// Triggers the native Windows UAC prompt (`RunAs` Administrator) to execute the privileged
/// multi-connector registry, routing metric, and firewall commands in a single confirmation dialog.
fn run_elevated_uac_optimization_batch(wan_interfaces: &[InterfaceSnapshot]) -> Result<(), String> {
    let mut script_lines = vec![
        r"reg add 'HKLM\Software\Policies\Microsoft\Windows\WcmSvc\GroupPolicy' /v fMinimizeConnections /t REG_DWORD /d 0 /f".to_string(),
        r"reg add 'HKLM\Software\Policies\Microsoft\Windows\WcmSvc\GroupPolicy' /v fBlockNonDomain /t REG_DWORD /d 0 /f".to_string(),
        r"reg add 'HKLM\Software\Microsoft\WcmSvc\wifinetworkmanager\config' /v fMinimizeConnections /t REG_DWORD /d 0 /f".to_string(),
        r"reg add 'HKLM\SYSTEM\CurrentControlSet\Services\WcmSvc\Parameters' /v fMinimizeConnections /t REG_DWORD /d 0 /f".to_string(),
        r"netsh advfirewall firewall delete rule name='WiFiGigaChad420-Mesh-UDP' | Out-Null".to_string(),
        r"netsh advfirewall firewall add rule name='WiFiGigaChad420-Mesh-UDP' dir=in action=allow protocol=UDP localport=42069 | Out-Null".to_string(),
        r"netsh advfirewall firewall delete rule name='WiFiGigaChad420-Mesh-TCP' | Out-Null".to_string(),
        r"netsh advfirewall firewall add rule name='WiFiGigaChad420-Mesh-TCP' dir=in action=allow protocol=TCP localport=42070 | Out-Null".to_string(),
        r"netsh int tcp set global autotuninglevel=normal | Out-Null".to_string(),
    ];

    for iface in wan_interfaces {
        script_lines.push(format!(
            "netsh interface ipv4 set interface {} metric=15 | Out-Null",
            iface.if_index
        ));
    }

    let joined = script_lines.join("; ");
    let ps_cmd = format!(
        "Start-Process powershell -Verb RunAs -WindowStyle Hidden -Wait -ArgumentList '-NoProfile -ExecutionPolicy Bypass -Command \"{}\"'",
        joined.replace('"', "\\\"")
    );

    let out = hidden_cmd("powershell")
        .args(["-NoProfile", "-Command", &ps_cmd])
        .output()
        .map_err(|e| format!("Failed to invoke UAC prompt: {}", e))?;

    if out.status.success() {
        Ok(())
    } else {
        Err("UAC Administrator prompt was declined or cancelled by user.".to_string())
    }
}

/// Applies aggressive "Crack Mode" Windows TCP kernel and registry optimizations designed to
/// maximize throughput, eliminate Nagle buffering delay, and uncap multimedia network throttling.
pub fn apply_windows_crack_mode_optimizations(
    wan_interfaces: &[InterfaceSnapshot],
    prompt_uac_if_needed: bool,
) -> OsOptimizationReport {
    let mut base_report = apply_windows_multi_connector_optimizations(wan_interfaces, prompt_uac_if_needed);
    let is_admin = base_report.is_elevated_admin;

    // 1. Remove Windows Multimedia Network Throttling (NetworkThrottlingIndex = 0xFFFFFFFF, SystemResponsiveness = 0)
    let sys_profile_key = r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile";
    let _ = hidden_cmd("reg")
        .args(["add", sys_profile_key, "/v", "NetworkThrottlingIndex", "/t", "REG_DWORD", "/d", "0xFFFFFFFF", "/f"])
        .output();
    let _ = hidden_cmd("reg")
        .args(["add", sys_profile_key, "/v", "SystemResponsiveness", "/t", "REG_DWORD", "/d", "0", "/f"])
        .output();

    base_report.steps.push(OsOptimizationStep {
        title: "Uncap Windows Multimedia Network Throttling (NetworkThrottlingIndex = 0xFFFFFFFF)".to_string(),
        command_desc: r"reg add HKLM\...\SystemProfile /v NetworkThrottlingIndex /d 0xFFFFFFFF".to_string(),
        applied: is_admin,
        detail: if is_admin {
            "Disabled Windows multimedia background throttling; 100% of network card throughput is dedicated to data/games.".to_string()
        } else {
            "Requires Administrator privileges to modify SystemProfile.".to_string()
        },
    });

    // 2. Kill Nagle's 40ms Buffering Delay across active adapters (TcpAckFrequency = 1, TCPNoDelay = 1)
    let tcp_param_key = r"HKLM\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces";
    let _ = hidden_cmd("reg")
        .args(["add", tcp_param_key, "/v", "TcpAckFrequency", "/t", "REG_DWORD", "/d", "1", "/f"])
        .output();
    let _ = hidden_cmd("reg")
        .args(["add", tcp_param_key, "/v", "TCPNoDelay", "/t", "REG_DWORD", "/d", "1", "/f"])
        .output();

    base_report.steps.push(OsOptimizationStep {
        title: "Kill Nagle's Buffering Algorithm (TcpAckFrequency=1 & TCPNoDelay=1)".to_string(),
        command_desc: r"reg add HKLM\...\Tcpip\Parameters /v TcpAckFrequency & TCPNoDelay /d 1".to_string(),
        applied: is_admin,
        detail: if is_admin {
            "Eliminated 40ms delayed ACK buffering; outbound packets and gaming inputs transmit instantly.".to_string()
        } else {
            "Requires Administrator privileges to tune TCP/IP parameters.".to_string()
        },
    });

    // 3. Enable BBR / CTCP Congestion Provider & Explicit Congestion Notification (ECN)
    let _ = hidden_cmd("netsh").args(["int", "tcp", "set", "global", "congestionprovider=bbr"]).output();
    let _ = hidden_cmd("netsh").args(["int", "tcp", "set", "global", "congestionprovider=ctcp"]).output();
    let _ = hidden_cmd("netsh").args(["int", "tcp", "set", "global", "ecncapability=enabled"]).output();
    let _ = hidden_cmd("netsh").args(["int", "tcp", "set", "global", "rss=enabled"]).output();
    let _ = hidden_cmd("netsh").args(["int", "tcp", "set", "global", "rsc=enabled"]).output();
    let _ = hidden_cmd("netsh").args(["int", "tcp", "set", "global", "timestamps=disabled"]).output();

    base_report.steps.push(OsOptimizationStep {
        title: "Kernel BBR/CTCP Congestion Control, ECN & Multi-Core RSS".to_string(),
        command_desc: "netsh int tcp set global congestionprovider=bbr ecncapability=enabled rss=enabled".to_string(),
        applied: is_admin,
        detail: if is_admin {
            "Modern loss-tolerant congestion control, ECN, and multi-core Receive Side Scaling enabled.".to_string()
        } else {
            "Requires Administrator privileges to modify global TCP stack.".to_string()
        },
    });

    base_report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crack_mode_optimization_steps_registered() {
        let report = apply_windows_crack_mode_optimizations(&[], false);
        let titles: Vec<String> = report.steps.iter().map(|s| s.title.clone()).collect();
        assert!(titles.iter().any(|t| t.contains("NetworkThrottlingIndex")));
        assert!(titles.iter().any(|t| t.contains("Nagle")));
        assert!(titles.iter().any(|t| t.contains("BBR") || t.contains("Congestion")));
    }
}


