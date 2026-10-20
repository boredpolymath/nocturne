# Project Nocturne (`MODEL NOCTURNE-MK IV`)

![Project Nocturne MK-IV — Paranormal Telemetry Unit](assets/readme-thumbnail-1.png)

**Ambient Workstation Poltergeist Daemon & Paranormal Telemetry Unit (`v1.0.4-RELEASE`)**

Nocturne is a local-first, zero-egress desktop daemon and paranormal telemetry instrument built on **Tauri v2** and **Rust** (`rust-version = "1.77.2"`). Designed as an industrial rackmount **Poltergeist Field Detector & REM-Pod**, it monitors workstation inactivity, typing cadence lulls (`4.0s–15.0s`), and live hardware sensors during nocturnal vigil brackets (`00:00–05:00`) to orchestrate spatial binaural acoustics (`cpal` direct ringbuffer), full-screen spectral color-shifts & hardware gamma LUT perturbations (`<= 8.0%` cubic-bezier ramp), and subtle Lissajous planchette cursor drift.

---

## Key Capabilities & Tonight's Changelog (`v1.0.4-RELEASE`)

### 1. Procedural Acoustic Manifestation Engine (`src-tauri/src/audio/mod.rs`)
- **Lock-Free Stereo PCM Ringbuffer (`0xDEADBEEF`)**: Streams 48.0 kHz stereo float32 audio directly through `cpal` with a 64-point real-time oscilloscope tap and a hard psychoacoustic decibel limiter (`+6 dB` to `+18 dB` over a `-44.2 dBFS` ambient floor).
- **6 Procedural Synthesis Interventions**:
  1. **Spatial Floorboard Creaks** (`synthesize_spatial_creak` / `dispatch_spatial_creak`): Multi-modal stick-slip wood resonance with binaural ITD/ILD localization and CPU thermal sympathy pitch scaling.
  2. **Phantom Keystroke Echoes** (`synthesize_phantom_keystroke` / `dispatch_phantom_keystroke_echo`): Buckling-spring switch transient (`3.2 kHz` click + `420 Hz` keycap bottom-out) triggered on `2.5s` cadence halts.
  3. **3-Formant Spirit-Box EVP Whispers** (`synthesize_evp_formant_whisper` / `dispatch_evp_whisper`): Band-limited aspiration noise sculpted through a 3-formant vocal tract (`F1`, `F2`, `F3`) with `5.5 Hz` syllabic modulation and binaural orbital panning.
  4. **Wall-Stud Double-Knocks & Reed-Relay Clicks** (`synthesize_wall_stud_double_knock` / `dispatch_wall_knock`): `145 Hz` structural timber double-raps (`165ms` interval) followed by a `2,400 Hz` metallic reed-relay click.
  5. **Redline 50/60 Hz Mains Hum** (`synthesize_redline_mains_hum` / `dispatch_redline_hum`): Transformer ground-loop hum (`55 Hz` / `110 Hz` / `165 Hz` harmonics + `1,800 Hz` corona crackle) paired with `20+ mG` CRT scanline tear glitches in the EVP spectrograph.
  6. **Infrasonic Room Breath & Sub-Perceptual Carriers** (`synthesize_infrasonic_room_breath` / `dispatch_room_breath` & `synthesize_sub_perceptual_carrier`): `18–24 Hz` standing-wave carrier and low-pass room resonance coupled directly to live CPU `Tctl` thermal load.
- **Manifestation Rotation**: Automatically cycles across all manifestation archetypes during armed cognitive lulls and manual `CAL: 50-60Hz` calibration pulses.

### 2. Full-Screen Spectral Color-Shift, Gamma LUT & Planchette Drift (`src-tauri/src/display/mod.rs`)
- **Native Hardware Gamma LUT Controller**: Captures and modulates native OS display transfer tables (`CGSetDisplayTransferByTable` on macOS, `SetDeviceGammaRamp` on Windows, `XF86VidModeSetGammaRamp` on Linux) along a time-stepped `>= 1,200ms` cubic-bezier curve (`P1=(0.42, 0.0)`, `P2=(0.58, 1.0)`) clamped to `<= 8.0%` baseline luminance delta.
- **Alternating Full-Screen Spectral Color-Shift Modes**:
  - **Mode 0 — Ectoplasmic Cyan-Violet Phase Inversion**: Crushes the hardware red LUT channel while surging green/blue gamma and applying a synchronized `hue-rotate(155deg)` viewport chromatic phase-shift.
  - **Mode 1 — Crimson Nightmare Solarization**: Surges the hardware red LUT channel while crushing green/blue channels into a high-contrast blood-red nightmare solarization (`hue-rotate(-55deg)`).
- **Planchette Cursor Micro-Drift** (`nudge_planchette_cursor`): Applies a subtle `1–3px` Lissajous figure-8 cursor nudge (`CGWarpMouseCursorPosition` / `SetCursorPos` / `XWarpPointer`) at the onset of cognitive lull manifestations.
- **Live Hardware Gauges**: Samples live CPU junction temperature (`cpu_tctl_celsius`), simulated cold-spot temperature dips (`-4.1°C`), and daemon resident set size (`daemon_mem_mb`) via `sysinfo`.

### 3. Non-Interceptive Telemetry, Cadence Analyzer & REM-Pod Array (`src-tauri/src/telemetry/`)
- **Privacy-Preserving Idle Hooks**: Queries OS input idle duration (`CGEventSourceSecondsSinceLastEventType` on macOS, `GetLastInputInfo` on Windows, `XScreenSaverQueryInfo` via `libXss.so.1` on Linux) with **zero keylogging** and **zero text capture**.
- **K-II EMF & REM-Pod Disturbance Array**: 5-step LED telemetry scale (`1.5 mG NORM`, `2.5 mG LOW`, `10 mG MID`, `20 mG HIGH`, `20+ mG SPIKE`) with automatic `22.40 mG` redline spike override during high-energy manifestations.
- **Witching Hour Vigil Chronometer**: Enforces nocturnal manifestation windows (`00:00–05:00` default) validated in 15-minute increments, with interactive 24-hour timeline scrubbing and a Curfew Time-Gate override.
- **Resource Footprint Profiler (`profiler.rs`)**: Continually verifies strict operational budgets (`< 0.5%` CPU utilization, `< 30.0 MB` resident memory, and `< 15.0ms` p99 dispatch latency).

### 4. Hardware Exorcise Failsafe Circuit (`src-tauri/src/panic/mod.rs`)
- **Triple-`[ESC]` Hardware Interrupt (`< 800ms`)**: Lock-free 3-slot monotonic ringbuffer that filters strictly for the `[ESC]` keycode (`0x1B` / `0x35`), discarding all other keystrokes immediately for privacy.
- **Sub-`50ms` Atomic Emergency Purge**: Depressing `[ESC]` 3 times under `800ms` (or clicking **MANUAL EXORCISE PURGE CIRCUIT**) immediately severs the `cpal` audio ringbuffer, restores unperturbed OS hardware monitor gamma LUTs, clears viewport chromatic filters, logs the incident, and forces the daemon into `NonHookStasis` deep sleep.
- **Crash & Signal Safety**: Installs `std::panic::set_hook` and POSIX signal handlers (`SIGINT`, `SIGTERM`, `SIGHUP`) to guarantee hardware gamma LUT restoration on unexpected termination.

### 5. Séance Panel UI, Tauri v2 IPC Bridge & System Tray (`src/`, `src-tauri/src/ipc/mod.rs`, `src-tauri/src/tray/mod.rs`)
- **Real-Time 150ms CRT Sweep Loop**: Streams `TelemetrySnapshot` payloads over Tauri v2 events (`telemetry:sweep`) to drive the 64-point phosphor-green **CRT EVP Spectrograph** (with `20+ mG` redline horizontal tear glitches), K-II LEDs, thermocouple gauges, and incident audit log.
- **Full Interactive Console Controls**: Wired to 14 strongly typed Tauri v2 IPC commands (`get_telemetry_snapshot`, `get_profile_config`, `get_recent_incident_logs`, `set_vigil_bracket`, `toggle_curfew_mode`, `trigger_calibration_pulse`, `set_surveillance_bound`, `toggle_intervention_channel`, `set_intensity_detent`, `set_cognitive_lull_window`, `trigger_manual_exorcise`, `record_escape_key_press`, `hide_seance_panel`, `quit_application`).
- **Headless Tray & Ergonomic Windowing**: Runs under macOS `ActivationPolicy::Accessory` with system tray menu controls (`1h` / `4h` / `8h` dormant timers, immediate Exorcise, clean Quit) and a centered `1240×820` frameless window with isolated close (`×` `Cmd+Q`) and hide-to-tray (`−` `Cmd+W`) controls.

### 6. Local SQLite WAL Persistence & Zero-Egress Supply-Chain Security (`src-tauri/src/db/`, `src-tauri/src/security/`)
- **Embedded SQLite Store (`nocturne_telemetry.db`)**: Configured with `PRAGMA journal_mode=WAL`, `STRICT` schema validation, `0700`/`0600` Unix file permissions, transactional profile upserts, and a 10,000-row rolling audit cap.
- **Zero-Egress Air-Gap Enforcement**:
  - Compile-time and runtime verification via `AirGapEnforcer` and POSIX `AF_INET`/`AF_INET6` open-descriptor scrubbing.
  - Strict WebView Content Security Policy (`default-src 'self'; connect-src 'none'`), macOS Seatbelt sandbox profile ([`nocturne-airgap.sb`](file:///Volumes/External/boredpolymath/projects/nocturne/src-tauri/nocturne-airgap.sb)), and zero-network [`Entitlements.plist`](file:///Volumes/External/boredpolymath/projects/nocturne/src-tauri/Entitlements.plist).
  - Automated supply-chain CI ([`.github/workflows/release-security.yml`](file:///Volumes/External/boredpolymath/projects/nocturne/.github/workflows/release-security.yml)), `cargo-deny` v2 license & source policy ([`deny.toml`](file:///Volumes/External/boredpolymath/projects/nocturne/deny.toml)), CycloneDX 1.5 SBOM ([`sbom.cdx.json`](file:///Volumes/External/boredpolymath/projects/nocturne/sbom.cdx.json)), and reproducible artifact checksums ([`SHA256SUMS.txt`](file:///Volumes/External/boredpolymath/projects/nocturne/SHA256SUMS.txt)).

---

## Repository Structure

- [`src/`](file:///Volumes/External/boredpolymath/projects/nocturne/src) — **Séance Panel Presentation Layer** (HTML5, CSS3 scanline & CRT styling, Canvas EVP Spectrograph)
  - [`src/index.html`](file:///Volumes/External/boredpolymath/projects/nocturne/src/index.html) — Main telemetry & control console markup
  - [`src/styles/main.css`](file:///Volumes/External/boredpolymath/projects/nocturne/src/styles/main.css) — Monochromatic occult telemetry & rackmount hardware stylesheet
  - [`src/js/app.js`](file:///Volumes/External/boredpolymath/projects/nocturne/src/js/app.js) — CRT EVP Spectrograph canvas renderer, full-screen spectral filter sync & Tauri v2 IPC bindings
- [`src-tauri/`](file:///Volumes/External/boredpolymath/projects/nocturne/src-tauri) — **Rust Core Daemon & Telemetry Engine**
  - [`src-tauri/src/lib.rs`](file:///Volumes/External/boredpolymath/projects/nocturne/src-tauri/src/lib.rs) & [`main.rs`](file:///Volumes/External/boredpolymath/projects/nocturne/src-tauri/src/main.rs) — Daemon entry point, 150ms sweep broadcaster & Tauri builder
  - [`src-tauri/src/ipc/mod.rs`](file:///Volumes/External/boredpolymath/projects/nocturne/src-tauri/src/ipc/mod.rs) — Strongly typed Tauri v2 IPC command handlers & manifestation rotation dispatcher
  - [`src-tauri/src/telemetry/mod.rs`](file:///Volumes/External/boredpolymath/projects/nocturne/src-tauri/src/telemetry/mod.rs) & [`profiler.rs`](file:///Volumes/External/boredpolymath/projects/nocturne/src-tauri/src/telemetry/profiler.rs) — Non-interceptive OS idle hooks, vigil chronometer, REM-Pod EMF calculator & resource profiler
  - [`src-tauri/src/audio/mod.rs`](file:///Volumes/External/boredpolymath/projects/nocturne/src-tauri/src/audio/mod.rs) — `cpal` lock-free stereo ringbuffer & 6-mode procedural acoustic synthesis engine
  - [`src-tauri/src/display/mod.rs`](file:///Volumes/External/boredpolymath/projects/nocturne/src-tauri/src/display/mod.rs) — Native OS display gamma LUT controller, full-screen spectral color-shift & Lissajous planchette cursor drift
  - [`src-tauri/src/panic/mod.rs`](file:///Volumes/External/boredpolymath/projects/nocturne/src-tauri/src/panic/mod.rs) — Hardware Exorcise failsafe circuit (`[ESC]` 3× under `800ms`) & signal restoration hooks
  - [`src-tauri/src/db/mod.rs`](file:///Volumes/External/boredpolymath/projects/nocturne/src-tauri/src/db/mod.rs) — Embedded SQLite (`WAL` mode) local persistence store with 10,000-row rolling retention
  - [`src-tauri/src/tray/mod.rs`](file:///Volumes/External/boredpolymath/projects/nocturne/src-tauri/src/tray/mod.rs) — Headless system tray lifecycle manager & dormant timer controller
  - [`src-tauri/src/security/mod.rs`](file:///Volumes/External/boredpolymath/projects/nocturne/src-tauri/src/security/mod.rs) — Zero-egress air-gap auditor & POSIX network socket scrubber
- [`scripts/generate_release_integrity.py`](file:///Volumes/External/boredpolymath/projects/nocturne/scripts/generate_release_integrity.py) — Deterministic CycloneDX 1.5 SBOM & `SHA256SUMS.txt` generator/verifier

---

## Development, Testing & Verification

```bash
# Run the full Rust unit & integration test suite (54 tests across audio, display, db, panic, security, telemetry, tray)
cargo test --workspace

# Enforce zero Clippy warnings and strict Rustfmt formatting
npm run lint:rust
npm run fmt:rust

# Verify deterministic CycloneDX SBOM & SHA-256 release integrity manifests
python3 scripts/generate_release_integrity.py --verify

# Launch the Nocturne MK-IV daemon & Séance Panel in development mode
npm run dev
```
