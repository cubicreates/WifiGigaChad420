# Phase Twelve: Security, Privacy, Legal & Driver Safety Architecture

**Document:** `docs/security/DRIVER_SECURITY.md`  
**Project:** WiFi GigaChad 420

---

## 1. Authorized Network & Credential Security Policy

1. **Zero Unauthorized Access or Authentication Bypass:**
   - `WiFi GigaChad 420` connects **exclusively** to Wi-Fi networks for which the user has already configured an authorized Windows WLAN profile (`WlanConnect` / `netsh wlan connect name="<AuthorizedProfile>"`).
2. **Zero Plaintext Password Extraction or Logging:**
   - The application never invokes `netsh wlan show profile key=clear`, never reads raw WPA2/WPA3 PSKs from Windows WLAN storage, and never transmits Wi-Fi credentials over the network or writes them into diagnostic logs.
   - All authentication handshakes (WPA2-Personal, WPA3-SAE, 802.1X Enterprise) are delegated strictly to the native Windows `wlansvc` and Windows Credential Manager subsystem.
3. **Zero MAC Spoofing / Device Impersonation:**
   - Outbound packets sent through physical interfaces use the physical adapter's legitimate hardware MAC address (`4C-23-38-76-35-A7` on the MT7921, `40-C2-BA-AF-8B-E3` on the Realtek GbE).
   - The `WiFi GigaChad 420 Virtual Network` logical interface uses a locally-administered IEEE 802 virtual MAC (`02:47:43:04:20:01`, where bit 1 of the first octet `0x02` is the standard IEEE Locally Administered Address bit) strictly on the local host virtual adapter.

---

## 2. Kernel Driver & Host OS Safety Boundaries (Rule 5 Compliance)

1. **No Weakening of Windows Security Features:**
   - `WifiGigaChad420.exe` **never** disables Windows Defender, **never** modifies Secure Boot, **never** runs `bcdedit /set testsigning on` automatically, and **never** installs unsigned kernel drivers on the user's production system.
2. **Isolated Experimental NDIS Driver (`Destop/Win/drivers/virtual_wifi_mux/`):**
   - The NDIS 6.x MUX driver source code is completely isolated in `Destop/Win/drivers/virtual_wifi_mux/` for controlled WDK test-VM usage only.
3. **Reversible Network Configuration Transactions:**
   - Every user-mode OS configuration change (`WcmSvc` `fMinimizeConnections = 0`, interface metric equalization, or `HKCU` system proxy toggle) is logged with a timestamp and can be reverted cleanly in 1 click via the **Rollback / Restore Default OS Settings** action or the **Uninstaller Wizard**.
4. **Least-Privilege Execution:**
   - Telemetry, capability inspection, DNS racing, SOCKS5/HTTP proxy multiplexing, and multi-path HTTP Range downloads run completely in user mode. Administrator privileges are used strictly for `HKLM\...\WcmSvc` anti-swap policy and `netsh interface ipv4 set interface` metric equalization.

