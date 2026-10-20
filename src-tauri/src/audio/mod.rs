//! Low-Latency Spatial Acoustic Engine (`src-tauri/src/audio/`).
//!
//! Bypasses high-level desktop sound stacks where possible via `cpal` at `48.0 kHz` stereo PCM,
//! utilizing a lock-free atomic ringbuffer (`0xDEADBEEF` sentinel) to guarantee `< 15ms`
//! acoustic dispatch latency.
//!
//! Procedural Acoustic Assets:
//! 1. **Spatial Binaural Floorboard Creaks:** Low-frequency stick-slip modal synthesis (`42–185 Hz`)
//!    with equal-power stereo panning, Interaural Time Difference (ITD) micro-delays (`<= 0.65ms`),
//!    and thermal sympathy pitch modulation.
//! 2. **Sub-Perceptual Carrier Weight:** Near-infrasonic (`18–24 Hz`) and high-frequency binaural
//!    phase modulation synced to REM-Pod disturbance tiers.
//! 3. **Phantom Tactile Keystrokes:** Muted, low-pass filtered mechanical keystroke transient echoes
//!    rendered `2.5s` after an abrupt cognitive typing halt.
//!
//! Hardware Safety Guardrail:
//! All synthesized samples pass through a hard-clamped internal decibel limiter referenced against
//! the ambient room acoustic floor (`-44.2 dBFS`), strictly bounded between `+6.0 dB` and `+18.0 dB`
//! above ambient floor (default operational peak cap: `+12.0 dB`).

use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::{Deserialize, Serialize};

/// Standard engine sample rate (`SWEEP: 48.0 kHz · BINAURAL · RAW PCM`).
pub const SAMPLE_RATE_HZ: u32 = 48_000;
/// Stereo channel count (`2` channels: Left, Right).
pub const CHANNELS: usize = 2;
/// Ringbuffer sentinel identifier displayed in the Séance Panel (`RING: 0xDEADBEEF`).
pub const RING_SENTINEL_ID: u32 = 0xDEAD_BEEF;
/// Lock-free stereo ringbuffer capacity in interleaved `f32` samples (~2 seconds at 48 kHz stereo).
pub const RINGBUFFER_CAPACITY_SAMPLES: usize = 48_000 * CHANNELS * 2;
/// Number of points in the CRT EVP Spectrograph `150ms SWEEP` tap buffer.
pub const SPECTROGRAPH_SWEEP_POINTS: usize = 64;
/// Estimated ambient room acoustic floor in dBFS (`-44.2 dBFS (BALANCED)`).
pub const AMBIENT_FLOOR_DBFS: f32 = -44.2;
/// Minimum allowed peak cap above ambient room floor (`+6.0 dB`).
pub const MIN_PEAK_CAP_OVER_FLOOR_DB: f32 = 6.0;
/// Default operational peak cap above ambient room floor (`PEAK CAP: +12 dB`).
pub const DEFAULT_PEAK_CAP_OVER_FLOOR_DB: f32 = 12.0;
/// Immutable hardware safety ceiling above ambient room floor (`+18.0 dB` hard clamp).
pub const MAX_HARD_CEILING_OVER_FLOOR_DB: f32 = 18.0;
/// Maximum Interaural Time Difference (ITD) wood-head delay in seconds (`0.65ms` -> 31 samples at 48kHz).
pub const MAX_ITD_DELAY_SEC: f32 = 0.00065;

/// Lock-free, wait-free atomic ringbuffer for real-time `cpal` audio stream ingestion.
pub struct LockFreePcmRingBuffer {
    buffer: Box<[AtomicU32]>,
    capacity: usize,
    write_idx: AtomicUsize,
    read_idx: AtomicUsize,
    underflow_count: AtomicU64,
    recent_peak_bits: AtomicU32,
    scope_tap: [AtomicU32; SPECTROGRAPH_SWEEP_POINTS],
    scope_write_idx: AtomicUsize,
}

impl LockFreePcmRingBuffer {
    pub fn with_capacity(capacity: usize) -> Self {
        let mut vec = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            vec.push(AtomicU32::new(0.0_f32.to_bits()));
        }
        Self {
            buffer: vec.into_boxed_slice(),
            capacity,
            write_idx: AtomicUsize::new(0),
            read_idx: AtomicUsize::new(0),
            underflow_count: AtomicU64::new(0),
            recent_peak_bits: AtomicU32::new(0.0_f32.to_bits()),
            scope_tap: std::array::from_fn(|_| AtomicU32::new(0.0_f32.to_bits())),
            scope_write_idx: AtomicUsize::new(0),
        }
    }

    /// Pushes interleaved stereo `f32` samples into the ringbuffer without blocking or allocating.
    /// Returns the number of samples written.
    pub fn push_interleaved(&self, samples: &[f32]) -> usize {
        let mut w = self.write_idx.load(Ordering::Relaxed);
        let r = self.read_idx.load(Ordering::Acquire);
        let mut written = 0;
        let mut max_abs = f32::from_bits(self.recent_peak_bits.load(Ordering::Relaxed));

        let stride = (samples.len() / SPECTROGRAPH_SWEEP_POINTS).max(1);

        for (i, &sample) in samples.iter().enumerate() {
            let next_w = (w + 1) % self.capacity;
            if next_w == r {
                // Ringbuffer full; drop remaining tail to preserve real-time wait-free guarantee
                break;
            }
            let abs = sample.abs();
            if abs > max_abs {
                max_abs = abs;
            }
            if i % stride == 0 {
                let tap_idx = self.scope_write_idx.fetch_add(1, Ordering::Relaxed)
                    % SPECTROGRAPH_SWEEP_POINTS;
                self.scope_tap[tap_idx].store(sample.to_bits(), Ordering::Relaxed);
            }
            self.buffer[w].store(sample.to_bits(), Ordering::Relaxed);
            w = next_w;
            written += 1;
        }

        self.recent_peak_bits
            .store(max_abs.to_bits(), Ordering::Relaxed);
        self.write_idx.store(w, Ordering::Release);
        written
    }

    /// Pops a single `f32` sample for the `cpal` real-time callback, returning `None` on empty.
    #[inline]
    pub fn pop_sample(&self) -> Option<f32> {
        let r = self.read_idx.load(Ordering::Relaxed);
        let w = self.write_idx.load(Ordering::Acquire);
        if r == w {
            // Apply smooth exponential decay to peak detector when buffer drains
            let prev = f32::from_bits(self.recent_peak_bits.load(Ordering::Relaxed));
            if prev > 1e-6 {
                let decayed = prev * 0.9995;
                self.recent_peak_bits.store(
                    if decayed < 1e-5 { 0.0_f32 } else { decayed }.to_bits(),
                    Ordering::Relaxed,
                );
            }
            return None;
        }
        let bits = self.buffer[r].swap(0.0_f32.to_bits(), Ordering::Relaxed);
        let next_r = (r + 1) % self.capacity;
        self.read_idx.store(next_r, Ordering::Release);
        Some(f32::from_bits(bits))
    }

    /// Decays the peak meter once per 150ms telemetry sweep when no audio stream callback is consuming.
    pub fn decay_peak_for_sweep(&self) {
        if self.available_samples() == 0 {
            let prev = f32::from_bits(self.recent_peak_bits.load(Ordering::Relaxed));
            if prev > 0.0 {
                let next = if prev * 0.72 < 1e-4 { 0.0 } else { prev * 0.72 };
                self.recent_peak_bits
                    .store(next.to_bits(), Ordering::Relaxed);
            }
        }
    }

    /// Instantly zeroes and severs all queued samples in the ringbuffer (used by Emergency Exorcise Circuit).
    pub fn sever_and_clear(&self) {
        let w = self.write_idx.load(Ordering::Acquire);
        self.read_idx.store(w, Ordering::Release);
        for slot in self.buffer.iter() {
            slot.store(0.0_f32.to_bits(), Ordering::Relaxed);
        }
        for tap in self.scope_tap.iter() {
            tap.store(0.0_f32.to_bits(), Ordering::Relaxed);
        }
        self.recent_peak_bits
            .store(0.0_f32.to_bits(), Ordering::Relaxed);
    }

    /// Reads a normalized snapshot of the 64-point oscilloscope tap buffer.
    pub fn read_scope_tap(&self) -> [f32; SPECTROGRAPH_SWEEP_POINTS] {
        let mut out = [0.0_f32; SPECTROGRAPH_SWEEP_POINTS];
        let base = self.scope_write_idx.load(Ordering::Relaxed);
        for (i, slot) in out.iter_mut().enumerate() {
            let idx = (base + i) % SPECTROGRAPH_SWEEP_POINTS;
            *slot = f32::from_bits(self.scope_tap[idx].load(Ordering::Relaxed));
        }
        out
    }

    /// Returns the number of queued interleaved samples ready for playback.
    pub fn available_samples(&self) -> usize {
        let w = self.write_idx.load(Ordering::Acquire);
        let r = self.read_idx.load(Ordering::Acquire);
        if w >= r {
            w - r
        } else {
            self.capacity - r + w
        }
    }

    pub fn record_underflow(&self) {
        self.underflow_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn underflow_count(&self) -> u64 {
        self.underflow_count.load(Ordering::Relaxed)
    }

    pub fn recent_peak_linear(&self) -> f32 {
        f32::from_bits(self.recent_peak_bits.load(Ordering::Relaxed))
    }
}

/// Telemetry snapshot for the CRT EVP Spectrograph (`150ms SWEEP`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcousticTelemetrySnapshot {
    pub sample_rate_hz: u32,
    pub armed: bool,
    pub stream_active: bool,
    pub peak_cap_db: f32,
    pub ambient_floor_dbfs: f32,
    pub current_level_dbfs: f32,
    pub current_delta_mv: f32,
    pub ring_sentinel_hex: String,
    pub queued_samples: usize,
    pub failover_recoveries: u64,
}

/// Converts a decibel value (dBFS) into linear amplitude `[0.0, 1.0]`.
#[inline]
pub fn dbfs_to_linear(dbfs: f32) -> f32 {
    10.0_f32.powf(dbfs / 20.0)
}

/// Converts a linear amplitude into dBFS, clamped to the ambient floor when silent.
#[inline]
pub fn linear_to_dbfs(linear: f32) -> f32 {
    let floor_lin = dbfs_to_linear(AMBIENT_FLOOR_DBFS);
    if linear <= floor_lin {
        AMBIENT_FLOOR_DBFS
    } else {
        20.0 * linear.log10()
    }
}

/// Computes the hard linear amplitude ceiling for a requested `peak_cap_over_floor_db`
/// (`+6.0 dB` to `+18.0 dB` above `-44.2 dBFS`).
#[inline]
pub fn compute_clamped_ceiling_linear(peak_cap_over_floor_db: f32) -> f32 {
    let clamped_over_db =
        peak_cap_over_floor_db.clamp(MIN_PEAK_CAP_OVER_FLOOR_DB, MAX_HARD_CEILING_OVER_FLOOR_DB);
    let ceiling_dbfs = AMBIENT_FLOOR_DBFS + clamped_over_db;
    dbfs_to_linear(ceiling_dbfs)
}

/// Soft-knee hyperbolic tangent limiter followed by a hard ceiling clamp.
#[inline]
pub fn apply_hard_decibel_limiter(sample: f32, ceiling_linear: f32) -> f32 {
    if ceiling_linear <= 0.0 {
        return 0.0;
    }
    let normalized = sample / ceiling_linear;
    let soft_limited = normalized.tanh() * ceiling_linear;
    soft_limited.clamp(-ceiling_linear, ceiling_linear)
}

/// Procedural synthesis routines for spatial creaks, sub-perceptual carrier weight,
/// and phantom tactile keystroke echoes.
pub mod synthesis {
    use super::*;

    fn compute_binaural_pan_and_itd(pan_position: f32) -> (f32, f32, usize, usize) {
        let pan = pan_position.clamp(-1.0, 1.0);
        let angle = (pan + 1.0) * 0.5 * FRAC_PI_2;
        let gain_left = angle.cos();
        let gain_right = angle.sin();
        let itd_samples =
            ((pan.abs() * MAX_ITD_DELAY_SEC) * SAMPLE_RATE_HZ as f32).round() as usize;
        let delay_left = if pan > 0.0 { itd_samples } else { 0 };
        let delay_right = if pan < 0.0 { itd_samples } else { 0 };
        (gain_left, gain_right, delay_left, delay_right)
    }

    /// Synthesizes a spatial binaural sub-floor creak using stick-slip modal resonance (`42–185 Hz`),
    /// equal-power stereo panning (`pan` in `[-1.0, 1.0]`), wood-head ITD micro-delays,
    /// and thermal sympathy pitch scaling (`pitch_factor` in `[0.90, 1.10]`).
    pub fn synthesize_spatial_creak_with_pitch(
        pan_position: f32,
        intensity_detent: u8,
        peak_cap_db: f32,
        pitch_factor: f32,
    ) -> Vec<f32> {
        let pan = pan_position.clamp(-1.0, 1.0);
        let pitch = pitch_factor.clamp(0.90, 1.10);
        let duration_sec = match intensity_detent {
            1 => 0.38, // Subtle micro-creak
            2 => 0.55, // Unsettle structural shift
            _ => 0.72, // Acute sub-floor groan
        };
        let total_frames = (SAMPLE_RATE_HZ as f32 * duration_sec) as usize;
        let ceiling_lin = compute_clamped_ceiling_linear(peak_cap_db);
        let detent_gain = match intensity_detent {
            1 => 0.65,
            2 => 0.85,
            _ => 1.0,
        };

        // Equal-power stereo panning angle in [0, PI/2]
        let angle = (pan + 1.0) * 0.5 * FRAC_PI_2;
        let gain_left = angle.cos();
        let gain_right = angle.sin();

        // Interaural Time Difference (ITD) in samples (up to 31 samples at 48 kHz)
        let itd_samples =
            ((pan.abs() * MAX_ITD_DELAY_SEC) * SAMPLE_RATE_HZ as f32).round() as usize;
        let delay_left = if pan > 0.0 { itd_samples } else { 0 };
        let delay_right = if pan < 0.0 { itd_samples } else { 0 };

        let mut mono = vec![0.0_f32; total_frames];
        for (i, slot) in mono.iter_mut().enumerate() {
            let t = i as f32 / SAMPLE_RATE_HZ as f32;
            let progress = i as f32 / total_frames as f32;

            // Stick-slip impulse train envelope + wood modal frequencies (54 Hz, 112 Hz, 168 Hz)
            let env = (progress * PI).sin().powi(2);
            let slip_rate = (14.0 + 18.0 * (1.0 - progress)) * pitch;
            let grain = (TAU * slip_rate * t).sin().abs().powf(3.0);

            let mode1 = (TAU * (54.0 * pitch) * t).sin() * 0.58;
            let mode2 = (TAU * (112.0 * pitch) * t + 0.4).sin() * 0.28;
            let mode3 = (TAU * (168.0 * pitch) * t + 1.1).sin() * 0.14;

            *slot = (mode1 + mode2 + mode3) * grain * env * ceiling_lin * detent_gain;
        }

        let out_frames = total_frames + itd_samples;
        let mut stereo = vec![0.0_f32; out_frames * CHANNELS];
        for frame in 0..out_frames {
            let src_l = frame
                .checked_sub(delay_left)
                .and_then(|idx| mono.get(idx).copied())
                .unwrap_or(0.0);
            let src_r = frame
                .checked_sub(delay_right)
                .and_then(|idx| mono.get(idx).copied())
                .unwrap_or(0.0);

            stereo[frame * 2] = apply_hard_decibel_limiter(src_l * gain_left, ceiling_lin);
            stereo[frame * 2 + 1] = apply_hard_decibel_limiter(src_r * gain_right, ceiling_lin);
        }

        stereo
    }

    /// Synthesizes a spatial binaural sub-floor creak at unity pitch (`1.0`).
    pub fn synthesize_spatial_creak(
        pan_position: f32,
        intensity_detent: u8,
        peak_cap_db: f32,
    ) -> Vec<f32> {
        synthesize_spatial_creak_with_pitch(pan_position, intensity_detent, peak_cap_db, 1.0)
    }

    /// Synthesizes a muted, low-pass filtered tactile keystroke echo (`2.5s` post-typing halt)
    /// with thermal sympathy pitch modulation.
    pub fn synthesize_phantom_keystroke_echo_with_pitch(
        pan_position: f32,
        peak_cap_db: f32,
        pitch_factor: f32,
    ) -> Vec<f32> {
        let pan = pan_position.clamp(-1.0, 1.0);
        let pitch = pitch_factor.clamp(0.90, 1.10);
        let duration_sec = 0.065; // 65ms damped tactile switch transient + housing decay
        let total_frames = (SAMPLE_RATE_HZ as f32 * duration_sec) as usize;
        let ceiling_lin = compute_clamped_ceiling_linear(peak_cap_db);

        let angle = (pan + 1.0) * 0.5 * FRAC_PI_2;
        let gain_l = angle.cos();
        let gain_r = angle.sin();

        let mut stereo = Vec::with_capacity(total_frames * CHANNELS);
        let mut lp_state = 0.0_f32;
        let lp_alpha = 0.22_f32; // One-pole low-pass filter to mute sharp upper harmonics

        for i in 0..total_frames {
            let t = i as f32 / SAMPLE_RATE_HZ as f32;
            // Fast exponential attack/decay for tactile plate contact + sub-keycap thud (210 Hz & 840 Hz)
            let click_env = (-t * 140.0).exp();
            let thud_env = (-t * 55.0).exp();

            let raw = ((TAU * (210.0 * pitch) * t).sin() * 0.72 * thud_env
                + (TAU * (840.0 * pitch) * t).sin() * 0.28 * click_env)
                * ceiling_lin
                * 0.75;

            lp_state += lp_alpha * (raw - lp_state);

            stereo.push(apply_hard_decibel_limiter(lp_state * gain_l, ceiling_lin));
            stereo.push(apply_hard_decibel_limiter(lp_state * gain_r, ceiling_lin));
        }

        stereo
    }

    /// Synthesizes a muted phantom keystroke echo at unity pitch (`1.0`).
    pub fn synthesize_phantom_keystroke_echo(pan_position: f32, peak_cap_db: f32) -> Vec<f32> {
        synthesize_phantom_keystroke_echo_with_pitch(pan_position, peak_cap_db, 1.0)
    }

    /// Synthesizes a near-infrasonic (`18–24 Hz`) & binaural phase-shifted carrier weight pulse.
    pub fn synthesize_sub_perceptual_carrier(
        base_freq_hz: f32,
        duration_sec: f32,
        peak_cap_db: f32,
    ) -> Vec<f32> {
        let freq_l = base_freq_hz.clamp(18.0, 24.0);
        let freq_r = (freq_l + 1.5).clamp(18.0, 25.5); // 1.5 Hz binaural delta beat
        let total_frames = (SAMPLE_RATE_HZ as f32 * duration_sec.clamp(0.05, 2.0)) as usize;
        let ceiling_lin = compute_clamped_ceiling_linear(peak_cap_db);

        let mut stereo = Vec::with_capacity(total_frames * CHANNELS);
        for i in 0..total_frames {
            let t = i as f32 / SAMPLE_RATE_HZ as f32;
            let progress = i as f32 / total_frames as f32;
            let window = (progress * PI).sin();

            let sample_l = (TAU * freq_l * t).sin() * window * ceiling_lin * 0.55;
            let sample_r = (TAU * freq_r * t).sin() * window * ceiling_lin * 0.55;

            stereo.push(apply_hard_decibel_limiter(sample_l, ceiling_lin));
            stereo.push(apply_hard_decibel_limiter(sample_r, ceiling_lin));
        }

        stereo
    }

    /// Synthesizes an EVP Spirit-Box Formant Whisper (`420ms` breathy vocal-tract formant sweep
    /// across F1/F2/F3 resonators with a faint AM heterodyne radio carrier whistle).
    pub fn synthesize_evp_formant_whisper(
        pan_position: f32,
        intensity_detent: u8,
        peak_cap_db: f32,
        pitch_factor: f32,
    ) -> Vec<f32> {
        let duration_sec = 0.42_f32;
        let total_frames = (SAMPLE_RATE_HZ as f32 * duration_sec) as usize;
        let ceiling_lin = compute_clamped_ceiling_linear(peak_cap_db);
        let pitch = pitch_factor.clamp(0.90, 1.10);
        let detent_scale = match intensity_detent {
            1 => 0.55,
            2 => 0.78,
            3 => 1.0,
            _ => 0.55,
        };

        let (gain_l, gain_r, delay_l, delay_r) = compute_binaural_pan_and_itd(pan_position);
        let mut mono = Vec::with_capacity(total_frames);
        let mut lfsr: u32 = 0x9E37_79B9;
        let mut bp1_z1 = 0.0_f32;
        let mut bp1_z2 = 0.0_f32;
        let mut bp2_z1 = 0.0_f32;
        let mut bp2_z2 = 0.0_f32;

        for i in 0..total_frames {
            let t = i as f32 / SAMPLE_RATE_HZ as f32;
            let progress = i as f32 / total_frames as f32;

            // Deterministic xorshift noise excitation
            lfsr ^= lfsr << 13;
            lfsr ^= lfsr >> 17;
            lfsr ^= lfsr << 5;
            let white = ((lfsr & 0xFFFF) as f32 / 32768.0) - 1.0;

            // Two-syllable breathy envelope ("hh-ah... shh")
            let syl_env = (progress * PI).sin() * (0.65 + 0.35 * (TAU * 4.2 * t).sin());

            // Sweeping vocal tract formants F1 (420 -> 610 Hz) and F2 (1280 -> 1720 Hz)
            let f1 = (420.0 + 190.0 * (progress * PI).sin()) * pitch;
            let f2 = (1720.0 - 440.0 * progress) * pitch;

            let w1 = TAU * f1 / SAMPLE_RATE_HZ as f32;
            let r1 = 0.965_f32;
            let y1 = white * (1.0 - r1) + 2.0 * r1 * w1.cos() * bp1_z1 - r1 * r1 * bp1_z2;
            bp1_z2 = bp1_z1;
            bp1_z1 = y1;

            let w2 = TAU * f2 / SAMPLE_RATE_HZ as f32;
            let r2 = 0.955_f32;
            let y2 = white * (1.0 - r2) + 2.0 * r2 * w2.cos() * bp2_z1 - r2 * r2 * bp2_z2;
            bp2_z2 = bp2_z1;
            bp2_z1 = y2;

            // Faint AM spirit-box heterodyne sweep (1480 Hz -> 920 Hz)
            let hetero_freq = (1480.0 - 560.0 * progress) * pitch;
            let heterodyne = (TAU * hetero_freq * t).sin() * 0.14;

            let aspiration = white * 0.18;
            let raw = (y1 * 0.52 + y2 * 0.36 + aspiration + heterodyne) * syl_env;
            mono.push((raw * 1.6).tanh() * ceiling_lin * detent_scale);
        }

        let mut stereo = Vec::with_capacity(total_frames * CHANNELS);
        for i in 0..total_frames {
            let s_l = if i >= delay_l {
                mono[i - delay_l] * gain_l
            } else {
                0.0
            };
            let s_r = if i >= delay_r {
                mono[i - delay_r] * gain_r
            } else {
                0.0
            };
            stereo.push(apply_hard_decibel_limiter(s_l, ceiling_lin));
            stereo.push(apply_hard_decibel_limiter(s_r, ceiling_lin));
        }
        stereo
    }

    /// Synthesizes a spatialized Wall-Stud Double-Knock (`0ms` and `185ms` cavity impacts at
    /// `92 Hz`/`196 Hz` preceded by a `3.8 kHz` electromagnetic reed-relay click).
    pub fn synthesize_wall_stud_double_knock(
        pan_position: f32,
        intensity_detent: u8,
        peak_cap_db: f32,
        pitch_factor: f32,
    ) -> Vec<f32> {
        let duration_sec = 0.38_f32;
        let total_frames = (SAMPLE_RATE_HZ as f32 * duration_sec) as usize;
        let second_knock_frame = (SAMPLE_RATE_HZ as f32 * 0.185) as usize;
        let ceiling_lin = compute_clamped_ceiling_linear(peak_cap_db);
        let pitch = pitch_factor.clamp(0.90, 1.10);
        let detent_scale = match intensity_detent {
            1 => 0.62,
            2 => 0.84,
            3 => 1.0,
            _ => 0.62,
        };

        let (gain_l, gain_r, delay_l, delay_r) = compute_binaural_pan_and_itd(pan_position);
        let mut mono = Vec::with_capacity(total_frames);

        for i in 0..total_frames {
            let t1 = i as f32 / SAMPLE_RATE_HZ as f32;

            // Electromagnetic reed-relay click precursor in first 3ms
            let relay_click = if t1 < 0.003 {
                (-t1 * 1100.0).exp() * (TAU * 3800.0 * pitch * t1).sin() * 0.45
            } else {
                0.0
            };

            // First drywall/stud cavity impact at t = 0
            let env1 = (-t1 * 28.0).exp();
            let knock1 = ((TAU * 92.0 * pitch * t1).sin() * 0.65
                + (TAU * 196.0 * pitch * t1).sin() * 0.25
                + (TAU * 540.0 * pitch * t1).sin() * (-t1 * 140.0).exp() * 0.30)
                * env1;

            // Second firmer stud impact at t = 185ms
            let knock2 = if i >= second_knock_frame {
                let t2 = (i - second_knock_frame) as f32 / SAMPLE_RATE_HZ as f32;
                let env2 = (-t2 * 24.0).exp();
                ((TAU * 88.0 * pitch * t2).sin() * 0.72
                    + (TAU * 184.0 * pitch * t2).sin() * 0.28
                    + (TAU * 490.0 * pitch * t2).sin() * (-t2 * 130.0).exp() * 0.35)
                    * env2
            } else {
                0.0
            };

            let combined = (relay_click + knock1 + knock2).clamp(-1.0, 1.0);
            mono.push(combined * ceiling_lin * detent_scale);
        }

        let mut stereo = Vec::with_capacity(total_frames * CHANNELS);
        for i in 0..total_frames {
            let s_l = if i >= delay_l {
                mono[i - delay_l] * gain_l
            } else {
                0.0
            };
            let s_r = if i >= delay_r {
                mono[i - delay_r] * gain_r
            } else {
                0.0
            };
            stereo.push(apply_hard_decibel_limiter(s_l, ceiling_lin));
            stereo.push(apply_hard_decibel_limiter(s_r, ceiling_lin));
        }
        stereo
    }

    /// Synthesizes a `20+ mG` Redline `60 Hz` Mains Hum & CRT Transformer Buzz (`260ms`).
    pub fn synthesize_redline_mains_hum(peak_cap_db: f32) -> Vec<f32> {
        let duration_sec = 0.26_f32;
        let total_frames = (SAMPLE_RATE_HZ as f32 * duration_sec) as usize;
        let ceiling_lin = compute_clamped_ceiling_linear(peak_cap_db);
        let mut stereo = Vec::with_capacity(total_frames * CHANNELS);

        for i in 0..total_frames {
            let t = i as f32 / SAMPLE_RATE_HZ as f32;
            let progress = i as f32 / total_frames as f32;
            let env = (progress * PI).sin();

            let hum_60 = (TAU * 60.0 * t).sin() * 0.55;
            let hum_120 = (TAU * 120.0 * t).sin() * 0.28;
            let buzz_180 = (TAU * 180.0 * t).sin() * 0.14;
            let corona_2400 = (TAU * 2400.0 * t).sin() * (TAU * 60.0 * t).cos() * 0.08;

            let sig = (hum_60 + hum_120 + buzz_180 + corona_2400) * env * ceiling_lin * 0.90;
            stereo.push(apply_hard_decibel_limiter(sig, ceiling_lin));
            stereo.push(apply_hard_decibel_limiter(-sig * 0.92, ceiling_lin));
        }
        stereo
    }

    /// Synthesizes an Infrasonic "Room Breath" (`18–24 Hz` carrier coupled with a `42 Hz`
    /// sub-bass pressure exhalation modulated by live CPU load).
    pub fn synthesize_infrasonic_room_breath(
        cpu_load_norm: f32,
        peak_cap_db: f32,
        pitch_factor: f32,
    ) -> Vec<f32> {
        let duration_sec = 0.55_f32;
        let total_frames = (SAMPLE_RATE_HZ as f32 * duration_sec) as usize;
        let ceiling_lin = compute_clamped_ceiling_linear(peak_cap_db);
        let load = cpu_load_norm.clamp(0.15, 1.0);
        let pitch = pitch_factor.clamp(0.90, 1.10);

        let f_infra = (19.5 + 4.0 * load) * pitch;
        let f_sub = (41.0 + 9.0 * load) * pitch;

        let mut stereo = Vec::with_capacity(total_frames * CHANNELS);
        for i in 0..total_frames {
            let t = i as f32 / SAMPLE_RATE_HZ as f32;
            let progress = i as f32 / total_frames as f32;
            // Raised-cosine inhalation/exhalation curve
            let breath_env = 0.5 * (1.0 - (TAU * progress).cos());

            let l = ((TAU * f_infra * t).sin() * 0.55 + (TAU * f_sub * t).sin() * 0.45)
                * breath_env
                * ceiling_lin
                * (0.55 + 0.35 * load);
            let r = ((TAU * (f_infra + 1.2) * t).sin() * 0.55
                + (TAU * (f_sub + 1.8) * t).sin() * 0.45)
                * breath_env
                * ceiling_lin
                * (0.55 + 0.35 * load);

            stereo.push(apply_hard_decibel_limiter(l, ceiling_lin));
            stereo.push(apply_hard_decibel_limiter(r, ceiling_lin));
        }
        stereo
    }
}

/// Low-Latency Spatial Acoustic Engine managing the `cpal` stream and lock-free ringbuffer.
pub struct CpalAcousticEngine {
    pub sample_rate_hz: u32,
    peak_cap_bits: AtomicU32,
    sympathy_pitch_bits: AtomicU32,
    armed: AtomicBool,
    stream_healthy: Arc<AtomicBool>,
    failover_recoveries: Arc<AtomicU64>,
    ringbuffer: Arc<LockFreePcmRingBuffer>,
    /// Holds the active `cpal::Stream` when bound to a host audio output device.
    active_stream: Mutex<Option<cpal::Stream>>,
}

// SAFETY: `cpal::Stream` is guarded by a `Mutex` and only accessed for lifecycle transitions
// (init, failover re-init, and emergency sever). Real-time audio threads only touch `Arc<LockFreePcmRingBuffer>`.
unsafe impl Send for CpalAcousticEngine {}
unsafe impl Sync for CpalAcousticEngine {}

impl Default for CpalAcousticEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CpalAcousticEngine {
    pub fn new() -> Self {
        let ringbuffer = Arc::new(LockFreePcmRingBuffer::with_capacity(
            RINGBUFFER_CAPACITY_SAMPLES,
        ));
        let stream_healthy = Arc::new(AtomicBool::new(false));
        let failover_recoveries = Arc::new(AtomicU64::new(0));

        let engine = Self {
            sample_rate_hz: SAMPLE_RATE_HZ,
            peak_cap_bits: AtomicU32::new(DEFAULT_PEAK_CAP_OVER_FLOOR_DB.to_bits()),
            sympathy_pitch_bits: AtomicU32::new(1.0_f32.to_bits()),
            armed: AtomicBool::new(true),
            stream_healthy,
            failover_recoveries,
            ringbuffer,
            active_stream: Mutex::new(None),
        };

        // Attempt non-blocking initialization of default `cpal` output stream;
        // if running in a headless environment without audio hardware, fail over silently.
        let _ = engine.try_initialize_cpal_stream();
        engine
    }

    /// Initializes or re-initializes the native `cpal` output stream.
    /// Drops frames silently on underflow and marks stream unhealthy on device disconnect.
    pub fn try_initialize_cpal_stream(&self) -> Result<(), String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| "No default audio output device available".to_string())?;

        let config =
            device
                .default_output_config()
                .map(|c| c.into())
                .unwrap_or(cpal::StreamConfig {
                    channels: CHANNELS as u16,
                    sample_rate: cpal::SampleRate(self.sample_rate_hz),
                    buffer_size: cpal::BufferSize::Default,
                });

        let rb = Arc::clone(&self.ringbuffer);
        let healthy_cb = Arc::clone(&self.stream_healthy);
        let healthy_err = Arc::clone(&self.stream_healthy);
        let recoveries = Arc::clone(&self.failover_recoveries);

        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let mut had_underflow = false;
                    for sample in data.iter_mut() {
                        match rb.pop_sample() {
                            Some(val) => *sample = (val * 16.0).clamp(-0.85, 0.85),
                            None => {
                                *sample = 0.0;
                                had_underflow = true;
                            }
                        }
                    }
                    if had_underflow {
                        rb.record_underflow();
                    }
                },
                move |_err| {
                    // Audio pipeline failover: mark unhealthy silently without panicking
                    healthy_err.store(false, Ordering::SeqCst);
                    recoveries.fetch_add(1, Ordering::Relaxed);
                },
                None,
            )
            .map_err(|e| format!("Failed to build cpal output stream: {e}"))?;

        stream
            .play()
            .map_err(|e| format!("Failed to start cpal output stream: {e}"))?;

        healthy_cb.store(true, Ordering::SeqCst);

        if let Ok(mut guard) = self.active_stream.lock() {
            *guard = Some(stream);
        }

        Ok(())
    }

    /// Ensures the `cpal` stream is healthy, attempting silent re-initialization if disconnected.
    pub fn ensure_stream_or_failover(&self) {
        if !self.armed.load(Ordering::Acquire) {
            return;
        }
        if !self.stream_healthy.load(Ordering::Acquire) {
            let _ = self.try_initialize_cpal_stream();
        }
    }

    /// Sets the operational decibel peak cap (`+6.0 dB` to `+18.0 dB` over ambient floor).
    pub fn set_peak_cap_db(&self, peak_cap_db: f32) {
        let clamped = peak_cap_db.clamp(MIN_PEAK_CAP_OVER_FLOOR_DB, MAX_HARD_CEILING_OVER_FLOOR_DB);
        self.peak_cap_bits
            .store(clamped.to_bits(), Ordering::Release);
    }

    pub fn peak_cap_db(&self) -> f32 {
        f32::from_bits(self.peak_cap_bits.load(Ordering::Acquire))
    }

    /// Sets the thermal sympathy pitch modulation multiplier (`0.90 ..= 1.10`).
    pub fn set_sympathy_pitch_factor(&self, pitch_factor: f32) {
        let clamped = pitch_factor.clamp(0.90, 1.10);
        self.sympathy_pitch_bits
            .store(clamped.to_bits(), Ordering::Release);
    }

    pub fn sympathy_pitch_factor(&self) -> f32 {
        f32::from_bits(self.sympathy_pitch_bits.load(Ordering::Acquire))
    }

    pub fn is_armed(&self) -> bool {
        self.armed.load(Ordering::Acquire)
    }

    pub fn set_armed(&self, armed: bool) {
        self.armed.store(armed, Ordering::Release);
        if !armed {
            self.ringbuffer.sever_and_clear();
        }
    }

    /// Synthesizes and dispatches a spatial binaural floorboard creak onto the lock-free ringbuffer.
    pub fn dispatch_spatial_creak(&self, pan_position: f32, intensity_detent: u8) -> usize {
        if !self.is_armed() {
            return 0;
        }
        self.ensure_stream_or_failover();
        let pcm = synthesis::synthesize_spatial_creak_with_pitch(
            pan_position,
            intensity_detent,
            self.peak_cap_db(),
            self.sympathy_pitch_factor(),
        );
        self.ringbuffer.push_interleaved(&pcm)
    }

    /// Synthesizes and dispatches a muted phantom tactile keystroke echo (`2.5s` post-halt).
    pub fn dispatch_phantom_keystroke_echo(&self, pan_position: f32) -> usize {
        if !self.is_armed() {
            return 0;
        }
        self.ensure_stream_or_failover();
        let pcm = synthesis::synthesize_phantom_keystroke_echo_with_pitch(
            pan_position,
            self.peak_cap_db(),
            self.sympathy_pitch_factor(),
        );
        self.ringbuffer.push_interleaved(&pcm)
    }

    /// Synthesizes and dispatches a near-infrasonic sub-perceptual carrier pulse (`18–24 Hz`).
    pub fn dispatch_sub_perceptual_carrier(&self, base_freq_hz: f32, duration_sec: f32) -> usize {
        if !self.is_armed() {
            return 0;
        }
        self.ensure_stream_or_failover();
        let pcm = synthesis::synthesize_sub_perceptual_carrier(
            base_freq_hz * self.sympathy_pitch_factor(),
            duration_sec,
            self.peak_cap_db(),
        );
        self.ringbuffer.push_interleaved(&pcm)
    }

    /// Synthesizes and dispatches an EVP Spirit-Box Formant Whisper (`F1/F2` vocal tract sweep + AM heterodyne).
    pub fn dispatch_evp_whisper(&self, pan_position: f32, intensity_detent: u8) -> usize {
        if !self.is_armed() {
            return 0;
        }
        self.ensure_stream_or_failover();
        let pcm = synthesis::synthesize_evp_formant_whisper(
            pan_position,
            intensity_detent,
            self.peak_cap_db(),
            self.sympathy_pitch_factor(),
        );
        self.ringbuffer.push_interleaved(&pcm)
    }

    /// Synthesizes and dispatches a spatialized Wall-Stud Double-Knock & Reed-Relay Click.
    pub fn dispatch_wall_knock(&self, pan_position: f32, intensity_detent: u8) -> usize {
        if !self.is_armed() {
            return 0;
        }
        self.ensure_stream_or_failover();
        let pcm = synthesis::synthesize_wall_stud_double_knock(
            pan_position,
            intensity_detent,
            self.peak_cap_db(),
            self.sympathy_pitch_factor(),
        );
        self.ringbuffer.push_interleaved(&pcm)
    }

    /// Synthesizes and dispatches a `20+ mG` Redline `60 Hz` Mains Hum & CRT Transformer Buzz.
    pub fn dispatch_redline_hum(&self) -> usize {
        if !self.is_armed() {
            return 0;
        }
        self.ensure_stream_or_failover();
        let pcm = synthesis::synthesize_redline_mains_hum(self.peak_cap_db());
        self.ringbuffer.push_interleaved(&pcm)
    }

    /// Synthesizes and dispatches an Infrasonic "Room Breath" modulated by live CPU load (`0.0 ..= 1.0`).
    pub fn dispatch_room_breath(&self, cpu_load_norm: f32) -> usize {
        if !self.is_armed() {
            return 0;
        }
        self.ensure_stream_or_failover();
        let pcm = synthesis::synthesize_infrasonic_room_breath(
            cpu_load_norm,
            self.peak_cap_db(),
            self.sympathy_pitch_factor(),
        );
        self.ringbuffer.push_interleaved(&pcm)
    }

    /// Immediately zeroes the lock-free ringbuffer and drops the active `cpal` stream
    /// (invoked by the Emergency Exorcise Circuit / Panic Hotkey).
    pub fn sever_ringbuffer(&self) {
        self.armed.store(false, Ordering::SeqCst);
        self.ringbuffer.sever_and_clear();
        self.stream_healthy.store(false, Ordering::SeqCst);
        if let Ok(mut guard) = self.active_stream.lock() {
            let _ = guard.take();
        }
    }

    /// Returns a 64-point normalized waveform frame representing a `150ms SWEEP`
    /// of the `cpal` stream for the CRT EVP Spectrograph canvas, blending live PCM tap
    /// samples with the binaural ambient carrier.
    pub fn sample_spectrograph_sweep_frame(&self, phase_rad: f32) -> Vec<f32> {
        self.ringbuffer.decay_peak_for_sweep();
        let peak_lin = self.ringbuffer.recent_peak_linear();
        let tap = self.ringbuffer.read_scope_tap();
        let ceiling_lin = compute_clamped_ceiling_linear(self.peak_cap_db()).max(1e-4);

        let active_boost = if self.is_armed() {
            1.0 + (peak_lin / ceiling_lin).clamp(0.0, 1.2) * 0.65
        } else {
            0.12
        };

        let mut frame = Vec::with_capacity(SPECTROGRAPH_SWEEP_POINTS);
        for (i, &tap_sample) in tap.iter().enumerate() {
            let norm_x = i as f32 / (SPECTROGRAPH_SWEEP_POINTS - 1) as f32;
            let carrier = (norm_x * PI * 5.2 + phase_rad).sin() * 0.42
                + (norm_x * PI * 2.1 - phase_rad * 0.5).sin() * 0.33
                + (norm_x * PI * 11.5 + phase_rad * 1.2).cos() * 0.11
                + (norm_x * PI * 29.0).sin() * 0.04;
            let live_pcm_norm = (tap_sample / ceiling_lin).clamp(-1.0, 1.0) * 0.55;
            frame.push((carrier * active_boost + live_pcm_norm).clamp(-1.0, 1.0));
        }
        frame
    }

    /// Returns a snapshot of the acoustic engine's state for the Séance Panel HUD.
    pub fn snapshot_telemetry(&self) -> AcousticTelemetrySnapshot {
        let peak_lin = self.ringbuffer.recent_peak_linear();
        let current_level_dbfs = (linear_to_dbfs(peak_lin) * 10.0).round() / 10.0;
        let current_delta_mv = if peak_lin <= 0.0 {
            0.041
        } else {
            ((0.041 + peak_lin * 1.25) * 1000.0).round() / 1000.0
        };

        AcousticTelemetrySnapshot {
            sample_rate_hz: self.sample_rate_hz,
            armed: self.is_armed(),
            stream_active: self.stream_healthy.load(Ordering::Relaxed),
            peak_cap_db: self.peak_cap_db(),
            ambient_floor_dbfs: AMBIENT_FLOOR_DBFS,
            current_level_dbfs,
            current_delta_mv,
            ring_sentinel_hex: format!("0x{:08X}", RING_SENTINEL_ID),
            queued_samples: self.ringbuffer.available_samples(),
            failover_recoveries: self.failover_recoveries.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_free_ringbuffer_push_pop_and_sever() {
        let rb = LockFreePcmRingBuffer::with_capacity(64);
        assert_eq!(rb.available_samples(), 0);

        let input = [0.01_f32, -0.01, 0.02, -0.02];
        let written = rb.push_interleaved(&input);
        assert_eq!(written, 4);
        assert_eq!(rb.available_samples(), 4);

        assert_eq!(rb.pop_sample(), Some(0.01));
        assert_eq!(rb.pop_sample(), Some(-0.01));
        assert_eq!(rb.available_samples(), 2);

        // Severing immediately zeroes and empties the ringbuffer
        rb.sever_and_clear();
        assert_eq!(rb.available_samples(), 0);
        assert_eq!(rb.pop_sample(), None);
    }

    #[test]
    fn test_hard_decibel_limiter_enforces_safety_ceiling() {
        // Even if caller passes +50 dB, ceiling is hard-clamped to +18 dB above -44.2 dBFS (-26.2 dBFS)
        let max_ceiling_lin = compute_clamped_ceiling_linear(50.0);
        let expected_max_lin = dbfs_to_linear(AMBIENT_FLOOR_DBFS + MAX_HARD_CEILING_OVER_FLOOR_DB);
        assert!((max_ceiling_lin - expected_max_lin).abs() < 1e-6);

        let creak = synthesis::synthesize_spatial_creak(-0.6, 3, 50.0);
        assert!(!creak.is_empty());
        for &sample in &creak {
            assert!(
                sample.abs() <= expected_max_lin + 1e-6,
                "Sample {} exceeded hard +18dB safety ceiling {}",
                sample,
                expected_max_lin
            );
        }

        let echo =
            synthesis::synthesize_phantom_keystroke_echo(0.4, DEFAULT_PEAK_CAP_OVER_FLOOR_DB);
        let default_ceiling_lin = compute_clamped_ceiling_linear(DEFAULT_PEAK_CAP_OVER_FLOOR_DB);
        for &sample in &echo {
            assert!(sample.abs() <= default_ceiling_lin + 1e-6);
        }
    }

    #[test]
    fn test_dispatch_latency_and_exorcise_sever() {
        let engine = CpalAcousticEngine::new();
        let start = std::time::Instant::now();
        let written = engine.dispatch_spatial_creak(-0.5, 1);
        let elapsed = start.elapsed();

        assert!(written > 0);
        // Dispatch latency onto the lock-free ringbuffer must be well under the 15ms requirement
        assert!(
            elapsed.as_millis() < 15,
            "Acoustic dispatch took {}ms (>= 15ms limit)",
            elapsed.as_millis()
        );

        engine.sever_ringbuffer();
        assert!(!engine.is_armed());
        assert_eq!(engine.snapshot_telemetry().queued_samples, 0);
    }
}
