import React from 'react';
import {
  CombinedSpeedTestReport,
  HealthScoreSnapshot,
  RoutingMode,
} from '../types/telemetry';

export interface WifiDashboardScreenProps {
  interfaces: HealthScoreSnapshot[];
  collectiveSpeedMbps: number;
  totalTransferredBytes: number;
  routingMode: RoutingMode;
  speedTest: CombinedSpeedTestReport | null;
}

export function renderWifiDashboardScreen(props: WifiDashboardScreenProps): string {
  const ifaces = props.interfaces || [];
  const currentMode = props.routingMode || 'max_speed';
  const st = props.speedTest;

  let speedTestHtml = `Click "Run Combined Speed Test" to benchmark Single Wi-Fi vs. Combined Multi-Wi-Fi Speed side-by-side.`;
  if (st) {
    if (st.running) {
      speedTestHtml = `Running live parallel multi-Wi-Fi benchmark across ${ifaces.length} connected adapter(s)...`;
    } else {
      const perLines = (st.per_adapter || [])
        .map(
          (a) =>
            `  * ${a.ssid ? 'Wi-Fi (' + a.ssid + ')' : a.interface_name} [if_${a.if_index}] : ${a.speed_mbps.toFixed(2)} Mbps (Ping: ${a.rtt_ms.toFixed(0)} ms)`
        )
        .join('\n');
      speedTestHtml =
        `Single Best Wi-Fi : ${st.single_best_mbps.toFixed(2)} Mbps\n` +
        `Combined Speed    : ${st.combined_mbps.toFixed(2)} Mbps (${st.speedup_factor.toFixed(2)}x Combined Multiplier)\n` +
        `Individual Breakdown:\n${perLines || '  No active adapters.'}`;
    }
  }

  const cardsHtml =
    ifaces.length === 0
      ? `<div class="card"><p style="color:var(--text-muted);">No active Wi-Fi or Ethernet connections detected.</p></div>`
      : ifaces
          .map((i) => {
            const indivMbps = (
              ((i.rx_throughput_bps || 0) + (i.tx_throughput_bps || 0)) /
              1_000_000
            ).toFixed(2);
            const badgeCls =
              i.state === 'Healthy'
                ? 'badge-healthy'
                : i.state === 'Degraded'
                ? 'badge-degraded'
                : 'badge-dead';
            const title = i.ssid ? `Wi-Fi: ${i.ssid}` : i.interface_name;
            const signalStr = i.ssid
              ? `${i.signal_quality_pct || 0}% (${i.rssi_dbm || 0} dBm)`
              : 'Wired / Direct Link';

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
                    <div class="speed-box-label">Individual Live Speed</div>
                    <div class="speed-box-val">${indivMbps} Mbps</div>
                  </div>
                  <div class="speed-box">
                    <div class="speed-box-label">Latency (Ping)</div>
                    <div class="speed-box-val">${i.ewma_rtt_ms.toFixed(0)} ms</div>
                  </div>
                </div>

                <div>
                  <div style="display:flex;justify-content:space-between;font-size:11px;color:var(--text-muted);margin-bottom:5px;">
                    <span>Connection Health Score</span>
                    <span>${i.score.toFixed(1)} / 100</span>
                  </div>
                  <div class="bar-track">
                    <div class="bar-fill" style="width:${Math.min(100, Math.max(5, i.score))}%;"></div>
                  </div>
                </div>

                <div class="meta-table">
                  <div class="meta-row"><span class="meta-k">Signal</span><span class="meta-v">${signalStr}</span></div>
                  <div class="meta-row"><span class="meta-k">Jitter / Loss</span><span class="meta-v">${i.jitter_ms.toFixed(1)} ms / ${(i.loss_rate * 100).toFixed(0)}%</span></div>
                  <div class="meta-row"><span class="meta-k">Local IP</span><span class="meta-v">${i.local_ipv4 || 'N/A'}</span></div>
                  <div class="meta-row"><span class="meta-k">Public WAN IP</span><span class="meta-v">${i.public_wan_ip || 'Detecting...'}</span></div>
                </div>
              </div>
            `;
          })
          .join('');

  return `
    <section class="screen-container">
      <div class="hero-card">
        <div class="hero-metric-group">
          <div>
            <div class="hero-label">Collective Combined Wi-Fi Speed</div>
            <div class="hero-speed">${props.collectiveSpeedMbps.toFixed(2)} Mbps</div>
            <div class="hero-sub">${ifaces.length} Active Connection(s) Pooled Together</div>
          </div>
          <div>
            <div class="hero-label">Total Session Data</div>
            <div class="hero-speed-sm">${(props.totalTransferredBytes / (1024 * 1024)).toFixed(2)} MB</div>
            <div class="hero-sub">Combined Rx/Tx Volume</div>
          </div>
        </div>
        <div class="hero-actions">
          <button class="btn" onclick="handleRunCombinedSpeedTest()">
            ${st && st.running ? 'Testing Combined Speed...' : 'Run Combined Speed Test'}
          </button>
          <button class="btn btn-secondary" onclick="fetch('/api/probe',{method:'POST'}).then(pollBackend)">Refresh Wi-Fi Status</button>
        </div>
      </div>

      <div class="card">
        <div class="card-header">
          <div class="card-title">Traffic Routing Mode (How Your Connected Wi-Fi Networks Are Used)</div>
        </div>
        <div class="mode-grid">
          <div class="mode-card ${currentMode === 'max_speed' ? 'active' : ''}" onclick="handleSelectRoutingMode('max_speed')">
            <div class="mode-title">
              <span>Max Speed Mode</span>
              <span class="badge ${currentMode === 'max_speed' ? 'badge-healthy' : ''}">${currentMode === 'max_speed' ? 'ACTIVE' : 'SELECT'}</span>
            </div>
            <div class="mode-desc">Stripes downloads and streams across ALL connected Wi-Fi &amp; Ethernet links simultaneously for maximum combined speed.</div>
          </div>

          <div class="mode-card ${currentMode === 'low_ping' ? 'active' : ''}" onclick="handleSelectRoutingMode('low_ping')">
            <div class="mode-title">
              <span>Low-Ping / Gaming Mode</span>
              <span class="badge ${currentMode === 'low_ping' ? 'badge-healthy' : ''}">${currentMode === 'low_ping' ? 'ACTIVE' : 'SELECT'}</span>
            </div>
            <div class="mode-desc">Pins real-time gaming and calls to your lowest-ping Wi-Fi and uses secondary Wi-Fi for background downloads &amp; instant failover.</div>
          </div>

          <div class="mode-card ${currentMode === 'data_saver' ? 'active' : ''}" onclick="handleSelectRoutingMode('data_saver')">
            <div class="mode-title">
              <span>Data-Saver Mode</span>
              <span class="badge ${currentMode === 'data_saver' ? 'badge-healthy' : ''}">${currentMode === 'data_saver' ? 'ACTIVE' : 'SELECT'}</span>
            </div>
            <div class="mode-desc">Protects metered phone hotspots by keeping traffic on your primary Wi-Fi unless the main Wi-Fi slows down or drops.</div>
          </div>
        </div>
      </div>

      <div class="card">
        <div class="card-header">
          <div class="card-title">Single Wi-Fi vs. Combined Multi-Wi-Fi Benchmark</div>
          <button class="btn" onclick="handleRunCombinedSpeedTest()">
            ${st && st.running ? 'Running Benchmark...' : 'Run Combined Speed Test'}
          </button>
        </div>
        <div class="console-box">${speedTestHtml}</div>
      </div>

      <div class="card">
        <div class="card-header">
          <div class="card-title">Connected Wi-Fi &amp; Network Adapters (Individual Speeds &amp; Status)</div>
        </div>
        <div class="wifi-grid">
          ${cardsHtml}
        </div>
      </div>
    </section>
  `;
}
