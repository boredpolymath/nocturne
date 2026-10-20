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
use std::time::Duration;

use tauri::Emitter;

pub mod audio;
pub mod db;
pub mod display;
pub mod ipc;
pub mod panic;
pub mod security;
pub mod telemetry;
pub mod tray;

pub fn run() {
    // Enforce Zero-Egress Air-Gap at process startup before initializing subsystems
    let airgap_enforcer = Arc::new(security::AirGapEnforcer::new());
    let _ = airgap_enforcer.enforce_process_airgap();

    let persistence_store = Arc::new(
        db::LocalPersistenceStore::init_wal_database()
            .expect("failed to initialize local SQLite WAL persistence database"),
    );
    let telemetry_engine = Arc::new(telemetry::TelemetryEngine::new());
    let acoustic_engine = Arc::new(audio::CpalAcousticEngine::new());
    let display_controller = Arc::new(display::DisplayGammaController::new());
    let exorcise_circuit = Arc::new(panic::ExorciseCircuit::new());
    let tray_manager = Arc::new(tray::SystemTrayManager::new());

    // Register crash & termination signal hooks to guarantee clean display restoration
    panic::ExorciseCircuit::install_panic_and_signal_hooks(
        Arc::clone(&acoustic_engine),
        Arc::clone(&display_controller),
    );

    // Spawn global non-interceptive Escape triple-tap (<800ms) hardware failsafe listener
    exorcise_circuit.spawn_hardware_esc_listener(
        Arc::clone(&persistence_store),
        Arc::clone(&telemetry_engine),
        Arc::clone(&acoustic_engine),
        Arc::clone(&display_controller),
    );

    let store_setup = Arc::clone(&persistence_store);
    let telemetry_setup = Arc::clone(&telemetry_engine);
    let audio_setup = Arc::clone(&acoustic_engine);
    let display_setup = Arc::clone(&display_controller);
    let exorcise_setup = Arc::clone(&exorcise_circuit);
    let tray_setup = Arc::clone(&tray_manager);

    tauri::Builder::default()
        .manage(persistence_store)
        .manage(telemetry_engine)
        .manage(acoustic_engine)
        .manage(display_controller)
        .manage(exorcise_circuit)
        .manage(tray_manager)
        .setup(move |app| {
            // Initialize headless system tray lifecycle & context menu
            tray::SystemTrayManager::initialize_tray(
                app,
                Arc::clone(&tray_setup),
                Arc::clone(&store_setup),
                Arc::clone(&telemetry_setup),
                Arc::clone(&audio_setup),
                Arc::clone(&display_setup),
                Arc::clone(&exorcise_setup),
            )
            .map_err(std::io::Error::other)?;

            // Spawn 150ms SWEEP telemetry & spectrograph emitter for the Séance Panel
            let app_handle = app.handle().clone();
            std::thread::Builder::new()
                .name("nocturne-150ms-sweep".to_string())
                .spawn(move || {
                    let mut phase_rad: f32 = 0.35;
                    let interval = Duration::from_millis(150);
                    loop {
                        let work_start = std::time::Instant::now();
                        let snap = ipc::build_telemetry_snapshot(
                            &store_setup,
                            &telemetry_setup,
                            &audio_setup,
                            &display_setup,
                            &exorcise_setup,
                            &tray_setup,
                            phase_rad,
                        );
                        let _ = app_handle.emit("nocturne://telemetry-sweep", &snap);
                        telemetry_setup
                            .profiler()
                            .record_cycle_cpu_work(work_start.elapsed(), interval);
                        phase_rad = (phase_rad + 0.18) % std::f32::consts::TAU;
                        std::thread::sleep(interval);
                    }
                })
                .ok();

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::get_telemetry_snapshot,
            ipc::get_profile_config,
            ipc::get_recent_incident_logs,
            ipc::set_vigil_bracket,
            ipc::toggle_curfew_mode,
            ipc::trigger_calibration_pulse,
            ipc::set_surveillance_bound,
            ipc::toggle_intervention_channel,
            ipc::set_intensity_detent,
            ipc::set_cognitive_lull_window,
            ipc::trigger_manual_exorcise,
            ipc::record_escape_key_press,
            ipc::hide_seance_panel,
            ipc::quit_application,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Nocturne daemon");
}
