import React from 'react';
import { SystemStatusPayload } from '../types/telemetry';

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

  const logs = data.logs || [];
  const isSingleRadioSwap = diag.physical_wifi_count <= 1 && diag.max_secondary_stas === 0;

  return `
    <!-- 1. HARDWARE MULTI-WIFI DIAGNOSTIC & WHY IT SWAPPED -->
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

      <!-- KPI Summary Row -->
      <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:12px;margin-bottom:14px;">
        <div style="background:rgba(15,23,42,0.75);border:1px solid var(--border);border-radius:10px;padding:12px;">
          <div style="font-size:11px;color:var(--text-muted);">PHYSICAL WI-FI RADIOS</div>
          <div style="font-size:20px;font-weight:800;font-family:var(--mono);color:${diag.physical_wifi_count >= 2 ? 'var(--green)' : 'var(--amber)'};margin-top:4px;">
            ${diag.physical_wifi_count} Adapter(s)
          </div>
          <div style="font-size:11px;color:var(--text-muted);margin-top:2px;">
            ${diag.wlan_adapters.map((a) => `${a.name}: ${a.description}`).join(' | ') || 'MediaTek Wi-Fi 6 MT7921'}
          </div>
        </div>

        <div style="background:rgba(15,23,42,0.75);border:1px solid var(--border);border-radius:10px;padding:12px;">
          <div style="font-size:11px;color:var(--text-muted);">CHIP SECONDARY STA SUPPORT</div>
          <div style="font-size:20px;font-weight:800;font-family:var(--mono);color:${diag.max_secondary_stas > 0 ? 'var(--green)' : '#f87171'};margin-top:4px;">
            Max Secondary STAs: ${diag.max_secondary_stas}
          </div>
          <div style="font-size:11px;color:var(--text-muted);margin-top:2px;">
            ${diag.max_secondary_stas === 0 ? 'Single-channel hardware radio (cannot tune to 2 APs on 1 card)' : 'Supports dual simultaneous SSIDs on 1 card'}
          </div>
        </div>

        <div style="background:rgba(15,23,42,0.75);border:1px solid var(--border);border-radius:10px;padding:12px;">
          <div style="font-size:11px;color:var(--text-muted);">WINDOWS ANTI-SWAP POLICY</div>
          <div style="font-size:20px;font-weight:800;font-family:var(--mono);color:var(--green);margin-top:4px;">
            fMinimizeConnections = 0
          </div>
          <div style="font-size:11px;color:var(--text-muted);margin-top:2px;">
            Locked across GroupPolicy, wifinetworkmanager &amp; WcmSvc
          </div>
        </div>
      </div>

      <!-- Root Cause Explanation Box -->
      <div style="background:${isSingleRadioSwap ? 'rgba(245,158,11,0.1)' : 'rgba(16,185,129,0.1)'};border:1px solid ${isSingleRadioSwap ? 'rgba(245,158,11,0.45)' : 'rgba(16,185,129,0.45)'};border-radius:10px;padding:14px;">
        <div style="font-size:13px;font-weight:800;color:${isSingleRadioSwap ? '#fbbf24' : '#34d399'};margin-bottom:6px;">
          ${diag.diagnosis_title}
        </div>
        <div style="font-size:12px;color:#e2e8f0;line-height:1.6;">
          ${diag.diagnosis_explanation}
        </div>
        ${
          isSingleRadioSwap
            ? `
          <div style="margin-top:12px;padding-top:10px;border-top:1px solid rgba(255,255,255,0.1);display:grid;grid-template-columns:1fr 1fr;gap:12px;">
            <div style="background:rgba(9,13,22,0.7);padding:10px;border-radius:8px;border:1px solid rgba(59,130,246,0.35);">
              <div style="font-size:12px;font-weight:700;color:#60a5fa;margin-bottom:4px;">
                Fix #1 (No Extra Dongle Needed): Phone USB Wi-Fi Bridge
              </div>
              <div style="font-size:11px;color:var(--text-muted);line-height:1.5;">
                1. Connect your <strong>Phone</strong> to the <strong>2nd Wi-Fi network</strong>.<br/>
                2. Plug your Phone into this laptop with a <strong>USB cable</strong>.<br/>
                3. Enable <strong>USB Tethering</strong> on the phone. Windows adds it as Adapter #2 while your internal MediaTek Wi-Fi stays on Wi-Fi #1 with <strong>zero swapping</strong>!
              </div>
            </div>
            <div style="background:rgba(9,13,22,0.7);padding:10px;border-radius:8px;border:1px solid rgba(16,185,129,0.35);">
              <div style="font-size:12px;font-weight:700;color:#34d399;margin-bottom:4px;">
                Fix #2: Plug in a USB Wi-Fi Adapter (Wi-Fi 2)
              </div>
              <div style="font-size:11px;color:var(--text-muted);line-height:1.5;">
                1. Plug any USB Wi-Fi dongle into a USB port (appears as <code>Wi-Fi 2</code>).<br/>
                2. Use the <strong>Per-Adapter Wi-Fi Connector</strong> below to pin <code>Wi-Fi</code> to SSID #1 and <code>Wi-Fi 2</code> to SSID #2 simultaneously!
              </div>
            </div>
          </div>
        `
            : ''
        }
      </div>
    </div>

    <!-- 2. PER-ADAPTER WI-FI CONNECTOR (PREVENTS WINDOWS TASKBAR SWAPPING) -->
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
          <div style="font-size:11px;color:var(--text-muted);margin-bottom:5px;">Select Physical Wi-Fi Adapter</div>
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
          </select>
        </div>

        <div>
          <div style="font-size:11px;color:var(--text-muted);margin-bottom:5px;">Select Saved Wi-Fi SSID Profile</div>
          <select id="wifi-conn-ssid">
            ${
              diag.saved_profiles.length > 0
                ? diag.saved_profiles.map((p) => `<option value="${p}">${p}</option>`).join('')
                : `<option value="KRMU STUDENTS">KRMU STUDENTS</option>`
            }
          </select>
        </div>

        <button class="btn" onclick="handleConnectWifiOnAdapter(false)">
          Bind &amp; Connect Adapter
        </button>
      </div>

      ${
        wifiConnectStatus
          ? `<div class="console-box" style="margin-top:12px;border-color:rgba(59,130,246,0.45);">${wifiConnectStatus}</div>`
          : ''
      }
    </div>

    <!-- 3. LIVE SYSTEM & DIAGNOSTIC EVENT LOG STREAM -->
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

