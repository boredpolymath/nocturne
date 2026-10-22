# Security Policy

Project Nocturne (`MK-IV`) is engineered from the ground up as a **local-first, zero-egress (air-gapped)** desktop daemon with strict privacy and hardware restoration guarantees. For our compliance disclosures and data handling policies, see our standalone [**Privacy Policy**](PRIVACY.md) or visit [**boredpolymath.github.io/nocturne/privacy.html**](https://boredpolymath.github.io/nocturne/privacy.html).

---

## Supported Versions

Security fixes and supply-chain audits are applied to the latest release on the `main` branch:

| Version | Supported |
| :--- | :--- |
| `1.0.x` (Latest) | :white_check_mark: Supported |
| `< 1.0.0` | :x: Not Supported |

---

## Core Security Architecture

1. **Compile-Time & Runtime Zero-Egress Enforcement**
   - **Dependency Bans ([`deny.toml`](deny.toml))**: All HTTP, WebSocket, TLS, and remote shell crates (`reqwest`, `hyper`, `ureq`, `curl`, `rustls`, `openssl`, `tauri-plugin-http`, `tauri-plugin-shell`, `tauri-plugin-updater`, etc.) are explicitly denied across all four target triples (`aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`).
   - **Runtime Socket Scrubbing ([`src-tauri/src/security/mod.rs`](src-tauri/src/security/mod.rs))**: `AirGapEnforcer` audits process file descriptors to verify zero `AF_INET` or `AF_INET6` sockets are open.
   - **OS Sandboxing & CSP**: Enforces `default-src 'self'; connect-src 'none'` in the WebView alongside a macOS Seatbelt sandbox profile ([`nocturne-airgap.sb`](src-tauri/nocturne-airgap.sb)) that denies `network*`.
2. **Non-Interceptive Input Telemetry**
   - Nocturne never installs global keyloggers or text-capture hooks. Idle duration is queried exclusively via aggregate OS idle counters (`CGEventSourceSecondsSinceLastEventType`, `GetLastInputInfo`, `XScreenSaverQueryInfo`).
3. **Local Database Hardening**
   - The local SQLite database (`nocturne_telemetry.db`) is created inside the user's private application data directory with `0700` directory and `0600` file permissions on Unix systems, running in `WAL` mode with a strict 10,000-row rolling cap.
4. **Reproducible Supply-Chain Integrity**
   - Every commit is verified against a deterministic CycloneDX 1.5 SBOM ([`sbom.cdx.json`](sbom.cdx.json)) and SHA-256 manifest ([`SHA256SUMS.txt`](SHA256SUMS.txt)) via [`scripts/generate_release_integrity.py`](scripts/generate_release_integrity.py).

---

## Known Transitive Advisory Evaluations

The following informational advisories exist in upstream Linux GTK3 bindings required by Tauri v2 (`webkit2gtk` / `gtk v0.18`) and have been audited for impact on Nocturne:

| Advisory ID | Crate | Classification | Impact Assessment |
| :--- | :--- | :--- | :--- |
| **[RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429)** (`GHSA-wrw7-89jp-8q8g`) | `glib 0.18.5` | `unsound` | **Not Affected (Unreachable Dead Code).** Affects `glib::VariantStrIter::impl_get` (constructed only via `Variant::array_iter_str`). Neither Nocturne nor any transitive dependency in the build graph calls `VariantStrIter` or `array_iter_str`. Furthermore, `glib` is only compiled on Linux targets and unused functions are stripped via `lto = true`. |
| **[RUSTSEC-2024-0370](https://rustsec.org/advisories/RUSTSEC-2024-0370)** | `proc-macro-error 1.0.4` | `unmaintained` | **No Runtime Impact.** Compile-time procedural macro helper pulled in transitively by `glib-macros 0.18.5` and `gtk3-macros 0.18.2` on Linux targets. Not linked into the final binary. |

---

## Reporting a Vulnerability

If you discover a security vulnerability, privacy leak, sandbox bypass, or failsafe failure in Project Nocturne, **please do not report it in a public GitHub issue.**

Instead, please report it privately using **[GitHub Private Vulnerability Reporting](https://github.com/boredpolymath/nocturne/security/advisories/new)**:

1. Navigate to the [**Security**](https://github.com/boredpolymath/nocturne/security) tab of this repository.
2. Click **"Report a vulnerability"** to open a private advisory draft with the maintainers.
3. Include affected platform(s), reproduction steps, and any relevant logs or proof-of-concept code.

We aim to acknowledge all security reports within **48 hours** and provide a remediation timeline or patch within **7 days**.
