# Phase Fifteen: Virtual Wi-Fi & Multi-Path Aggregation Test Plan

**Document:** `docs/testing/VIRTUAL_WIFI_TEST_PLAN.md`  
**Target Hardware:** `MediaTek Wi-Fi 6 MT7921` (`PCI\VEN_14C3&DEV_7961`, Driver `25.40.2.585`, Firmware `20250806200931`) + `Realtek PCIe GbE` (`PCI\VEN_10EC&DEV_8168`)

---

## 1. Automated Unit Tests (`cargo test --offline -p giga_virtual_wifi`)

| Test Case ID | Module Under Test | Verification Objective |
| :--- | :--- | :--- |
| `UT-01` | `capability_detection.rs` | Verifies parsing of `netsh wlan show wirelesscapabilities` & `show drivers` output for MT7921 (`MaxSecondaryStas = 0`, `WDI 0.1.1.12`, `P2P Clients = 2`) and strict status classification (`SUPPORTED` vs `UNSUPPORTED` vs `EXPERIMENTAL`). |
| `UT-02` | `network_resolver.rs` | Verifies INR Composite Health Score calculation and **Hysteresis Deadband (`15.0` points + dwell timer)** so small signal fluctuations do not trigger network switching. |
| `UT-03` | `network_resolver.rs` | Verifies automatic exclusion of dead/unhealthy networks (`INR_NETWORK_EXCLUDED`) and re-admission upon recovery (`INR_NETWORK_RESTORED`). |
| `UT-04` | `radio_scheduler.rs` | Verifies bounded packet buffer queue (`256` packets), `PACKET_QUEUE_OVERFLOW` detection, dwell-time enforcement, and honest rejection of zero-latency concurrent STA claims when `max_secondary_stas == 0`. |
| `UT-05` | `packet_mapper.rs` | Verifies deterministic 5-tuple flow mapping across bound physical interfaces without MAC spoofing, plus automatic failover remapping when a primary interface drops. |
| `UT-06` | `virtual_interface.rs` | Verifies lifecycle state transitions (`Standby` -> `Active` -> `RolledBack`) of the `WiFi GigaChad 420 Virtual Network` logical interface and reversible configuration rollback. |
| `UT-07` | `telemetry.rs` | Verifies structured event emission (`VIRTUAL_INTERFACE_CREATED`, `DRIVER_CAPABILITY_UNSUPPORTED`, `RADIO_SWITCH_STARTED`, `RADIO_SWITCH_COMPLETED`, etc.) and ensures zero credential leakage. |

---

## 2. Integration & Hardware Tests

| Test ID | Scenario | Expected & Verified Outcome |
| :--- | :--- | :--- |
| `IT-01` | Physical & Hidden Virtual Adapter Discovery on Host PC | Enumerates `Wi-Fi` (`if_index: 4`, MT7921), `Ethernet` (`if_index: 15`, Realtek GbE), `vEthernet` (`if_index: 34`), and hidden `Microsoft Wi-Fi Direct Virtual Adapter` #1 (`if_index: 19`) & #2 (`if_index: 11`). |
| `IT-02` | Single-Radio Multi-STA Hardware Verification | Correctly detects `physical_wifi_radios = 1`, `max_secondary_stas = 0`, and classifies concurrent Multi-STA on single MT7921 as `UNSUPPORTED` while classifying Radio Time-Sharing as `EXPERIMENTAL`. |
| `IT-03` | Interface-Bound Socket & DNS Race Test | Binds UDP/TCP sockets via `IP_UNICAST_IF` (`0x1F`) to active interface (`Gheras_5G` / `KRMU STUDENTS`) and measures real handshake RTT and DNS resolution latency. |
| `IT-04` | HTTP Range Multi-Path Download & SHA-256 Integrity | Downloads test payload using bound workers, verifies HTTP `206 Partial Content`, reassembles chunks via `RandomAccessFileAssembler`, and validates NIST FIPS-180-4 SHA-256 hash. |
| `IT-05` | Reversible Rollback Verification | Applies user-mode Virtual Network & OS optimization state, then executes rollback and verifies original state is restored cleanly. |

