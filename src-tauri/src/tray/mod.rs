//! Headless System Tray Lifecycle & Context Menu (`src-tauri/src/tray/`).
//!
//! Section 4.1 Native Desktop & System Integration:
//! - **Headless Default:** Operates without an active dock or taskbar icon during standard
//!   runtime (`ActivationPolicy::Accessory` on macOS, close-to-tray interception on `seance-panel`).
//! - **Tray Context Controls:**
//!   1. *Invoke Séance Panel* (`invoke-seance-panel`)
//!   2. *Exorcise / Dormant Mode Toggle (1h / 4h / 8h / Resume)* (`dormant-1h`, `dormant-4h`, `dormant-8h`, `dormant-resume`)
//!   3. *Emergency Calibration Reset* (`emergency-calibration-reset`)
//!   4. *Terminate Application* (`terminate-application`)

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, LogicalSize, Manager, Size, WindowEvent};

use crate::audio::CpalAcousticEngine;
use crate::db::{LocalPersistenceStore, NewIncidentRecord};
use crate::display::DisplayGammaController;
use crate::panic::{ExorciseCircuit, ExorciseTriggerSource};
use crate::telemetry::TelemetryEngine;

pub const MENU_ID_INVOKE_SEANCE: &str = "invoke-seance-panel";
pub const MENU_ID_DORMANT_1H: &str = "dormant-1h";
pub const MENU_ID_DORMANT_4H: &str = "dormant-4h";
pub const MENU_ID_DORMANT_8H: &str = "dormant-8h";
pub const MENU_ID_DORMANT_RESUME: &str = "dormant-resume";
pub const MENU_ID_EMERGENCY_RESET: &str = "emergency-calibration-reset";
pub const MENU_ID_TERMINATE: &str = "terminate-application";

fn current_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Manages headless system tray state and timed Dormant / Exorcise suspensions (1h, 4h, 8h).
pub struct SystemTrayManager {
    dormant_until_epoch_sec: AtomicU64,
    terminating: AtomicBool,
}

impl Default for SystemTrayManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemTrayManager {
    pub fn new() -> Self {
        Self {
            dormant_until_epoch_sec: AtomicU64::new(0),
            terminating: AtomicBool::new(false),
        }
    }

    /// Marks the daemon as terminating so window close-to-tray interception does not block exit.
    pub fn mark_terminating(&self) {
        self.terminating.store(true, Ordering::SeqCst);
    }

    /// Returns whether the daemon is currently in the process of terminating.
    pub fn is_terminating(&self) -> bool {
        self.terminating.load(Ordering::Acquire)
    }

    /// Suspends all triggers for `hours` (`1`, `4`, or `8` hours) from `now_epoch_sec`.
    pub fn engage_dormant_for_hours(&self, hours: u64, now_epoch_sec: u64) -> u64 {
        let clamped_hours = match hours {
            1 | 4 | 8 => hours,
            _ => hours.clamp(1, 8),
        };
        let until = now_epoch_sec.saturating_add(clamped_hours * 3_600);
        self.dormant_until_epoch_sec.store(until, Ordering::SeqCst);
        until
    }

    /// Clears any active timed dormant suspension.
    pub fn clear_dormant_timer(&self) {
        self.dormant_until_epoch_sec.store(0, Ordering::SeqCst);
    }

    /// Evaluates whether the daemon is currently suspended by a 1h/4h/8h tray dormant timer.
    pub fn is_timed_dormant_active(&self, now_epoch_sec: u64) -> bool {
        let until = self.dormant_until_epoch_sec.load(Ordering::Acquire);
        until > now_epoch_sec
    }

    /// Queries whether the timed dormant suspension is currently active using the system clock.
    pub fn is_currently_dormant(&self) -> bool {
        self.is_timed_dormant_active(current_epoch_secs())
    }

    /// Remaining seconds in the active dormant window (`0` if not dormant).
    pub fn remaining_dormant_secs(&self) -> u64 {
        let now = current_epoch_secs();
        let until = self.dormant_until_epoch_sec.load(Ordering::Acquire);
        until.saturating_sub(now)
    }

    /// Configures headless runtime mode, builds the system notification tray context menu,
    /// and intercepts window close events so the daemon stays resident in the tray.
    pub fn initialize_tray(
        app: &mut tauri::App,
        tray_manager: Arc<Self>,
        store: Arc<LocalPersistenceStore>,
        telemetry: Arc<TelemetryEngine>,
        audio: Arc<CpalAcousticEngine>,
        display: Arc<DisplayGammaController>,
        exorcise: Arc<ExorciseCircuit>,
    ) -> Result<(), String> {
        // Headless Default (Section 4.1): Hide dock icon on macOS during standard runtime
        #[cfg(target_os = "macos")]
        {
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
        }

        // Intercept close request on `seance-panel` so closing the window hides it to the tray,
        // and clamp initial window dimensions to the active monitor height so the bottom is never off-screen.
        if let Some(window) = app.get_webview_window("seance-panel") {
            let max_logical_height = window
                .current_monitor()
                .ok()
                .flatten()
                .map(|m| {
                    let scale = m.scale_factor().max(1.0);
                    let screen_h = m.size().height as f64 / scale;
                    (screen_h * 0.85).clamp(620.0, 860.0)
                })
                .unwrap_or(820.0);

            let _ = window.set_size(Size::Logical(LogicalSize::new(1240.0, max_logical_height)));
            let _ = window.center();

            let win_clone = window.clone();
            let tray_for_close = Arc::clone(&tray_manager);
            window.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    if !tray_for_close.is_terminating() {
                        api.prevent_close();
                        let _ = win_clone.hide();
                    }
                }
            });
        }

        let handle = app.handle();
        let invoke_item = MenuItemBuilder::with_id(
            MENU_ID_INVOKE_SEANCE,
            "Invoke Séance Panel (Configuration Console)",
        )
        .build(handle)
        .map_err(|e| format!("Failed to build invoke menu item: {e}"))?;

        let dormant_1h =
            MenuItemBuilder::with_id(MENU_ID_DORMANT_1H, "Suspend Triggers for 1 Hour")
                .build(handle)
                .map_err(|e| format!("Failed to build 1h dormant item: {e}"))?;
        let dormant_4h =
            MenuItemBuilder::with_id(MENU_ID_DORMANT_4H, "Suspend Triggers for 4 Hours")
                .build(handle)
                .map_err(|e| format!("Failed to build 4h dormant item: {e}"))?;
        let dormant_8h =
            MenuItemBuilder::with_id(MENU_ID_DORMANT_8H, "Suspend Triggers for 8 Hours")
                .build(handle)
                .map_err(|e| format!("Failed to build 8h dormant item: {e}"))?;
        let dormant_resume =
            MenuItemBuilder::with_id(MENU_ID_DORMANT_RESUME, "Resume Nocturnal Surveillance")
                .build(handle)
                .map_err(|e| format!("Failed to build resume item: {e}"))?;

        let dormant_submenu = SubmenuBuilder::new(handle, "Exorcise / Dormant Mode")
            .item(&dormant_1h)
            .item(&dormant_4h)
            .item(&dormant_8h)
            .separator()
            .item(&dormant_resume)
            .build()
            .map_err(|e| format!("Failed to build dormant submenu: {e}"))?;

        let reset_item = MenuItemBuilder::with_id(
            MENU_ID_EMERGENCY_RESET,
            "Emergency Calibration Reset (Audio & Gamma)",
        )
        .build(handle)
        .map_err(|e| format!("Failed to build reset menu item: {e}"))?;

        let terminate_item =
            MenuItemBuilder::with_id(MENU_ID_TERMINATE, "Terminate Nocturne Daemon")
                .build(handle)
                .map_err(|e| format!("Failed to build terminate menu item: {e}"))?;

        let menu = MenuBuilder::new(handle)
            .item(&invoke_item)
            .separator()
            .item(&dormant_submenu)
            .item(&reset_item)
            .separator()
            .item(&terminate_item)
            .build()
            .map_err(|e| format!("Failed to build tray context menu: {e}"))?;

        let mut tray_builder = TrayIconBuilder::with_id("nocturne-tray")
            .menu(&menu)
            .show_menu_on_left_click(true)
            .tooltip("NOCTURNE MK-IV // PARANORMAL TELEMETRY UNIT");

        if let Some(default_icon) = app.default_window_icon().cloned() {
            tray_builder = tray_builder.icon(default_icon);
        }

        tray_builder
            .on_menu_event(move |app_handle: &AppHandle, event| {
                let id = event.id().as_ref();
                match id {
                    MENU_ID_INVOKE_SEANCE => {
                        if let Some(win) = app_handle.get_webview_window("seance-panel") {
                            let _ = win.show();
                            let _ = win.unminimize();
                            let _ = win.set_focus();
                        }
                    }
                    MENU_ID_DORMANT_1H | MENU_ID_DORMANT_4H | MENU_ID_DORMANT_8H => {
                        let hours = match id {
                            MENU_ID_DORMANT_1H => 1,
                            MENU_ID_DORMANT_4H => 4,
                            _ => 8,
                        };
                        tray_manager.engage_dormant_for_hours(hours, current_epoch_secs());
                        audio.sever_ringbuffer();
                        display.restore_default_gamma();
                        exorcise.set_dormant_deep_sleep(true);

                        let (_, station_time) = telemetry.query_local_station_time();
                        let _ = store.record_incident(&NewIncidentRecord {
                            timestamp_utc: station_time,
                            trigger_category: format!("TRAY_DORMANT_{hours}H"),
                            intensity_detent: 1,
                            milligauss_reading: 0.0,
                            lull_duration_ms: 0,
                            cpu_temp_c: 18.2,
                            gamma_delta_pct: 0.0,
                            suppressed_flag: true,
                        });
                    }
                    MENU_ID_DORMANT_RESUME => {
                        tray_manager.clear_dormant_timer();
                        exorcise.set_dormant_deep_sleep(false);
                        audio.set_armed(true);
                        display.set_enabled(true);
                    }
                    MENU_ID_EMERGENCY_RESET => {
                        let _ = exorcise.execute_emergency_purge(
                            ExorciseTriggerSource::ManualPurgeCircuit,
                            &store,
                            &telemetry,
                            &audio,
                            &display,
                        );
                    }
                    MENU_ID_TERMINATE => {
                        tray_manager.mark_terminating();
                        audio.sever_ringbuffer();
                        display.restore_default_gamma();
                        app_handle.exit(0);
                        std::process::exit(0);
                    }
                    _ => {}
                }
            })
            .build(handle)
            .map_err(|e| format!("Failed to register system tray icon: {e}"))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dormant_mode_1h_4h_8h_timers() {
        let tray = SystemTrayManager::new();
        let now = 1_700_000_000;
        assert!(!tray.is_timed_dormant_active(now));

        // Engage 4-hour dormant mode
        let until = tray.engage_dormant_for_hours(4, now);
        assert_eq!(until, now + 4 * 3_600);
        assert!(tray.is_timed_dormant_active(now + 3_600));
        assert!(!tray.is_timed_dormant_active(now + 4 * 3_600 + 1));

        // Clear dormant timer
        tray.clear_dormant_timer();
        assert!(!tray.is_timed_dormant_active(now + 10));
    }
}
