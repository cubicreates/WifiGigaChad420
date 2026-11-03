import React from 'react';
import { renderSidebar } from './components/Sidebar';
import { renderUpdateModal } from './components/UpdateModal';
import {
  InstallerState,
  UninstallerState,
  renderInstallerWizard,
  renderUninstallerPopup,
} from './screens/InstallerUninstallerScreens';
import { renderOngoingProcessesScreen } from './screens/OngoingProcessesScreen';
import { renderSystemMeshInfoScreen } from './screens/SystemMeshInfoScreen';
import { renderWifiDashboardScreen } from './screens/WifiDashboardScreen';
import { ActiveScreenId, SystemStatusPayload } from './types/telemetry';

export interface AppState {
  appMode: 'normal' | 'installer' | 'uninstaller';
  activeScreen: ActiveScreenId;
  showUpdatePopup: boolean;
  data: SystemStatusPayload & { initial_mode?: 'normal' | 'installer' | 'uninstaller' };
  dlForm: {
    url: string;
    output: string;
    workers: number;
  };
  dnsHost: string;
  dnsOutput: string;
  osLogOutput: string | null;
}

export const InstallerStore: InstallerState & {
  setStep: (s: number) => void;
  toggle: (key: string) => void;
} = {
  step: 1,
  installDir: 'C:\\Users\\Public\\WiFiGigaChad420',
  createDesktopShortcut: true,
  createStartMenuShortcut: true,
  optimizeWindowsMultiWifi: true,
  progressPct: 0,
  logLines: ['Ready to install.'],
  setStep(s: number) {
    this.step = s;
    renderApp();
  },
  toggle(key: string) {
    const selfAny = this as unknown as Record<string, boolean>;
    selfAny[key] = !selfAny[key];
    renderApp();
  },
};

export const UninstallerStore: UninstallerState & {
  toggle: (key: string) => void;
} = {
  step: 1,
  resetSystemProxy: true,
  removeShortcuts: true,
  deleteSavedConfigs: false,
  reason: '',
  progressPct: 0,
  logLines: ['Ready to uninstall.'],
  toggle(key: string) {
    const selfAny = this as unknown as Record<string, boolean>;
    selfAny[key] = !selfAny[key];
    renderApp();
  },
};

export const Store: AppState & { setState: (patch: Partial<AppState>) => void } = {
  appMode: 'normal',
  activeScreen: 'dashboard',
  showUpdatePopup: false,
  data: {
    interfaces: [],
    proxy: {
      socks5_addr: '127.0.0.1:1080',
      http_addr: '127.0.0.1:1081',
      active_connections: 0,
      total_connections: 0,
      hedged_secondary_wins: 0,
      total_rx_bytes: 0,
      total_tx_bytes: 0,
    },
    download: null,
    routing_mode: 'max_speed',
    speed_test: null,
    update_info: {
      current_version: '0.1.0',
      latest_version: '0.2.0',
      update_available: true,
      channel: 'STABLE',
      release_title: 'WiFi GigaChad 420 v0.2.0 Available',
      highlights: [
        'Enhanced Multi-Wi-Fi Kalman Scheduler & 150ms Happy Eyeballs Failover',
        'Faster Cross-Device Phone <-> Laptop Wi-Fi Mesh Tunneling',
        'Improved Windows 11 802.11ax/be PHY Telemetry Polling',
      ],
      installing: false,
      progress_pct: 0,
    },
    os_status: { is_elevated_admin: false, system_proxy_enabled: false },
    mesh: {
      local_peer_id: '...',
      local_device_name: 'Windows-Laptop',
      pairing_pin: '------',
      peers: [],
    },
  },
  dlForm: {
    url: 'https://static.rust-lang.org/dist/channel-rust-stable.toml',
    output: 'target/gigachad_download.bin',
    workers: 4,
  },
  dnsHost: 'github.com',
  dnsOutput: 'Click "Test Fastest DNS" to race DNS across your connected Wi-Fi adapters.',
  osLogOutput: null,

  setState(patch: Partial<AppState>) {
    Object.assign(this, patch);
    renderApp();
  },
};

let initializedModeFromServer = false;

export async function pollBackend(): Promise<void> {
  try {
    const res = await fetch('/api/status');
    const json = await res.json();
    Store.data = json;
    if (!initializedModeFromServer && json.initial_mode) {
      initializedModeFromServer = true;
      Store.appMode = json.initial_mode;
    }
    renderApp();
  } catch (_) {}
}

export async function runInstallNow(): Promise<void> {
  const dirEl = document.getElementById('inst-dir-input') as HTMLInputElement | null;
  if (dirEl) InstallerStore.installDir = dirEl.value.trim();
  InstallerStore.step = 3;
  InstallerStore.progressPct = 35;
  InstallerStore.logLines = ['Creating installation directory & copying binaries...'];
  renderApp();

  const res = await fetch('/api/installer-run', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      install_dir: InstallerStore.installDir,
      desktop_shortcut: InstallerStore.createDesktopShortcut,
      start_menu_shortcut: InstallerStore.createStartMenuShortcut,
      optimize_os: InstallerStore.optimizeWindowsMultiWifi,
    }),
  });
  const out = await res.json();
  InstallerStore.progressPct = 100;
  InstallerStore.logLines = out.logs || ['Installation completed.'];
  renderApp();
}

export async function runUninstallNow(): Promise<void> {
  const reasonEl = document.getElementById('uninst-reason') as HTMLInputElement | null;
  if (reasonEl) UninstallerStore.reason = reasonEl.value.trim();
  UninstallerStore.step = 2;
  UninstallerStore.progressPct = 40;
  UninstallerStore.logLines = ['Restoring Windows network settings & removing shortcuts...'];
  renderApp();

  const res = await fetch('/api/uninstaller-run', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      reset_proxy: UninstallerStore.resetSystemProxy,
      remove_shortcuts: UninstallerStore.removeShortcuts,
      delete_configs: UninstallerStore.deleteSavedConfigs,
      reason: UninstallerStore.reason,
    }),
  });
  const out = await res.json();
  UninstallerStore.progressPct = 100;
  UninstallerStore.logLines = out.logs || ['Uninstallation completed.'];
  renderApp();
}

export async function handleSelectRoutingMode(mode: string): Promise<void> {
  await fetch('/api/routing-mode', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ mode }),
  });
  pollBackend();
}

export async function handleRunCombinedSpeedTest(): Promise<void> {
  await fetch('/api/speed-test', { method: 'POST' });
  pollBackend();
}

export async function handleBoostOs(): Promise<void> {
  Store.setState({
    osLogOutput: 'Requesting Windows UAC Administrator permission to enable Multi-WiFi...',
  });
  const res = await fetch('/api/os-optimize', { method: 'POST' });
  const rep = await res.json();
  const summary = (rep.steps || [])
    .map(
      (s: { applied: boolean; title: string; detail: string }) =>
        `[${s.applied ? 'OK' : 'NEEDS ADMIN'}] ${s.title} -> ${s.detail}`
    )
    .join('\n');
  Store.setState({ osLogOutput: summary });
  pollBackend();
}

export async function handleToggleSystemProxy(): Promise<void> {
  const res = await fetch('/api/system-proxy-toggle', { method: 'POST' });
  const out = await res.json();
  Store.setState({ osLogOutput: out.message || 'Toggled Windows System Proxy.' });
  pollBackend();
}

export async function handleStartDownload(): Promise<void> {
  const urlEl = document.getElementById('dl-url-input') as HTMLInputElement | null;
  const outEl = document.getElementById('dl-out-input') as HTMLInputElement | null;
  if (urlEl) Store.dlForm.url = urlEl.value.trim();
  if (outEl) Store.dlForm.output = outEl.value.trim();

  await fetch('/api/download', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      url: Store.dlForm.url,
      output: Store.dlForm.output,
      workers_per_if: Store.dlForm.workers,
    }),
  });
  pollBackend();
}

export async function handleDnsRace(): Promise<void> {
  const input = document.getElementById('dns-host-input') as HTMLInputElement | null;
  if (input) Store.dnsHost = input.value.trim();
  Store.setState({ dnsOutput: `Racing DNS for ${Store.dnsHost}...` });
  const res = await fetch('/api/dns-race?host=' + encodeURIComponent(Store.dnsHost));
  const d = await res.json();
  if (d.error) {
    Store.setState({ dnsOutput: `Error: ${d.error}` });
  } else {
    Store.setState({
      dnsOutput: `Domain  : ${d.hostname} -> ${d.resolved_ips.join(', ')}\nWinner  : ${d.winning_interface} (${d.winning_resolver})\nLatency : ${d.resolution_time_ms.toFixed(2)} ms (Cached: ${d.from_cache})`,
    });
  }
}

export async function handleMeshScan(): Promise<void> {
  await fetch('/api/mesh-scan', { method: 'POST' });
  pollBackend();
}

export async function handleCheckUpdates(): Promise<void> {
  const res = await fetch('/api/update-check', { method: 'POST' });
  const info = await res.json();
  Store.data.update_info = info;
  Store.setState({ showUpdatePopup: true });
}

export async function handleInstallUpdate(): Promise<void> {
  await fetch('/api/update-install', { method: 'POST' });
  pollBackend();
  setTimeout(() => {
    Store.setState({ showUpdatePopup: false });
    pollBackend();
  }, 1800);
}

export function renderApp(): void {
  const root = document.getElementById('root');
  if (!root) return;

  const activeId = document.activeElement ? document.activeElement.id : null;
  if (
    activeId === 'dl-url-input' ||
    activeId === 'dl-out-input' ||
    activeId === 'dns-host-input' ||
    activeId === 'inst-dir-input' ||
    activeId === 'uninst-reason'
  ) {
    return;
  }

  if (Store.appMode === 'installer') {
    root.innerHTML = renderInstallerWizard(InstallerStore);
    return;
  }

  if (Store.appMode === 'uninstaller') {
    root.innerHTML = renderUninstallerPopup(UninstallerStore);
    return;
  }

  const ifaces = Store.data.interfaces || [];
  let collectiveLiveMbps = 0;
  let totalTransferredBytes = 0;

  ifaces.forEach((i) => {
    collectiveLiveMbps +=
      ((i.rx_throughput_bps || 0) + (i.tx_throughput_bps || 0)) / 1_000_000;
    totalTransferredBytes += (i.total_rx_bytes || 0) + (i.total_tx_bytes || 0);
  });

  const dl = Store.data.download;
  if (dl && dl.state === 'Active' && dl.aggregate_speed_mbps > collectiveLiveMbps) {
    collectiveLiveMbps = dl.aggregate_speed_mbps;
  }
  const st = Store.data.speed_test;
  if (st && !st.running && st.combined_mbps > collectiveLiveMbps) {
    collectiveLiveMbps = st.combined_mbps;
  }

  let activeScreenHtml = '';
  if (Store.activeScreen === 'dashboard') {
    activeScreenHtml = renderWifiDashboardScreen({
      interfaces: ifaces,
      collectiveSpeedMbps: collectiveLiveMbps,
      totalTransferredBytes,
      routingMode: Store.data.routing_mode,
      speedTest: Store.data.speed_test,
    });
  } else if (Store.activeScreen === 'processes') {
    activeScreenHtml = renderOngoingProcessesScreen({
      proxy: Store.data.proxy,
      download: Store.data.download,
      systemProxyEnabled:
        Store.data.os_status && Store.data.os_status.system_proxy_enabled,
      dlUrl: Store.dlForm.url,
      dlOutput: Store.dlForm.output,
    });
  } else {
    activeScreenHtml = renderSystemMeshInfoScreen({
      isElevatedAdmin:
        Store.data.os_status && Store.data.os_status.is_elevated_admin,
      systemProxyEnabled:
        Store.data.os_status && Store.data.os_status.system_proxy_enabled,
      routingMode: Store.data.routing_mode,
      localPeerId: Store.data.mesh.local_peer_id,
      pairingPin: Store.data.mesh.pairing_pin,
      peers: Store.data.mesh.peers || [],
      osLogOutput: Store.osLogOutput,
      dnsHost: Store.dnsHost,
      dnsOutput: Store.dnsOutput,
    });
  }

  const updatePopupHtml = renderUpdateModal({
    updateInfo: Store.data.update_info,
    visible: Store.showUpdatePopup,
    onDismiss: () => Store.setState({ showUpdatePopup: false }),
    onInstallUpdate: handleInstallUpdate,
  });

  root.innerHTML = `
    <div class="app-layout">
      ${renderSidebar({
        activeScreen: Store.activeScreen,
        collectiveSpeedMbps: collectiveLiveMbps,
        activeAdapterCount: ifaces.length,
        isElevatedAdmin:
          Store.data.os_status && Store.data.os_status.is_elevated_admin,
        currentVersion: Store.data.update_info?.current_version || '0.1.0',
      })}
      <main class="main-viewport">
        ${activeScreenHtml}
      </main>
      ${updatePopupHtml}
    </div>
  `;
}

renderApp();
pollBackend();
setInterval(pollBackend, 1000);
setTimeout(() => {
  if (
    Store.appMode === 'normal' &&
    Store.data.update_info &&
    Store.data.update_info.update_available
  ) {
    Store.setState({ showUpdatePopup: true });
  }
}, 2500);
