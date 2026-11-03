import React from 'react';

export interface InstallerState {
  step: number; // 1 = Welcome, 2 = Options, 3 = Installing, 4 = Completed
  installDir: string;
  createDesktopShortcut: boolean;
  createStartMenuShortcut: boolean;
  optimizeWindowsMultiWifi: boolean;
  progressPct: number;
  logLines: string[];
}

export interface UninstallerState {
  step: number; // 1 = Confirm & Options, 2 = Uninstalling, 3 = Completed
  resetSystemProxy: boolean;
  removeShortcuts: boolean;
  deleteSavedConfigs: boolean;
  reason: string;
  progressPct: number;
  logLines: string[];
}

export function renderInstallerWizard(state: InstallerState): string {
  return `
    <div style="min-height:100vh;display:flex;align-items:center;justify-content:center;padding:24px;background:radial-gradient(circle at top,#13203d 0%,#090d16 70%);">
      <div class="card" style="max-width:640px;width:100%;border-color:rgba(59,130,246,0.55);box-shadow:0 25px 60px rgba(0,0,0,0.8);">
        <div class="card-header">
          <div style="display:flex;align-items:center;gap:12px;">
            <span class="brand-logo">GIGACHAD 420</span>
            <div>
              <div class="card-title">WiFi GigaChad 420 — Professional Setup Wizard</div>
              <div style="font-size:11px;color:var(--text-muted);font-family:var(--mono);">Windows Multi-Interface Aggregator &amp; Intelligent Network Resolver v0.1.0</div>
            </div>
          </div>
          <span class="badge badge-healthy">STEP ${state.step} OF 3</span>
        </div>

        ${
          state.step === 1
            ? `
          <div style="display:flex;flex-direction:column;gap:14px;">
            <p style="font-size:13px;color:#e2e8f0;line-height:1.6;">
              Welcome to the <strong>WiFi GigaChad 420</strong> Setup Wizard. This installer will configure the application on your Windows PC, register it in Windows Installed Apps, and optimize your OS network stack for simultaneous Multi-Wi-Fi and Ethernet connections.
            </p>
            <div class="console-box" style="max-height:160px;overflow-y:auto;">
GNU GENERAL PUBLIC LICENSE (GPLv3) — WiFi GigaChad 420
• Kernel-Level Socket Binding (IP_UNICAST_IF Strong-Host Mode)
• Windows Multi-Connector Policy (Disables fMinimizeConnections Auto-Disconnect)
• Zero-Latency Parallel DNS Race Resolver &amp; 150ms Happy Eyeballs Failover
• Cross-Device Phone &lt;-&gt; Laptop Common Wi-Fi Mesh Bridge
            </div>
            <div style="display:flex;justify-content:space-between;gap:10px;margin-top:8px;">
              <button class="btn btn-secondary" onclick="Store.setState({appMode:'normal'});renderApp();">Run Portable (Skip Setup)</button>
              <button class="btn" onclick="InstallerStore.setStep(2)">Accept &amp; Continue &rarr;</button>
            </div>
          </div>
        `
            : state.step === 2
            ? `
          <div style="display:flex;flex-direction:column;gap:14px;">
            <div>
              <div style="font-size:12px;color:var(--text-muted);margin-bottom:6px;">Destination Installation Folder</div>
              <input id="inst-dir-input" type="text" value="${state.installDir}" />
            </div>

            <div class="console-box" style="display:flex;flex-direction:column;gap:10px;">
              <label style="display:flex;align-items:center;gap:10px;cursor:pointer;">
                <input type="checkbox" ${state.createDesktopShortcut ? 'checked' : ''} onchange="InstallerStore.toggle('createDesktopShortcut')" style="width:16px;height:16px;" />
                <span>Create Desktop Shortcut (WiFi GigaChad 420)</span>
              </label>
              <label style="display:flex;align-items:center;gap:10px;cursor:pointer;">
                <input type="checkbox" ${state.createStartMenuShortcut ? 'checked' : ''} onchange="InstallerStore.toggle('createStartMenuShortcut')" style="width:16px;height:16px;" />
                <span>Create Windows Start Menu Entry &amp; Register Uninstaller</span>
              </label>
              <label style="display:flex;align-items:center;gap:10px;cursor:pointer;">
                <input type="checkbox" ${state.optimizeWindowsMultiWifi ? 'checked' : ''} onchange="InstallerStore.toggle('optimizeWindowsMultiWifi')" style="width:16px;height:16px;" />
                <span>Configure Windows OS for Simultaneous Multi-WiFi Connectors (Recommended)</span>
              </label>
            </div>

            <div style="display:flex;justify-content:space-between;margin-top:8px;">
              <button class="btn btn-secondary" onclick="InstallerStore.setStep(1)">&larr; Back</button>
              <button class="btn" onclick="runInstallNow()">Install Now</button>
            </div>
          </div>
        `
            : `
          <div style="display:flex;flex-direction:column;gap:14px;">
            <div style="font-size:14px;font-weight:700;color:var(--green);">
              ${state.progressPct >= 100 ? 'Installation Completed Successfully!' : 'Installing WiFi GigaChad 420...'}
            </div>
            <div class="bar-track" style="height:10px;">
              <div class="bar-fill" style="width:${state.progressPct}%;"></div>
            </div>
            <div class="console-box">${state.logLines.join('\n')}</div>
            ${
              state.progressPct >= 100
                ? `<div style="display:flex;justify-content:flex-end;gap:10px;">
                     <button class="btn" onclick="Store.setState({appMode:'normal'});renderApp();">Launch WiFi GigaChad 420 Now</button>
                   </div>`
                : ''
            }
          </div>
        `
        }
      </div>
    </div>
  `;
}

export function renderUninstallerPopup(state: UninstallerState): string {
  return `
    <div style="min-height:100vh;display:flex;align-items:center;justify-content:center;padding:24px;background:radial-gradient(circle at top,#2b1219 0%,#090d16 70%);">
      <div class="card" style="max-width:600px;width:100%;border-color:rgba(239,68,68,0.55);box-shadow:0 25px 60px rgba(0,0,0,0.85);">
        <div class="card-header">
          <div style="display:flex;align-items:center;gap:12px;">
            <span class="brand-logo" style="background:linear-gradient(135deg,#ef4444,#f59e0b);">UNINSTALL</span>
            <div>
              <div class="card-title">Uninstall WiFi GigaChad 420</div>
              <div style="font-size:11px;color:var(--text-muted);font-family:var(--mono);">Windows Application &amp; Network Stack Cleanup Wizard</div>
            </div>
          </div>
          <span class="badge badge-dead">UNINSTALLER</span>
        </div>

        ${
          state.step === 1
            ? `
          <div style="display:flex;flex-direction:column;gap:14px;">
            <p style="font-size:13px;color:#e2e8f0;line-height:1.55;">
              Are you sure you want to remove <strong>WiFi GigaChad 420</strong> from your computer? Select which OS settings and components should be cleaned up:
            </p>

            <div class="console-box" style="display:flex;flex-direction:column;gap:10px;">
              <label style="display:flex;align-items:center;gap:10px;cursor:pointer;">
                <input type="checkbox" ${state.removeShortcuts ? 'checked' : ''} onchange="UninstallerStore.toggle('removeShortcuts')" style="width:16px;height:16px;" />
                <span>Remove Desktop &amp; Start Menu shortcuts and Installed Apps registry entry</span>
              </label>
              <label style="display:flex;align-items:center;gap:10px;cursor:pointer;">
                <input type="checkbox" ${state.resetSystemProxy ? 'checked' : ''} onchange="UninstallerStore.toggle('resetSystemProxy')" style="width:16px;height:16px;" />
                <span>Restore default Windows System Proxy &amp; remove Mesh Firewall rules</span>
              </label>
              <label style="display:flex;align-items:center;gap:10px;cursor:pointer;">
                <input type="checkbox" ${state.deleteSavedConfigs ? 'checked' : ''} onchange="UninstallerStore.toggle('deleteSavedConfigs')" style="width:16px;height:16px;" />
                <span>Delete saved download manifests and pairing PIN history</span>
              </label>
            </div>

            <div>
              <div style="font-size:11px;color:var(--text-muted);margin-bottom:6px;">Optional Feedback — Reason for uninstalling:</div>
              <input id="uninst-reason" type="text" value="${state.reason}" placeholder="e.g., Upgrading to a newer version / Troubleshooting network" />
            </div>

            <div style="display:flex;justify-content:flex-end;gap:10px;margin-top:6px;">
              <button class="btn btn-secondary" onclick="Store.setState({appMode:'normal'});renderApp();">Cancel / Back to App</button>
              <button class="btn" style="background:linear-gradient(135deg,#ef4444,#dc2626);" onclick="runUninstallNow()">Uninstall Now</button>
            </div>
          </div>
        `
            : `
          <div style="display:flex;flex-direction:column;gap:14px;">
            <div style="font-size:14px;font-weight:700;color:#f87171;">
              ${state.progressPct >= 100 ? 'WiFi GigaChad 420 Has Been Uninstalled' : 'Removing WiFi GigaChad 420 &amp; Restoring OS Settings...'}
            </div>
            <div class="bar-track" style="height:10px;">
              <div class="bar-fill" style="width:${state.progressPct}%;background:linear-gradient(90deg,#ef4444,#f59e0b);"></div>
            </div>
            <div class="console-box">${state.logLines.join('\n')}</div>
            ${
              state.progressPct >= 100
                ? `<div style="display:flex;justify-content:flex-end;">
                     <button class="btn btn-secondary" onclick="window.close()">Close Uninstaller</button>
                   </div>`
                : ''
            }
          </div>
        `
        }
      </div>
    </div>
  `;
}

