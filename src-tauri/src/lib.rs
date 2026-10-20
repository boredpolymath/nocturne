//! Project Nocturne (MK-IV) — Core Rust Daemon
//!
//! Decoupled, local-first ambient workstation daemon architecture:
//! - `telemetry`: Non-interceptive OS idle hooks, cadence lull detection, curfew filter, hardware sensors
//! - `audio`: Direct `cpal` ringbuffer engine for spatial binaural acoustics & procedural assets
//! - `display`: Native OS gamma perturbation bindings (<=8.0% clamp, 1200ms cubic-bezier curve)
//! - `panic`: Global hardware exorcise failsafe interrupt (3x ESC < 800ms) & state restoration
//! - `db`: Local SQLite persistence layer running in WAL mode
//! - `tray`: Headless system notification tray lifecycle and context menu
//! - `ipc`: Strongly typed Tauri IPC commands and event emitters for the Séance Panel

use std::sync::Arc;

pub mod audio;
pub mod db;
pub mod display;
pub mod ipc;
pub mod panic;
pub mod telemetry;
pub mod tray;

pub fn run() {
    let persistence_store = Arc::new(
        db::LocalPersistenceStore::init_wal_database()
            .expect("failed to initialize local SQLite WAL persistence database"),
    );

    tauri::Builder::default()
        .manage(persistence_store)
        .invoke_handler(tauri::generate_handler![
            ipc::get_telemetry_snapshot,
            ipc::get_profile_config,
            ipc::get_recent_incident_logs,
            ipc::set_vigil_bracket,
            ipc::toggle_intervention_channel,
            ipc::set_intensity_detent,
            ipc::set_cognitive_lull_window,
            ipc::trigger_manual_exorcise,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Nocturne daemon");
}
