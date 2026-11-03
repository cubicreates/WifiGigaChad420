export type ActiveScreenId = 'dashboard' | 'processes' | 'system' | 'logs';

export type RoutingMode = 'max_speed' | 'low_ping' | 'data_saver';

export interface SystemLogEntry {
  timestamp: string;
  level: 'INFO' | 'OK' | 'WARN' | 'ERROR';
  category: string;
  message: string;
}

export interface WlanAdapterDiag {
  name: string;
  description: string;
  state: string;
  ssid: string;
  band: string;
}

export interface WlanHardwareDiagnostics {
  physical_wifi_count: number;
  max_secondary_stas: number;
  can_simultaneous_wifi_natively: boolean;
  wlan_adapters: WlanAdapterDiag[];
  saved_profiles: string[];
  diagnosis_code: string;
  diagnosis_title: string;
  diagnosis_explanation: string;
}

export type InterfaceHealthState =
  | 'Standby'
  | 'Healthy'
  | 'Degraded'
  | 'FailingOver'
  | 'Dead'
  | 'Recovering';

export interface HealthScoreSnapshot {
  if_index: number;
  interface_name: string;
  description: string;
  medium: string;
  local_ipv4: string | null;
  gateway_v4: string | null;
  public_wan_ip: string | null;
  score: number;
  state: InterfaceHealthState;
  ewma_rtt_ms: number;
  min_rtt_ms: number;
  jitter_ms: number;
  loss_rate: number;
  rssi_dbm: number | null;
  rssi_slope_dbm_per_sec: number;
  signal_quality_pct: number | null;
  ssid: string | null;
  rx_throughput_bps: number;
  tx_throughput_bps: number;
  total_rx_bytes: number;
  total_tx_bytes: number;
  is_metered: boolean;
}

export interface ProxyTelemetrySnapshot {
  socks5_addr: string;
  http_addr: string;
  active_connections: number;
  total_connections: number;
  hedged_secondary_wins: number;
  total_rx_bytes: number;
  total_tx_bytes: number;
}

export interface DownloadProgressSnapshot {
  url: string;
  output_path: string;
  state: 'Queued' | 'Active' | 'Paused' | 'VerifyingIntegrity' | 'Completed' | 'Failed';
  total_bytes: number;
  downloaded_bytes: number;
  progress_pct: number;
  aggregate_speed_mbps: number;
  elapsed_sec: number;
  active_chunks: number;
  completed_chunks: number;
  total_chunks: number;
  work_stolen_chunks: number;
  per_interface_bytes: Record<string, number>;
  sha256_hex: string | null;
}

export interface SpeedTestAdapterResult {
  if_index: number;
  interface_name: string;
  ssid: string | null;
  speed_mbps: number;
  rtt_ms: number;
}

export interface CombinedSpeedTestReport {
  running: boolean;
  per_adapter: SpeedTestAdapterResult[];
  single_best_mbps: number;
  combined_mbps: number;
  speedup_factor: number;
  timestamp: string;
}

export interface DiscoveredMeshPeer {
  peer_id: string;
  device_name: string;
  platform: string;
  lan_ip: string;
  tunnel_port: number;
  uplink_medium: string;
  advertised_bandwidth_mbps: number;
  battery_pct: number;
  paired: boolean;
  last_seen_ms_ago: number;
}

export interface UpdateReleaseInfo {
  current_version: string;
  latest_version: string;
  update_available: boolean;
  channel: string;
  release_title: string;
  highlights: string[];
  installing: boolean;
  progress_pct: number;
}

export interface SystemStatusPayload {
  interfaces: HealthScoreSnapshot[];
  proxy: ProxyTelemetrySnapshot;
  download: DownloadProgressSnapshot | null;
  routing_mode: RoutingMode;
  speed_test: CombinedSpeedTestReport | null;
  update_info: UpdateReleaseInfo;
  logs?: SystemLogEntry[];
  wlan_diag?: WlanHardwareDiagnostics;
  os_status: {
    is_elevated_admin: boolean;
    system_proxy_enabled: boolean;
  };
  mesh: {
    local_peer_id: string;
    local_device_name: string;
    pairing_pin: string;
    peers: DiscoveredMeshPeer[];
  };
}
