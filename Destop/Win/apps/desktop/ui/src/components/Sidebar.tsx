import React from 'react';
import { ActiveScreenId } from '../types/telemetry';

export interface SidebarProps {
  activeScreen: ActiveScreenId;
  collectiveSpeedMbps: number;
  activeAdapterCount: number;
  isElevatedAdmin: boolean;
  currentVersion: string;
}

export function renderSidebar(props: SidebarProps): string {
  const items: Array<{ id: ActiveScreenId; step: string; title: string; subtitle: string }> = [
    { id: 'dashboard', step: '01', title: 'Wi-Fi Dashboard', subtitle: 'Speeds, Modes & Benchmark' },
    { id: 'processes', step: '02', title: 'Ongoing Processes', subtitle: 'Active Streams & Downloads' },
    { id: 'system', step: '03', title: 'System & Mesh Info', subtitle: 'OS Config, DNS & Phone Mesh' },
    { id: 'logs', step: '04', title: 'Logs & Diagnostics', subtitle: 'Multi-WiFi Inspector & Events' },
  ];

  const navHtml = items
    .map(
      (item) => `
      <button class="nav-item ${props.activeScreen === item.id ? 'active' : ''}" onclick="Store.setState({activeScreen:'${item.id}'})">
        <span class="nav-step">${item.step}</span>
        <div>
          <div class="nav-title">${item.title}</div>
          <div class="nav-subtitle">${item.subtitle}</div>
        </div>
      </button>
    `
    )
    .join('');

  return `
    <aside class="sidebar">
      <div class="sidebar-brand">
        <div class="brand-logo">GIGACHAD 420</div>
        <div class="brand-text">
          <h1>Wi-Fi Aggregator</h1>
          <span>v${props.currentVersion} • Multi-NIC</span>
        </div>
      </div>

      <div class="sidebar-speed-widget">
        <div class="widget-label">COLLECTIVE SPEED</div>
        <div class="widget-speed">${props.collectiveSpeedMbps.toFixed(2)} Mbps</div>
        <div class="widget-sub">${props.activeAdapterCount} Active Wi-Fi / LAN Links</div>
      </div>

      <nav class="sidebar-nav">
        ${navHtml}
      </nav>

      <div class="sidebar-footer">
        <div class="status-pill">
          <span class="dot ${props.isElevatedAdmin ? 'dot-green' : 'dot-amber'}"></span>
          <span>${props.isElevatedAdmin ? 'OS ADMIN ACTIVE' : 'STANDARD ACCESS'}</span>
        </div>
        <button class="btn btn-full" onclick="handleBoostOs()">Allow Multi-WiFi (UAC)</button>
        <button class="btn btn-secondary btn-full" onclick="handleCheckUpdates()">Check for Updates</button>
      </div>
    </aside>
  `;
}
