# Project Nocturne (`MODEL NOCTURNE-MK IV`)

**Ambient Workstation Poltergeist Daemon & Paranormal Telemetry Unit (`v1.0.4-RELEASE`)**

Nocturne is a local-first, zero-egress desktop daemon and telemetry interface built on **Tauri v2** and **Rust**. It monitors workstation inactivity, typing cadence lulls (`4.0s–15.0s`), and hardware telemetry during nocturnal vigil brackets (`00:00–05:00`) to orchestrate subtle spatial binaural acoustics (`cpal` direct ringbuffer) and clamped display gamma perturbations (`<= 8.0%` cubic-bezier).

## Repository Structure

- `src/` — **Séance Panel Presentation Layer** (HTML5, CSS3 scanline & CRT styling, Canvas EVP Spectrograph)
  - `src/index.html` — Main telemetry & control console markup
  - `src/styles/main.css` — Monochromatic occult telemetry stylesheet
  - `src/js/app.js` — CRT EVP Spectrograph canvas renderer & skeleton UI bindings
- `src-tauri/` — **Rust Core Engine Skeleton**
  - `src-tauri/src/lib.rs` & `main.rs` — Daemon entry point & Tauri builder
  - `src-tauri/src/ipc/mod.rs` — Strongly typed Tauri IPC command stubs
  - `src-tauri/src/telemetry/mod.rs` — Non-interceptive OS idle hooks, vigil chronometer & cadence analyzer
  - `src-tauri/src/audio/mod.rs` — `cpal` direct ringbuffer spatial acoustic engine
  - `src-tauri/src/display/mod.rs` — Native OS display gamma perturbation controller
  - `src-tauri/src/panic/mod.rs` — Hardware exorcise failsafe circuit (`[ESC]` 3x under `800ms`)
  - `src-tauri/src/db/mod.rs` — Embedded SQLite (WAL mode) local persistence store
  - `src-tauri/src/tray/mod.rs` — Headless system tray lifecycle manager
- `docs/` — Engineering & security checklists (git-ignored via `.gitignore`)
