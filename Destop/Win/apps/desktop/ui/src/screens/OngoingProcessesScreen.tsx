import React from 'react';
import {
  DownloadProgressSnapshot,
  ProxyTelemetrySnapshot,
} from '../types/telemetry';

export interface OngoingProcessesScreenProps {
  proxy: ProxyTelemetrySnapshot;
  download: DownloadProgressSnapshot | null;
  systemProxyEnabled: boolean;
  dlUrl: string;
  dlOutput: string;
}

export function renderOngoingProcessesScreen(props: OngoingProcessesScreenProps): string {
  const p = props.proxy;
  const dl = props.download;
  const sysProxyOn = props.systemProxyEnabled;

  let dlStatusHtml =
    'No active multi-Wi-Fi download running right now. Start one above to stripe chunks across all adapters.';
  let progressPct = 0;

  if (dl) {
    progressPct = dl.progress_pct || 0;
    const perNic = Object.entries(dl.per_interface_bytes || {})
      .map(([idx, bytes]) => `Adapter #${idx}: ${(bytes / 1024).toFixed(1)} KB`)
      .join('  |  ');
    dlStatusHtml =
      `Process Status : ${dl.state} (${progressPct.toFixed(1)}%)\n` +
      `Live Speed     : ${dl.aggregate_speed_mbps.toFixed(2)} Mbps  |  Transferred: ${(dl.downloaded_bytes / 1024).toFixed(1)} / ${(dl.total_bytes / 1024).toFixed(1)} KB\n` +
      `Parallel Chunks: ${dl.completed_chunks} / ${dl.total_chunks} completed (${dl.work_stolen_chunks} work-stolen stragglers)\n` +
      `Per-Wi-Fi Load : ${perNic || 'Initializing...'}\n` +
      `Verified Hash  : ${dl.sha256_hex || 'Calculating SHA-256 on completion...'}`;
  }

  return `
    <section class="screen-container">
      <div class="card">
        <div class="card-header">
          <div class="card-title">Ongoing System-Wide Internet Traffic (Multi-WiFi Proxy Engine)</div>
          <button class="btn ${sysProxyOn ? 'btn-secondary' : ''}" onclick="handleToggleSystemProxy()">
            ${sysProxyOn ? 'Disable Windows System-Wide Routing (ON)' : 'Route All Windows Apps Through GigaChad'}
          </button>
        </div>
        <div class="wifi-grid">
          <div class="speed-box">
            <div class="speed-box-label">Ongoing Active Internet Streams</div>
            <div class="speed-box-val">${p.active_connections || 0} Active (${p.total_connections || 0} Total)</div>
          </div>
          <div class="speed-box">
            <div class="speed-box-label">Proxy Data Transferred</div>
            <div class="speed-box-val">${(((p.total_rx_bytes || 0) + (p.total_tx_bytes || 0)) / (1024 * 1024)).toFixed(2)} MB</div>
          </div>
          <div class="speed-box">
            <div class="speed-box-label">Automatic Failover Rescues</div>
            <div class="speed-box-val">${p.hedged_secondary_wins || 0} Connections Saved</div>
          </div>
        </div>
      </div>

      <div class="card">
        <div class="card-header">
          <div class="card-title">Multi-WiFi Accelerated File Downloader (Ongoing Transfer Process)</div>
        </div>
        <div class="input-row">
          <input id="dl-url-input" type="text" value="${props.dlUrl}" placeholder="Paste any HTTP or HTTPS download URL" />
        </div>
        <div class="input-row">
          <input id="dl-out-input" type="text" value="${props.dlOutput}" placeholder="Save file path (e.g. target/file.zip)" />
          <button class="btn" onclick="handleStartDownload()">Start Multi-WiFi Download</button>
        </div>
        <div class="bar-track" style="height:10px;margin-bottom:12px;">
          <div class="bar-fill" style="width:${progressPct}%;"></div>
        </div>
        <div class="console-box">${dlStatusHtml}</div>
      </div>
    </section>
  `;
}
