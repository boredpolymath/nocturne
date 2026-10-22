# Privacy Policy for Project Nocturne

**Last Updated:** October 22, 2026  
**Effective Date:** October 22, 2026  
**Software:** Project Nocturne / Nocturne: Haunted Desktop  
**Publisher / Maintainer:** boredpolymath  
**Official Repository:** [https://github.com/boredpolymath/nocturne](https://github.com/boredpolymath/nocturne)  
**Official Website:** [https://boredpolymath.github.io/nocturne/](https://boredpolymath.github.io/nocturne/)  
**Privacy Policy Web URL:** [https://boredpolymath.github.io/nocturne/privacy.html](https://boredpolymath.github.io/nocturne/privacy.html)

---

## 1. Executive Summary & Core Guarantee

Project Nocturne ("Nocturne", "the Software") is designed and engineered from the ground up under a strict **Zero-Data-Collection, Zero-Egress, Air-Gapped** architectural philosophy.

* **We do not collect any personal data.**
* **We do not collect keystrokes, clipboard contents, typed text, or mouse positions.**
* **We do not record audio, video, or screen contents.**
* **We do not maintain user accounts, profiles, or tracking identifiers.**
* **We do not open network sockets or transmit telemetry across the internet.**
* **We do not share, sell, lease, or monetize user data with any third party.**

Every feature in Nocturne executes entirely on your local computer.

---

## 2. Information Nocturne Does NOT Collect

To provide complete transparency regarding Microsoft Store Policy 10.5.1 / 10.5.2, Apple Developer Guidelines, GDPR, CCPA, and CalOPPA requirements:

| Category | Status | Details |
| :--- | :--- | :--- |
| **Personal Identifiers** (Name, email, phone, IP address) | **Never Collected** | No account creation, login, or identity gathering of any kind exists. |
| **Keystroke Logging & Content** | **Never Collected** | Nocturne never inspects, captures, logs, or transmits what you type or read. |
| **Screen / Window Content** | **Never Captured** | Nocturne has no screen-recording, screenshot, OCR, or window-title inspection capabilities. |
| **Microphone & Ambient Audio** | **Never Captured** | All sound is procedurally synthesized via mathematical algorithms. No audio input device or microphone permission is requested or used. |
| **Location & Geolocation** | **Never Collected** | Nocturne does not access GPS, Wi-Fi tri-angulation, IP geolocation, or location services. |
| **Analytics & Telemetry** | **Never Transmitted** | No analytics SDKs, crash trackers, or telemetry payloads are included in the binary. |
| **Cookies & Persistent Web IDs** | **Never Used** | The standalone software uses no cookies or web beacons. |

---

## 3. How Nocturne Operates on Your System

Because Nocturne monitors activity to respond during quiet lulls in the late night, this section explains the exact technical mechanisms used:

### A. Aggregate Idle Time Monitoring (No Keylogging)
* **What it does:** Nocturne determines when you have paused typing or mouse movement by querying standard operating system idle counters:
  * **Windows:** `GetLastInputInfo` (Win32 API)
  * **macOS:** `CGEventSourceSecondsSinceLastEventType` (CoreGraphics API)
  * **Linux:** `XScreenSaverQueryInfo` (X11)
* **Privacy protection:** These OS counters return only a single scalar number representing the elapsed time (in seconds or milliseconds) since *any* generic input event occurred. They **cannot and do not** disclose which keys were pressed, which programs were running, or where the mouse was clicked.

### B. Emergency Purge / Triple-ESC Failsafe
* **What it does:** Nocturne includes an emergency panic mechanism allowing users to tap <kbd>ESC</kbd> three times rapidly to immediately silence audio, reset display color temperatures, and disengage background processing.
* **Privacy protection:** The failsafe keyboard listener inspects exclusively keycode `0x1B` (the standard hardware Escape key) inside an in-memory 3-slot timestamp buffer. Any and all other keystrokes are immediately discarded unread without being stored, processed, or logged.

### C. Display Color Profile (Gamma Ramp) Adjustments
* **What it does:** During subtle manifestation phases, Nocturne temporarily adjusts monitor color-temperature ramps using native operating system GDI/CoreGraphics display gamma tables (`SetDeviceGammaRamp` on Windows).
* **Privacy protection:** Nocturne does not read the pixels on your screen or take screenshots. Display adjustments are strictly outbound hardware LUT commands, clamped to safety limits (&le; 8% luminance delta) and automatically restored on exit or failsafe trigger.

### D. Procedural Audio Synthesis
* **What it does:** Soundscapes (floorboard creaks, binaural hums, phantom buckling-spring echoes) are computed in real time using 48 kHz stereo DSP mathematical oscillators in pure Rust.
* **Privacy protection:** Nocturne never records, listens to, or analyzes sound from your microphone or speakers.

---

## 4. Local-First Storage & Data Retention

Nocturne stores minimal operational records strictly on your local device:

1. **Local Incident Log (`nocturne_telemetry.db`)**:
   - Stores purely synthetic simulation events (e.g., timestamps of triggered sound phenomena, simulated EMF/temperature readings, and session duration).
   - Stored in a restricted SQLite database inside your personal OS user directory:
     - **Windows:** `%LOCALAPPDATA%\com.nocturne.app\`
     - **macOS:** `~/Library/Application Support/com.nocturne.app/`
     - **Linux:** `~/.config/nocturne/`
   - File permissions on Unix platforms are restricted to `0600` (read/write only by the current user), with directories set to `0700`.
   - The database maintains a strict rolling cap of **10,000 incidents**; older entries are pruned automatically.
2. **User Preferences**:
   - Stores your chosen settings (e.g., master volume, curfew override state, idle threshold).
3. **Data Deletion & Retention Control**:
   - You can completely delete all stored data at any time by simply deleting the local database file or directory listed above, or by uninstalling the application.

---

## 5. Network Egress & Air-Gap Enforcement

Nocturne is **100% air-gapped** and performs zero network egress:

* **Compile-Time Dependency Bans (`deny.toml`)**: Networking, telemetry, and HTTP client crates (`reqwest`, `hyper`, `curl`, `openssl`, `rustls`, `tauri-plugin-http`) are explicitly prohibited and audited via `cargo-deny`.
* **Runtime File-Descriptor Auditing**: The runtime `AirGapEnforcer` inspects open file descriptors on startup to ensure zero `AF_INET` or `AF_INET6` network sockets exist.
* **Sandboxing & Content Security Policy (CSP)**: The desktop WebView enforces `default-src 'self'; connect-src 'none'; frame-src 'none'; object-src 'none'`. Under macOS, Nocturne executes within a strict Seatbelt sandbox (`nocturne-airgap.sb`) denying all network operations.

---

## 6. Official Website Privacy

The project website ([https://boredpolymath.github.io/nocturne/](https://boredpolymath.github.io/nocturne/)) follows the same privacy-preserving ethos:

* **No Trackers or Third-Party Analytics:** We do not embed Google Analytics, Facebook Pixel, tracking scripts, or advertising networks.
* **No Third-Party CDNs or Fonts:** All scripts, stylesheets, and audio synthesis generators are self-hosted directly from the repository.
* **Content Security Policy:** The website enforces `connect-src 'none'`.
* **Hosting Provider:** The website is hosted via GitHub Pages (operated by GitHub, Inc. / Microsoft Corporation). When you visit the site, GitHub may collect standard HTTP access logs (such as IP addresses, user agents, and referral headers) strictly for operational security and service stability, subject to the [GitHub Privacy Statement](https://docs.github.com/en/site-policy/privacy-policies/github-privacy-statement).

---

## 7. Third-Party Services & Subprocessors

Nocturne contains **no third-party SDKs, no ad networks, and no external data processors**. Your interactions remain entirely private to your local computer.

---

## 8. Children’s Privacy

Nocturne does not collect, solicit, or maintain personal information from any individuals, including children under the age of 13 (or under 16 in applicable jurisdictions).

---

## 9. Security & Supply-Chain Transparency

Nocturne is published as transparent, open-source software:
* Every official build is compiled in isolated GitHub Actions runners with full public build logs.
* Every release includes a deterministic CycloneDX 1.5 Software Bill of Materials ([`sbom.cdx.json`](sbom.cdx.json)) and SHA-256 cryptographic checksums ([`SHA256SUMS.txt`](SHA256SUMS.txt)).
* For our vulnerability reporting policy and security disclosures, see [SECURITY.md](SECURITY.md).

---

## 10. Changes to This Privacy Policy

If this Privacy Policy is updated, changes will be published directly to this repository and reflected on the hosted website at `https://boredpolymath.github.io/nocturne/privacy.html` with an updated "Last Updated" timestamp.

---

## 11. Contact & Support

If you have questions, concerns, or inquiries regarding this Privacy Policy or Nocturne's technical privacy architecture, please contact us:

* **GitHub Issues:** [https://github.com/boredpolymath/nocturne/issues](https://github.com/boredpolymath/nocturne/issues)
* **Security & Vulnerability Reports:** [https://github.com/boredpolymath/nocturne/security/advisories/new](https://github.com/boredpolymath/nocturne/security/advisories/new)
* **Project Maintainer:** boredpolymath
