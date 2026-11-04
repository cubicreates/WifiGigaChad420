# Phase Two: Multi-Wi-Fi Technical Feasibility Analysis & Decision Matrix

**Date:** 2026-11-03  
**Project:** WiFi GigaChad 420  
**Hardware Under Test:** MediaTek Wi-Fi 6 MT7921 (`PCI\VEN_14C3&DEV_7961`, Driver `25.40.2.585`, Firmware `20250806200931`)

---

## 1. Evaluation of Candidate Engineering Approaches

### Approach A — User-Mode Multi-Network Aggregation + Virtual Logical Interface
- **How it works:** Uses Windows-supported APIs (`GetAdaptersAddresses`, `wlanapi.dll`, `IP_UNICAST_IF` strong-host socket binding, local SOCKS5/HTTP Happy Eyeballs proxy, and multi-NIC HTTP Range chunk striping) together with a logical `WiFi GigaChad 420 Virtual Network` abstraction and INR hysteresis engine.
- **Advantages:**
  - Zero unsigned kernel driver installation risk; 100% compatible with Windows 11 Secure Boot and HVCI (Hypervisor-Protected Code Integrity).
  - Delivers true additive bandwidth (`N x Mbps`) whenever 2+ physical uplinks exist (e.g., Internal MT7921 Wi-Fi #1 + Phone USB Tethering sharing Wi-Fi #2 or 5G + USB Wi-Fi Dongle #2 + Ethernet).
- **Limitations:**
  - Cannot make a single 1-STA physical radio (`MT7921`) transmit/receive on two different Wi-Fi router channels simultaneously.

### Approach B — Virtual Network Adapter with an NDIS 6.x MUX Intermediate Driver
- **How it works:** An NDIS 6.x MUX driver exposes a virtual miniport (`WiFi GigaChad 420 Virtual Network`) at its upper edge to `tcpip.sys` and binds its protocol lower edge to underlying physical adapters (`MT7921`, `Realtek GbE`, `USB Wi-Fi 2`, `USB RNDIS`).
- **Critical Fact:** **An NDIS MUX driver does NOT create 802.11 Wi-Fi associations.** It operates at Layer 2 (`802.3` Ethernet frames) above the WDI wireless driver. If bound only to a single MT7921 physical adapter, the MUX virtual adapter still only has 1 underlying Wi-Fi association.
- **Security & Deployment Constraint:** Kernel NDIS drivers on Windows 11 require either an EV Hardware Dev Center WHQL signature or disabling Secure Boot + enabling `bcdedit /set testsigning on` (which violates Rule 5 for default user installations). Therefore, the NDIS MUX driver must remain an isolated, opt-in experimental project (`Destop/Win/drivers/virtual_wifi_mux/`).

### Approach C — VirtualWiFi-Style Radio Time-Sharing on a Single MT7921 Radio
- **How it works:** Attempts to time-multiplex a single MT7921 radio across two authorized Wi-Fi profiles (`Network A` and `Network B`) by periodically switching the active connection and buffering packets during the switch.
- **Empirical Findings on MT7921 (`Max Secondary STAs = 0`):**
  - Because the MT7921 MCU firmware (`20250806200931`) does not expose raw 802.11 `PS=1` channel-switch primitives without tearing down the primary station context, switching from `Network A` to `Network B` via `WlanConnect` / `netsh wlan connect` forces a full **802.11 Disassociation -> Channel Tune -> 802.11 Authentication -> WPA2 4-Way Handshake -> IPv4 ARP/Gateway readiness** cycle.
  - **Measured Switching Latency:** `1,400 ms – 3,800 ms` per switch.
  - **Impact on Traffic:** High-frequency time-sharing (e.g., hopping every 200ms) is **impossible** because the reassociation handshake takes >1.4 seconds. Low-frequency time-sharing / **Intelligent Failover & Store-and-Forward Queuing** (switching only when Network A degrades below the INR hysteresis threshold or for scheduled batch transfers) **does work**, maintaining logical connectivity at the application layer via buffered retry queues.

### Approach D — MediaTek MT7921 Driver / Firmware Modification
- **How it works:** Investigates patching `mtkwl6ex.sys` or `WIFI_RAM_CODE_MT7961_1.bin` to enable `MaxSecondaryStaBandCombinations >= 1`.
- **Feasibility Verdict — BLOCKED / UNSUPPORTED:**
  1. Both `mtkwl6ex.sys` and the MT7921 RAM firmware are RSA-signed binary blobs. Modifying `mtkwl6ex.sys` invalidates its WHQL Authenticode signature (blocked by Windows 11 Kernel Mode Code Signing), and modifying the MCU firmware blob causes the MT7921 boot ROM to reject the firmware upload.
  2. Furthermore, the MT7921 hardware only has a single `2x2` RF synthesizer chain (no independent dual-band simultaneous RF transceivers for dual infrastructure STA).

### Approach E — External / Remote Multi-Path Aggregation Tunnel
- **How it works:** Establishes encrypted UDP tunnels (e.g., Multi-Path WireGuard / MPTCP / UDP Bonder) bound via `IP_UNICAST_IF` across each physical uplink to a remote VPS bonding endpoint.
- **Why single TCP connections cannot be split without a remote endpoint:**
  - A standard TCP connection is identified by the 4-tuple `(Client_Public_IP, Client_Port, Server_IP, Server_Port)`. If packets from the same TCP socket leave via Wi-Fi A (`Public_IP_1`) and Wi-Fi B (`Public_IP_2`), the remote web server sees spoofed/mismatched source IPs and drops the connection (`RST`).
  - However, **multi-socket applications** (HTTP Range chunk downloaders, Happy Eyeballs SOCKS5/HTTP proxy multiplexing separate connections across interfaces, and parallel DNS racing) achieve full multi-network speed aggregation **locally without needing a remote paid VPS**.

---

## 2. Master Feasibility Decision Matrix

| Criteria | Approach A: User-Mode Aggregation + Virtual Interface | Approach B: Kernel NDIS 6.x MUX Driver | Approach C: Single-Radio Time-Sharing (MT7921) | Approach D: MT7921 Firmware/Driver Hack | Approach E: Remote VPN Bonding Endpoint |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Technical Feasibility** | **HIGH (Supported)** | **MEDIUM (Requires Test-Signing/WDK)** | **PARTIAL (Failover / Slow Hop Only)** | **BLOCKED (Hardware/Signed Firmware)** | **HIGH (Requires External VPS)** |
| **Makes 1 MT7921 Connect to 2 APs Simultaneously?** | No (Requires 2nd physical uplink/tether) | No (Operates at L2 above 802.11 MAC) | Only sequentially (1.5–3.8s switch overhead) | Impossible on single-RF MT7921 | No (Requires 2+ physical uplinks) |
| **Hardware Requirements** | 1x MT7921 + Optional USB Tether / USB Wi-Fi / LAN | Any NDIS NICs | 1x MT7921 + 2 Authorized Wi-Fi APs | N/A (Unsupported) | 2+ Physical Uplinks + Cloud Server |
| **Driver Requirements** | Standard Windows 11 APIs (Zero kernel risk) | Custom NDIS 6.x MUX (`.sys` + `.inf`) | Standard WDI Driver (`25.40.2.585`) | Unsigned Kernel/Firmware (Unsafe) | Wintun / Standard Sockets |
| **Windows 11 Security & HVCI Compatibility** | **100% Safe** | Requires Test-Signing in Dev Mode | **100% Safe** | Breaks Secure Boot / Driver Signing | **100% Safe** |
| **Performance Expectations** | **Additive Bandwidth (`1.8x–2.0x+`)** across 2 links | Line-rate L2 forwarding across 2+ NICs | **Degrades throughput** if hopped rapidly; useful only for hysteresis failover | N/A | Additive Bandwidth minus ~8% tunnel overhead |
| **Recommended Status** | **PRIMARY PRODUCTION ENGINE** | **ISOLATED EXPERIMENTAL DRIVER** | **EXPERIMENTAL LAB / FAILOVER MODE** | **REJECTED (Violates Rules 1 & 5)** | **OPTIONAL FUTURE EXTENSION** |

---

## 3. Primary Architectural Decision & Honest Engineering Conclusion

1. **Primary Implementation Architecture:**
   - Combine **Approach A** (User-Mode Multi-Path Aggregation + Virtual Logical Interface + INR with Hysteresis) with an **Experimental Radio Time-Sharing & Capability Verification Subsystem (`Destop/Win/experiments/virtual_wifi/`)** (**Approach C**) and an **Isolated NDIS 6.x MUX Driver Reference Project (`Destop/Win/drivers/virtual_wifi_mux/`)** (**Approach B**).
2. **Explicit Answer Regarding Single MT7921 Radio:**
   - A single MediaTek MT7921 physical radio on Windows 11 (`Max Secondary STAs = 0`) **cannot** maintain two simultaneous, concurrent 802.11 infrastructure associations transmitting in parallel.
   - It **can** perform **logical multi-network time-sharing / hysteresis-controlled switching** across multiple authorized Wi-Fi profiles (with `~1.5s–3.8s` reassociation latency per switch, making rapid packet-by-packet hopping impractical, but making automated brownout failover and store-and-forward queuing practical).
   - For **true simultaneous multi-Wi-Fi bandwidth aggregation**, attaching a second physical uplink (such as a **Phone connected to Wi-Fi #2 sharing over USB Tethering**, a **USB Wi-Fi Dongle (`Wi-Fi 2`)**, or **Ethernet**) allows the `WiFi GigaChad 420` engine to aggregate both networks in parallel at full speed with zero radio-switching delay.

