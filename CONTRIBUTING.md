# Contributing to Project Nocturne (`MK-IV`)

Welcome! Thank you for your interest in contributing to **Project Nocturne**. Whether you're crafting a new procedural acoustic synthesis recipe, refining low-level display/input FFI bindings, polishing the retro CRT Séance Panel, or improving documentation, we're thrilled to have you here.

---

## Core Design & Safety Invariants

Because Nocturne runs in the background, monitors idle state, and manipulates hardware audio and monitor color tables, every contribution **must** uphold four non-negotiable invariants:

1. **Zero Network Egress (100% Air-Gapped)**
   - Nocturne must never open network sockets, make HTTP requests, load remote fonts/assets, or pull in networking crates (`reqwest`, `hyper`, `curl`, `rustls`, `openssl`, etc.).
   - All network crates are explicitly banned in [`deny.toml`](deny.toml) and verified in CI across all desktop targets.
2. **Strict Input Privacy (Zero Keylogging)**
   - Never capture, buffer, or log arbitrary keystrokes or clipboard contents.
   - Idle detection must rely strictly on aggregate OS idle timers (`CGEventSourceSecondsSinceLastEventType`, `GetLastInputInfo`, `XScreenSaverQueryInfo`).
   - The triple-`ESC` failsafe circuit ([`src-tauri/src/panic/mod.rs`](src-tauri/src/panic/mod.rs)) filters exclusively for `0x1B` (`ESC`) timestamps and discards everything else unread.
3. **Bounded Sensory & Hardware Perturbations**
   - **Audio**: All synthesized waveforms in [`src-tauri/src/audio/mod.rs`](src-tauri/src/audio/mod.rs) must pass through the psychoacoustic limiter (`+6 dB` to `+18 dB` over the `-44.2 dBFS` ambient floor)—no sudden ear-splitting spikes.
   - **Display Gamma**: Hardware LUT changes in [`src-tauri/src/display/mod.rs`](src-tauri/src/display/mod.rs) must remain clamped within `<= 8.0%` luminance delta and ramp smoothly over `>= 1,200ms`.
   - **Failsafe Restoration**: Every hardware state modification must be immediately reversible by the triple-`ESC` Exorcise circuit (`< 50ms`) and cleaned up on process exit or panic.
4. **Featherweight Resource Budget**
   - Keep background CPU usage `< 0.5%`, resident memory `< 30 MB`, and p99 IPC dispatch latency `< 15.0ms`.

---

## Setting Up Your Development Environment

### 1. Prerequisites
- **Rust** (`1.77.2+` via `rustup` — automatically selected by [`rust-toolchain.toml`](rust-toolchain.toml))
- **Node.js** (`18+` for `@tauri-apps/cli`)
- **Python 3** (for the reproducible SBOM & SHA-256 integrity script)
- **Platform System Libraries**:
  - **macOS**: Xcode Command Line Tools (`xcode-select --install`)
  - **Windows**: Microsoft Visual Studio C++ Build Tools & WebView2 Runtime
  - **Linux (Ubuntu/Debian)**:
    ```bash
    sudo apt-get update
    sudo apt-get install -y build-essential pkg-config libssl-dev libwebkit2gtk-4.1-dev \
      libayatana-appindicator3-dev librsvg2-dev patchelf libasound2-dev libxss-dev libxrandr-dev
    ```

### 2. Clone, Test & Run
```bash
git clone https://github.com/boredpolymath/nocturne.git
cd nocturne
npm install

# Run the Rust test suite
cargo test --workspace

# Start the Tauri v2 development server & Séance Panel
npm run dev
```

---

## How to Add a New Procedural Manifestation

One of the most rewarding ways to contribute is adding a new real-time procedural sound synthesis routine:

1. **Synthesize the Waveform ([`src-tauri/src/audio/mod.rs`](src-tauri/src/audio/mod.rs))**:
   - Write a pure synthesis function `pub fn synthesize_<name>(sample_rate: u32, ...) -> Vec<(f32, f32)>` that generates stereo `(left, right)` PCM frames mathematically (using oscillators, formant filters, exponential decay envelopes, or ITD/ILD binaural delays).
   - Add a `dispatch_<name>(&self, ...)` method on `AudioEngine` that applies the psychoacoustic limiter and pushes samples into the lock-free `StereoRingBuffer`.
2. **Wire Up the Dispatcher ([`src-tauri/src/ipc/mod.rs`](src-tauri/src/ipc/mod.rs))**:
   - Register your new manifestation in the rotation cycle inside `trigger_calibration_pulse` and the cognitive lull dispatcher.
3. **Add Unit Tests**:
   - Add a test in `src-tauri/src/audio/mod.rs` verifying frame count, stereo non-zero energy, and peak amplitude clamping.

---

## Pre-Submit Checklist

Before opening a Pull Request, run the full local verification suite (matching our GitHub Actions CI):

```bash
# 1. Format all Rust code
cargo fmt --all

# 2. Run Clippy with zero warnings
npm run lint:rust

# 3. Run all unit & integration tests
cargo test --workspace

# 4. Regenerate & verify SHA256SUMS.txt and sbom.cdx.json if you modified tracked files
python3 scripts/generate_release_integrity.py
python3 scripts/generate_release_integrity.py --verify
```

> **Note on `SHA256SUMS.txt` & `sbom.cdx.json`**: Our CI pipeline verifies deterministic checksums for core source files via `python3 scripts/generate_release_integrity.py --verify`. Always run `python3 scripts/generate_release_integrity.py` and commit the updated `SHA256SUMS.txt` and `sbom.cdx.json` whenever you edit source files under `src/` or `src-tauri/`.

---

## Submitting a Pull Request

1. Fork the repository and create a descriptive feature branch (`git checkout -b feat/geiger-counter-synth`).
2. Follow [Conventional Commits](https://www.conventionalcommits.org/) for commit messages (`feat(audio): ...`, `fix(display): ...`, `docs: ...`).
3. Open a Pull Request against `main` and fill out the PR template.

By contributing to Project Nocturne, you agree that your contributions will be dual-licensed under the **MIT** and **Apache-2.0** licenses, and that you will abide by our [Code of Conduct](CODE_OF_CONDUCT.md).
