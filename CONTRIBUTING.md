# Contributing to WiFi GigaChad 420

Thank you for considering contributing to **WiFi GigaChad 420**.  
Whether you are resolving a socket-binding issue on Windows or Linux, refining the **Intelligent Network Resolver (INR)** scoring model, building the **Tauri 2 + React 19** desktop interface, developing the **Kotlin + Jetpack Compose** Android client, or working on the **`giga_mesh` Phone-to-Laptop QUIC bridge**, your contributions are welcome.

Before writing code, please review the [**Master Technical Project Blueprint (`Docs/WiFi_GigaChad_420_Plan.md`)**](./Docs/WiFi_GigaChad_420_Plan.md) to understand the system architecture, mathematical models, and design invariants.

---

## Table of Contents
1. [Architecture & Crate Map](#architecture--crate-map)
2. [Prerequisites & Development Environment](#prerequisites--development-environment)
3. [Engineering Rules & Invariants](#engineering-rules--invariants)
4. [Development & Testing Workflow](#development--testing-workflow)
5. [Commit Message & PR Conventions](#commit-message--pr-conventions)

---

## Architecture & Crate Map

All core networking, health resolution, scheduling, and mesh logic resides in the **Rust Workspace (`crates/`)** so that improvements immediately benefit Desktop, Mobile, and CLI targets simultaneously:

| Crate / Directory | Responsibility |
| :--- | :--- |
| `crates/giga_pal` | **Platform Abstraction Layer:** OS socket binding (`socket2`, `IP_UNICAST_IF`, `SO_BINDTODEVICE`, Android C-NDK `android_setsocknetwork`), Wi-Fi RSSI telemetry (`wlanapi`, `nl80211`), shared-gateway ARP/STUN checks, and TUN/eBPF hooks. |
| `crates/giga_inr` | **Intelligent Network Resolver:** 11-metric Composite Health Score ($CHS$), 7-state hysteresis state machine (`Discovered`, `Testing`, `Active`, `Degraded`, `Excluded`, `Offline`, `Recovering`), captive portal detection, and `hickory-resolver` bound parallel DNS. |
| `crates/giga_scheduler` | **Adaptive Traffic & Chunk Brain:** Work-stealing HTTP Range chunk scheduler, zero-loss in-flight chunk splitting on exclusion, multi-class QoS (`Interactive`, `Streaming`, `BulkDownload`), and metered data-cap enforcement. |
| `crates/giga_transport` | **Transport & Proxy Engine:** Custom `hyper` 1.x + `tokio-rustls` + `socket2` connector, local `127.0.0.1:8080` SOCKS5/HTTP proxy, SNI domain-affinity classifier, `smoltcp` user-space TCP terminator, and sparse file reassembler. |
| `crates/giga_mesh` | **Cross-Device Mesh:** `mdns-sd` LAN discovery, QR/SPAKE2+ pairing, and `iroh`/`quinn` mTLS 1.3 QUIC streams turning a Phone's 5G modem into a `RemotePeerInterface` for a Laptop over shared Wi-Fi. |
| `crates/giga_store` | **Persistence:** Pure-Rust `redb` + `rkyv` crash-safe chunk bitmap and state storage (plus optional SQLite telemetry export). |
| `crates/giga_ffi` | **Mobile Bindings:** Mozilla `UniFFI` proc-macro interface exported to Kotlin and Swift. |
| `apps/desktop` | **Desktop App:** Tauri 2 + React 19 + TypeScript 5 + Tailwind v4 + Zustand + `uPlot` + `OffscreenCanvas`. |
| `apps/android` | **Android App:** Kotlin 2.1 + Jetpack Compose (Material 3) + `ConnectivityManager` + `VpnService`. |

---

## Prerequisites & Development Environment

### 1. Core Rust Engine (All Platforms)
* **Rust Toolchain:** `rustup` with stable Rust **1.82+ (2024 Edition ready)**
  ```bash
  rustup update stable
  rustup component add clippy rustfmt
  ```

### 2. Desktop Application (`apps/desktop`)
* **Node.js:** `v20+` (or `pnpm` / `bun`)
* **Tauri 2 OS Prerequisites:**
  * **Windows:** Microsoft Visual Studio C++ Build Tools & WebView2 Runtime (pre-installed on Windows 10/11).
  * **Linux (Ubuntu/Debian):**
    ```bash
    sudo apt update
    sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
    ```

### 3. Android Application (`apps/android`)
* **Android Studio Ladybug (or newer)** with **Android SDK 35** and **NDK r27+**.
* **Rust Android Targets & `cargo-ndk`:**
  ```bash
  rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
  cargo install cargo-ndk
  ```

---

## Engineering Rules & Invariants

When submitting a Pull Request, your code **must** respect these core rules:

1. **No Pure-C Dependencies in Core Crates Unless Strictly Gated:** Keep `giga_inr`, `giga_scheduler`, `giga_transport`, `giga_mesh`, and `giga_store` 100% pure Rust (`tokio-rustls`, `redb`, `hickory-resolver`, `quinn`) so cross-compiling to Android NDK and Windows never breaks on missing CMake/OpenSSL toolchains.
2. **Never Block Tokio Async Threads:** File I/O (`pwrite` / sparse pre-allocation) must use async positional I/O or `tokio::task::spawn_blocking`. Never call blocking OS network APIs on the async reactor thread.
3. **Never Rely Solely on Wi-Fi RSSI for Health:** Any modification to `giga_inr` must preserve the separation between physical signal strength and active upstream WAN validation.
4. **Preserve Completed Bytes on Interface Exclusion:** When an interface transitions to `Excluded`, already-flushed disk blocks ($\ge 256\text{ KiB}$) must be preserved via chunk splitting—never discard a partially downloaded 64 MiB chunk from byte zero.
5. **No Hardware Adapter Disabling by Default:** Exclusion must remain strictly in user-space/routing-space unless the user explicitly triggers an opt-in hardware reset.

---

## Development & Testing Workflow

1. **Fork & Clone the Repository:**
   ```bash
   git clone https://github.com/<your-username>/WifiGigaChad420.git
   cd WifiGigaChad420
   ```
2. **Create a Feature Branch:**
   ```bash
   git checkout -b feat/inr-captive-portal-probe
   ```
3. **Run Formatting, Linting & Unit Tests:**
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```

---

## Commit Message & PR Conventions

We follow the **[Conventional Commits](https://www.conventionalcommits.org/)** specification:

* `feat(inr): add bound DoH fallback probe for degraded Wi-Fi`
* `feat(mesh): implement mdns-sd peer discovery and QR pairing`
* `fix(pal/windows): bind outbound socket via IP_UNICAST_IF before connect`
* `perf(transport): align sparse file chunk writes to 64 KiB boundaries`
* `docs(plan): update Mermaid sequence diagrams and tech stack`

### Pull Request Checklist
- [ ] Code compiles cleanly across workspace targets with zero `clippy` warnings.
- [ ] Unit and integration tests added for new scheduling, probing, or socket-binding logic.
- [ ] Documentation in [Docs/WiFi_GigaChad_420_Plan.md](./Docs/WiFi_GigaChad_420_Plan.md) updated if architectural interfaces changed.
- [ ] Adheres to our [Code of Conduct](./CODE_OF_CONDUCT.md) and [Security Policy](./SECURITY.md).
