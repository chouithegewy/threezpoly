//! ADSR envelope with a linear attack and exponential decay and release.

/// Envelope times in seconds, sustain as a level in `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnvSettings {
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
}

impl Default for EnvSettings {
    fn default() -> Self {
        Self { attack: 0.005, decay: 0.3, sustain: 0.7, release: 0.3 }
    }
}

/// Per-sample increments and coefficients derived from [`EnvSettings`]. Computed once per block
/// and shared by all voices.
#[derive(Debug, Clone, Copy)]
pub struct EnvCoeffs {
    attack_step: f32,
    decay_coeff: f32,
    release_coeff: f32,
    sustain: f32,
}

/// The exponential segments count as finished after this many time constants (about -60 dB).
const TIME_CONSTANTS: f32 = 6.9;
/// Below this level a releasing envelope is considered silent.
const SILENCE: f32 = 1.0e-4;

impl EnvCoeffs {
    pub fn new(settings: &EnvSettings, sample_rate: f32) -> Self {
        let coeff = |seconds: f32| {
            let samples = (seconds * sample_rate).max(1.0);
            (-TIME_CONSTANTS / samples).exp()
        };
        Self {
            attack_step: 1.0 / (settings.attack * sample_rate).max(1.0),
            decay_coeff: coeff(settings.decay),
            release_coeff: coeff(settings.release),
            sustain: settings.sustain.clamp(0.0, 1.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Idle,
    Attack,
    Decay,
    Release,
}

#[derive(Debug, Clone)]
pub struct Envelope {
    stage: Stage,
    value: f32,
}

impl Default for Envelope {
    fn default() -> Self {
        Self { stage: Stage::Idle, value: 0.0 }
    }
}

impl Envelope {
    /// Starts the attack from the current value, so retriggering a sounding voice doesn't click.
    pub fn trigger(&mut self) {
        self.stage = Stage::Attack;
    }

    pub fn release(&mut self) {
        if self.stage != Stage::Idle {
            self.stage = Stage::Release;
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn is_idle(&self) -> bool {
        self.stage == Stage::Idle
    }

    pub fn is_releasing(&self) -> bool {
        self.stage == Stage::Release
    }

    pub fn value(&self) -> f32 {
        self.value
    }

    #[inline]
    pub fn next(&mut self, c: &EnvCoeffs) -> f32 {
        match self.stage {
            Stage::Idle => {}
            Stage::Attack => {
                self.value += c.attack_step;
                if self.value >= 1.0 {
                    self.value = 1.0;
                    self.stage = Stage::Decay;
                }
            }
            // Decay settles into the sustain level, so there is no separate sustain stage
            Stage::Decay => self.value = c.sustain + (self.value - c.sustain) * c.decay_coeff,
            Stage::Release => {
                self.value *= c.release_coeff;
                if self.value < SILENCE {
                    self.reset();
                }
            }
        }
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    #[test]
    fn full_cycle_reaches_sustain_then_idle() {
        let s = EnvSettings { attack: 0.01, decay: 0.05, sustain: 0.5, release: 0.05 };
        let c = EnvCoeffs::new(&s, SR);
        let mut env = Envelope::default();
        env.trigger();

        let attack_samples = (0.01 * SR) as usize;
        for _ in 0..attack_samples {
            env.next(&c);
        }
        assert!((env.value() - 1.0).abs() < 1e-3);

        for _ in 0..(0.05 * SR) as usize {
            env.next(&c);
        }
        assert!((env.value() - 0.5).abs() < 0.01);

        env.release();
        for _ in 0..(0.08 * SR) as usize {
            env.next(&c);
        }
        assert!(env.is_idle());
    }
}
