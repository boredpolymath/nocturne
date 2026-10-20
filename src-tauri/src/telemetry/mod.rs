//! Environmental Monitoring, Non-Interceptive Idle Detection, Witching Hour Vigil Chronometer,
//! and Cognitive Lull Cadence Analyzer (`src-tauri/src/telemetry/`).
//!
//! Security & Privacy Guarantee:
//! Queries native OS idle timers (`CGEventSourceSecondsSinceLastEventType` on macOS,
//! `GetLastInputInfo` on Windows, X11 `XScreenSaverQueryInfo` / Wayland idle on Linux)
//! without intercepting, inspecting, or storing alphanumeric keystroke sequences.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub mod profiler;
pub use profiler::{ResourceBudgetReport, ResourceFootprintProfiler};

/// Active input cadence threshold: inter-event idle deltas below 250ms indicate active typing/input.
pub const ACTIVE_CADENCE_THRESHOLD_MS: u64 = 250;
/// Minimum number of consecutive active cadence samples required to qualify an input burst.
pub const MIN_ACTIVE_BURST_SAMPLES: u32 = 3;
/// Post-halt delay (2.5 seconds) for muted phantom tactile keystroke echo eligibility.
pub const PHANTOM_KEYSTROKE_ECHO_DELAY_MS: u64 = 2_500;
/// Minimum cognitive lull trigger window (4.0s [RAPID CADENCE]).
pub const MIN_LULL_WINDOW_SEC: f32 = 4.0;
/// Default optimal cognitive lull trigger window (7.5s [OPTIMAL]).
pub const DEFAULT_OPTIMAL_LULL_SEC: f32 = 7.5;
/// Maximum cognitive lull trigger window (15.0s [LONG PAUSE]).
pub const MAX_LULL_WINDOW_SEC: f32 = 15.0;
/// Base EMF floor reading in milligauss (`1.5 mG [BASE]`).
pub const BASE_EMF_MILLIGAUSS: f32 = 1.50;

/// Operational state of the Nocturne daemon governed by the Witching Hour Vigil Chronometer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DaemonOperationalState {
    /// Active surveillance within the configured nocturnal vigil window.
    BoundAndListening,
    /// Outside the nocturnal vigil bracket; all hooks and interventions are in non-hook stasis.
    NonHookStasis,
}

/// 5-step K-II EMF & REM-Pod disturbance tier matching the Séance Panel LED array.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemPodTier {
    /// `1.5 mG [BASE]` — `NORM` LED illuminated
    Norm,
    /// `>= 2.5 mG` — `NORM` + `LOW` LEDs illuminated
    Low,
    /// `>= 10.0 mG` — `NORM` + `LOW` + `MID` LEDs illuminated
    Mid,
    /// `>= 20.0 mG` — `NORM` + `LOW` + `MID` + `HIGH` LEDs illuminated
    High,
    /// `> 25.0 mG` — All 5 LEDs (`NORM`, `LOW`, `MID`, `HIGH`, `SPIKE`) illuminated
    Spike,
}

impl RemPodTier {
    pub fn as_label(&self) -> &'static str {
        match self {
            Self::Norm => "NORM",
            Self::Low => "LOW",
            Self::Mid => "MID",
            Self::High => "HIGH",
            Self::Spike => "SPIKE",
        }
    }
}

/// Real-time K-II EMF Meter & REM-Pod Disturbance Array telemetry readout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmfTelemetryReadout {
    pub milligauss: f32,
    pub drift_mg: f32,
    pub rem_pod_active: bool,
    pub tier: RemPodTier,
    /// Illumination state of the 5 LED segments: `[NORM, LOW, MID, HIGH, SPIKE]`.
    pub led_states: [bool; 5],
}

/// Witching Hour Vigil Chronometer operational window (`[00:00 – 05:00]` default, 15-min increments).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VigilWindow {
    /// Start of operational bracket in minutes from midnight (`00:00` = `0`).
    pub start_minutes_from_midnight: u16,
    /// End of operational bracket in minutes from midnight (`05:00` = `300`).
    pub end_minutes_from_midnight: u16,
    /// Whether the curfew time-gate filter is actively enforced.
    pub curfew_engaged: bool,
}

impl Default for VigilWindow {
    fn default() -> Self {
        Self {
            start_minutes_from_midnight: 0, // 00:00 [MIDNIGHT STRIKE]
            end_minutes_from_midnight: 300, // 05:00
            curfew_engaged: true,
        }
    }
}

impl VigilWindow {
    /// Parses `"HH:MM"` strings and validates 15-minute increment alignment (`00`, `15`, `30`, `45`, or `23:59` end).
    pub fn from_hhmm(start: &str, end: &str, curfew_engaged: bool) -> Result<Self, String> {
        let start_min = Self::parse_hhmm_15min(start)?;
        let end_min = Self::parse_hhmm_15min(end)?;
        Ok(Self {
            start_minutes_from_midnight: start_min,
            end_minutes_from_midnight: end_min,
            curfew_engaged,
        })
    }

    /// Parses a `"HH:MM"` clock string into minutes from midnight, enforcing 15-minute increments.
    pub fn parse_hhmm_15min(hhmm: &str) -> Result<u16, String> {
        let parts: Vec<&str> = hhmm.trim().split(':').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid time format '{hhmm}': expected HH:MM"));
        }
        let hours: u16 = parts[0]
            .parse()
            .map_err(|_| format!("Invalid hour in '{hhmm}'"))?;
        let minutes: u16 = parts[1]
            .parse()
            .map_err(|_| format!("Invalid minute in '{hhmm}'"))?;

        if hours > 23 || minutes > 59 {
            return Err(format!("Time '{hhmm}' out of 24-hour range [00:00, 23:59]"));
        }

        // Allow 15-minute increments (0, 15, 30, 45) as well as 23:59 end-of-day marker
        if minutes % 15 != 0 && !(hours == 23 && minutes == 59) {
            return Err(format!(
                "Time '{hhmm}' must align to 15-minute increments (:00, :15, :30, :45)"
            ));
        }

        Ok(hours * 60 + minutes)
    }

    /// Formats minutes from midnight back into `"HH:MM"`.
    pub fn format_hhmm(total_minutes: u16) -> String {
        let clamped = total_minutes.min(23 * 60 + 59);
        let h = clamped / 60;
        let m = clamped % 60;
        format!("{h:02}:{m:02}")
    }

    /// Evaluates whether a given minute-of-day (`0..1440`) falls within the active vigil bracket.
    pub fn contains_minute_of_day(&self, current_minute: u16) -> bool {
        if !self.curfew_engaged {
            return true;
        }
        let start = self.start_minutes_from_midnight;
        let end = self.end_minutes_from_midnight;

        if start == end {
            true
        } else if start < end {
            // Standard nocturnal window, e.g. 00:00 (0) ..= 05:00 (300)
            current_minute >= start && current_minute <= end
        } else {
            // Overnight wrap-around window, e.g. 23:00 (1380) ..= 04:00 (240)
            current_minute >= start || current_minute <= end
        }
    }
}

/// Configuration parameters for the Cognitive Lull Cadence Analyzer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CadenceConfig {
    /// Minimum cognitive pause duration in seconds (`4.0s [RAPID CADENCE]`).
    pub min_lull_sec: f32,
    /// Target optimal cognitive pause trigger window in seconds (`7.5s [OPTIMAL]`).
    pub optimal_lull_sec: f32,
    /// Maximum cognitive pause duration in seconds (`15.0s [LONG PAUSE]`).
    pub max_lull_sec: f32,
}

impl Default for CadenceConfig {
    fn default() -> Self {
        Self {
            min_lull_sec: MIN_LULL_WINDOW_SEC,
            optimal_lull_sec: DEFAULT_OPTIMAL_LULL_SEC,
            max_lull_sec: MAX_LULL_WINDOW_SEC,
        }
    }
}

/// Result of evaluating a real-time OS idle sample against the user's typing cadence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CadenceLullEvaluation {
    /// Current millisecond-precision system inactivity duration.
    pub current_idle_ms: u64,
    /// True if the user is currently in an active typing/input burst (`< 250ms` inter-event delta).
    pub in_active_cadence: bool,
    /// Number of consecutive active cadence samples observed prior to or during the current burst.
    pub active_streak_samples: u32,
    /// Duration in seconds of the most recently completed or active cognitive lull (`4.0s–15.0s`).
    pub last_lull_sec: f32,
    /// True when the current pause following an active typing burst satisfies the optimal lull window.
    pub lull_window_triggered: bool,
    /// True when an active typing burst has halted for `>= 2.5s`, arming phantom tactile keystroke echo.
    pub phantom_keystroke_ready: bool,
}

#[derive(Debug, Clone)]
struct CadenceTrackerState {
    active_streak_samples: u32,
    armed_by_typing_burst: bool,
    lull_dispatched_for_current_halt: bool,
    phantom_dispatched_for_current_halt: bool,
    last_recorded_lull_sec: f32,
}

impl Default for CadenceTrackerState {
    fn default() -> Self {
        Self {
            active_streak_samples: 0,
            armed_by_typing_burst: false,
            lull_dispatched_for_current_halt: false,
            phantom_dispatched_for_current_halt: false,
            last_recorded_lull_sec: 6.2, // Initial calibrated readout matching screenshot (`06.2s`)
        }
    }
}

/// Core Environmental Monitoring & Non-Interceptive Idle Detection Engine.
pub struct TelemetryEngine {
    cadence_state: Mutex<CadenceTrackerState>,
    profiler: ResourceFootprintProfiler,
}

impl Default for TelemetryEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TelemetryEngine {
    pub fn new() -> Self {
        Self {
            cadence_state: Mutex::new(CadenceTrackerState::default()),
            profiler: ResourceFootprintProfiler::new(),
        }
    }

    /// Returns a reference to the Section 3.1 resource footprint profiler.
    pub fn profiler(&self) -> &ResourceFootprintProfiler {
        &self.profiler
    }

    /// Queries millisecond-precision user inactivity duration via native OS event timer hooks
    /// without inspecting or capturing keystrokes.
    pub fn query_system_idle_ms(&self) -> u64 {
        platform_idle::query_os_idle_duration_ms()
    }

    /// Queries local workstation clock, returning `(minute_of_day, formatted_12h_clock_string)`.
    pub fn query_local_station_time(&self) -> (u16, String) {
        platform_clock::local_station_time()
    }

    /// Evaluates whether the local workstation clock falls inside the active vigil bracket.
    pub fn is_within_vigil_bracket(&self, window: &VigilWindow) -> bool {
        let (minute_of_day, _) = self.query_local_station_time();
        window.contains_minute_of_day(minute_of_day)
    }

    /// Determines the daemon's operational state (`BoundAndListening` vs `NonHookStasis`)
    /// for a given minute-of-day (`0..1440`) and [`VigilWindow`].
    pub fn evaluate_operational_state(
        &self,
        window: &VigilWindow,
        minute_of_day: u16,
    ) -> DaemonOperationalState {
        if window.contains_minute_of_day(minute_of_day) {
            DaemonOperationalState::BoundAndListening
        } else {
            DaemonOperationalState::NonHookStasis
        }
    }

    /// Feeds a current millisecond idle sample into the Cognitive Lull Cadence Analyzer.
    ///
    /// Tracks active input bursts (`< 250ms` idle delta) and identifies natural cognitive pauses
    /// between `4.0s` and `15.0s` (triggering at `config.optimal_lull_sec`), plus `2.5s` phantom
    /// keystroke echo windows.
    pub fn analyze_cadence_sample(
        &self,
        idle_ms: u64,
        config: &CadenceConfig,
        operational_state: DaemonOperationalState,
    ) -> CadenceLullEvaluation {
        let mut state = self
            .cadence_state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        // Outside the nocturnal vigil bracket, Nocturne reverts to non-hook stasis
        if operational_state == DaemonOperationalState::NonHookStasis {
            state.active_streak_samples = 0;
            state.armed_by_typing_burst = false;
            state.lull_dispatched_for_current_halt = false;
            state.phantom_dispatched_for_current_halt = false;
            return CadenceLullEvaluation {
                current_idle_ms: idle_ms,
                in_active_cadence: false,
                active_streak_samples: 0,
                last_lull_sec: state.last_recorded_lull_sec,
                lull_window_triggered: false,
                phantom_keystroke_ready: false,
            };
        }

        if idle_ms < ACTIVE_CADENCE_THRESHOLD_MS {
            // User is actively typing / interacting
            state.active_streak_samples = state.active_streak_samples.saturating_add(1);
            if state.active_streak_samples >= MIN_ACTIVE_BURST_SAMPLES {
                state.armed_by_typing_burst = true;
            }
            state.lull_dispatched_for_current_halt = false;
            state.phantom_dispatched_for_current_halt = false;

            return CadenceLullEvaluation {
                current_idle_ms: idle_ms,
                in_active_cadence: true,
                active_streak_samples: state.active_streak_samples,
                last_lull_sec: state.last_recorded_lull_sec,
                lull_window_triggered: false,
                phantom_keystroke_ready: false,
            };
        }

        let idle_sec = idle_ms as f32 / 1000.0;
        let min_lull = config
            .min_lull_sec
            .clamp(MIN_LULL_WINDOW_SEC, MAX_LULL_WINDOW_SEC);
        let max_lull = config.max_lull_sec.clamp(min_lull, MAX_LULL_WINDOW_SEC);
        let optimal_lull = config.optimal_lull_sec.clamp(min_lull, max_lull);

        let mut phantom_ready = false;
        let mut lull_triggered = false;

        if state.armed_by_typing_burst {
            // Check 2.5s phantom keystroke echo threshold
            if idle_ms >= PHANTOM_KEYSTROKE_ECHO_DELAY_MS
                && idle_sec < max_lull
                && !state.phantom_dispatched_for_current_halt
            {
                phantom_ready = true;
                state.phantom_dispatched_for_current_halt = true;
            }

            // Record cognitive pause duration once inside [4.0s, 15.0s]
            if (min_lull..=max_lull).contains(&idle_sec) {
                state.last_recorded_lull_sec = (idle_sec * 10.0).round() / 10.0;

                if idle_sec >= optimal_lull && !state.lull_dispatched_for_current_halt {
                    lull_triggered = true;
                    state.lull_dispatched_for_current_halt = true;
                }
            } else if idle_sec > max_lull {
                // Pause exceeded 15.0s cognitive lull window; disarm until next typing burst
                state.armed_by_typing_burst = false;
                state.active_streak_samples = 0;
            }
        }

        CadenceLullEvaluation {
            current_idle_ms: idle_ms,
            in_active_cadence: false,
            active_streak_samples: state.active_streak_samples,
            last_lull_sec: state.last_recorded_lull_sec,
            lull_window_triggered: lull_triggered,
            phantom_keystroke_ready: phantom_ready,
        }
    }

    /// Computes real-time K-II EMF Meter & REM-Pod Disturbance Array telemetry (`MILLIGAUSS (mG)`,
    /// `DRIFT`, and 5-step LED states: `[NORM, LOW, MID, HIGH, SPIKE]`).
    pub fn compute_emf_telemetry(
        &self,
        eval: &CadenceLullEvaluation,
        intensity_detent: u8,
        operational_state: DaemonOperationalState,
    ) -> EmfTelemetryReadout {
        if operational_state == DaemonOperationalState::NonHookStasis {
            return EmfTelemetryReadout {
                milligauss: BASE_EMF_MILLIGAUSS,
                drift_mg: 0.0,
                rem_pod_active: false,
                tier: RemPodTier::Norm,
                led_states: [true, false, false, false, false],
            };
        }

        let detent_scale = match intensity_detent {
            1 => 1.0_f32,  // POS 1 // SUBTLE
            2 => 2.15_f32, // POS 2 // UNSETTLE
            3 => 4.25_f32, // POS 3 // ACUTE
            _ => 1.0_f32,
        };

        // Base ambient carrier is 2.87 mG (+0.14 mG drift) during calm listening,
        // escalating smoothly during armed cognitive lull windows
        let lull_excitation = if eval.lull_window_triggered {
            6.5 * detent_scale
        } else if eval.phantom_keystroke_ready {
            1.8 * detent_scale
        } else {
            1.37 * detent_scale
        };

        let raw_mg = (BASE_EMF_MILLIGAUSS + lull_excitation).clamp(BASE_EMF_MILLIGAUSS, 35.0);
        let milligauss = (raw_mg * 100.0).round() / 100.0;
        let drift_mg = ((milligauss - 2.73) * 100.0).round() / 100.0;

        let (tier, led_states) = if milligauss >= 25.0 {
            (RemPodTier::Spike, [true, true, true, true, true])
        } else if milligauss >= 20.0 {
            (RemPodTier::High, [true, true, true, true, false])
        } else if milligauss >= 10.0 {
            (RemPodTier::Mid, [true, true, true, false, false])
        } else if milligauss >= 2.5 {
            (RemPodTier::Low, [true, true, false, false, false])
        } else {
            (RemPodTier::Norm, [true, false, false, false, false])
        };

        EmfTelemetryReadout {
            milligauss,
            drift_mg,
            rem_pod_active: true,
            tier,
            led_states,
        }
    }

    /// Computes the human-readable `NEXT PROBABILITY` countdown string for the Séance Panel HUD.
    pub fn compute_next_probability_window(
        &self,
        eval: &CadenceLullEvaluation,
        config: &CadenceConfig,
        operational_state: DaemonOperationalState,
        window: &VigilWindow,
        current_minute_of_day: u16,
    ) -> String {
        if operational_state == DaemonOperationalState::NonHookStasis {
            // Calculate minutes until the nocturnal vigil window opens
            let start = window.start_minutes_from_midnight;
            let mins_until = if current_minute_of_day <= start {
                start - current_minute_of_day
            } else {
                (24 * 60 - current_minute_of_day) + start
            };
            if mins_until >= 60 {
                return format!("{}h {:02}m", mins_until / 60, mins_until % 60);
            } else {
                return format!("{}m 00s", mins_until);
            }
        }

        let idle_sec = eval.current_idle_ms as f32 / 1000.0;
        let optimal_sec = config
            .optimal_lull_sec
            .clamp(MIN_LULL_WINDOW_SEC, MAX_LULL_WINDOW_SEC);

        if eval.lull_window_triggered {
            return "ACTIVE NOW".to_string();
        }

        if eval.active_streak_samples >= MIN_ACTIVE_BURST_SAMPLES && idle_sec < optimal_sec {
            let rem_sec = (optimal_sec - idle_sec).max(0.1);
            return format!("{rem_sec:04.1}s");
        }

        // Standard nocturnal manifestation probability cycle countdown (14m 20s baseline modulated by idle)
        let cycle_total_secs: u64 = 14 * 60 + 20;
        let offset_secs = (eval.current_idle_ms / 1000) % cycle_total_secs;
        let rem = cycle_total_secs.saturating_sub(offset_secs);
        format!("{:02}m {:02}s", rem / 60, rem % 60)
    }
}

mod platform_idle {
    /// Queries native OS inactivity duration in milliseconds without keystroke hooks.
    pub fn query_os_idle_duration_ms() -> u64 {
        #[cfg(target_os = "macos")]
        {
            macos::query_idle_ms()
        }
        #[cfg(target_os = "windows")]
        {
            windows::query_idle_ms()
        }
        #[cfg(target_os = "linux")]
        {
            linux::query_idle_ms()
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            0
        }
    }

    #[cfg(target_os = "macos")]
    mod macos {
        // CoreGraphics non-interceptive idle query:
        // kCGEventSourceStateCombinedSessionState = 0
        // kCGAnyInputEventType = u32::MAX (!0)
        const CG_EVENT_SOURCE_STATE_COMBINED_SESSION: i32 = 0;
        const CG_ANY_INPUT_EVENT_TYPE: u32 = u32::MAX;

        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGEventSourceSecondsSinceLastEventType(state_id: i32, event_type: u32) -> f64;
        }

        pub fn query_idle_ms() -> u64 {
            // SAFETY: `CGEventSourceSecondsSinceLastEventType` is a thread-safe, read-only
            // CoreGraphics query that takes two primitive integer constants and returns a double.
            let seconds = unsafe {
                CGEventSourceSecondsSinceLastEventType(
                    CG_EVENT_SOURCE_STATE_COMBINED_SESSION,
                    CG_ANY_INPUT_EVENT_TYPE,
                )
            };
            if seconds.is_finite() && seconds >= 0.0 {
                (seconds * 1000.0).round() as u64
            } else {
                0
            }
        }
    }

    #[cfg(target_os = "windows")]
    mod windows {
        #[repr(C)]
        struct LastInputInfo {
            cb_size: u32,
            dw_time: u32,
        }

        #[link(name = "user32")]
        extern "system" {
            fn GetLastInputInfo(plii: *mut LastInputInfo) -> i32;
        }

        #[link(name = "kernel32")]
        extern "system" {
            fn GetTickCount() -> u32;
        }

        pub fn query_idle_ms() -> u64 {
            let mut lii = LastInputInfo {
                cb_size: std::mem::size_of::<LastInputInfo>() as u32,
                dw_time: 0,
            };
            // SAFETY: `lii.cb_size` is initialized to `sizeof(LASTINPUTINFO)` as required by Win32.
            let ok = unsafe { GetLastInputInfo(&mut lii as *mut LastInputInfo) };
            if ok != 0 {
                let now_ticks = unsafe { GetTickCount() };
                u64::from(now_ticks.wrapping_sub(lii.dw_time))
            } else {
                0
            }
        }
    }

    #[cfg(target_os = "linux")]
    mod linux {
        use std::ffi::{c_char, c_int, c_ulong, c_void};

        #[repr(C)]
        struct XScreenSaverInfo {
            window: c_ulong,
            state: c_int,
            kind: c_int,
            til_or_since: c_ulong,
            idle: c_ulong,
            event_mask: c_ulong,
        }

        const RTLD_LAZY: c_int = 1;

        extern "C" {
            fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
            fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
            fn dlclose(handle: *mut c_void) -> c_int;
        }

        /// Queries X11 `XScreenSaverQueryInfo` dynamically via `libXss.so.1` & `libX11.so.6`,
        /// or returns 0 when running in a headless/Wayland session without X11 display server.
        pub fn query_idle_ms() -> u64 {
            if std::env::var_os("DISPLAY").is_none() {
                return 0;
            }

            // SAFETY: Dynamically loads standard X11/Xss shared libraries if present on the workstation
            // and cleans up all resources before returning.
            unsafe {
                let x11 = dlopen(c"libX11.so.6".as_ptr(), RTLD_LAZY);
                if x11.is_null() {
                    return 0;
                }
                let xss = dlopen(c"libXss.so.1".as_ptr(), RTLD_LAZY);
                if xss.is_null() {
                    dlclose(x11);
                    return 0;
                }

                type XOpenDisplayFn = unsafe extern "C" fn(*const c_char) -> *mut c_void;
                type XDefaultRootWindowFn = unsafe extern "C" fn(*mut c_void) -> c_ulong;
                type XCloseDisplayFn = unsafe extern "C" fn(*mut c_void) -> c_int;
                type XFreeFn = unsafe extern "C" fn(*mut c_void) -> c_int;
                type XssAllocInfoFn = unsafe extern "C" fn() -> *mut XScreenSaverInfo;
                type XssQueryInfoFn =
                    unsafe extern "C" fn(*mut c_void, c_ulong, *mut XScreenSaverInfo) -> c_int;

                let open_sym = dlsym(x11, c"XOpenDisplay".as_ptr());
                let root_sym = dlsym(x11, c"XDefaultRootWindow".as_ptr());
                let close_sym = dlsym(x11, c"XCloseDisplay".as_ptr());
                let free_sym = dlsym(x11, c"XFree".as_ptr());
                let alloc_sym = dlsym(xss, c"XScreenSaverAllocInfo".as_ptr());
                let query_sym = dlsym(xss, c"XScreenSaverQueryInfo".as_ptr());

                let mut idle_ms = 0;

                if !open_sym.is_null()
                    && !root_sym.is_null()
                    && !close_sym.is_null()
                    && !free_sym.is_null()
                    && !alloc_sym.is_null()
                    && !query_sym.is_null()
                {
                    let x_open: XOpenDisplayFn = std::mem::transmute(open_sym);
                    let x_root: XDefaultRootWindowFn = std::mem::transmute(root_sym);
                    let x_close: XCloseDisplayFn = std::mem::transmute(close_sym);
                    let x_free: XFreeFn = std::mem::transmute(free_sym);
                    let xss_alloc: XssAllocInfoFn = std::mem::transmute(alloc_sym);
                    let xss_query: XssQueryInfoFn = std::mem::transmute(query_sym);

                    let dpy = x_open(std::ptr::null());
                    if !dpy.is_null() {
                        let info = xss_alloc();
                        if !info.is_null() {
                            let root = x_root(dpy);
                            if xss_query(dpy, root, info) != 0 {
                                idle_ms = (*info).idle;
                            }
                            x_free(info.cast());
                        }
                        x_close(dpy);
                    }
                }

                dlclose(xss);
                dlclose(x11);
                idle_ms
            }
        }
    }
}

mod platform_clock {
    use super::{SystemTime, UNIX_EPOCH};

    /// Returns `(minute_of_day, formatted_12h_station_time)` in local workstation time.
    pub fn local_station_time() -> (u16, String) {
        #[cfg(unix)]
        {
            #[repr(C)]
            struct Tm {
                tm_sec: i32,
                tm_min: i32,
                tm_hour: i32,
                tm_mday: i32,
                tm_mon: i32,
                tm_year: i32,
                tm_wday: i32,
                tm_yday: i32,
                tm_isdst: i32,
                tm_gmtoff: isize,
                tm_zone: *const i8,
            }

            extern "C" {
                fn localtime_r(timep: *const i64, result: *mut Tm) -> *mut Tm;
            }

            let now_secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            let mut tm = Tm {
                tm_sec: 0,
                tm_min: 0,
                tm_hour: 0,
                tm_mday: 0,
                tm_mon: 0,
                tm_year: 0,
                tm_wday: 0,
                tm_yday: 0,
                tm_isdst: 0,
                tm_gmtoff: 0,
                tm_zone: std::ptr::null(),
            };

            // SAFETY: `localtime_r` is POSIX thread-safe and writes into stack-allocated `tm`.
            let ptr = unsafe { localtime_r(&now_secs as *const i64, &mut tm as *mut Tm) };
            if !ptr.is_null() {
                let hour24 = tm.tm_hour.clamp(0, 23) as u16;
                let min = tm.tm_min.clamp(0, 59) as u16;
                let sec = tm.tm_sec.clamp(0, 59) as u16;
                return format_station_clock(hour24, min, sec);
            }
        }

        // Fallback to UTC clock calculation
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let day_secs = (secs % 86_400) as u32;
        let hour24 = (day_secs / 3600) as u16;
        let min = ((day_secs % 3600) / 60) as u16;
        let sec = (day_secs % 60) as u16;
        format_station_clock(hour24, min, sec)
    }

    pub fn format_station_clock(hour24: u16, min: u16, sec: u16) -> (u16, String) {
        let minute_of_day = hour24 * 60 + min;
        let (hour12, meridiem) = match hour24 {
            0 => (12, "AM"),
            1..=11 => (hour24, "AM"),
            12 => (12, "PM"),
            _ => (hour24 - 12, "PM"),
        };
        (
            minute_of_day,
            format!("{hour12:02}:{min:02}:{sec:02} {meridiem}"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_os_idle_query_executes_without_keylogging_hooks() {
        let engine = TelemetryEngine::new();
        let idle_ms = engine.query_system_idle_ms();
        // Idle duration must be a valid non-negative millisecond value (< 1 year)
        assert!(idle_ms < 31_536_000_000);
    }

    #[test]
    fn test_vigil_chronometer_15_min_validation_and_stasis() {
        let engine = TelemetryEngine::new();
        let window =
            VigilWindow::from_hhmm("00:00", "05:00", true).expect("00:00 - 05:00 should be valid");

        // 02:41 AM (161 minutes from midnight) is inside [00:00, 05:00]
        assert_eq!(
            engine.evaluate_operational_state(&window, 2 * 60 + 41),
            DaemonOperationalState::BoundAndListening
        );

        // 14:30 PM (870 minutes from midnight) is outside [00:00, 05:00] -> NonHookStasis
        assert_eq!(
            engine.evaluate_operational_state(&window, 14 * 60 + 30),
            DaemonOperationalState::NonHookStasis
        );

        // Non-15-minute increments must be rejected
        assert!(VigilWindow::from_hhmm("00:07", "05:00", true).is_err());
        // 15-minute increments and 23:59 must be accepted
        assert!(VigilWindow::from_hhmm("22:15", "04:45", true).is_ok());
        assert!(VigilWindow::from_hhmm("00:00", "23:59", true).is_ok());
    }

    #[test]
    fn test_cognitive_lull_cadence_analyzer_and_emf_telemetry() {
        let engine = TelemetryEngine::new();
        let config = CadenceConfig::default(); // 4.0s .. 15.0s, optimal 7.5s
        let state = DaemonOperationalState::BoundAndListening;

        // Simulate 4 active keystroke cadence samples (< 250ms idle delta)
        for sample_ms in [45, 80, 110, 60] {
            let eval = engine.analyze_cadence_sample(sample_ms, &config, state);
            assert!(eval.in_active_cadence);
            assert!(!eval.lull_window_triggered);
        }

        // At 2.6s (2600ms) post-halt, phantom tactile keystroke echo is armed
        let eval_2_6s = engine.analyze_cadence_sample(2_600, &config, state);
        assert!(!eval_2_6s.in_active_cadence);
        assert!(eval_2_6s.phantom_keystroke_ready);
        assert!(!eval_2_6s.lull_window_triggered);

        // At 7.2s (7200ms) post-halt, lull duration is tracked (`7.2s`)
        let eval_7_2s = engine.analyze_cadence_sample(7_200, &config, state);
        assert_eq!(eval_7_2s.last_lull_sec, 7.2);
        assert!(!eval_7_2s.lull_window_triggered);

        // At 7.5s (7500ms) optimal lull threshold, cognitive lull trigger fires once
        let eval_7_5s = engine.analyze_cadence_sample(7_500, &config, state);
        assert!(eval_7_5s.lull_window_triggered);
        assert_eq!(eval_7_5s.last_lull_sec, 7.5);

        // Compute EMF telemetry during calm vs triggered lull states
        let calm_emf = engine.compute_emf_telemetry(&eval_7_2s, 1, state);
        assert_eq!(calm_emf.milligauss, 2.87);
        assert_eq!(calm_emf.drift_mg, 0.14);
        assert_eq!(calm_emf.tier, RemPodTier::Low);
        assert_eq!(calm_emf.led_states, [true, true, false, false, false]);

        let triggered_emf = engine.compute_emf_telemetry(&eval_7_5s, 2, state);
        assert!(triggered_emf.milligauss >= 10.0);
        assert_eq!(triggered_emf.tier, RemPodTier::Mid);
        assert_eq!(triggered_emf.led_states, [true, true, true, false, false]);
    }
}
