# Security Policy

**WiFi GigaChad 420** is a system-level networking and multi-interface traffic aggregation platform. Because it interacts directly with operating system sockets, DNS resolution, local loopback proxies, virtual network adapters (`TUN`/`WinTun`/`VpnService`), and cross-device LAN mesh tunnels (`QUIC`), security and user privacy are foundational engineering requirements.

---

## Supported Versions

| Version | Supported | Status |
| :--- | :--- | :--- |
| **1.x (Main / Architecture & MVP)** | Yes | Active Development |
| **< 1.0 (Experimental Spikes)** | No | Superseded |

---

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub issues, discussions, or pull requests.**

If you discover a security vulnerability, privilege escalation vector, SSRF bypass, path traversal flaw, or cryptographic weakness in `giga_pal`, `giga_inr`, `giga_transport`, `giga_mesh`, or the Desktop/Mobile applications:

1. **Direct Email Disclosure:** Send a detailed report to **[blubrdpirts@gmail.com](mailto:blubrdpirts@gmail.com)** (`@cubicreates`) with the subject line: `[SECURITY] WiFi GigaChad 420 Vulnerability Report`.
2. **GitHub Private Vulnerability Reporting:** Alternatively, use the [**Report a vulnerability**](https://github.com/cubicreates/WifiGigaChad420/security/advisories/new) button under the repository's **Security** tab.

### What to Include in Your Report
* Affected crate or module (e.g., `crates/giga_transport`, `crates/giga_mesh`, `apps/desktop`, `apps/android`) and operating system platform.
* Step-by-step reproduction instructions or a minimal Proof-of-Concept (PoC).
* Potential impact (e.g., local privilege escalation, arbitrary file overwrite, LAN peer spoofing, DNS leak).
* Suggested mitigation or patch (if available).

### Response SLA
* **Initial Acknowledgment:** Within **48 hours**.
* **Triage & Severity Assessment:** Within **5 business days**.
* **Fix & Coordinated Disclosure:** Within **14–30 days** depending on complexity.

---

## Core Security & Privacy Architecture Guarantees

All contributors and code changes must strictly uphold the following architectural invariants defined in [Docs/WiFi_GigaChad_420_Plan.md](./Docs/WiFi_GigaChad_420_Plan.md):

### 1. SSRF & Loopback Protection (`giga_transport`)
* Outbound download workers and proxy forwarders validate resolved IP addresses *after* DNS resolution and *before* `connect()`.
* Connections targeting loopback (`127.0.0.0/8`, `::1`), link-local (`169.254.0.0/16` cloud metadata), and RFC 1918 private subnets (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`) are blocked by default unless explicitly initiated by the authenticated `giga_mesh` peer pairing subsystem or enabled via Developer Mode.

### 2. Path Traversal & Sparse File Safety
* Remote filenames extracted from URLs or `Content-Disposition` headers are sanitized of all path separators (`/`, `\`), drive letters, `..` segments, null bytes, and control characters.
* Sparse files are pre-allocated and written strictly inside user-authorized download directories.

### 3. Zero Silent TLS Interception (No Root CA MITM)
* WiFi GigaChad 420 enforces **TLS 1.3** (`tokio-rustls` + `aws-lc-rs`) and **never** installs a rogue Root CA or decrypts HTTPS traffic silently.
* System-wide HTTPS flow optimization uses unencrypted TLS ClientHello **SNI (Server Name Indication)** inspection for domain-affinity routing, preserving end-to-end TLS integrity and certificate pinning.

### 4. Captive Portal Quarantine (`giga_inr`)
* When an interface is detected behind an untrusted Captive Portal (`HTTP 302/200` interception), the Intelligent Network Resolver immediately isolates that interface in the `Excluded` state so background application traffic, cookies, and tokens cannot leak into an unauthenticated public portal.

### 5. Zero-Trust Cross-Device Wi-Fi Mesh (`giga_mesh`)
* Because `giga_mesh` operates over shared public or campus Wi-Fi networks, mDNS discovery is never trusted for authentication.
* Device pairing between Phone and Laptop requires explicit cryptographic verification (QR code Ed25519 public key exchange or SPAKE2+ PIN) and enforces **Mutual TLS 1.3 (mTLS) over QUIC (`iroh` / `quinn`)** for all relayed traffic.

### 6. Non-Destructive Hardware Policy
* By default, automatic interface exclusion operates strictly in user-space scheduling and routing tables. The engine never disables physical network adapters at the OS driver level without explicit user opt-in.
