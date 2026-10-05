//! Unison: stacks of detuned copies of each oscillator, spread across the stereo field.

pub const MAX_UNISON: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnisonSettings {
    /// Copies per oscillator, `1..=MAX_UNISON`.
    pub voices: usize,
    /// Distance in cents between the outermost copy and the center pitch.
    pub detune_cents: f32,
    /// `0.0` keeps every copy centered, `1.0` pans the outermost copies hard left and right.
    pub spread: f32,
}

impl Default for UnisonSettings {
    fn default() -> Self {
        Self { voices: 1, detune_cents: 20.0, spread: 0.5 }
    }
}

/// Per-copy detune and pan gains, computed once per block.
#[derive(Debug, Clone, Copy)]
pub struct UnisonLayout {
    pub voices: usize,
    /// Detune of each copy in semitones.
    pub detune: [f32; MAX_UNISON],
    pub gain_l: [f32; MAX_UNISON],
    pub gain_r: [f32; MAX_UNISON],
    /// True when left and right differ, so the voice needs its second filter.
    pub stereo: bool,
}

impl UnisonLayout {
    pub fn new(settings: &UnisonSettings) -> Self {
        let voices = settings.voices.clamp(1, MAX_UNISON);
        let spread = settings.spread.clamp(0.0, 1.0);
        // Summing N uncorrelated copies grows the level by about sqrt(N)
        let norm = 1.0 / (voices as f32).sqrt();

        let mut layout = Self {
            voices,
            detune: [0.0; MAX_UNISON],
            gain_l: [0.0; MAX_UNISON],
            gain_r: [0.0; MAX_UNISON],
            stereo: voices > 1 && spread > 0.0,
        };
        for i in 0..voices {
            // Evenly spaced positions in -1..=1, or 0 for a single copy
            let pos = if voices == 1 { 0.0 } else { 2.0 * i as f32 / (voices - 1) as f32 - 1.0 };
            let pan = pos * spread;
            layout.detune[i] = pos * settings.detune_cents / 100.0;
            // Balance law: unity in the center, so spread 0 sounds exactly like mono
            layout.gain_l[i] = (1.0 - pan).min(1.0) * norm;
            layout.gain_r[i] = (1.0 + pan).min(1.0) * norm;
        }
        layout
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_voice_is_centered_and_in_tune() {
        let l = UnisonLayout::new(&UnisonSettings { voices: 1, detune_cents: 50.0, spread: 1.0 });
        assert_eq!(l.detune[0], 0.0);
        assert_eq!((l.gain_l[0], l.gain_r[0]), (1.0, 1.0));
        assert!(!l.stereo);
    }

    #[test]
    fn detune_and_pan_are_symmetric() {
        let l = UnisonLayout::new(&UnisonSettings { voices: 3, detune_cents: 30.0, spread: 1.0 });
        assert_eq!(&l.detune[..3], &[-0.3, 0.0, 0.3]);
        // Outer copies hard left/right, middle copy centered
        assert_eq!(l.gain_r[0], 0.0);
        assert_eq!(l.gain_l[2], 0.0);
        assert_eq!(l.gain_l[1], l.gain_r[1]);
        assert!(l.stereo);
    }
}
