import React from 'react';
import { UpdateReleaseInfo } from '../types/telemetry';

export interface UpdateModalProps {
  updateInfo: UpdateReleaseInfo | null;
  visible: boolean;
  onDismiss: () => void;
  onInstallUpdate: () => void;
}

export function renderUpdateModal(props: UpdateModalProps): string {
  if (!props.visible || !props.updateInfo) {
    return '';
  }

  const u = props.updateInfo;
  const isMismatch = !!u.version_mismatch;
  const highlightsText = (u.highlights || []).map((h) => `• ${h}`).join('\n');

  return `
    <div class="update-toast" style="border-color:${isMismatch ? '#f59e0b' : 'var(--cyan)'};box-shadow:0 20px 50px rgba(0,0,0,0.85);">
      <div class="update-toast-top">
        <span class="update-badge" style="${isMismatch ? 'background:#f59e0b;color:#0f172a;font-weight:800;' : ''}">
          ${isMismatch ? 'VERSION MISMATCH DETECTED' : u.channel + ' RELEASE'} • ${u.latest_version}
        </span>
        <span style="font-size:11px;color:var(--text-muted);font-family:var(--mono);">
          Installed: ${u.previous_version || u.current_version}
        </span>
      </div>
      <div class="update-title" style="margin-top:6px;">
        ${isMismatch ? 'Version Mismatch: Updating App to ' + u.latest_version : u.release_title}
      </div>
      <div style="font-size:12px;color:#94a3b8;margin-bottom:8px;">
        Current: <strong style="color:#e2e8f0;">${u.previous_version || u.current_version}</strong> ➔ Target: <strong style="color:#34d399;">${u.latest_version}</strong>
      </div>
      <div class="update-notes">${highlightsText}</div>
      ${
        u.installing
          ? `<div class="bar-track" style="height:8px;margin-top:10px;"><div class="bar-fill" style="width:${u.progress_pct}%;"></div></div>
             <div style="font-size:11px;color:var(--cyan);font-family:var(--mono);margin-top:5px;">Applying update package (${u.progress_pct.toFixed(0)}%)...</div>`
          : `<div class="update-actions" style="margin-top:10px;">
               <button class="btn btn-secondary" onclick="Store.setState({showUpdatePopup:false})">Dismiss</button>
               <button class="btn" style="background:linear-gradient(135deg, #10b981, #059669);border-color:#34d399;" onclick="handleInstallUpdate()">
                 ${isMismatch ? 'Apply Update &amp; Match Version' : 'Update &amp; Restart'}
               </button>
             </div>`
      }
    </div>
  `;
}

