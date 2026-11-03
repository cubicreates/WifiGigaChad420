import React from 'react';
import { DiscoveredMeshPeer, RoutingMode } from '../types/telemetry';

export interface SystemMeshInfoScreenProps {
  isElevatedAdmin: boolean;
  systemProxyEnabled: boolean;
  routingMode: RoutingMode;
  localPeerId: string;
  pairingPin: string;
  peers: DiscoveredMeshPeer[];
  osLogOutput: string | null;
  dnsHost: string;
  dnsOutput: string;
}

export function renderSystemMeshInfoScreen(props: SystemMeshInfoScreenProps): string {
  const peerText =
    props.peers.length === 0
      ? 'No phones or other laptops detected on this Wi-Fi yet.'
      : props.peers
          .map(
            (p) =>
              `* ${p.device_name} (${p.platform}) at ${p.lan_ip}:${p.tunnel_port} — ${p.uplink_medium}`
          )
          .join('\n');

  const defaultOsInfo =
    `Administrator Access     : ${props.isElevatedAdmin ? 'YES (Full OS Multi-WiFi Control Active)' : 'NO (Click button in Sidebar to grant UAC access)'}\n` +
    `Windows System Proxy     : ${props.systemProxyEnabled ? 'ENABLED (127.0.0.1:1081)' : 'DISABLED (Direct OS Routing)'}\n` +
    `Active Traffic Mode      : ${(props.routingMode || 'max_speed').toUpperCase()}\n` +
    `Local SOCKS5 Proxy Port  : 127.0.0.1:1080\n` +
    `Local HTTP Proxy Port    : 127.0.0.1:1081\n` +
    `Kernel Socket Binding    : Windows IP_UNICAST_IF Strong-Host Mode Active`;

  return `
    <section class="screen-container">
      <div class="two-col">
        <div class="card">
          <div class="card-header">
            <div class="card-title">Windows OS Multi-WiFi Configuration &amp; Permissions</div>
            <button class="btn" onclick="handleBoostOs()">Grant UAC &amp; Modify OS</button>
          </div>
          <div class="console-box">${props.osLogOutput || defaultOsInfo}</div>
        </div>

        <div class="card">
          <div class="card-header">
            <div class="card-title">Phone &lt;-&gt; Laptop Common Wi-Fi Mesh</div>
            <button class="btn btn-secondary" onclick="handleMeshScan()">Scan Common Wi-Fi</button>
          </div>
          <div class="console-box">Laptop Mesh ID  : ${props.localPeerId}\n6-Digit Pair PIN: ${props.pairingPin}\nMesh Ports      : UDP :42069 / TCP :42070\n\n${peerText}</div>
        </div>
      </div>

      <div class="card">
        <div class="card-header">
          <div class="card-title">Multi-WiFi Parallel DNS Speed Tester</div>
        </div>
        <div class="input-row">
          <input id="dns-host-input" type="text" value="${props.dnsHost}" placeholder="Enter domain (e.g. github.com, google.com)" />
          <button class="btn" onclick="handleDnsRace()">Test Fastest DNS</button>
        </div>
        <div class="console-box">${props.dnsOutput}</div>
      </div>
    </section>
  `;
}
