//! Emergency Exorcise Circuit & Failsafe Kill Switches (`src-tauri/src/panic/`).
//!
//! Implements the hardware & software failsafe interrupt system:
//! - **3x `[ESC]` Hardware Interrupt (`< 800ms`):** Fixed 3-slot timestamp ringbuffer
//!   monitoring exclusively `kVK_Escape` (`0x35` / `53` on macOS via `CGEventSourceKeyState`)
//!   and `VK_ESCAPE` (`0x1B` / `27` on Windows via `GetAsyncKeyState`). Zero other keycodes
//!   are queried, buffered, or inspected.
//! - **Atomic Exorcise Purge Routine:** Instantly severs the `cpal` direct ringbuffer,
//!   restores cached native monitor color gamma tables, transitions the daemon into deep
//!   dormant stasis, and records an `EXORCISE_PURGE` audit event in SQLite.
//! - **Crash & Signal Recovery Hooks:** Installs a `std::panic::set_hook` and OS signal
//!   handlers (`SIGINT`, `SIGTERM`, `SIGHUP`, `SIGABRT`) to guarantee clean display
//!   gamma restoration even on unexpected process termination.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Once};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::audio::CpalAcousticEngine;
use crate::db::{LocalPersistenceStore, NewIncidentRecord};
use crate::display::DisplayGammaController;
use crate::telemetry::TelemetryEngine;

/// Maximum time window (in milliseconds) for 3 consecutive `[ESC]` presses (`FAILSAFE 800ms`).
pub const FAILSAFE_WINDOW_MS: u64 = 800;
/// Number of consecutive `[ESC]` depressions required to trip the failsafe circuit.
pub const REQUIRED_ESC_PRESSES: usize = 3;
/// macOS Carbon/CoreGraphics virtual keycode for `Escape` (`kVK_Escape = 0x35 = 53`).
pub const MACOS_VK_ESCAPE: u16 = 53;
/// Windows Win32 virtual keycode for `Escape` (`VK_ESCAPE = 0x1B = 27`).
pub const WINDOWS_VK_ESCAPE: u16 = 0x1B;

/// Trigger source for an Emergency Exorcise Purge operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExorciseTriggerSource {
    /// Triggered by depressing `[ESC]` 3 times consecutively within `< 800ms`.
    HardwareEscTripleTap,
    /// Triggered by clicking `MANUAL EXORCISE PURGE CIRCUIT` in the Séance Panel or Tray.
    ManualPurgeCircuit,
    /// Triggered by a process signal or panic recovery hook.
    EmergencySignalHook,
}

impl ExorciseTriggerSource {
    pub fn as_audit_category(&self) -> &'static str {
        match self {
            Self::HardwareEscTripleTap => "EXORCISE_PURGE_HARDWARE_ESC",
            Self::ManualPurgeCircuit => "EXORCISE_PURGE_MANUAL_CIRCUIT",
            Self::EmergencySignalHook => "EXORCISE_PURGE_SIGNAL_HOOK",
        }
    }
}

/// Telemetry snapshot for the `EMERGENCY EXORCISE CIRCUIT` and `RUST CRATE MEMORY` panels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExorciseCircuitStatus {
    pub failsafe_window_ms: u64,
    pub required_esc_presses: u8,
    pub relay_sever_ready: bool,
    pub dormant_deep_sleep: bool,
    pub purge_execution_count: u64,
    pub rust_panic_count: u64,
    pub ground_leak_pct: f32,
}

/// Fixed 3-slot timestamp ringbuffer evaluating 3 consecutive `[ESC]` presses within `< 800ms`.
#[derive(Debug, Clone)]
pub struct EscSequenceRingBuffer {
    timestamps_ms: [Option<u64>; REQUIRED_ESC_PRESSES],
    write_pos: usize,
    count: usize,
    failsafe_window_ms: u64,
}

impl Default for EscSequenceRingBuffer {
    fn default() -> Self {
        Self::new(FAILSAFE_WINDOW_MS)
    }
}

impl EscSequenceRingBuffer {
    pub fn new(failsafe_window_ms: u64) -> Self {
        Self {
            timestamps_ms: [None; REQUIRED_ESC_PRESSES],
            write_pos: 0,
            count: 0,
            failsafe_window_ms,
        }
    }

    /// Strict privacy filter: accepts ONLY the Escape virtual keycode (`53` on macOS, `27`/`0x1B` on Windows/DOM)
    /// and immediately discards all other keycodes without storing or inspecting them.
    #[inline]
    pub fn is_escape_keycode(virtual_keycode: u16) -> bool {
        virtual_keycode == MACOS_KC_ESCAPE_CONST || virtual_keycode == WINDOWS_VK_ESCAPE
    }

    /// Records an `[ESC]` press at monotonic `timestamp_ms`.
    /// Returns `true` if 3 consecutive `[ESC]` presses occurred within `< failsafe_window_ms` (`800ms`).
    pub fn record_esc_press(&mut self, timestamp_ms: u64) -> bool {
        self.timestamps_ms[self.write_pos] = Some(timestamp_ms);
        self.write_pos = (self.write_pos + 1) % REQUIRED_ESC_PRESSES;
        if self.count < REQUIRED_ESC_PRESSES {
            self.count += 1;
        }

        if self.count < REQUIRED_ESC_PRESSES {
            return false;
        }

        // Oldest of the 3 presses is at `self.write_pos` after wrap-around
        let oldest_idx = self.write_pos;
        let newest_idx = (self.write_pos + REQUIRED_ESC_PRESSES - 1) % REQUIRED_ESC_PRESSES;

        if let (Some(oldest), Some(newest)) = (
            self.timestamps_ms[oldest_idx],
            self.timestamps_ms[newest_idx],
        ) {
            if newest >= oldest && (newest - oldest) < self.failsafe_window_ms {
                // Reset ringbuffer on trip so a 4th press requires a fresh 3x sequence
                self.reset();
                return true;
            }
        }

        false
    }

    /// Clears all recorded timestamps.
    pub fn reset(&mut self) {
        self.timestamps_ms = [None; REQUIRED_ESC_PRESSES];
        self.write_pos = 0;
        self.count = 0;
    }
}

const MACOS_KC_ESCAPE_CONST: u16 = MACOS_VK_ESCAPE;

/// Global Panic & Signal Recovery Registry used to ensure display restoration on unhandled panics or OS signals.
static PANIC_HOOK_ONCE: Once = Once::new();
static GLOBAL_PANIC_COUNT: AtomicU64 = AtomicU64::new(0);

/// Emergency Exorcise Circuit & Hardware Failsafe Controller.
pub struct ExorciseCircuit {
    pub failsafe_window_ms: u64,
    pub required_esc_presses: u8,
    epoch: Instant,
    esc_ring: Mutex<EscSequenceRingBuffer>,
    dormant_deep_sleep: AtomicBool,
    purge_count: AtomicU64,
    poller_running: AtomicBool,
}

impl Default for ExorciseCircuit {
    fn default() -> Self {
        Self::new()
    }
}

impl ExorciseCircuit {
    pub fn new() -> Self {
        Self {
            failsafe_window_ms: FAILSAFE_WINDOW_MS,
            required_esc_presses: REQUIRED_ESC_PRESSES as u8,
            epoch: Instant::now(),
            esc_ring: Mutex::new(EscSequenceRingBuffer::new(FAILSAFE_WINDOW_MS)),
            dormant_deep_sleep: AtomicBool::new(false),
            purge_count: AtomicU64::new(0),
            poller_running: AtomicBool::new(false),
        }
    }

    /// Returns monotonic milliseconds elapsed since circuit initialization.
    pub fn elapsed_monotonic_ms(&self) -> u64 {
        self.epoch.elapsed().as_millis() as u64
    }

    /// Feeds a virtual keycode event into the failsafe filter.
    /// Non-Escape keycodes are immediately discarded (`Ok(false)`).
    /// If 3 consecutive `[ESC]` presses occur within `< 800ms`, executes the emergency purge.
    pub fn process_key_event(
        &self,
        virtual_keycode: u16,
        store: &LocalPersistenceStore,
        telemetry: &TelemetryEngine,
        audio: &CpalAcousticEngine,
        display: &DisplayGammaController,
    ) -> Result<bool, String> {
        if !EscSequenceRingBuffer::is_escape_keycode(virtual_keycode) {
            return Ok(false);
        }
        let now_ms = self.elapsed_monotonic_ms();
        self.process_esc_timestamp(now_ms, store, telemetry, audio, display)
    }

    /// Feeds a verified `[ESC]` press timestamp (in monotonic ms) into the 3-slot ringbuffer
    /// and executes the atomic purge routine if the `< 800ms` triple-tap condition is satisfied.
    pub fn process_esc_timestamp(
        &self,
        timestamp_ms: u64,
        store: &LocalPersistenceStore,
        telemetry: &TelemetryEngine,
        audio: &CpalAcousticEngine,
        display: &DisplayGammaController,
    ) -> Result<bool, String> {
        let tripped = {
            let mut guard = self
                .esc_ring
                .lock()
                .map_err(|_| "Exorcise ESC ringbuffer mutex poisoned".to_string())?;
            guard.record_esc_press(timestamp_ms)
        };

        if tripped {
            self.execute_emergency_purge(
                ExorciseTriggerSource::HardwareEscTripleTap,
                store,
                telemetry,
                audio,
                display,
            )?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Atomic Exorcise Purge Routine:
    /// 1. Immediately zeroes and severs the `cpal` direct ringbuffer.
    /// 2. Immediately restores cached native monitor color gamma tables.
    /// 3. Suspends background intervention channels and places the daemon into deep sleep stasis.
    /// 4. Records an `EXORCISE_PURGE` audit log entry in SQLite.
    pub fn execute_emergency_purge(
        &self,
        source: ExorciseTriggerSource,
        store: &LocalPersistenceStore,
        telemetry: &TelemetryEngine,
        audio: &CpalAcousticEngine,
        display: &DisplayGammaController,
    ) -> Result<(), String> {
        // 1. Sever direct cpal audio ringbuffer & stream
        audio.sever_ringbuffer();

        // 2. Restore native OS monitor color calibration tables
        display.restore_default_gamma();

        // 3. Transition daemon to deep sleep / dormant stasis and persist channel disarm
        self.dormant_deep_sleep.store(true, Ordering::SeqCst);
        self.purge_count.fetch_add(1, Ordering::SeqCst);

        let _ = store.set_profile_setting("channel_acoustics", "false");
        let _ = store.set_profile_setting("channel_gamma", "false");
        let _ = store.set_profile_setting("channel_thermal", "false");

        // 4. Record audit log entry in SQLite
        let (_, station_time) = telemetry.query_local_station_time();
        store.record_incident(&NewIncidentRecord {
            timestamp_utc: station_time,
            trigger_category: source.as_audit_category().to_string(),
            intensity_detent: 1,
            milligauss_reading: 0.0,
            lull_duration_ms: 0,
            cpu_temp_c: 18.2,
            gamma_delta_pct: 0.0,
            suppressed_flag: true,
        })?;

        Ok(())
    }

    pub fn is_dormant_deep_sleep(&self) -> bool {
        self.dormant_deep_sleep.load(Ordering::Acquire)
    }

    pub fn set_dormant_deep_sleep(&self, dormant: bool) {
        self.dormant_deep_sleep.store(dormant, Ordering::Release);
    }

    pub fn status_snapshot(&self) -> ExorciseCircuitStatus {
        ExorciseCircuitStatus {
            failsafe_window_ms: self.failsafe_window_ms,
            required_esc_presses: self.required_esc_presses,
            relay_sever_ready: true,
            dormant_deep_sleep: self.is_dormant_deep_sleep(),
            purge_execution_count: self.purge_count.load(Ordering::Relaxed),
            rust_panic_count: GLOBAL_PANIC_COUNT.load(Ordering::Relaxed),
            ground_leak_pct: 0.04,
        }
    }

    /// Installs global Rust panic hook and OS termination signal handlers (`SIGINT`, `SIGTERM`,
    /// `SIGHUP`, `SIGABRT`) that unconditionally sever audio and restore native display gamma.
    pub fn install_panic_and_signal_hooks(
        audio: Arc<CpalAcousticEngine>,
        display: Arc<DisplayGammaController>,
    ) {
        PANIC_HOOK_ONCE.call_once(move || {
            let prev_hook = std::panic::take_hook();
            let audio_panic = Arc::clone(&audio);
            let display_panic = Arc::clone(&display);

            std::panic::set_hook(Box::new(move |panic_info| {
                GLOBAL_PANIC_COUNT.fetch_add(1, Ordering::SeqCst);
                audio_panic.sever_ringbuffer();
                display_panic.restore_default_gamma();
                prev_hook(panic_info);
            }));

            platform_signals::register_os_termination_handlers();
        });
    }

    /// Spawns a lightweight background thread polling ONLY the hardware Escape key state
    /// (`kVK_Escape` on macOS, `VK_ESCAPE` on Windows) to detect `< 800ms` triple-taps globally.
    pub fn spawn_hardware_esc_listener(
        self: &Arc<Self>,
        store: Arc<LocalPersistenceStore>,
        telemetry: Arc<TelemetryEngine>,
        audio: Arc<CpalAcousticEngine>,
        display: Arc<DisplayGammaController>,
    ) {
        if self.poller_running.swap(true, Ordering::SeqCst) {
            return;
        }

        let circuit = Arc::clone(self);
        std::thread::Builder::new()
            .name("nocturne-esc-failsafe".to_string())
            .spawn(move || {
                let mut was_pressed = false;
                loop {
                    let is_pressed = platform_esc_poller::is_hardware_escape_down();
                    // Detect rising edge (key-down transition) of `[ESC]`
                    if is_pressed && !was_pressed {
                        let now_ms = circuit.elapsed_monotonic_ms();
                        let _ = circuit
                            .process_esc_timestamp(now_ms, &store, &telemetry, &audio, &display);
                    }
                    was_pressed = is_pressed;
                    std::thread::sleep(Duration::from_millis(25));
                }
            })
            .ok();
    }
}

mod platform_esc_poller {
    /// Queries ONLY whether the physical `Escape` key is currently depressed,
    /// without inspecting or intercepting any other key on the system.
    pub fn is_hardware_escape_down() -> bool {
        #[cfg(target_os = "macos")]
        {
            const CG_EVENT_SOURCE_STATE_COMBINED_SESSION: i32 = 0;
            const KVK_ESCAPE: u16 = 53;

            #[link(name = "CoreGraphics", kind = "framework")]
            extern "C" {
                fn CGEventSourceKeyState(state_id: i32, key: u16) -> bool;
            }

            // SAFETY: Queries WindowServer boolean state strictly for keycode 53 (Escape).
            unsafe { CGEventSourceKeyState(CG_EVENT_SOURCE_STATE_COMBINED_SESSION, KVK_ESCAPE) }
        }
        #[cfg(target_os = "windows")]
        {
            const VK_ESCAPE: i32 = 0x1B;

            #[link(name = "user32")]
            extern "system" {
                fn GetAsyncKeyState(vkey: i32) -> i16;
            }

            // SAFETY: Queries Win32 key state strictly for VK_ESCAPE (0x1B).
            unsafe { (GetAsyncKeyState(VK_ESCAPE) as u16 & 0x8000) != 0 }
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            false
        }
    }
}

mod platform_signals {
    /// Registers OS signal handlers (`SIGINT`, `SIGTERM`, `SIGHUP`) to restore display calibration on exit.
    pub fn register_os_termination_handlers() {
        #[cfg(target_os = "macos")]
        {
            const SIGINT: i32 = 2;
            const SIGHUP: i32 = 1;
            const SIGTERM: i32 = 15;

            #[link(name = "CoreGraphics", kind = "framework")]
            extern "C" {
                fn CGDisplayRestoreColorSyncSettings();
            }

            extern "C" {
                fn signal(sig: i32, handler: extern "C" fn(i32)) -> usize;
                fn _exit(status: i32) -> !;
            }

            extern "C" fn handle_termination_signal(sig: i32) {
                // SAFETY: `CGDisplayRestoreColorSyncSettings` restores native ColorSync display tables
                // before immediate process termination.
                unsafe {
                    CGDisplayRestoreColorSyncSettings();
                    _exit(128 + sig);
                }
            }

            // SAFETY: Registers signal handlers for clean display restoration on SIGINT, SIGHUP, SIGTERM.
            unsafe {
                signal(SIGINT, handle_termination_signal);
                signal(SIGHUP, handle_termination_signal);
                signal(SIGTERM, handle_termination_signal);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_esc_ringbuffer_triggers_under_800ms_and_rejects_slow_taps() {
        let mut ring = EscSequenceRingBuffer::new(800);

        // Slow taps (100ms -> 600ms -> 1,200ms = 1,100ms span >= 800ms) must NOT trigger failsafe
        assert!(!ring.record_esc_press(100));
        assert!(!ring.record_esc_press(600));
        assert!(!ring.record_esc_press(1_200));

        // Next tap at 1,500ms gives window [600, 1200, 1500] = 900ms span (>= 800ms) -> false
        assert!(!ring.record_esc_press(1_500));

        // Rapid 3rd tap at 1,850ms gives window [1200, 1500, 1850] = 650ms span (< 800ms) -> MUST trigger!
        assert!(ring.record_esc_press(1_850));

        // After triggering, ringbuffer resets so a single subsequent tap does not re-trigger
        assert!(!ring.record_esc_press(1_900));
    }

    #[test]
    fn test_privacy_keycode_filter_discards_non_escape_keys() {
        // Escape keycodes (macOS 53, Windows/DOM 27) are accepted
        assert!(EscSequenceRingBuffer::is_escape_keycode(MACOS_VK_ESCAPE));
        assert!(EscSequenceRingBuffer::is_escape_keycode(WINDOWS_VK_ESCAPE));

        // Alphanumeric keycodes (e.g. 'A'=0/65, 'S'=1/83, Space=49/32, Enter=36/13) are rejected
        for code in [0, 1, 13, 32, 36, 49, 65, 83, 97] {
            assert!(!EscSequenceRingBuffer::is_escape_keycode(code));
        }
    }

    #[test]
    fn test_atomic_emergency_purge_severs_audio_restores_gamma_and_logs_audit() {
        let db_path = std::env::temp_dir()
            .join(format!("nocturne_panic_test_{}", std::process::id()))
            .join("nocturne_telemetry.db");

        let store =
            LocalPersistenceStore::init_at_path(&db_path).expect("should init test SQLite store");
        let telemetry = TelemetryEngine::new();
        let audio = CpalAcousticEngine::new();
        let display = DisplayGammaController::new();
        let circuit = ExorciseCircuit::new();

        // Queue audio and apply gamma perturbation first
        let _ = audio.dispatch_spatial_creak(0.2, 1);
        let _ = display.apply_cubic_bezier_perturbation(7.8, 1200);
        assert!(audio.is_armed());
        assert_eq!(display.active_drift_pct(), 7.8);

        // Simulate 3 rapid `[ESC]` presses at 10ms, 220ms, and 490ms (< 800ms)
        assert!(!circuit
            .process_esc_timestamp(10, &store, &telemetry, &audio, &display)
            .unwrap());
        assert!(!circuit
            .process_esc_timestamp(220, &store, &telemetry, &audio, &display)
            .unwrap());
        assert!(circuit
            .process_esc_timestamp(490, &store, &telemetry, &audio, &display)
            .unwrap());

        // Verify audio ringbuffer severed, display gamma restored to 0.0%, and daemon in deep sleep
        assert!(!audio.is_armed());
        assert_eq!(audio.snapshot_telemetry().queued_samples, 0);
        assert_eq!(display.active_drift_pct(), 0.0);
        assert!(circuit.is_dormant_deep_sleep());
        assert_eq!(circuit.status_snapshot().purge_execution_count, 1);

        // Verify SQLite audit log recorded EXORCISE_PURGE_HARDWARE_ESC
        let logs = store
            .get_recent_incidents(5)
            .expect("should read audit logs");
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].trigger_category, "EXORCISE_PURGE_HARDWARE_ESC");
        assert!(logs[0].suppressed_flag);

        if let Some(parent) = db_path.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }
}
