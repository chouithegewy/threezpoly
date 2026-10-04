//! A single synth voice: three oscillators into a filter into a VCA.

use crate::envelope::{EnvCoeffs, Envelope};
use crate::filter::Filter;
use crate::osc::Oscillator;
use crate::synth::{SynthParams, TerminatedVoice, NUM_OSCS};
use crate::note_to_freq;

/// Keeps a single voice well below full scale so chords have headroom.
const VOICE_GAIN: f32 = 0.25;
/// PolyBLEP needs the phase increment below 0.5; this keeps us comfortably under Nyquist.
const MAX_PHASE_INC: f32 = 0.45;

/// Values that are the same for every voice during one block.
pub(crate) struct BlockContext<'a> {
    pub params: &'a SynthParams,
    pub amp_coeffs: EnvCoeffs,
    pub filter_coeffs: EnvCoeffs,
    pub sample_rate: f32,
    /// Current pitch bend in semitones.
    pub bend: f32,
}

#[derive(Debug, Clone)]
pub struct Voice {
    pub(crate) note: u8,
    pub(crate) channel: u8,
    pub(crate) voice_id: Option<i32>,
    pub(crate) velocity: f32,
    /// Higher is newer. Used to pick which voice to steal.
    pub(crate) age: u64,
    oscs: [Oscillator; NUM_OSCS],
    filter: Filter,
    amp_env: Envelope,
    filter_env: Envelope,
}

impl Voice {
    pub(crate) fn new(index: usize) -> Self {
        Self {
            note: 0,
            channel: 0,
            voice_id: None,
            velocity: 0.0,
            age: 0,
            oscs: std::array::from_fn(|i| Oscillator::new(0x9E37_79B9 ^ ((index * NUM_OSCS + i) as u32 + 1))),
            filter: Filter::default(),
            amp_env: Envelope::default(),
            filter_env: Envelope::default(),
        }
    }

    pub fn is_active(&self) -> bool {
        !self.amp_env.is_idle()
    }

    /// True while the key is held (the voice is sounding and not yet released).
    pub fn is_held(&self) -> bool {
        self.is_active() && !self.amp_env.is_releasing()
    }

    pub(crate) fn info(&self) -> TerminatedVoice {
        TerminatedVoice { note: self.note, channel: self.channel, voice_id: self.voice_id }
    }

    pub(crate) fn start(&mut self, note: u8, channel: u8, voice_id: Option<i32>, velocity: f32, age: u64) {
        if !self.is_active() {
            // A fresh voice starts from a clean state. A stolen or retriggered one keeps its
            // filter state and envelope level so it doesn't click.
            for osc in &mut self.oscs {
                osc.reset_phase(0.0);
            }
            self.filter.reset();
        }
        self.note = note;
        self.channel = channel;
        self.voice_id = voice_id;
        self.velocity = velocity;
        self.age = age;
        self.amp_env.trigger();
        self.filter_env.trigger();
    }

    pub(crate) fn release(&mut self) {
        self.amp_env.release();
        self.filter_env.release();
    }

    pub(crate) fn kill(&mut self) {
        self.amp_env.reset();
        self.filter_env.reset();
        self.filter.reset();
    }

    /// Adds this voice's output to `out`.
    pub(crate) fn render(&mut self, ctx: &BlockContext, out: &mut [f32]) {
        let p = ctx.params;
        let note = self.note as f32;

        // Pitch only changes between blocks, which are kept short by the caller
        let mut phase_inc = [0.0; NUM_OSCS];
        for (inc, osc) in phase_inc.iter_mut().zip(&p.oscs) {
            *inc = (note_to_freq(osc.pitch(note, ctx.bend)) / ctx.sample_rate).min(MAX_PHASE_INC);
        }

        let f = &p.filter;
        let key_octaves = f.key_track * (note - 60.0) / 12.0;
        let vel_gain = 1.0 - p.velocity_sensitivity + p.velocity_sensitivity * self.velocity;
        let gain = VOICE_GAIN * vel_gain;

        for sample in out.iter_mut() {
            let mut mix = 0.0;
            for ((osc, settings), &inc) in self.oscs.iter_mut().zip(&p.oscs).zip(&phase_inc) {
                if settings.level > 0.0 {
                    mix += osc.next(settings.waveform, inc, settings.pulse_width) * settings.level;
                }
            }

            let fenv = self.filter_env.next(&ctx.filter_coeffs);
            let cutoff = f.cutoff * (fenv * f.env_amount + key_octaves).exp2();
            let filtered = self.filter.process(mix, f.mode, cutoff, f.resonance, f.drive, ctx.sample_rate);

            let amp = self.amp_env.next(&ctx.amp_coeffs);
            *sample += filtered * amp * gain;

            if self.amp_env.is_idle() {
                break;
            }
        }
    }
}
