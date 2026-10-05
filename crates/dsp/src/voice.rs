//! A single synth voice, routed like Thor's: each oscillator stack feeds filter slot 1, slot 2 or
//! both. Slot 1 either goes to the amp or, in series mode, into slot 2.

use crate::envelope::{EnvCoeffs, Envelope};
use crate::filter::{Filter, FilterCoeffs, FilterMode, FilterSettings};
use crate::osc::Oscillator;
use crate::synth::{SynthParams, TerminatedVoice, NUM_OSCS};
use crate::unison::{UnisonLayout, MAX_UNISON};
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
    pub unison: UnisonLayout,
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
    /// `oscs[osc][copy]`: one stack of unison copies per oscillator.
    oscs: [[Oscillator; MAX_UNISON]; NUM_OSCS],
    filter1: FilterSlot,
    filter2: FilterSlot,
    /// xorshift state for randomizing unison phases.
    rng: u32,
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
            oscs: std::array::from_fn(|osc| {
                std::array::from_fn(|copy| {
                    let n = (index * NUM_OSCS + osc) * MAX_UNISON + copy;
                    Oscillator::new(0x9E37_79B9 ^ (n as u32 + 1))
                })
            }),
            filter1: FilterSlot::default(),
            filter2: FilterSlot::default(),
            rng: 0x2545_F491 ^ (index as u32 + 1),
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
            // The first copy starts at phase 0 for a consistent attack. The others start at random
            // phases, otherwise the detuned copies all line up and swoosh on every note.
            for stack in &mut self.oscs {
                stack[0].reset_phase(0.0);
                for osc in &mut stack[1..] {
                    osc.reset_phase(next_random(&mut self.rng));
                }
            }
            self.filter1.reset();
            self.filter2.reset();
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
        self.filter1.reset();
        self.filter2.reset();
    }

    /// Adds this voice's output to `left` and `right`.
    pub(crate) fn render(&mut self, ctx: &BlockContext, left: &mut [f32], right: &mut [f32]) {
        let p = ctx.params;
        let u = &ctx.unison;
        let note = self.note as f32;

        // Pitch only changes between blocks, which are kept short by the caller
        let mut phase_inc = [[0.0; MAX_UNISON]; NUM_OSCS];
        for (incs, osc) in phase_inc.iter_mut().zip(&p.oscs) {
            let pitch = osc.pitch(note, ctx.bend);
            for (inc, detune) in incs.iter_mut().zip(&u.detune[..u.voices]) {
                *inc = (note_to_freq(pitch + detune) / ctx.sample_rate).min(MAX_PHASE_INC);
            }
        }

        let vel_gain = 1.0 - p.velocity_sensitivity + p.velocity_sensitivity * self.velocity;
        let gain = VOICE_GAIN * vel_gain;

        // Work out once per block which slots receive any signal, so idle ones cost nothing
        let sounding = |to: fn(&crate::osc::OscSettings) -> bool| p.oscs.iter().any(|o| o.level > 0.0 && to(o));
        let serial = p.filter1_to_filter2;
        let f1_used = sounding(|o| o.to_filter1);
        let f2_used = sounding(|o| o.to_filter2) || (serial && f1_used);

        for (out_l, out_r) in left.iter_mut().zip(right.iter_mut()) {
            // Oscillator stacks summed into the two filter buses
            let (mut bus1_l, mut bus1_r, mut bus2_l, mut bus2_r) = (0.0, 0.0, 0.0, 0.0);
            for ((stack, settings), incs) in self.oscs.iter_mut().zip(&p.oscs).zip(&phase_inc) {
                if settings.level <= 0.0 || !(settings.to_filter1 || settings.to_filter2) {
                    continue;
                }
                let (mut mix_l, mut mix_r) = (0.0, 0.0);
                for i in 0..u.voices {
                    let s = stack[i].next(settings.waveform, incs[i], settings.pulse_width) * settings.level;
                    mix_l += s * u.gain_l[i];
                    mix_r += s * u.gain_r[i];
                }
                if settings.to_filter1 {
                    bus1_l += mix_l;
                    bus1_r += mix_r;
                }
                if settings.to_filter2 {
                    bus2_l += mix_l;
                    bus2_r += mix_r;
                }
            }

            let fenv = self.filter_env.next(&ctx.filter_coeffs);
            let (mut sum_l, mut sum_r) = (0.0, 0.0);

            if f1_used {
                let (y_l, y_r) = self.filter1.process(&p.filter1, bus1_l, bus1_r, fenv, note, u.stereo, ctx.sample_rate);
                if serial {
                    bus2_l += y_l;
                    bus2_r += y_r;
                } else {
                    sum_l += y_l;
                    sum_r += y_r;
                }
            }
            if f2_used {
                let (y_l, y_r) = self.filter2.process(&p.filter2, bus2_l, bus2_r, fenv, note, u.stereo, ctx.sample_rate);
                sum_l += y_l;
                sum_r += y_r;
            }

            let amp = self.amp_env.next(&ctx.amp_coeffs) * gain;
            *out_l += sum_l * amp;
            *out_r += sum_r * amp;

            if self.amp_env.is_idle() {
                break;
            }
        }

        for (slot, used) in [(&mut self.filter1, f1_used), (&mut self.filter2, f2_used)] {
            if !used {
                // Start from silence when the slot gets signal again
                slot.reset();
            } else if !u.stereo {
                // Keeps the right channel in step so turning spread up mid-note doesn't click
                slot.r = slot.l.clone();
            }
        }
    }
}

/// One filter slot: a left/right pair sharing coefficients. In mono only the left one runs.
#[derive(Debug, Clone, Default)]
struct FilterSlot {
    l: Filter,
    r: Filter,
}

impl FilterSlot {
    fn reset(&mut self) {
        self.l.reset();
        self.r.reset();
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn process(
        &mut self,
        s: &FilterSettings,
        in_l: f32,
        in_r: f32,
        fenv: f32,
        note: f32,
        stereo: bool,
        sample_rate: f32,
    ) -> (f32, f32) {
        if s.mode == FilterMode::Off {
            return (in_l, in_r);
        }
        let octaves = fenv * s.env_amount + s.key_track * (note - 60.0) / 12.0;
        let coeffs = FilterCoeffs::new(s.cutoff * octaves.exp2(), s.resonance, s.drive, sample_rate);
        let y_l = self.l.tick(in_l, s.mode, &coeffs);
        let y_r = if stereo { self.r.tick(in_r, s.mode, &coeffs) } else { y_l };
        (y_l, y_r)
    }
}

/// Returns a pseudo-random value in `0.0..1.0`.
fn next_random(state: &mut u32) -> f32 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    (x >> 8) as f32 / (1u32 << 24) as f32
}
