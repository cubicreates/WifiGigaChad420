import React from 'react';
import {
  CombinedSpeedTestReport,
  HealthScoreSnapshot,
  RoutingMode,
  WlanHardwareDiagnostics,
} from '../types/telemetry';

export interface WifiDashboardScreenProps {
  interfaces: HealthScoreSnapshot[];
  collectiveSpeedMbps: number;
  totalTransferredBytes: number;
  routingMode: RoutingMode;
  speedTest: CombinedSpeedTestReport | null;
  wlanDiag?: WlanHardwareDiagnostics;
}

function savedProfilesOptions(profiles: string[], defaultVal: string): string {
  if (!profiles || profiles.length === 0) {
    return `<option value="">No saved profiles detected</option>`;
  }
  return profiles
    .map(
      (p) =>
        `<option value="${p}" ${p.toLowerCase() === defaultVal.toLowerCase() ? 'selected' : ''}>${p}</option>`
    )
    .join('');
}

export function renderWifiDashboardScreen(props: WifiDashboardScreenProps): string {
  const ifaces = props.interfaces || [];
  const currentMode = props.routingMode || 'max_speed';
  const st = props.speedTest;
  const diag = props.wlanDiag;
  const physRadios = diag ? diag.physical_wifi_count : 1;

  let speedTestHtml = `Click "Run Live Speed Benchmark" to test actual download throughput (via Cloudflare CDN) across your active interfaces.`;
  if (st) {
    if (st.running) {
      speedTestHtml = `Testing live download speed across ${ifaces.length} adapter(s) via Cloudflare high-speed CDN...`;
    } else {
      const perLines = (st.per_adapter || [])
        .map(
          (a) =>
            `  * ${a.ssid ? 'Wi-Fi (' + a.ssid + ')' : a.interface_name} [Adapter #${a.if_index}] : ${a.speed_mbps.toFixed(2)} Mbps (Ping: ${a.rtt_ms.toFixed(0)} ms)`
        )
        .join('\n');
      speedTestHtml =
        `Single Best Adapter : ${st.single_best_mbps.toFixed(2)} Mbps\n` +
        `Combined Speed      : ${st.combined_mbps.toFixed(2)} Mbps (${st.speedup_factor.toFixed(2)}x Aggregation Multiplier)\n` +
        `Adapter Breakdown:\n${perLines || '  No active adapters tested.'}`;
    }
  }

  const cardsHtml =
    ifaces.length === 0
      ? `<div class="card"><p style="color:var(--text-muted);">No active network adapters currently connected. Connect to Wi-Fi or plug in Ethernet / USB dongle.</p></div>`
      : ifaces
          .map((i) => {
            const rxMbps = ((i.rx_throughput_bps || 0) / 1_000_000).toFixed(2);
            const txMbps = ((i.tx_throughput_bps || 0) / 1_000_000).toFixed(2);
            const indivMbps = (((i.rx_throughput_bps || 0) + (i.tx_throughput_bps || 0)) / 1_000_000).toFixed(2);
            const badgeCls =
              i.state === 'Healthy'
                ? 'badge-healthy'
                : i.state === 'Degraded'
                ? 'badge-degraded'
                : 'badge-dead';
            const title = i.ssid ? `Wi-Fi: ${i.ssid}` : i.interface_name;
            const signalStr = i.signal_quality_pct
              ? `${i.signal_quality_pct}% (${i.rssi_dbm || -50} dBm)`
              : (i.medium === 'Wi-Fi' ? 'Active' : 'Direct Link');

            const isWifi = i.medium.toLowerCase().includes('wi-fi') || i.ssid != null;

            return `
              <div class="wifi-item">
                <div class="wifi-top">
                  <div>
                    <div class="wifi-ssid">${title}</div>
                    <div class="wifi-adapter">[Adapter #${i.if_index}] ${i.description}</div>
                  </div>
                  <span class="badge ${badgeCls}">${i.state} • ${i.score.toFixed(0)}%</span>
                </div>

                <div class="speed-boxes">
                  <div class="speed-box">
                    <div class="speed-box-label">Real-Time Bandwidth (Live)</div>
                    <div class="speed-box-val">${indivMbps} Mbps</div>
                    <div style="font-size:10px;color:var(--text-muted);margin-top:2px;">↓ ${rxMbps} Mbps Rx | ↑ ${txMbps} Mbps Tx</div>
                  </div>
                  <div class="speed-box">
                    <div class="speed-box-label">Latency (Ping)</div>
                    <div class="speed-box-val">${i.ewma_rtt_ms.toFixed(0)} ms</div>
                    <div style="font-size:10px;color:var(--text-muted);margin-top:2px;">Jitter: ${i.jitter_ms.toFixed(1)} ms</div>
                  </div>
                </div>

                <div>
                  <div style="display:flex;justify-content:space-between;font-size:11px;color:var(--text-muted);margin-bottom:5px;">
                    <span>Interface Health Score</span>
                    <span>${i.score.toFixed(1)} / 100</span>
                  </div>
                  <div class="bar-track">
                    <div class="bar-fill" style="width:${Math.min(100, Math.max(5, i.score))}%;"></div>
                  </div>
                </div>

                <div class="meta-table">
                  <div class="meta-row"><span class="meta-k">Medium</span><span class="meta-v">${i.medium}</span></div>
                  <div class="meta-row"><span class="meta-k">Signal</span><span class="meta-v">${signalStr}</span></div>
                  <div class="meta-row"><span class="meta-k">Packet Loss</span><span class="meta-v">${(i.loss_rate * 100).toFixed(0)}%</span></div>
                  <div class="meta-row"><span class="meta-k">Local IP</span><span class="meta-v">${i.local_ipv4 || 'N/A'}</span></div>
                  <div class="meta-row"><span class="meta-k">Gateway IP</span><span class="meta-v">${i.gateway_v4 || 'N/A'}</span></div>
                  <div class="meta-row"><span class="meta-k">Total Session Rx</span><span class="meta-v">${(i.total_rx_bytes / (1024 * 1024)).toFixed(2)} MB</span></div>
                </div>

                ${
                  isWifi
                    ? `
                  <div style="margin-top:12px;padding-top:10px;border-top:1px solid rgba(255,255,255,0.06);">
                    <div style="font-size:11px;color:var(--text-muted);margin-bottom:5px;">Connect / Switch Profile:</div>
                    <div style="display:flex;gap:8px;">
                      <select id="adapter-sel-${i.if_index}" style="flex:1;padding:6px 8px;background:rgba(2,6,23,0.8);border:1px solid var(--border);border-radius:4px;color:#f8fafc;font-size:11px;">
                        ${savedProfilesOptions(diag?.saved_profiles || [], i.ssid || '')}
                      </select>
                      <button class="btn btn-secondary" style="padding:6px 12px;font-size:11px;white-space:nowrap;" onclick="handleConnectAdapterSsid('${i.interface_name}', 'adapter-sel-${i.if_index}')">
                        Switch
                      </button>
                    </div>
                  </div>
                `
                    : ''
                }
              </div>
            `;
          })
          .join('');

  return `
    <section class="screen-container">
      <div class="hero-card">
        <div class="hero-metric-group">
          <div>
            <div class="hero-label">Collective Live Throughput (Windows NDIS Kernel)</div>
            <div class="hero-speed">${props.collectiveSpeedMbps.toFixed(2)} Mbps</div>
            <div class="hero-sub">${ifaces.length} Active Hardware Connection(s) Monitored</div>
          </div>
          <div>
            <div class="hero-label">Total Transferred Volume</div>
            <div class="hero-speed-sm">${(props.totalTransferredBytes / (1024 * 1024)).toFixed(2)} MB</div>
            <div class="hero-sub">Direct Hardware NDIS Octets</div>
          </div>
        </div>
        <div class="hero-actions">
          <button class="btn" onclick="handleRunCombinedSpeedTest()">
            ${st && st.running ? 'Running Live Benchmark...' : 'Run Live Speed Benchmark'}
          </button>
          <button class="btn btn-secondary" onclick="fetch('/api/probe',{method:'POST'}).then(pollBackend)">
            Refresh Interfaces
          </button>
        </div>
      </div>

      <div class="card">
        <div class="card-header">
          <div class="card-title">Traffic Scheduling Mode</div>
        </div>
        <div class="mode-grid">
          <div class="mode-card ${currentMode === 'max_speed' ? 'active' : ''}" onclick="handleSelectRoutingMode('max_speed')">
            <div class="mode-title">
              <span>Max Speed Mode</span>
              <span class="badge ${currentMode === 'max_speed' ? 'badge-healthy' : ''}">${currentMode === 'max_speed' ? 'ACTIVE' : 'SELECT'}</span>
            </div>
            <div class="mode-desc">Stripes downloads and streams across ALL connected Wi-Fi, Ethernet, and USB links concurrently.</div>
          </div>

          <div class="mode-card ${currentMode === 'low_ping' ? 'active' : ''}" onclick="handleSelectRoutingMode('low_ping')">
            <div class="mode-title">
              <span>Low-Ping Mode</span>
              <span class="badge ${currentMode === 'low_ping' ? 'badge-healthy' : ''}">${currentMode === 'low_ping' ? 'ACTIVE' : 'SELECT'}</span>
            </div>
            <div class="mode-desc">Pins real-time gaming and VoIP packets to your lowest-latency link and offloads background downloads to secondary adapters.</div>
          </div>

          <div class="mode-card ${currentMode === 'data_saver' ? 'active' : ''}" onclick="handleSelectRoutingMode('data_saver')">
            <div class="mode-title">
              <span>Data-Saver Mode</span>
              <span class="badge ${currentMode === 'data_saver' ? 'badge-healthy' : ''}">${currentMode === 'data_saver' ? 'ACTIVE' : 'SELECT'}</span>
            </div>
            <div class="mode-desc">Avoids metered phone tethering and preserves secondary links unless the primary connection degrades.</div>
          </div>
        </div>
      </div>

      <div class="card">
        <div class="card-header">
          <div class="card-title">Live Speed Benchmark vs. Speedtest.net</div>
          <button class="btn" onclick="handleRunCombinedSpeedTest()">
            ${st && st.running ? 'Testing Speed...' : 'Run Benchmark'}
          </button>
        </div>
        <div class="console-box">${speedTestHtml}</div>
      </div>

      <div class="card" style="border:1px solid rgba(245,158,11,0.5);background:rgba(30,27,75,0.4);">
        <div class="card-header">
          <div>
            <div class="card-title" style="color:#fbbf24;">⚡ Windows Network "Crack Mode" Optimizer</div>
            <div style="font-size:12px;color:var(--text-muted);margin-top:2px;">
              Uncap Windows multimedia throttling, kill Nagle's 40ms buffering delay, enable BBR/ECN, and scale across all CPU cores.
            </div>
          </div>
          <button class="btn" style="background:linear-gradient(135deg, #f59e0b, #d97706);border-color:#fbbf24;color:#0f172a;font-weight:700;" onclick="handleEngageCrackMode()">
            🚀 Engage Crack Mode
          </button>
        </div>
        <div style="margin-top:8px;font-size:11px;color:#cbd5e1;line-height:1.5;">
          • <strong>NetworkThrottlingIndex = 0xFFFFFFFF</strong>: Disables Windows multimedia 10% network throttle.<br/>
          • <strong>SystemResponsiveness = 0</strong>: Prioritizes network packet processing over background OS tasks.<br/>
          • <strong>TcpAckFrequency = 1 &amp; TCPNoDelay = 1</strong>: Kills Nagle's 40ms packet delay for instant gaming ping &amp; web clicks.<br/>
          • <strong>BBR &amp; ECN</strong>: Enables loss-tolerant congestion control and multi-core Receive Side Scaling.
        </div>
      </div>

      <div class="card" style="border:1px solid rgba(16,185,129,0.5);background:rgba(6,78,59,0.25);">
        <div class="card-header">
          <div>
            <div class="card-title" style="color:#34d399;">🔥 Multi-Stream Turbo Downloader (IDM-Grade Parallel Acceleration)</div>
            <div style="font-size:12px;color:var(--text-muted);margin-top:2px;">
              Saturate 100% of your bandwidth by pulling files in parallel range chunks across all active connections.
            </div>
          </div>
        </div>
        <div style="display:flex;gap:10px;margin-top:10px;align-items:center;">
          <input id="turbo-dl-url" type="text" placeholder="Paste direct download URL (e.g. https://.../file.zip)" style="flex:1;padding:8px 12px;background:rgba(2,6,23,0.8);border:1px solid var(--border);border-radius:6px;color:#f8fafc;font-size:12px;" />
          <select id="turbo-dl-workers" style="padding:8px;background:rgba(2,6,23,0.8);border:1px solid var(--border);border-radius:6px;color:#f8fafc;font-size:12px;">
            <option value="4">4 Threads</option>
            <option value="8" selected>8 Threads (Recommended)</option>
            <option value="16">16 Threads (Max Speed)</option>
          </select>
          <button class="btn" style="background:linear-gradient(135deg, #10b981, #059669);border-color:#34d399;white-space:nowrap;" onclick="handleStartTurboDownload()">
            ⚡ Turbo Download
          </button>
        </div>
      </div>

      <div class="card" style="border:1px solid rgba(59,130,246,0.3);background:rgba(15,23,42,0.65);">
        <div class="card-header">
          <div>
            <div class="card-title" style="color:#60a5fa;">📡 Multi-Adapter &amp; Wireless Hardware Diagnostics</div>
            <div style="font-size:12px;color:var(--text-muted);margin-top:2px;">
              System has ${ifaces.length} active network interface(s) and ${physRadios} physical Wi-Fi radio(s) detected.
            </div>
          </div>
          <span class="badge ${physRadios > 1 ? 'badge-healthy' : 'badge-degraded'}">
            ${physRadios > 1 ? 'MULTI-RADIO DETECTED' : 'SINGLE RADIO DETECTED'}
          </span>
        </div>

        <div style="margin-top:8px;font-size:12px;line-height:1.6;color:#cbd5e1;background:rgba(2,6,23,0.7);padding:12px;border-radius:6px;border:1px solid var(--border);">
          ${
            physRadios <= 1
              ? `<strong>ℹ️ Hardware Reality Notice:</strong> Your laptop has 1 internal physical Wi-Fi radio (MediaTek MT7921, 0 secondary STAs). A single physical radio transceiver cannot be tuned to two different frequencies/channels at the same time. When instructed to connect to another Wi-Fi network, Windows swaps the active connection. 
              <br/><br/>
              <strong>How to aggregate multiple Wi-Fi networks concurrently:</strong> Simply plug in 1 or more external USB Wi-Fi dongles (or combine with Ethernet / USB phone tethering). WiFi GigaChad 420 will immediately detect all <em>N</em> adapters and pool their bandwidth together!`
              : `<strong>⚡ Multi-Radio Active:</strong> ${physRadios} physical Wi-Fi radios detected! Each radio can be independently associated with a different Wi-Fi network simultaneously for full multi-link bandwidth aggregation.`
          }
        </div>
      </div>

      <div class="card">
        <div class="card-header">
          <div class="card-title">Active Network Interfaces (${ifaces.length} Connected)</div>
          <button class="btn btn-secondary" onclick="Store.setState({activeScreen:'logs'})">
            Open Diagnostics &amp; Logs &rarr;
          </button>
        </div>
        <div class="wifi-grid">
          ${cardsHtml}
        </div>
      </div>
    </section>
  `;
}
