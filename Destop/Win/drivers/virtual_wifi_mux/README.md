# Isolated NDIS 6.80 Intermediate MUX Driver (`virtual_wifi_mux`)

## Overview
This directory contains the isolated Phase 5 NDIS 6.80 Intermediate MUX driver skeleton (`gigachadmux.sys` / `gigachadmux.inf`) that exposes `WiFi GigaChad 420 Virtual Network` at the NDIS upper edge.

## Safety & Rule 5 Isolation
- **Not Auto-Installed:** `WifiGigaChad420.exe` never installs unsigned kernel drivers or alters Secure Boot / Driver Signature Enforcement.
- **Physical Reality:** An NDIS MUX driver operates at Layer 2 (`NdisMedium802_3`) above the WDI 802.11 layer and does not bypass the MediaTek MT7921 firmware limit (`Maximum Number of Secondary STAs Supported = 0`).
- **Rollback:** Run `.\uninstall_driver.ps1` to cleanly remove the driver in a WDK test VM.

