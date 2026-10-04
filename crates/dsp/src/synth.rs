//! Polyphonic voice management.

use crate::envelope::{EnvCoeffs, EnvSettings};
use crate::filter::FilterSettings;
use crate::osc::{OscSettings, Waveform};
use crate::voice::{BlockContext, Voice};

pub const MAX_VOICES: usize = 16;
pub const NUM_OSCS: usize = 3;
/// How many voice terminations can queue up between calls to [`Synth::drain_terminated`].
const TERMINATED_CAPACITY: usize = 128;

/// A snapshot of every global parameter, taken once per block.
#[derive(Debug, Clone, PartialEq)]
pub struct SynthParams {
    pub oscs: [OscSettings; NUM_OSCS],
    pub filter: FilterSettings,
    pub amp_env: EnvSettings,
    pub filter_env: EnvSettings,
    /// `0.0` ignores velocity, `1.0` scales the amplitude by velocity fully.
    pub velocity_sensitivity: f32,
    /// Pitch bend range in semitones.
    pub bend_range: f32,
    /// Linear output gain.
    pub gain: f32,
}

impl Default for SynthParams {
    fn default() -> Self {
        let mut oscs = [OscSettings::default(); NUM_OSCS];
        oscs[0].level = 0.8;
        oscs[1].waveform = Waveform::Square;
        oscs[2].waveform = Waveform::Sine;
        oscs[2].octave = -1;
        Self {
            oscs,
            filter: FilterSettings::default(),
            amp_env: EnvSettings::default(),
            filter_env: EnvSettings { attack: 0.001, decay: 0.4, sustain: 0.0, release: 0.3 },
            velocity_sensitivity: 0.7,
            bend_range: 2.0,
            gain: 1.0,
        }
    }
}

/// A voice that finished playing, so the host can be told about it (CLAP `VoiceTerminated`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminatedVoice {
    pub note: u8,
    pub channel: u8,
    pub voice_id: Option<i32>,
}

pub struct Synth {
    voices: [Voice; MAX_VOICES],
    sample_rate: f32,
    next_age: u64,
    /// Normalized pitch bend, `-1.0..=1.0`.
    pitch_bend: f32,
    terminated: [TerminatedVoice; TERMINATED_CAPACITY],
    num_terminated: usize,
}

impl Synth {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            voices: std::array::from_fn(Voice::new),
            sample_rate,
            next_age: 0,
            pitch_bend: 0.0,
            terminated: [TerminatedVoice { note: 0, channel: 0, voice_id: None }; TERMINATED_CAPACITY],
            num_terminated: 0,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.reset();
    }

    pub fn reset(&mut self) {
        for voice in &mut self.voices {
            voice.kill();
        }
        self.pitch_bend = 0.0;
        self.num_terminated = 0;
    }

    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.is_active()).count()
    }

    /// Sets pitch bend, normalized to `-1.0..=1.0`.
    pub fn set_pitch_bend(&mut self, bend: f32) {
        self.pitch_bend = bend.clamp(-1.0, 1.0);
    }

    pub fn note_on(&mut self, note: u8, channel: u8, voice_id: Option<i32>, velocity: f32) {
        let age = self.next_age;
        self.next_age += 1;

        // Retrigger a voice that is already playing this note, then prefer an idle voice, then the
        // oldest released one, then the oldest held one.
        let idx = self
            .voices
            .iter()
            .position(|v| v.is_active() && v.note == note && v.channel == channel)
            .or_else(|| self.voices.iter().position(|v| !v.is_active()))
            .unwrap_or_else(|| {
                let releasing = self.voices.iter().enumerate().filter(|(_, v)| !v.is_held());
                let any = self.voices.iter().enumerate();
                releasing
                    .min_by_key(|(_, v)| v.age)
                    .or_else(|| any.min_by_key(|(_, v)| v.age))
                    .map(|(i, _)| i)
                    .unwrap()
            });

        let voice = &self.voices[idx];
        let same_voice = voice.note == note && voice.channel == channel && voice.voice_id == voice_id;
        if voice.is_active() && !same_voice {
            self.push_terminated(voice.info());
        }
        self.voices[idx].start(note, channel, voice_id, velocity, age);
    }

    pub fn note_off(&mut self, note: u8, channel: u8, voice_id: Option<i32>) {
        for voice in self.voices.iter_mut().filter(|v| v.is_held()) {
            if Self::matches(voice, note, channel, voice_id) {
                voice.release();
            }
        }
    }

    /// Silences matching voices immediately.
    pub fn choke(&mut self, note: u8, channel: u8, voice_id: Option<i32>) {
        for i in 0..MAX_VOICES {
            let voice = &mut self.voices[i];
            if voice.is_active() && Self::matches(voice, note, channel, voice_id) {
                voice.kill();
                let t = voice.info();
                self.push_terminated(t);
            }
        }
    }

    fn matches(voice: &Voice, note: u8, channel: u8, voice_id: Option<i32>) -> bool {
        match (voice_id, voice.voice_id) {
            (Some(a), Some(b)) => a == b,
            _ => voice.note == note && voice.channel == channel,
        }
    }

    fn push_terminated(&mut self, t: TerminatedVoice) {
        // On overflow the host simply doesn't hear about a few voices; better than allocating
        if self.num_terminated < TERMINATED_CAPACITY {
            self.terminated[self.num_terminated] = t;
            self.num_terminated += 1;
        }
    }

    /// Calls `f` for every voice that finished since the last call.
    pub fn drain_terminated(&mut self, mut f: impl FnMut(TerminatedVoice)) {
        for t in &self.terminated[..self.num_terminated] {
            f(*t);
        }
        self.num_terminated = 0;
    }

    /// Renders one block, overwriting `left` and `right`. Keep blocks short (e.g. 64 samples):
    /// pitch and envelope settings are only read once per block.
    pub fn render(&mut self, params: &SynthParams, left: &mut [f32], right: &mut [f32]) {
        left.fill(0.0);

        let ctx = BlockContext {
            params,
            amp_coeffs: EnvCoeffs::new(&params.amp_env, self.sample_rate),
            filter_coeffs: EnvCoeffs::new(&params.filter_env, self.sample_rate),
            sample_rate: self.sample_rate,
            bend: self.pitch_bend * params.bend_range,
        };

        for i in 0..MAX_VOICES {
            if !self.voices[i].is_active() {
                continue;
            }
            self.voices[i].render(&ctx, left);
            if !self.voices[i].is_active() {
                self.push_terminated(self.voices[i].info());
            }
        }

        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            *l *= params.gain;
            *r = *l;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn run(synth: &mut Synth, params: &SynthParams, blocks: usize) -> f32 {
        let (mut l, mut r) = ([0.0; 64], [0.0; 64]);
        let mut peak: f32 = 0.0;
        for _ in 0..blocks {
            synth.render(params, &mut l, &mut r);
            for s in l {
                assert!(s.is_finite());
                peak = peak.max(s.abs());
            }
        }
        peak
    }

    #[test]
    fn note_sounds_then_terminates() {
        let params = SynthParams::default();
        let mut synth = Synth::new(SR);
        synth.note_on(60, 0, Some(7), 1.0);
        assert!(run(&mut synth, &params, 100) > 0.01);

        synth.note_off(60, 0, Some(7));
        run(&mut synth, &params, 2000);
        assert_eq!(synth.active_voices(), 0);

        let mut ended = Vec::new();
        synth.drain_terminated(|t| ended.push(t));
        assert_eq!(ended, vec![TerminatedVoice { note: 60, channel: 0, voice_id: Some(7) }]);
    }

    #[test]
    fn steals_oldest_voice_when_full() {
        let mut synth = Synth::new(SR);
        for n in 0..MAX_VOICES as u8 + 1 {
            synth.note_on(40 + n, 0, None, 0.8);
        }
        assert_eq!(synth.active_voices(), MAX_VOICES);

        let mut ended = Vec::new();
        synth.drain_terminated(|t| ended.push(t.note));
        assert_eq!(ended, vec![40]);
    }

    #[test]
    fn same_note_retriggers_instead_of_stacking() {
        let mut synth = Synth::new(SR);
        synth.note_on(60, 0, None, 1.0);
        synth.note_on(60, 0, None, 1.0);
        assert_eq!(synth.active_voices(), 1);
    }

    #[test]
    fn full_chord_with_resonance_stays_finite() {
        let mut params = SynthParams::default();
        params.filter.resonance = 1.0;
        params.filter.drive = 1.0;
        for osc in &mut params.oscs {
            osc.level = 1.0;
        }
        let mut synth = Synth::new(SR);
        for n in 0..MAX_VOICES as u8 {
            synth.note_on(30 + n * 4, 0, None, 1.0);
        }
        run(&mut synth, &params, 500);
    }
}
