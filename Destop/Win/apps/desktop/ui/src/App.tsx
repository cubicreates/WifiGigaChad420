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
import { renderLogsScreen } from './screens/LogsScreen';
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
  wifiConnectStatus: string | null;
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
      current_version: 'TST-0.1.3.11.2026',
      latest_version: 'TST-0.2.3.11.2026',
      update_available: true,
      version_mismatch: false,
      channel: 'STABLE',
      release_title: 'WiFi GigaChad 420 TST-0.2.3.11.2026 Update Ready',
      highlights: [
        'Version Format: Standardized TST-0.X.3.11.2026 Release Architecture',
        'Automatic Version Mismatch Verification & Zero-Downtime Migration',
        'Enhanced Laptop-Only Tri-Link Multi-SSID Direct Selector (Anti-Swap Active)',
        '150ms Happy Eyeballs Multi-Path Proxy & Parallel DNS Race Resolver',
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
  wifiConnectStatus: null,

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

export async function handleRunWifiDiagnostics(): Promise<void> {
  await fetch('/api/wifi-diagnostics', { method: 'POST' });
  pollBackend();
}

export async function handleConnectAdapterSsid(interfaceName: string, ssidSelectId: string): Promise<void> {
  const selectEl = document.getElementById(ssidSelectId) as HTMLSelectElement | null;
  const profile_ssid = selectEl ? selectEl.value : '';
  if (!profile_ssid) return;

  Store.setState({
    wifiConnectStatus: `Connecting adapter '${interfaceName}' to Wi-Fi profile '${profile_ssid}'...`,
  });

  const res = await fetch('/api/wifi-connect', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ interface_name: interfaceName, profile_ssid, force_swap: true }),
  });
  const out = await res.json();
  Store.setState({ wifiConnectStatus: out.message || 'Done.' });
  pollBackend();
}

export async function handleConnectWifiOnAdapter(forceSwap: boolean): Promise<void> {
  const ifEl = document.getElementById('wifi-conn-iface') as HTMLSelectElement | null;
  const ssidEl = document.getElementById('wifi-conn-ssid') as HTMLSelectElement | null;
  const interface_name = ifEl ? ifEl.value : 'Wi-Fi';
  const profile_ssid = ssidEl ? ssidEl.value : '';

  Store.setState({
    wifiConnectStatus: `Binding Wi-Fi profile '${profile_ssid}' to adapter '${interface_name}'...`,
  });

  const res = await fetch('/api/wifi-connect', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ interface_name, profile_ssid, force_swap: forceSwap }),
  });
  const out = await res.json();
  Store.setState({ wifiConnectStatus: out.message || 'Done.' });
  pollBackend();
}

export async function handleConnect3Wifi(): Promise<void> {
  await handleConnectSelectedMultiWifi();
}

export async function handleConnectSelectedMultiWifi(): Promise<void> {
  const el1 = document.getElementById('multi-wifi-1') as HTMLSelectElement | null;
  const el2 = document.getElementById('multi-wifi-2') as HTMLSelectElement | null;
  const el3 = document.getElementById('multi-wifi-3') as HTMLSelectElement | null;
  const wifi1 = el1 ? el1.value : 'Gheras_5G';
  const wifi2 = el2 ? el2.value : 'Gheras_4G';
  const wifi3 = el3 ? el3.value : 'Saurabh_4G';

  Store.setState({
    wifiConnectStatus: `Connecting & pooling [${wifi1}] + [${wifi2}] + [${wifi3}] directly on laptop (Zero Phone)...`,
  });

  const res = await fetch('/api/connect-multi-wifi', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ wifi1, wifi2, wifi3 }),
  });
  const out = await res.json();
  Store.setState({ wifiConnectStatus: out.message || 'All 3 Wi-Fi connections active on laptop!' });
  pollBackend();
}

export async function handleEngageCrackMode(): Promise<void> {
  Store.setState({
    wifiConnectStatus: 'Engaging Windows Kernel Crack Mode (Uncapping throttle, 0ms Nagle, BBR/ECN)...',
  });
  const res = await fetch('/api/optimize-crack-mode', { method: 'POST' });
  const out = await res.json();
  const applied = out.steps ? out.steps.filter((s: any) => s.applied).length : 0;
  Store.setState({
    wifiConnectStatus: `Crack Mode Engaged! ${applied} Windows TCP & kernel optimizations active.`,
  });
  pollBackend();
}

export async function handleStartTurboDownload(): Promise<void> {
  const urlEl = document.getElementById('turbo-dl-url') as HTMLInputElement | null;
  const workersEl = document.getElementById('turbo-dl-workers') as HTMLSelectElement | null;
  const url = urlEl ? urlEl.value.trim() : '';
  const workers = workersEl ? parseInt(workersEl.value, 10) : 8;
  if (!url) {
    alert('Please enter a download URL.');
    return;
  }
  const filename = url.split('/').pop()?.split('?')[0] || 'gigachad_download.bin';
  const output = `C:\\Users\\Public\\Downloads\\${filename}`;

  Store.setState({
    wifiConnectStatus: `Starting Turbo Download across ${workers} concurrent threads...`,
  });

  const res = await fetch('/api/download', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ url, output, workers_per_if: workers }),
  });
  const out = await res.json();
  Store.setState({
    wifiConnectStatus: out.message || 'Download task dispatched to Multi-Stream Scheduler.',
  });
  pollBackend();
}

export async function handleClearLogs(): Promise<void> {
  await fetch('/api/logs-clear', { method: 'POST' });
  pollBackend();
}

export async function handleExpProbeCapabilities(): Promise<void> {
  await fetch('/api/exp-probe-capabilities', { method: 'POST' });
  pollBackend();
}

export async function handleExpRunTimeShare(): Promise<void> {
  await fetch('/api/exp-time-share-run', { method: 'POST' });
  pollBackend();
}

export async function handleExpToggleVirtualIf(): Promise<void> {
  await fetch('/api/exp-virtual-if-toggle', { method: 'POST' });
  pollBackend();
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
    activeId === 'uninst-reason' ||
    activeId === 'wifi-conn-iface' ||
    activeId === 'wifi-conn-ssid' ||
    activeId === 'multi-wifi-1' ||
    activeId === 'multi-wifi-2' ||
    activeId === 'multi-wifi-3'
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
    const rawSpeed = ((i.rx_throughput_bps || 0) + (i.tx_throughput_bps || 0)) / 1_000_000;
    collectiveLiveMbps += rawSpeed;
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
      wlanDiag: Store.data.wlan_diag,
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
  } else if (Store.activeScreen === 'logs') {
    activeScreenHtml = renderLogsScreen(Store.data, Store.wifiConnectStatus);
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
        currentVersion: Store.data.update_info?.current_version || 'TST-0.1.3.11.2026',
      })}
      <main class="main-viewport">
        ${activeScreenHtml}
      </main>
      ${updatePopupHtml}
    </div>
  `;
}

window.addEventListener('beforeunload', () => {
  try {
    navigator.sendBeacon('/api/app-close');
  } catch (_) {}
});

renderApp();
pollBackend();
setInterval(pollBackend, 1000);
setTimeout(() => {
  if (
    Store.appMode === 'normal' &&
    Store.data.update_info &&
    (Store.data.update_info.update_available || Store.data.update_info.version_mismatch)
  ) {
    Store.setState({ showUpdatePopup: true });
  }
}, 1800);
