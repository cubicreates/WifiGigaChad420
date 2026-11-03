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
  const highlightsText = (u.highlights || []).map((h) => `• ${h}`).join('\n');

  return `
    <div class="update-toast">
      <div class="update-toast-top">
        <span class="update-badge">${u.channel} RELEASE • v${u.latest_version}</span>
        <span style="font-size:11px;color:var(--text-muted);font-family:var(--mono);">Current: v${u.current_version}</span>
      </div>
      <div class="update-title">${u.release_title}</div>
      <div class="update-notes">${highlightsText}</div>
      ${
        u.installing
          ? `<div class="bar-track" style="height:8px;"><div class="bar-fill" style="width:${u.progress_pct}%;"></div></div>
             <div style="font-size:11px;color:var(--cyan);font-family:var(--mono);">Applying update package (${u.progress_pct.toFixed(0)}%)...</div>`
          : `<div class="update-actions">
               <button class="btn btn-secondary" onclick="Store.setState({showUpdatePopup:false})">Remind Me Later</button>
               <button class="btn" onclick="handleInstallUpdate()">Update &amp; Restart</button>
             </div>`
      }
    </div>
  `;
}

