//! Zero-delay-feedback state variable filter (Andrew Simper / Cytomic design).

use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    /// Passes the signal through untouched (an empty filter slot).
    Off,
    LowPass12,
    LowPass24,
    BandPass,
    HighPass,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FilterSettings {
    pub mode: FilterMode,
    /// Base cutoff in Hz.
    pub cutoff: f32,
    /// `0.0..=1.0`; self-oscillates near the top.
    pub resonance: f32,
    /// Input drive, `0.0..=1.0`.
    pub drive: f32,
    /// Filter envelope depth in octaves (can be negative).
    pub env_amount: f32,
    /// Keyboard tracking, `0.0..=1.0` (1.0 follows the played note exactly).
    pub key_track: f32,
}

impl Default for FilterSettings {
    fn default() -> Self {
        Self {
            mode: FilterMode::LowPass24,
            cutoff: 2_000.0,
            resonance: 0.2,
            drive: 0.0,
            env_amount: 2.0,
            key_track: 0.5,
        }
    }
}

impl FilterSettings {
    /// Settings for a slot that starts out empty.
    pub fn off() -> Self {
        Self { mode: FilterMode::Off, cutoff: 1_000.0, env_amount: 0.0, ..Self::default() }
    }
}

/// The global filter acts on the summed output of all voices, so it has no per-note envelope or
/// key tracking.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlobalFilterSettings {
    pub mode: FilterMode,
    pub cutoff: f32,
    pub resonance: f32,
    pub drive: f32,
}

impl Default for GlobalFilterSettings {
    fn default() -> Self {
        Self { mode: FilterMode::Off, cutoff: 8_000.0, resonance: 0.0, drive: 0.0 }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Svf {
    ic1: f32,
    ic2: f32,
}

#[derive(Debug, Clone, Copy)]
struct SvfOut {
    low: f32,
    band: f32,
    high: f32,
}

impl Svf {
    #[inline]
    fn tick(&mut self, x: f32, g: f32, k: f32) -> SvfOut {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let v3 = x - self.ic2;
        let v1 = a1 * self.ic1 + a2 * v3;
        let v2 = self.ic2 + a2 * self.ic1 + a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        SvfOut { low: v2, band: v1, high: x - k * v1 - v2 }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Filter {
    stage1: Svf,
    stage2: Svf,
}

impl Filter {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Filters one sample. `cutoff` is the final, modulated cutoff in Hz.
    #[inline]
    pub fn process(
        &mut self,
        x: f32,
        mode: FilterMode,
        cutoff: f32,
        resonance: f32,
        drive: f32,
        sample_rate: f32,
    ) -> f32 {
        self.tick(x, mode, &FilterCoeffs::new(cutoff, resonance, drive, sample_rate))
    }

    /// Filters one sample with precomputed coefficients, so a stereo pair can share them.
    #[inline]
    pub fn tick(&mut self, x: f32, mode: FilterMode, c: &FilterCoeffs) -> f32 {
        if mode == FilterMode::Off {
            return x;
        }
        let (g, k) = (c.g, c.k);
        let x = if c.drive_gain > 1.0 {
            (x * c.drive_gain).tanh() / c.drive_gain.sqrt()
        } else {
            x
        };

        let y = match mode {
            FilterMode::Off => unreachable!(),
            FilterMode::LowPass12 => self.stage1.tick(x, g, k).low,
            FilterMode::LowPass24 => {
                // Splitting the resonance keeps the cascade from ringing twice as hard
                let low = self.stage1.tick(x, g, 2.0 - (2.0 - k) * 0.5).low;
                self.stage2.tick(low, g, k).low
            }
            FilterMode::BandPass => self.stage1.tick(x, g, k).band,
            FilterMode::HighPass => self.stage1.tick(x, g, k).high,
        };

        // Keeps a self-oscillating filter from running away
        if y.is_finite() {
            y
        } else {
            self.reset();
            0.0
        }
    }
}

/// Per-sample filter coefficients.
#[derive(Debug, Clone, Copy)]
pub struct FilterCoeffs {
    g: f32,
    k: f32,
    drive_gain: f32,
}

impl FilterCoeffs {
    #[inline]
    pub fn new(cutoff: f32, resonance: f32, drive: f32, sample_rate: f32) -> Self {
        let cutoff = cutoff.clamp(20.0, sample_rate * 0.45);
        Self {
            g: (PI * cutoff / sample_rate).tan(),
            // k = 2 is no resonance, k -> 0 self-oscillates
            k: 2.0 - 1.98 * resonance.clamp(0.0, 1.0),
            drive_gain: 1.0 + drive.max(0.0) * 9.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms_through(mode: FilterMode, cutoff: f32, freq: f32) -> f32 {
        let sr = 48_000.0;
        let mut f = Filter::default();
        let mut sum = 0.0;
        let n = 48_000;
        for i in 0..n {
            let x = (2.0 * PI * freq * i as f32 / sr).sin();
            let y = f.process(x, mode, cutoff, 0.0, 0.0, sr);
            if i > n / 2 {
                sum += y * y;
            }
        }
        (sum / (n / 2) as f32).sqrt()
    }

    #[test]
    fn lowpass_attenuates_highs() {
        let pass = rms_through(FilterMode::LowPass24, 1_000.0, 100.0);
        let stop = rms_through(FilterMode::LowPass24, 1_000.0, 8_000.0);
        assert!(pass > 0.6, "pass band rms {pass}");
        assert!(stop < 0.02, "stop band rms {stop}");
    }

    #[test]
    fn highpass_attenuates_lows() {
        let pass = rms_through(FilterMode::HighPass, 1_000.0, 8_000.0);
        let stop = rms_through(FilterMode::HighPass, 1_000.0, 50.0);
        assert!(pass > 0.6 && stop < 0.05, "pass {pass} stop {stop}");
    }
}
