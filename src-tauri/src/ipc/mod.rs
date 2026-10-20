//! Strongly typed Tauri IPC commands and event payloads for the Séance Panel.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

static MANIFESTATION_ROTATION: AtomicU64 = AtomicU64::new(0);
static REDLINE_GLITCH_UNTIL_MS: AtomicU64 = AtomicU64::new(0);

use crate::audio::CpalAcousticEngine;
use crate::db::{IncidentAuditRecord, LocalPersistenceStore, NewIncidentRecord, ProfileConfig};
use crate::display::DisplayGammaController;
use crate::panic::{
    ExorciseCircuit, ExorciseCircuitStatus, ExorciseTriggerSource, WINDOWS_VK_ESCAPE,
};
use crate::security::{AirGapAuditReport, AirGapEnforcer};
use crate::telemetry::{
    CadenceConfig, DaemonOperationalState, ResourceBudgetReport, TelemetryEngine, VigilWindow,
};
use crate::tray::SystemTrayManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetrySnapshot {
    pub milligauss: f32,
    pub drift_mg: f32,
    pub rem_pod_active: bool,
    pub rem_pod_tier: String,
    pub led_states: [bool; 5],
    pub operational_state: DaemonOperationalState,
    pub current_idle_ms: u64,
    pub in_active_cadence: bool,
    pub lull_window_triggered: bool,
    pub phantom_keystroke_ready: bool,
    pub station_time: String,
    pub curfew_engaged: bool,
    pub vigil_start: String,
    pub vigil_end: String,
    pub intensity_detent: u8,
    pub intensity_detent_label: String,
    pub cognitive_lull_window_sec: f32,
    pub channel_acoustics: bool,
    pub channel_gamma: bool,
    pub channel_thermal: bool,
    pub last_lull_sec: f32,
    pub gamma_drift_pct: f32,
    pub next_probability_window: String,
    pub cold_spot_celsius: f32,
    pub cold_spot_dip_celsius: f32,
    pub cpu_tctl_celsius: f32,
    pub sympathy_pitch_factor: f32,
    pub gamma_modulation_pct: f32,
    pub acoustic_level_dbfs: f32,
    pub acoustic_delta_mv: f32,
    pub ring_sentinel_hex: String,
    pub spectrograph_sweep: Vec<f32>,
    pub recent_logs: Vec<IncidentAuditRecord>,
    pub exorcise_status: ExorciseCircuitStatus,
    pub resource_budgets: ResourceBudgetReport,
    pub airgap_audit: AirGapAuditReport,
    pub daemon_mem_mb: f32,
    pub pid: u32,
    pub redline_glitch_active: bool,
    pub color_shift_mode: u32,
    pub color_shift_intensity: f32,
}

pub fn build_telemetry_snapshot(
    store: &LocalPersistenceStore,
    telemetry: &TelemetryEngine,
    audio: &CpalAcousticEngine,
    display: &DisplayGammaController,
    exorcise: &ExorciseCircuit,
    tray: &SystemTrayManager,
    phase_rad: f32,
) -> TelemetrySnapshot {
    let profile = store.load_profile_config().unwrap_or_default();
    let label = profile.intensity_detent_label().to_string();
    let is_dormant = exorcise.is_dormant_deep_sleep() || tray.is_currently_dormant();

    // Sample hardware thermocouple & optical gauges first so sympathy pitch factor is fresh
    let mut gauges = display.sample_thermocouple_and_optical_gauges(profile.channel_thermal);

    audio.set_peak_cap_db(profile.peak_cap_db);
    audio.set_sympathy_pitch_factor(gauges.sympathy_pitch_factor);
    audio.set_armed(profile.channel_acoustics && !is_dormant);
    display.set_enabled(profile.channel_gamma && !is_dormant);

    let vigil_window = VigilWindow::from_hhmm(
        &profile.vigil_start,
        &profile.vigil_end,
        profile.curfew_engaged,
    )
    .unwrap_or_default();

    let (minute_of_day, station_time) = telemetry.query_local_station_time();
    let operational_state = if is_dormant {
        DaemonOperationalState::NonHookStasis
    } else {
        telemetry.evaluate_operational_state(&vigil_window, minute_of_day)
    };

    let idle_ms = telemetry.query_system_idle_ms();
    let cadence_cfg = CadenceConfig {
        optimal_lull_sec: profile.cognitive_lull_window_sec,
        ..CadenceConfig::default()
    };

    let cadence_eval = telemetry.analyze_cadence_sample(idle_ms, &cadence_cfg, operational_state);

    let short_ts = station_time
        .split_whitespace()
        .next()
        .unwrap_or(&station_time)
        .to_string();

    let now_mono_ms = exorcise.elapsed_monotonic_ms();

    // Dispatch procedural interventions during armed cognitive pauses
    if operational_state == DaemonOperationalState::BoundAndListening {
        let mut dispatched_category: Option<(String, f32)> = None;

        if profile.channel_acoustics && cadence_eval.phantom_keystroke_ready {
            let t0 = std::time::Instant::now();
            let _ = audio.dispatch_phantom_keystroke_echo(0.35);
            telemetry.profiler().record_dispatch_latency(t0.elapsed());

            let _ = store.record_incident(&NewIncidentRecord {
                timestamp_utc: short_ts.clone(),
                trigger_category: "PHANTOM_KEYSTROKE_ECHO: 2.5s HALT".to_string(),
                intensity_detent: profile.intensity_detent,
                milligauss_reading: 3.30,
                lull_duration_ms: cadence_eval.current_idle_ms as i64,
                cpu_temp_c: gauges.cold_spot_celsius,
                gamma_delta_pct: gauges.gamma_drift_pct,
                suppressed_flag: false,
            });
        }

        if cadence_eval.lull_window_triggered {
            let mode = MANIFESTATION_ROTATION.fetch_add(1, Ordering::Relaxed) % 4;
            let cpu_load_norm = ((gauges.cpu_tctl_celsius - 40.0) / 45.0).clamp(0.25, 1.0);

            if profile.channel_acoustics {
                let t0 = std::time::Instant::now();
                match mode {
                    0 => {
                        let _ = audio.dispatch_spatial_creak(-0.55, profile.intensity_detent);
                        let _ = audio.dispatch_room_breath(cpu_load_norm);
                        dispatched_category = Some((
                            format!(
                                "FLOORBOARD_CREAK & ROOM_BREATH: {:.1}s",
                                cadence_eval.last_lull_sec
                            ),
                            8.4,
                        ));
                    }
                    1 => {
                        let _ = audio.dispatch_evp_whisper(0.65, profile.intensity_detent);
                        let _ = audio.dispatch_room_breath(cpu_load_norm);
                        dispatched_category = Some((
                            format!(
                                "EVP_SPIRIT_BOX_WHISPER: {:.1}s LULL",
                                cadence_eval.last_lull_sec
                            ),
                            12.6,
                        ));
                    }
                    2 => {
                        let _ = audio.dispatch_wall_knock(-0.78, profile.intensity_detent);
                        dispatched_category = Some((
                            format!(
                                "WALL_STUD_DOUBLE_KNOCK: {:.1}s LULL",
                                cadence_eval.last_lull_sec
                            ),
                            15.2,
                        ));
                    }
                    _ => {
                        let _ = audio.dispatch_redline_hum();
                        let _ = audio.dispatch_evp_whisper(-0.60, profile.intensity_detent);
                        REDLINE_GLITCH_UNTIL_MS.store(now_mono_ms + 1_350, Ordering::Release);
                        dispatched_category =
                            Some(("REDLINE_20MG_SPIKE & 60HZ_HUM".to_string(), 23.4));
                    }
                }
                telemetry.profiler().record_dispatch_latency(t0.elapsed());
            }

            if profile.channel_gamma {
                display.trigger_bezier_modulation_cycle();
                // Apply subtle 1-3px Planchette Cursor Micro-Drift on lull trigger
                let _ = display.nudge_planchette_cursor(phase_rad);
            }

            let suppressed = !profile.channel_acoustics && !profile.channel_gamma;
            let (cat_label, mg_val) = dispatched_category.unwrap_or_else(|| {
                (
                    format!(
                        "COLD_SPOT_BLUE_SHIFT & PLANCHETTE: {:.1}s",
                        cadence_eval.last_lull_sec
                    ),
                    8.0,
                )
            });

            let _ = store.record_incident(&NewIncidentRecord {
                timestamp_utc: short_ts,
                trigger_category: cat_label,
                intensity_detent: profile.intensity_detent,
                milligauss_reading: mg_val,
                lull_duration_ms: cadence_eval.current_idle_ms as i64,
                cpu_temp_c: gauges.cold_spot_celsius,
                gamma_delta_pct: if profile.channel_gamma {
                    display.configured_modulation_pct()
                } else {
                    0.0
                },
                suppressed_flag: suppressed,
            });
        }
    }

    if !is_dormant && profile.channel_gamma {
        // Advance progressive 1,200ms cubic-bezier Full-Screen Spectral Color-Shift on each 150ms sweep tick
        gauges.gamma_drift_pct = display.tick_bezier_modulation();
    } else if display.active_drift_pct() > 0.0 {
        display.restore_default_gamma();
        gauges.gamma_drift_pct = 0.0;
    }

    let mut emf =
        telemetry.compute_emf_telemetry(&cadence_eval, profile.intensity_detent, operational_state);
    let redline_glitch_active = !is_dormant
        && (now_mono_ms < REDLINE_GLITCH_UNTIL_MS.load(Ordering::Acquire)
            || emf.milligauss >= 20.0);
    if redline_glitch_active && emf.milligauss < 22.4 {
        emf.milligauss = 22.40;
        emf.drift_mg = 6.85;
        emf.led_states = [true, true, true, true, true];
    }

    let next_probability_window = telemetry.compute_next_probability_window(
        &cadence_eval,
        &cadence_cfg,
        operational_state,
        &vigil_window,
        minute_of_day,
    );
    let audio_snap = audio.snapshot_telemetry();
    let spectrograph_sweep = audio.sample_spectrograph_sweep_frame(phase_rad);
    let recent_logs = store.get_recent_incidents(3).unwrap_or_default();
    let exorcise_status = exorcise.status_snapshot();
    let resource_budgets = telemetry.profiler().evaluate_budgets();
    let airgap_audit = AirGapEnforcer::new().audit_runtime_airgap();
    let color_shift_mode = display.active_color_shift_mode();
    let color_shift_intensity = display.active_color_shift_intensity();

    TelemetrySnapshot {
        milligauss: emf.milligauss,
        drift_mg: emf.drift_mg,
        rem_pod_active: emf.rem_pod_active,
        rem_pod_tier: emf.tier.as_label().to_string(),
        led_states: emf.led_states,
        operational_state,
        current_idle_ms: cadence_eval.current_idle_ms,
        in_active_cadence: cadence_eval.in_active_cadence,
        lull_window_triggered: cadence_eval.lull_window_triggered,
        phantom_keystroke_ready: cadence_eval.phantom_keystroke_ready,
        station_time,
        curfew_engaged: profile.curfew_engaged,
        vigil_start: profile.vigil_start,
        vigil_end: profile.vigil_end,
        intensity_detent: profile.intensity_detent,
        intensity_detent_label: label,
        cognitive_lull_window_sec: profile.cognitive_lull_window_sec,
        channel_acoustics: profile.channel_acoustics,
        channel_gamma: profile.channel_gamma,
        channel_thermal: profile.channel_thermal,
        last_lull_sec: cadence_eval.last_lull_sec,
        gamma_drift_pct: gauges.gamma_drift_pct,
        next_probability_window,
        cold_spot_celsius: gauges.cold_spot_celsius,
        cold_spot_dip_celsius: gauges.cold_spot_dip_celsius,
        cpu_tctl_celsius: gauges.cpu_tctl_celsius,
        sympathy_pitch_factor: gauges.sympathy_pitch_factor,
        gamma_modulation_pct: gauges.gamma_modulation_pct,
        acoustic_level_dbfs: audio_snap.current_level_dbfs,
        acoustic_delta_mv: audio_snap.current_delta_mv,
        ring_sentinel_hex: audio_snap.ring_sentinel_hex,
        spectrograph_sweep,
        recent_logs,
        exorcise_status,
        resource_budgets,
        airgap_audit,
        daemon_mem_mb: gauges.daemon_mem_mb,
        pid: std::process::id(),
        redline_glitch_active,
        color_shift_mode,
        color_shift_intensity,
    }
}

#[tauri::command]
pub async fn get_telemetry_snapshot(
    store: State<'_, Arc<LocalPersistenceStore>>,
    telemetry: State<'_, Arc<TelemetryEngine>>,
    audio: State<'_, Arc<CpalAcousticEngine>>,
    display: State<'_, Arc<DisplayGammaController>>,
    exorcise: State<'_, Arc<ExorciseCircuit>>,
    tray: State<'_, Arc<SystemTrayManager>>,
) -> Result<TelemetrySnapshot, String> {
    let phase_rad = (exorcise.elapsed_monotonic_ms() as f32 / 150.0) * 0.35;
    Ok(build_telemetry_snapshot(
        &store, &telemetry, &audio, &display, &exorcise, &tray, phase_rad,
    ))
}

#[tauri::command]
pub async fn get_profile_config(
    store: State<'_, Arc<LocalPersistenceStore>>,
) -> Result<ProfileConfig, String> {
    store.load_profile_config()
}

#[tauri::command]
pub async fn get_recent_incident_logs(
    store: State<'_, Arc<LocalPersistenceStore>>,
    limit: Option<usize>,
) -> Result<Vec<IncidentAuditRecord>, String> {
    store.get_recent_incidents(limit.unwrap_or(25).min(200))
}

#[tauri::command]
pub async fn set_vigil_bracket(
    store: State<'_, Arc<LocalPersistenceStore>>,
    start: String,
    end: String,
) -> Result<(), String> {
    let mut config = store.load_profile_config()?;
    // Enforce 15-minute increment validation before persisting
    let _ = VigilWindow::from_hhmm(&start, &end, config.curfew_engaged)?;
    config.vigil_start = start;
    config.vigil_end = end;
    store.save_profile_config(&config)
}

#[tauri::command]
pub async fn toggle_curfew_mode(
    store: State<'_, Arc<LocalPersistenceStore>>,
    engaged: bool,
) -> Result<(), String> {
    let mut config = store.load_profile_config()?;
    config.curfew_engaged = engaged;
    store.save_profile_config(&config)
}

#[tauri::command]
pub async fn trigger_calibration_pulse(
    store: State<'_, Arc<LocalPersistenceStore>>,
    telemetry: State<'_, Arc<TelemetryEngine>>,
    audio: State<'_, Arc<CpalAcousticEngine>>,
    display: State<'_, Arc<DisplayGammaController>>,
    exorcise: State<'_, Arc<ExorciseCircuit>>,
    tray: State<'_, Arc<SystemTrayManager>>,
) -> Result<(), String> {
    tray.clear_dormant_timer();
    exorcise.set_dormant_deep_sleep(false);

    let profile = store.load_profile_config().unwrap_or_default();
    let mode = MANIFESTATION_ROTATION.fetch_add(1, Ordering::Relaxed) % 4;
    let now_mono_ms = exorcise.elapsed_monotonic_ms();

    let (trigger_category, milligauss_reading) = match mode {
        0 => ("CAL_SPIRIT_BOX_FORMANT_WHISPER".to_string(), 12.8),
        1 => ("CAL_WALL_STUD_DOUBLE_KNOCK".to_string(), 16.4),
        2 => ("CAL_REDLINE_20MG_SPIKE_HUM".to_string(), 23.8),
        _ => ("CAL_CREAK_AND_ROOM_BREATH".to_string(), 10.4),
    };

    if profile.channel_acoustics {
        audio.set_armed(true);
        let t0 = std::time::Instant::now();
        match mode {
            0 => {
                let _ = audio.dispatch_evp_whisper(0.60, profile.intensity_detent);
                let _ = audio.dispatch_room_breath(0.65);
            }
            1 => {
                let _ = audio.dispatch_wall_knock(-0.75, profile.intensity_detent);
            }
            2 => {
                let _ = audio.dispatch_redline_hum();
                let _ = audio.dispatch_evp_whisper(-0.55, profile.intensity_detent);
                REDLINE_GLITCH_UNTIL_MS.store(now_mono_ms + 1_350, Ordering::Release);
            }
            _ => {
                let _ = audio.dispatch_spatial_creak(-0.45, profile.intensity_detent);
                let _ = audio.dispatch_room_breath(0.55);
            }
        }
        telemetry.profiler().record_dispatch_latency(t0.elapsed());
    }
    if profile.channel_gamma {
        display.set_enabled(true);
        display.trigger_bezier_modulation_cycle();
        let _ = display.nudge_planchette_cursor((now_mono_ms as f32 / 150.0) * 0.35);
    }

    let (_, station_time) = telemetry.query_local_station_time();
    let short_ts = station_time
        .split_whitespace()
        .next()
        .unwrap_or(&station_time)
        .to_string();

    store.record_incident(&NewIncidentRecord {
        timestamp_utc: short_ts,
        trigger_category,
        intensity_detent: profile.intensity_detent,
        milligauss_reading,
        lull_duration_ms: 0,
        cpu_temp_c: 18.2,
        gamma_delta_pct: if profile.channel_gamma {
            display.configured_modulation_pct()
        } else {
            0.0
        },
        suppressed_flag: false,
    })?;

    Ok(())
}

#[tauri::command]
pub async fn set_surveillance_bound(
    store: State<'_, Arc<LocalPersistenceStore>>,
    audio: State<'_, Arc<CpalAcousticEngine>>,
    display: State<'_, Arc<DisplayGammaController>>,
    exorcise: State<'_, Arc<ExorciseCircuit>>,
    tray: State<'_, Arc<SystemTrayManager>>,
    bound: bool,
) -> Result<(), String> {
    if bound {
        tray.clear_dormant_timer();
        exorcise.set_dormant_deep_sleep(false);
        let profile = store.load_profile_config().unwrap_or_default();
        audio.set_armed(profile.channel_acoustics);
        display.set_enabled(profile.channel_gamma);
    } else {
        exorcise.set_dormant_deep_sleep(true);
        audio.sever_ringbuffer();
        display.restore_default_gamma();
    }
    Ok(())
}

#[tauri::command]
pub async fn toggle_intervention_channel(
    store: State<'_, Arc<LocalPersistenceStore>>,
    audio: State<'_, Arc<CpalAcousticEngine>>,
    display: State<'_, Arc<DisplayGammaController>>,
    exorcise: State<'_, Arc<ExorciseCircuit>>,
    tray: State<'_, Arc<SystemTrayManager>>,
    channel_id: String,
    enabled: bool,
) -> Result<(), String> {
    if enabled {
        tray.clear_dormant_timer();
        exorcise.set_dormant_deep_sleep(false);
    }
    let key = match channel_id.as_str() {
        "acoustics" | "channel_acoustics" => {
            audio.set_armed(enabled);
            "channel_acoustics"
        }
        "gamma" | "channel_gamma" => {
            display.set_enabled(enabled);
            "channel_gamma"
        }
        "thermal" | "channel_thermal" => "channel_thermal",
        _ => return Err(format!("Unknown intervention channel_id: {channel_id}")),
    };
    store.set_profile_setting(key, &enabled.to_string())
}

#[tauri::command]
pub async fn set_intensity_detent(
    store: State<'_, Arc<LocalPersistenceStore>>,
    detent_pos: u8,
) -> Result<(), String> {
    let mut config = store.load_profile_config()?;
    config.intensity_detent = detent_pos;
    store.save_profile_config(&config)
}

#[tauri::command]
pub async fn set_cognitive_lull_window(
    store: State<'_, Arc<LocalPersistenceStore>>,
    seconds: f32,
) -> Result<(), String> {
    let mut config = store.load_profile_config()?;
    config.cognitive_lull_window_sec = seconds;
    store.save_profile_config(&config)
}

#[tauri::command]
pub async fn trigger_manual_exorcise(
    store: State<'_, Arc<LocalPersistenceStore>>,
    telemetry: State<'_, Arc<TelemetryEngine>>,
    audio: State<'_, Arc<CpalAcousticEngine>>,
    display: State<'_, Arc<DisplayGammaController>>,
    exorcise: State<'_, Arc<ExorciseCircuit>>,
) -> Result<(), String> {
    exorcise.execute_emergency_purge(
        ExorciseTriggerSource::ManualPurgeCircuit,
        &store,
        &telemetry,
        &audio,
        &display,
    )
}

#[tauri::command]
pub async fn record_escape_key_press(
    store: State<'_, Arc<LocalPersistenceStore>>,
    telemetry: State<'_, Arc<TelemetryEngine>>,
    audio: State<'_, Arc<CpalAcousticEngine>>,
    display: State<'_, Arc<DisplayGammaController>>,
    exorcise: State<'_, Arc<ExorciseCircuit>>,
) -> Result<bool, String> {
    exorcise.process_key_event(WINDOWS_VK_ESCAPE, &store, &telemetry, &audio, &display)
}

#[tauri::command]
pub async fn hide_seance_panel(app_handle: AppHandle) -> Result<(), String> {
    if let Some(win) = app_handle.get_webview_window("seance-panel") {
        win.hide()
            .map_err(|e| format!("Failed to hide seance-panel window: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn quit_application(
    app_handle: AppHandle,
    audio: State<'_, Arc<CpalAcousticEngine>>,
    display: State<'_, Arc<DisplayGammaController>>,
    tray: State<'_, Arc<SystemTrayManager>>,
) -> Result<(), String> {
    tray.mark_terminating();
    audio.sever_ringbuffer();
    display.restore_default_gamma();
    app_handle.exit(0);
    std::process::exit(0);
}
