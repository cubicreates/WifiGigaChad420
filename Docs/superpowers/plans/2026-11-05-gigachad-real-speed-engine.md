# WiFi GigaChad 420: Real-Speed Acceleration Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform WiFi GigaChad 420 into a true Windows network acceleration powerhouse by delivering kernel-level TCP stack optimization, instant plug-and-play multi-link bonding (Wi-Fi + Phone USB Tethering / Ethernet / Dongle), parallel DNS racing, and an IDM-grade multi-stream chunk downloader.

**Architecture:**
A multi-tier Windows networking engine combining: (1) Windows kernel NDIS & TCP parameter tuning (`os_optimizer`), (2) Automatic multi-adapter discovery and independent WAN binding (`giga_pal` & `giga_inr`), (3) Low-latency UDP/TCP DNS racing (`DnsRaceResolver`), (4) 16-worker parallel HTTP range-chunk downloader (`giga_download`), and (5) Real-time NDIS kernel throughput monitoring (`GetIfEntry2`).

**Tech Stack:** Rust 2021, Windows Win32 APIs (`windows-sys`), NDIS MibIfRow2, Windows Registry, Windows Socket Architecture (`IP_UNICAST_IF`), TypeScript, React.

**Spec:** [`docs/WiFi_GigaChad_420_Plan.md`](file:///d:/Github/LightningMCQueen/docs/WiFi_GigaChad_420_Plan.md)

## Global Constraints
- Target platform: Windows 10 & 11 (x64) with native administrator elevation support.
- Output binary must compile to a single portable executable at `prod\Desktop\win\WifiGigaChad420.exe`.
- Subsystem: Windows GUI (`#![windows_subsystem = "windows"]`) to prevent console popup flashes.
- Embedded UAC Manifest: `/MANIFESTUAC:level='requireAdministrator' uiAccess='false'`.
- All throughput and telemetry metrics must be 100% genuine and sourced from Windows kernel counters (`InOctets`, `OutOctets`); no mock or fabricated data.
- Offline compilation (`--offline`) must be maintained.

---

### Task 1: Windows Kernel TCP & Registry "Crack Mode" Optimizer

**Files:**
- Modify: `Destop/Win/crates/giga_pal/src/os_optimizer.rs`
- Test: `Destop/Win/crates/giga_pal/src/os_optimizer.rs`

**Interfaces:**
- Consumes: `InterfaceSnapshot` from `crates/giga_pal/src/interface.rs`.
- Produces: `apply_windows_crack_mode_optimizations(&[InterfaceSnapshot]) -> OsOptimizationReport`.

- [ ] **Step 1: Write the unit test for Crack Mode TCP optimization steps**
Add `test_crack_mode_optimization_steps` in `crates/giga_pal/src/os_optimizer.rs` verifying that NetworkThrottlingIndex, SystemResponsiveness, Nagle disable, BBR/ECN, and Receive Window scaling are registered in the optimization report.

- [ ] **Step 2: Run test to verify test structure**
Run `cargo test --offline -p giga_pal -- test_crack_mode` to confirm the test executes.

- [ ] **Step 3: Implement Crack Mode optimizations in `os_optimizer.rs`**
Add:
- `NetworkThrottlingIndex = 0xFFFFFFFF` (disables multimedia network throttling).
- `SystemResponsiveness = 0` (dedicates 100% network CPU priority to gaming/data).
- `TcpAckFrequency = 1` and `TCPNoDelay = 1` across active adapter GUIDs to remove Nagle's 40ms buffering delay.
- `netsh int tcp set global congestionprovider=bbr` (with fallback to `ctcp`).
- `netsh int tcp set global ecncapability=enabled`.
- `netsh int tcp set global rss=enabled` and `rsc=enabled`.

- [ ] **Step 4: Run tests and ensure all pass**
Run `cargo test --offline -p giga_pal` and confirm all tests pass cleanly.

---

### Task 2: Plug-and-Play Multi-Link Bonding & Phone USB Auto-Detection

**Files:**
- Modify: `Destop/Win/crates/giga_inr/src/lib.rs`
- Modify: `Destop/Win/apps/desktop/src/main.rs`
- Test: `Destop/Win/crates/giga_inr/src/lib.rs`

**Interfaces:**
- Consumes: `query_interface_octets`, `create_platform_manager` from `giga_pal`.
- Produces: Automatic multi-adapter pooling when a phone (USB Tethering RNDIS), Ethernet cable, or USB Wi-Fi dongle is connected, dynamically rebalancing scheduling weights.

- [ ] **Step 1: Write unit test for multi-medium WAN detection**
Add test in `crates/giga_inr/src/lib.rs` verifying that when multiple distinct network adapters are present (e.g. Wi-Fi + RNDIS USB Tethering or Ethernet), the INR classifies both as independent WANs with positive weights.

- [ ] **Step 2: Implement dynamic multi-medium aggregation in `giga_inr`**
Ensure `refresh_and_probe` automatically discovers newly plugged USB tethering / Ethernet interfaces without needing an app restart, computes independent WAN status, and binds them to the scheduler.

- [ ] **Step 3: Run cargo test**
Run `cargo test --offline -p giga_inr` to verify detection logic.

---

### Task 3: Interactive UI Multi-Link Dashboard & IDM-Grade Download Accelerator

**Files:**
- Modify: `Destop/Win/apps/desktop/ui/src/screens/WifiDashboardScreen.tsx`
- Modify: `Destop/Win/apps/desktop/ui/src/screens/OngoingProcessesScreen.tsx`
- Modify: `Destop/Win/apps/desktop/ui/src/App.tsx`
- Modify: `Destop/Win/apps/desktop/src/main.rs`

**Interfaces:**
- Consumes: `/api/telemetry`, `/api/download`, `/api/os-optimize`
- Produces: In-app download accelerator tab where users can paste any URL and download it with 16 parallel range workers, plus an explicit "Turbo / Crack Mode" toggle button in the UI.

- [ ] **Step 1: Add Crack Mode trigger endpoint to `main.rs`**
Add `POST /api/optimize-crack-mode` to invoke `apply_windows_crack_mode_optimizations` and return the optimization report.

- [ ] **Step 2: Add Turbo Downloader and Crack Mode card to UI**
Update `WifiDashboardScreen.tsx` and `OngoingProcessesScreen.tsx` with:
- "⚡ Windows Network Crack Mode (Unlock Kernel Throughput & 0ms Nagle Delay)" toggle button.
- Live Multi-Link Connection status clearly displaying pooled interfaces (e.g., "Wi-Fi [11 Mbps] + Phone USB 5G [52 Mbps] = 63 Mbps Combined").
- Interactive Downloader box allowing URL input, thread selection (4–16 workers), and real-time chunk progress bars.

- [ ] **Step 3: Verify build with `bundle.mjs` and cargo check**
Run `cargo check --offline -p giga-desktop` to ensure the UI builds cleanly.

---

### Task 4: Documentation Synchronization & Blueprint Updates

**Files:**
- Modify: `docs/WiFi_GigaChad_420_Plan.md`
- Modify: `docs/architecture/VIRTUAL_WIFI_ARCHITECTURE.md`
- Modify: `docs/research/WINDOWS_VIRTUAL_WIFI_RESEARCH.md`
- Modify: `README.md`

**Interfaces:**
- Documents the definitive architecture: Multi-Link Aggregation, Kernel TCP Crack Tuning, Parallel DNS Racing, and Multi-Stream Chunk Acceleration, replacing all outdated virtual Wi-Fi time-sharing myths with empirical production specs.

- [ ] **Step 1: Update `docs/WiFi_GigaChad_420_Plan.md`**
Align the master plan with the 5 pillars of real acceleration.

- [ ] **Step 2: Update architecture and research markdown files**
Ensure documentation reflects the real-world hardware boundaries and software acceleration mechanics.

---

### Task 5: Production Release Build & Verification

**Files:**
- Output: `prod/Desktop/win/WifiGigaChad420.exe`

- [ ] **Step 1: Execute full test suite**
Run `cargo test --offline` across the entire workspace.

- [ ] **Step 2: Build release executable with Administrator manifest**
Run `$env:GIGA_REQUIRE_ADMIN_MANIFEST="1"; cargo build --release --offline -p giga-desktop`.

- [ ] **Step 3: Copy to production path and verify binary**
Copy to `prod\Desktop\win\WifiGigaChad420.exe` and verify size, subsystem, and manifest.

