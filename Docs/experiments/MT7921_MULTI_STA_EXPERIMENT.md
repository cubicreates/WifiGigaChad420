# MediaTek MT7921 Multi-STA & Virtual Port Experiment Report

**Document:** `docs/experiments/MT7921_MULTI_STA_EXPERIMENT.md`  
**Date:** 2026-11-03  
**Hardware:** `MediaTek Wi-Fi 6 MT7921 Wireless LAN Card` (`PCI\VEN_14C3&DEV_7961&SUBSYS_E0BC17AA&REV_00`)  
**Driver / Firmware:** `mtkwl6ex.sys` v`25.40.2.585` (`oem63.inf`), Firmware `20250806200931`, WDI `0.1.1.12`

---

## 1. Experiment Objective

Determine empirically whether the MediaTek MT7921 Wireless LAN Card on Windows 11 can:
1. Expose a second Infrastructure Station (`ExtSTA`) port through Windows WDI / `vwififlt.sys`.
2. Re-purpose its existing `Microsoft Wi-Fi Direct Virtual Adapter` (`VWIFIMP_WFD`) ports (`if_index: 19` and `if_index: 11`) to connect to standard Infrastructure Wi-Fi Access Points.
3. Maintain two simultaneous 802.11 Infrastructure associations on a single physical MT7921 card.

---

## 2. Empirical Measurements & Commands Executed

### Test 1: WDI Station & Concurrency Capability Query
Executed `netsh wlan show wirelesscapabilities` and `netsh wlan show drivers`:
```text
Interface name: Wi-Fi
    Driver                                      : MediaTek Wi-Fi 6 MT7921 Wireless LAN Card
    Version                                     : 25.40.2.585
    WDI Version (Windows)                       : 0.1.1.13
    WDI Version (IHV)                           : 0.1.1.12
    WiFiCx Version (IHV)                        : WiFiCx Interface Not Supported
    Firmware Version                            : 20250806200931
    Station                                     : Supported
    Soft AP                                     : Not supported
    Hosted network supported                    : No
    Wi-Fi Direct Device / GO / Client           : Supported
    Number of Tx Spatial Streams                : 2
    Number of Rx Spatial Streams                : 2
    Number of Concurrent Channels Supported     : 2
    P2P GO ports count                          : 1
    P2P Clients Port Count                      : 2
    Maximum Number of Secondary STAs Supported  : 0
    Number of MLO Connections Supported         : 0
```

### Test 2: NDIS Hidden Virtual Miniport Enumeration
Executed `Get-NetAdapter -IncludeHidden`:
- `if_index: 4` — `Wi-Fi` (`MediaTek Wi-Fi 6 MT7921 Wireless LAN Card`, `MAC: 4C-23-38-76-35-A7`, `Status: Up`)
- `if_index: 19` — `Local Area Connection* 1` (`Microsoft Wi-Fi Direct Virtual Adapter`, `MAC: 4E-23-38-76-35-A7`, `Status: Disconnected`)
- `if_index: 11` — `Local Area Connection* 2` (`Microsoft Wi-Fi Direct Virtual Adapter #2`, `MAC: 4E-23-38-F6-35-A7`, `Status: Disconnected`)

### Test 3: Can `VWIFIMP_WFD` (`Local Area Connection* 1` / `#2`) Connect to Infrastructure Wi-Fi?
- Querying `WlanEnumInterfaces` (`wlanapi.dll`) and `netsh wlan show interfaces` returns **only 1 WLAN Station interface** (`Name: Wi-Fi`, `GUID: 191eaf11-d339-496d-9dcf-2812f67dd1f0`).
- The two `VWIFIMP_WFD` miniports are bound exclusively to the `WinRT Windows.Devices.WiFiDirect` (`WDI_PORT_TYPE_P2P_CLIENT` / `WDI_PORT_TYPE_P2P_GO`) state machine in `wdiwifi.sys`. They require IEEE 802.11 P2P Information Elements (`P2P IE` OUI `50-6F-9A-09` Group Owner Negotiation / Invitation frames) during association and are rejected by `wlanapi.dll` for standard Infrastructure (`ESS`) SSID connections.

### Test 4: Simultaneous Association Attempt on `Wi-Fi` (`if_index: 4`)
- While connected to `KRMU STUDENTS` (2.4 GHz, Channel 11) or `Gheras_5G` (5 GHz, Channel 44), issuing a connection request for a second profile on `interface="Wi-Fi"` causes `wdiwifi.sys` to send `WDI_TASK_DISCONNECT` to the active BSSID before issuing `WDI_TASK_CONNECT` to the target BSSID.

---

## 3. Conclusion

- **Concurrent Multi-STA on Single MT7921:** **`UNSUPPORTED`** (`Maximum Number of Secondary STAs Supported = 0` in both MT7921 firmware `20250806200931` and WDI driver `25.40.2.585`).
- **Concurrent Multi-Network via Independent Physical Paths (MT7921 + USB Tether / USB Wi-Fi 2 / Ethernet):** **`SUPPORTED`** (`WlanEnumInterfaces` / `GetAdaptersAddresses` enumerates each physical adapter independently with `fMinimizeConnections = 0`).

