import React from 'react';
import { CapabilityStatusLabel, SystemStatusPayload } from '../types/telemetry';

function statusBadgeClass(label: CapabilityStatusLabel | string): string {
  switch (label) {
    case 'SUPPORTED':
      return 'badge-healthy';
    case 'EXPERIMENTAL':
    case 'UNKNOWN':
    case 'NOT TESTED':
      return 'badge-degraded';
    case 'UNSUPPORTED':
    case 'BLOCKED':
    default:
      return 'badge-dead';
  }
}

export function renderLogsScreen(data: SystemStatusPayload, wifiConnectStatus: string | null): string {
  const diag = data.wlan_diag || {
    physical_wifi_count: 1,
    max_secondary_stas: 0,
    can_simultaneous_wifi_natively: false,
    wlan_adapters: [],
    saved_profiles: [],
    diagnosis_code: 'SINGLE_RADIO_HARDWARE_SWAP',
    diagnosis_title: 'Inspecting Wi-Fi Hardware Capabilities...',
    diagnosis_explanation: 'Click Run Hardware Diagnostic to inspect physical Wi-Fi radios and WcmSvc anti-swap policies.',
  };

  const vwifi = data.virtual_wifi || {
    hardware_capability_status: 'UNSUPPORTED' as CapabilityStatusLabel,
    driver_capability_status: 'UNSUPPORTED' as CapabilityStatusLabel,
    firmware_capability_status: 'UNSUPPORTED' as CapabilityStatusLabel,
    virtual_adapter_status: 'EXPERIMENTAL' as CapabilityStatusLabel,
    physical_radios_count: diag.physical_wifi_count,
    actual_connected_wifi_count: 1,
    logical_virtual_interfaces_count: 1,
    radio_switching_status: 'EXPERIMENTAL' as CapabilityStatusLabel,
    current_active_channel: null,
    current_network: 'Detecting...',
    current_scheduler_state: 'HysteresisFailoverGuard',
    packet_loss_pct: 0,
    switching_latency_ms: 1840,
    throughput_mbps: 0,
    experiment_status: 'EXPERIMENTAL' as CapabilityStatusLabel,
    last_error: 'DRIVER_CAPABILITY_UNSUPPORTED: MediaTek MT7921 reports Maximum Number of Secondary STAs Supported = 0.',
    driver_installation_status: 'NOT TESTED' as CapabilityStatusLabel,
    virtual_interface_name: 'WiFi GigaChad 420 Virtual Network',
    virtual_interface_enabled: true,
    hysteresis_switch_count: 0,
    last_experiment_summary: 'Click "Run Radio Time-Sharing Experiment" to measure MT7921 association & switching latency.',
    structured_events: [],
  };

  const logs = data.logs || [];
  const isSingleRadioSwap = diag.physical_wifi_count <= 1 && diag.max_secondary_stas === 0;

  return `
    <!-- PHASE 13: EXPERIMENTAL VIRTUAL WI-FI RESEARCH & TELEMETRY PANEL -->
    <div class="card" style="border-color:rgba(245,158,11,0.65);box-shadow:0 12px 32px rgba(0,0,0,0.45);">
      <div class="card-header">
        <div>
          <div class="card-title">Experimental Virtual Wi-Fi — Hardware, NDIS MUX &amp; Radio Time-Sharing Lab</div>
          <div style="font-size:12px;color:var(--text-muted);margin-top:2px;">
            Logical Interface: <code>${vwifi.virtual_interface_name}</code> • Honest Hardware/Firmware/Driver Verification Matrix
          </div>
        </div>
        <div style="display:flex;gap:8px;align-items:center;">
          <span class="badge ${statusBadgeClass(vwifi.experiment_status)}">${vwifi.experiment_status}</span>
          <button class="btn btn-secondary" onclick="handleExpProbeCapabilities()">Probe Capabilities</button>
          <button class="btn" onclick="handleExpRunTimeShare()">Run Radio Time-Sharing Experiment</button>
          <button class="btn btn-secondary" onclick="handleExpToggleVirtualIf()">
            ${vwifi.virtual_interface_enabled ? 'Rollback Virtual Interface' : 'Activate Virtual Interface'}
          </button>
        </div>
      </div>

      <!-- Mandatory Experimental Disclaimer -->
      <div style="background:rgba(245,158,11,0.12);border:1px solid rgba(245,158,11,0.45);border-radius:8px;padding:10px 12px;margin-bottom:14px;font-size:12px;color:#fde68a;line-height:1.5;">
        <strong>EXPERIMENTAL ENGINEERING DISCLAIMER:</strong> Creating a virtual network interface (<code>${vwifi.virtual_interface_name}</code>) does <strong>not</strong> create additional physical 802.11 RF radios or bypass hardware/firmware limits. On a single MediaTek MT7921 radio (<code>MaxSecondaryStas = 0</code>), concurrent Multi-STA is <strong>UNSUPPORTED</strong> by firmware; single-radio time-sharing requires sequential reassociation (~1.8s blackout per switch). True simultaneous bandwidth aggregation requires 2+ physical uplinks (e.g., MT7921 + Phone USB Tethering / USB Wi-Fi 2 / Ethernet).
      </div>

      <!-- 16 Mandatory Phase 13 Telemetry Fields Grid -->
      <div style="display:grid;grid-template-columns:repeat(4,1fr);gap:10px;margin-bottom:14px;">
        <div style="background:rgba(15,23,42,0.8);border:1px solid var(--border);border-radius:8px;padding:10px;">
          <div style="font-size:10px;color:var(--text-muted);">1. HARDWARE MULTI-STA</div>
          <div style="margin-top:5px;"><span class="badge ${statusBadgeClass(vwifi.hardware_capability_status)}">${vwifi.hardware_capability_status}</span></div>
        </div>
        <div style="background:rgba(15,23,42,0.8);border:1px solid var(--border);border-radius:8px;padding:10px;">
          <div style="font-size:10px;color:var(--text-muted);">2. DRIVER MULTI-STA (WDI)</div>
          <div style="margin-top:5px;"><span class="badge ${statusBadgeClass(vwifi.driver_capability_status)}">${vwifi.driver_capability_status}</span></div>
        </div>
        <div style="background:rgba(15,23,42,0.8);border:1px solid var(--border);border-radius:8px;padding:10px;">
          <div style="font-size:10px;color:var(--text-muted);">3. FIRMWARE MULTI-STA</div>
          <div style="margin-top:5px;"><span class="badge ${statusBadgeClass(vwifi.firmware_capability_status)}">${vwifi.firmware_capability_status}</span></div>
        </div>
        <div style="background:rgba(15,23,42,0.8);border:1px solid var(--border);border-radius:8px;padding:10px;">
          <div style="font-size:10px;color:var(--text-muted);">4. VIRTUAL ADAPTER STATUS</div>
          <div style="margin-top:5px;"><span class="badge ${statusBadgeClass(vwifi.virtual_adapter_status)}">${vwifi.virtual_adapter_status}</span></div>
        </div>

        <div style="background:rgba(15,23,42,0.8);border:1px solid var(--border);border-radius:8px;padding:10px;">
          <div style="font-size:10px;color:var(--text-muted);">5. PHYSICAL RADIOS</div>
          <div style="font-size:16px;font-weight:800;font-family:var(--mono);color:#e2e8f0;margin-top:4px;">${vwifi.physical_radios_count} Radio(s)</div>
        </div>
        <div style="background:rgba(15,23,42,0.8);border:1px solid var(--border);border-radius:8px;padding:10px;">
          <div style="font-size:10px;color:var(--text-muted);">6. ACTUAL CONNECTED WI-FI</div>
          <div style="font-size:16px;font-weight:800;font-family:var(--mono);color:#e2e8f0;margin-top:4px;">${vwifi.actual_connected_wifi_count} Network(s)</div>
        </div>
        <div style="background:rgba(15,23,42,0.8);border:1px solid var(--border);border-radius:8px;padding:10px;">
          <div style="font-size:10px;color:var(--text-muted);">7. LOGICAL VIRTUAL IFACES</div>
          <div style="font-size:16px;font-weight:800;font-family:var(--mono);color:#60a5fa;margin-top:4px;">${vwifi.logical_virtual_interfaces_count} (${vwifi.virtual_interface_enabled ? 'Active' : 'Rolled Back'})</div>
        </div>
        <div style="background:rgba(15,23,42,0.8);border:1px solid var(--border);border-radius:8px;padding:10px;">
          <div style="font-size:10px;color:var(--text-muted);">8. RADIO SWITCHING STATUS</div>
          <div style="margin-top:5px;"><span class="badge ${statusBadgeClass(vwifi.radio_switching_status)}">${vwifi.radio_switching_status}</span></div>
        </div>

        <div style="background:rgba(15,23,42,0.8);border:1px solid var(--border);border-radius:8px;padding:10px;">
          <div style="font-size:10px;color:var(--text-muted);">9. ACTIVE CHANNEL</div>
          <div style="font-size:15px;font-weight:800;font-family:var(--mono);color:#e2e8f0;margin-top:4px;">${vwifi.current_active_channel !== null ? 'Ch ' + vwifi.current_active_channel : 'N/A'}</div>
        </div>
        <div style="background:rgba(15,23,42,0.8);border:1px solid var(--border);border-radius:8px;padding:10px;">
          <div style="font-size:10px;color:var(--text-muted);">10. CURRENT NETWORK</div>
          <div style="font-size:14px;font-weight:800;font-family:var(--mono);color:var(--green);margin-top:4px;">${vwifi.current_network}</div>
        </div>
        <div style="background:rgba(15,23,42,0.8);border:1px solid var(--border);border-radius:8px;padding:10px;">
          <div style="font-size:10px;color:var(--text-muted);">11. SCHEDULER STATE</div>
          <div style="font-size:12px;font-weight:700;font-family:var(--mono);color:#fbbf24;margin-top:4px;">${vwifi.current_scheduler_state}</div>
        </div>
        <div style="background:rgba(15,23,42,0.8);border:1px solid var(--border);border-radius:8px;padding:10px;">
          <div style="font-size:10px;color:var(--text-muted);">12. PACKET LOSS</div>
          <div style="font-size:15px;font-weight:800;font-family:var(--mono);color:#e2e8f0;margin-top:4px;">${vwifi.packet_loss_pct.toFixed(1)}%</div>
        </div>

        <div style="background:rgba(15,23,42,0.8);border:1px solid var(--border);border-radius:8px;padding:10px;">
          <div style="font-size:10px;color:var(--text-muted);">13. SWITCHING LATENCY</div>
          <div style="font-size:15px;font-weight:800;font-family:var(--mono);color:#f87171;margin-top:4px;">~${vwifi.switching_latency_ms.toFixed(0)} ms</div>
        </div>
        <div style="background:rgba(15,23,42,0.8);border:1px solid var(--border);border-radius:8px;padding:10px;">
          <div style="font-size:10px;color:var(--text-muted);">14. LIVE THROUGHPUT</div>
          <div style="font-size:15px;font-weight:800;font-family:var(--mono);color:#60a5fa;margin-top:4px;">${vwifi.throughput_mbps.toFixed(2)} Mbps</div>
        </div>
        <div style="background:rgba(15,23,42,0.8);border:1px solid var(--border);border-radius:8px;padding:10px;">
          <div style="font-size:10px;color:var(--text-muted);">15. NDIS KERNEL MUX DRIVER</div>
          <div style="margin-top:5px;"><span class="badge ${statusBadgeClass(vwifi.driver_installation_status)}">${vwifi.driver_installation_status} (Opt-In)</span></div>
        </div>
        <div style="background:rgba(15,23,42,0.8);border:1px solid var(--border);border-radius:8px;padding:10px;">
          <div style="font-size:10px;color:var(--text-muted);">16. LAST HARDWARE BLOCKER</div>
          <div style="font-size:10px;font-family:var(--mono);color:#fca5a5;margin-top:4px;line-height:1.35;">${vwifi.last_error || 'None'}</div>
        </div>
      </div>

      <!-- Latest Radio Time-Sharing Experiment Result -->
      <div class="console-box" style="margin-bottom:12px;">
        <strong>Radio Time-Sharing &amp; Multi-STA Experiment Output:</strong><br/>
        ${vwifi.last_experiment_summary}
      </div>

      <!-- Phase 14 Structured Experimental Events Table -->
      <div style="font-size:12px;font-weight:700;color:#e2e8f0;margin-bottom:6px;">
        Phase 14 Structured Experimental Telemetry Events (${(vwifi.structured_events || []).length} events)
      </div>
      <div style="display:flex;flex-direction:column;gap:6px;max-height:220px;overflow-y:auto;">
        ${
          (vwifi.structured_events || []).length === 0
            ? `<div class="console-box">No structured experimental events recorded yet.</div>`
            : (vwifi.structured_events || [])
                .map(
                  (ev) => `
                <div style="background:rgba(9,13,22,0.9);border:1px solid var(--border);border-radius:6px;padding:8px 10px;display:flex;flex-wrap:wrap;gap:8px;align-items:center;font-family:var(--mono);font-size:11px;">
                  <span style="color:var(--text-muted);">${ev.timestamp}</span>
                  <span class="badge ${statusBadgeClass(ev.result)}">${ev.result}</span>
                  <span style="color:#fbbf24;font-weight:700;">${ev.event}</span>
                  <span style="color:#60a5fa;">[${ev.component}]</span>
                  <span style="color:#94a3b8;">Phy: ${ev.physical_interface} &rarr; Virt: ${ev.virtual_interface} (Net: ${ev.network_identifier})</span>
                  ${ev.duration_ms !== null ? `<span style="color:#34d399;">${ev.duration_ms.toFixed(1)} ms</span>` : ''}
                  <div style="width:100%;color:#e2e8f0;margin-top:2px;">${ev.measurement}</div>
                </div>
              `
                )
                .join('')
        }
      </div>
    </div>

    <!-- 2. HARDWARE MULTI-WIFI DIAGNOSTIC & PRACTICAL FIXES -->
    <div class="card" style="border-color:${isSingleRadioSwap ? 'rgba(245,158,11,0.6)' : 'rgba(16,185,129,0.55)'};">
      <div class="card-header">
        <div>
          <div class="card-title">Multi-WiFi Hardware &amp; Anti-Swap Diagnostic Inspector</div>
          <div style="font-size:12px;color:var(--text-muted);margin-top:2px;">
            Live inspection of Windows <code>netsh wlan show interfaces</code> &amp; <code>wirelesscapabilities</code>
          </div>
        </div>
        <div style="display:flex;gap:8px;align-items:center;">
          <span class="badge ${isSingleRadioSwap ? 'badge-degraded' : 'badge-healthy'}">
            ${isSingleRadioSwap ? '1 RADIO DETECTED (SINGLE-STA CHIP)' : 'MULTI-WIFI HARDWARE READY'}
          </span>
          <button class="btn" onclick="handleRunWifiDiagnostics()">Run Hardware Diagnostic</button>
        </div>
      </div>

      <div style="background:${isSingleRadioSwap ? 'rgba(245,158,11,0.1)' : 'rgba(16,185,129,0.1)'};border:1px solid ${isSingleRadioSwap ? 'rgba(245,158,11,0.45)' : 'rgba(16,185,129,0.45)'};border-radius:10px;padding:14px;">
        <div style="font-size:13px;font-weight:800;color:${isSingleRadioSwap ? '#fbbf24' : '#34d399'};margin-bottom:6px;">
          ${diag.diagnosis_title}
        </div>
        <div style="font-size:12px;color:#e2e8f0;line-height:1.6;">
          ${diag.diagnosis_explanation}
        </div>
      </div>
    </div>

    <!-- 3. PER-ADAPTER WI-FI CONNECTOR (PREVENTS WINDOWS TASKBAR SWAPPING) -->
    <div class="card">
      <div class="card-header">
        <div>
          <div class="card-title">Per-Adapter Wi-Fi Pinning &amp; Anti-Swap Connector</div>
          <div style="font-size:12px;color:var(--text-muted);margin-top:2px;">
            Connects a specific Wi-Fi interface (<code>Wi-Fi</code>, <code>Wi-Fi 2</code>) to a saved Wi-Fi SSID via <code>netsh wlan connect interface="..."</code> without disconnecting other adapters
          </div>
        </div>
        <span class="badge badge-standby">${diag.saved_profiles.length} SAVED PROFILES</span>
      </div>

      <div style="display:grid;grid-template-columns:1fr 1fr auto;gap:10px;align-items:end;">
        <div>
          <div style="font-size:11px;color:var(--text-muted);margin-bottom:5px;">Select Wi-Fi Adapter / Channel</div>
          <select id="wifi-conn-iface">
            ${
              diag.wlan_adapters.length > 0
                ? diag.wlan_adapters
                    .map(
                      (a) =>
                        `<option value="${a.name}">${a.name} — ${a.description} (${a.state}${a.ssid ? ': ' + a.ssid : ''})</option>`
                    )
                    .join('')
                : `<option value="Wi-Fi">Wi-Fi (Primary Adapter)</option>`
            }
            <option value="Wi-Fi Direct 1">Wi-Fi Direct 1 (VWIFIMP_WFD Virtual Port 1)</option>
            <option value="Wi-Fi Direct 2">Wi-Fi Direct 2 (Wireless Mesh Virtual Port 2)</option>
          </select>
        </div>

        <div>
          <div style="font-size:11px;color:var(--text-muted);margin-bottom:5px;">Select Saved Wi-Fi SSID Profile</div>
          <select id="wifi-conn-ssid">
            ${
              diag.saved_profiles.length > 0
                ? diag.saved_profiles.map((p) => `<option value="${p}">${p}</option>`).join('')
                : `<option value="Gheras_5G">Gheras_5G</option>`
            }
          </select>
        </div>

        <div style="display:flex;gap:8px;">
          <button class="btn" onclick="handleConnectWifiOnAdapter(false)">
            Bind &amp; Connect
          </button>
          <button class="btn" style="background:linear-gradient(135deg, #10b981, #059669);border-color:#34d399;" onclick="handleConnect3Wifi()">
            ⚡ Connect 3 Wi-Fi
          </button>
        </div>
      </div>

      ${
        wifiConnectStatus
          ? `<div class="console-box" style="margin-top:12px;border-color:rgba(59,130,246,0.45);">${wifiConnectStatus}</div>`
          : ''
      }
    </div>

    <!-- 4. LIVE SYSTEM & DIAGNOSTIC EVENT LOG STREAM -->
    <div class="card">
      <div class="card-header">
        <div>
          <div class="card-title">Live System, OS Routing &amp; Multi-WiFi Event Logs</div>
          <div style="font-size:12px;color:var(--text-muted);margin-top:2px;">
            Real-time diagnostic log stream (replaces the legacy black CLI window)
          </div>
        </div>
        <div style="display:flex;gap:8px;">
          <button class="btn btn-secondary" onclick="handleClearLogs()">Clear Logs</button>
        </div>
      </div>

      <div style="display:flex;flex-direction:column;gap:8px;max-height:380px;overflow-y:auto;">
        ${
          logs.length === 0
            ? `<div class="console-box">No log entries recorded yet.</div>`
            : logs
                .map((entry) => {
                  const badgeCls =
                    entry.level === 'OK'
                      ? 'badge-healthy'
                      : entry.level === 'WARN'
                      ? 'badge-degraded'
                      : entry.level === 'ERROR'
                      ? 'badge-dead'
                      : 'badge-standby';
                  return `
                    <div style="background:rgba(9,13,22,0.85);border:1px solid var(--border);border-radius:8px;padding:10px 12px;display:flex;align-items:flex-start;gap:10px;font-family:var(--mono);font-size:12px;">
                      <span style="color:var(--text-muted);white-space:nowrap;">${entry.timestamp}</span>
                      <span class="badge ${badgeCls}" style="min-width:54px;text-align:center;">${entry.level}</span>
                      <span style="color:#60a5fa;font-weight:700;white-space:nowrap;">[${entry.category}]</span>
                      <span style="color:#e2e8f0;line-height:1.45;word-break:break-word;">${entry.message}</span>
                    </div>
                  `;
                })
                .join('')
        }
      </div>
    </div>
  `;
}
