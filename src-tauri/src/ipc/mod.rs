//! Strongly typed Tauri IPC commands and event payloads for the Séance Panel.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::db::{IncidentAuditRecord, LocalPersistenceStore, NewIncidentRecord, ProfileConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetrySnapshot {
    pub milligauss: f32,
    pub drift_mg: f32,
    pub rem_pod_active: bool,
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
    pub gamma_modulation_pct: f32,
    pub daemon_mem_mb: f32,
    pub pid: u32,
}

#[tauri::command]
pub async fn get_telemetry_snapshot(
    store: State<'_, Arc<LocalPersistenceStore>>,
) -> Result<TelemetrySnapshot, String> {
    let profile = store.load_profile_config().unwrap_or_default();
    let label = profile.intensity_detent_label().to_string();

    Ok(TelemetrySnapshot {
        milligauss: 2.87,
        drift_mg: 0.14,
        rem_pod_active: true,
        station_time: "02:41:18 AM".to_string(),
        curfew_engaged: profile.curfew_engaged,
        vigil_start: profile.vigil_start,
        vigil_end: profile.vigil_end,
        intensity_detent: profile.intensity_detent,
        intensity_detent_label: label,
        cognitive_lull_window_sec: profile.cognitive_lull_window_sec,
        channel_acoustics: profile.channel_acoustics,
        channel_gamma: profile.channel_gamma,
        channel_thermal: profile.channel_thermal,
        last_lull_sec: 6.2,
        gamma_drift_pct: 0.0,
        next_probability_window: "14m 20s".to_string(),
        cold_spot_celsius: 18.2,
        cold_spot_dip_celsius: -4.1,
        gamma_modulation_pct: 7.8,
        daemon_mem_mb: 28.4,
        pid: std::process::id(),
    })
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
    config.vigil_start = start;
    config.vigil_end = end;
    store.save_profile_config(&config)
}

#[tauri::command]
pub async fn toggle_intervention_channel(
    store: State<'_, Arc<LocalPersistenceStore>>,
    channel_id: String,
    enabled: bool,
) -> Result<(), String> {
    let key = match channel_id.as_str() {
        "acoustics" | "channel_acoustics" => "channel_acoustics",
        "gamma" | "channel_gamma" => "channel_gamma",
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
) -> Result<(), String> {
    store.record_incident(&NewIncidentRecord {
        timestamp_utc: "02:41:18".to_string(),
        trigger_category: "MANUAL_EXORCISE_PURGE".to_string(),
        intensity_detent: 1,
        milligauss_reading: 0.0,
        lull_duration_ms: 0,
        cpu_temp_c: 18.2,
        gamma_delta_pct: 0.0,
        suppressed_flag: true,
    })?;
    Ok(())
}
