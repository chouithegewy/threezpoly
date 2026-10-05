//! Band-limited (PolyBLEP) oscillators and per-oscillator tuning.

use std::f32::consts::TAU;

/// The note an oscillator plays at when key tracking is off.
pub const FIXED_PITCH_NOTE: f32 = 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waveform {
    Sine,
    Saw,
    Square,
    Triangle,
    Noise,
}

/// Per-oscillator settings, shared by every voice.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OscSettings {
    pub waveform: Waveform,
    /// Whole octaves, e.g. `-2..=2`.
    pub octave: i32,
    /// Semitones, e.g. `-12..=12`.
    pub semitone: i32,
    /// Fine tuning in cents, `-100.0..=100.0`.
    pub fine_cents: f32,
    /// Output level, `0.0..=1.0`. An oscillator at level zero is skipped entirely.
    pub level: f32,
    /// Pulse width for the square wave, `0.0..=1.0` (0.5 is a square).
    pub pulse_width: f32,
    /// When false the oscillator ignores the played note and stays at [`FIXED_PITCH_NOTE`]
    /// (plus its tuning offset), which is useful for drones and fixed-pitch FM sources.
    pub key_track: bool,
    /// Routes this oscillator into the voice's first filter slot.
    pub to_filter1: bool,
    /// Routes this oscillator into the voice's second filter slot.
    pub to_filter2: bool,
}

impl Default for OscSettings {
    fn default() -> Self {
        Self {
            waveform: Waveform::Saw,
            octave: 0,
            semitone: 0,
            fine_cents: 0.0,
            level: 0.0,
            pulse_width: 0.5,
            key_track: true,
            to_filter1: true,
            to_filter2: false,
        }
    }
}

impl OscSettings {
    /// The total tuning offset in semitones.
    #[inline]
    pub fn offset_semitones(&self) -> f32 {
        (self.octave * 12 + self.semitone) as f32 + self.fine_cents / 100.0
    }

    /// The (fractional) note this oscillator plays for `note`, with `bend` semitones of pitch bend.
    #[inline]
    pub fn pitch(&self, note: f32, bend: f32) -> f32 {
        let base = if self.key_track { note } else { FIXED_PITCH_NOTE };
        base + bend + self.offset_semitones()
    }
}

/// PolyBLEP residual for a discontinuity at phase 0, with `t` the phase and `dt` the phase
/// increment, both in `0.0..1.0`.
#[inline]
fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let t = t / dt;
        t + t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + t + t + 1.0
    } else {
        0.0
    }
}

#[derive(Debug, Clone)]
pub struct Oscillator {
    phase: f32,
    noise_state: u32,
}

impl Oscillator {
    pub fn new(seed: u32) -> Self {
        Self {
            phase: 0.0,
            // xorshift must never be seeded with zero
            noise_state: seed | 1,
        }
    }

    pub fn reset_phase(&mut self, phase: f32) {
        self.phase = phase.rem_euclid(1.0);
    }

    /// Generates one sample and advances the phase by `dt` (frequency / sample rate). `dt` must be
    /// below 0.5.
    #[inline]
    pub fn next(&mut self, waveform: Waveform, dt: f32, pulse_width: f32) -> f32 {
        let t = self.phase;
        let out = match waveform {
            Waveform::Sine => (t * TAU).sin(),
            Waveform::Saw => (2.0 * t - 1.0) - poly_blep(t, dt),
            Waveform::Square => {
                let pw = pulse_width.clamp(0.02, 0.98);
                let naive = if t < pw { 1.0 } else { -1.0 };
                // Rising edge at phase 0, falling edge at phase `pw`
                let t2 = (t - pw).rem_euclid(1.0);
                // Remove the DC offset that a non-50% pulse would otherwise have
                naive + poly_blep(t, dt) - poly_blep(t2, dt) - (2.0 * pw - 1.0)
            }
            // Only has slope discontinuities, so naive aliasing is mild. Could use PolyBLAMP later.
            Waveform::Triangle => 1.0 - 4.0 * (t - 0.5).abs(),
            Waveform::Noise => {
                let mut x = self.noise_state;
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                self.noise_state = x;
                (x as f32 / u32::MAX as f32) * 2.0 - 1.0
            }
        };

        self.phase += dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note_to_freq;

    #[test]
    fn octave_tuning_doubles_frequency() {
        let base = OscSettings::default();
        let up = OscSettings { octave: 1, ..base };
        let down = OscSettings { octave: -2, ..base };
        let f = note_to_freq(base.pitch(57.0, 0.0));
        assert!((note_to_freq(up.pitch(57.0, 0.0)) - 2.0 * f).abs() < 1e-3);
        assert!((note_to_freq(down.pitch(57.0, 0.0)) - f / 4.0).abs() < 1e-3);
    }

    #[test]
    fn semitone_and_fine_tuning_add_up() {
        let s = OscSettings { octave: -1, semitone: 7, fine_cents: -50.0, ..Default::default() };
        assert!((s.offset_semitones() - (-12.0 + 7.0 - 0.5)).abs() < 1e-6);
    }

    #[test]
    fn key_track_off_ignores_note() {
        let s = OscSettings { key_track: false, semitone: 2, ..Default::default() };
        assert_eq!(s.pitch(30.0, 0.0), s.pitch(90.0, 0.0));
        assert_eq!(s.pitch(30.0, 0.0), FIXED_PITCH_NOTE + 2.0);
    }

    #[test]
    fn waveforms_stay_bounded() {
        for wf in [Waveform::Sine, Waveform::Saw, Waveform::Square, Waveform::Triangle, Waveform::Noise] {
            let mut osc = Oscillator::new(1);
            for _ in 0..10_000 {
                let s = osc.next(wf, 0.0371, 0.3);
                assert!(s.is_finite() && s.abs() <= 1.6, "{wf:?} produced {s}");
            }
        }
    }
}
