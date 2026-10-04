//! Host-facing parameters. Changing an `#[id]` breaks saved projects and presets, so treat them as
//! permanent once released.

use nih_plug::prelude::*;
use nih_plug_vizia::ViziaState;
use std::sync::Arc;
use threezpoly_dsp as dsp;

use crate::editor;

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waveform {
    Sine,
    Saw,
    Square,
    Triangle,
    Noise,
}

impl From<Waveform> for dsp::Waveform {
    fn from(w: Waveform) -> Self {
        match w {
            Waveform::Sine => dsp::Waveform::Sine,
            Waveform::Saw => dsp::Waveform::Saw,
            Waveform::Square => dsp::Waveform::Square,
            Waveform::Triangle => dsp::Waveform::Triangle,
            Waveform::Noise => dsp::Waveform::Noise,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    #[name = "LP 12"]
    LowPass12,
    #[name = "LP 24"]
    LowPass24,
    #[name = "BP"]
    BandPass,
    #[name = "HP"]
    HighPass,
}

impl From<FilterMode> for dsp::FilterMode {
    fn from(m: FilterMode) -> Self {
        match m {
            FilterMode::LowPass12 => dsp::FilterMode::LowPass12,
            FilterMode::LowPass24 => dsp::FilterMode::LowPass24,
            FilterMode::BandPass => dsp::FilterMode::BandPass,
            FilterMode::HighPass => dsp::FilterMode::HighPass,
        }
    }
}

#[derive(Params)]
pub struct ThreezParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<ViziaState>,

    /// IDs get `_1`, `_2` and `_3` suffixes.
    #[nested(array, group = "Oscillator")]
    pub oscs: [OscParams; dsp::NUM_OSCS],

    #[nested(group = "Filter")]
    pub filter: FilterParams,

    #[nested(id_prefix = "fenv", group = "Filter Envelope")]
    pub filter_env: EnvParams,

    #[nested(id_prefix = "aenv", group = "Amp Envelope")]
    pub amp_env: EnvParams,

    #[id = "velsens"]
    pub velocity_sensitivity: FloatParam,

    #[id = "bend"]
    pub bend_range: IntParam,

    #[id = "gain"]
    pub gain: FloatParam,
}

#[derive(Params)]
pub struct OscParams {
    #[id = "wave"]
    pub waveform: EnumParam<Waveform>,
    #[id = "oct"]
    pub octave: IntParam,
    #[id = "semi"]
    pub semitone: IntParam,
    #[id = "fine"]
    pub fine: FloatParam,
    #[id = "level"]
    pub level: FloatParam,
    #[id = "pw"]
    pub pulse_width: FloatParam,
    #[id = "ktrack"]
    pub key_track: BoolParam,
}

#[derive(Params)]
pub struct FilterParams {
    #[id = "fmode"]
    pub mode: EnumParam<FilterMode>,
    #[id = "cutoff"]
    pub cutoff: FloatParam,
    #[id = "res"]
    pub resonance: FloatParam,
    #[id = "drive"]
    pub drive: FloatParam,
    #[id = "envamt"]
    pub env_amount: FloatParam,
    #[id = "fktrack"]
    pub key_track: FloatParam,
}

#[derive(Params)]
pub struct EnvParams {
    #[id = "a"]
    pub attack: FloatParam,
    #[id = "d"]
    pub decay: FloatParam,
    #[id = "s"]
    pub sustain: FloatParam,
    #[id = "r"]
    pub release: FloatParam,
}

fn percent(name: impl Into<String>, default: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min: 0.0, max: 1.0 })
        .with_unit("%")
        .with_value_to_string(formatters::v2s_f32_percentage(0))
        .with_string_to_value(formatters::s2v_f32_percentage())
}

fn seconds(name: impl Into<String>, default: f32) -> FloatParam {
    FloatParam::new(
        name,
        default,
        FloatRange::Skewed { min: 0.001, max: 10.0, factor: FloatRange::skew_factor(-2.5) },
    )
    .with_unit(" s")
    .with_value_to_string(formatters::v2s_f32_rounded(3))
}

impl OscParams {
    fn new(index: usize) -> Self {
        let defaults = dsp::SynthParams::default().oscs[index];
        let n = index + 1;
        let waveform = match defaults.waveform {
            dsp::Waveform::Sine => Waveform::Sine,
            dsp::Waveform::Saw => Waveform::Saw,
            dsp::Waveform::Square => Waveform::Square,
            dsp::Waveform::Triangle => Waveform::Triangle,
            dsp::Waveform::Noise => Waveform::Noise,
        };
        Self {
            waveform: EnumParam::new(format!("Osc {n} Wave"), waveform),
            octave: IntParam::new(format!("Osc {n} Octave"), defaults.octave, IntRange::Linear { min: -4, max: 4 }),
            semitone: IntParam::new(format!("Osc {n} Semi"), defaults.semitone, IntRange::Linear { min: -12, max: 12 })
                .with_unit(" st"),
            fine: FloatParam::new(format!("Osc {n} Fine"), 0.0, FloatRange::Linear { min: -100.0, max: 100.0 })
                .with_unit(" ct")
                .with_step_size(0.1),
            level: percent(format!("Osc {n} Level"), defaults.level)
                .with_smoother(SmoothingStyle::Linear(10.0)),
            pulse_width: percent(format!("Osc {n} PW"), 0.5).with_smoother(SmoothingStyle::Linear(10.0)),
            key_track: BoolParam::new(format!("Osc {n} Key Track"), true),
        }
    }

    pub fn snapshot(&self, steps: u32) -> dsp::OscSettings {
        dsp::OscSettings {
            waveform: self.waveform.value().into(),
            octave: self.octave.value(),
            semitone: self.semitone.value(),
            fine_cents: self.fine.value(),
            level: self.level.smoothed.next_step(steps),
            pulse_width: self.pulse_width.smoothed.next_step(steps),
            key_track: self.key_track.value(),
        }
    }
}

impl EnvParams {
    fn new(prefix: &str, d: dsp::EnvSettings) -> Self {
        Self {
            attack: seconds(format!("{prefix} Attack"), d.attack),
            decay: seconds(format!("{prefix} Decay"), d.decay),
            sustain: percent(format!("{prefix} Sustain"), d.sustain),
            release: seconds(format!("{prefix} Release"), d.release),
        }
    }

    pub fn snapshot(&self) -> dsp::EnvSettings {
        dsp::EnvSettings {
            attack: self.attack.value(),
            decay: self.decay.value(),
            sustain: self.sustain.value(),
            release: self.release.value(),
        }
    }
}

impl Default for FilterParams {
    fn default() -> Self {
        let d = dsp::FilterSettings::default();
        Self {
            mode: EnumParam::new("Filter Mode", FilterMode::LowPass24),
            cutoff: FloatParam::new(
                "Cutoff",
                d.cutoff,
                FloatRange::Skewed { min: 20.0, max: 20_000.0, factor: FloatRange::skew_factor(-2.2) },
            )
            .with_smoother(SmoothingStyle::Logarithmic(20.0))
            .with_value_to_string(formatters::v2s_f32_hz_then_khz(1))
            .with_string_to_value(formatters::s2v_f32_hz_then_khz()),
            resonance: percent("Resonance", d.resonance).with_smoother(SmoothingStyle::Linear(10.0)),
            drive: percent("Drive", d.drive).with_smoother(SmoothingStyle::Linear(10.0)),
            env_amount: FloatParam::new("Env Amount", d.env_amount, FloatRange::Linear { min: -8.0, max: 8.0 })
                .with_unit(" oct")
                .with_value_to_string(formatters::v2s_f32_rounded(2)),
            key_track: percent("Filter Key Track", d.key_track),
        }
    }
}

impl FilterParams {
    pub fn snapshot(&self, steps: u32) -> dsp::FilterSettings {
        dsp::FilterSettings {
            mode: self.mode.value().into(),
            cutoff: self.cutoff.smoothed.next_step(steps),
            resonance: self.resonance.smoothed.next_step(steps),
            drive: self.drive.smoothed.next_step(steps),
            env_amount: self.env_amount.value(),
            key_track: self.key_track.value(),
        }
    }
}

impl Default for ThreezParams {
    fn default() -> Self {
        let d = dsp::SynthParams::default();
        Self {
            editor_state: editor::default_state(),
            oscs: std::array::from_fn(OscParams::new),
            filter: FilterParams::default(),
            filter_env: EnvParams::new("Filter Env", d.filter_env),
            amp_env: EnvParams::new("Amp Env", d.amp_env),
            velocity_sensitivity: percent("Velocity", d.velocity_sensitivity),
            bend_range: IntParam::new("Bend Range", d.bend_range as i32, IntRange::Linear { min: 0, max: 24 })
                .with_unit(" st"),
            gain: FloatParam::new(
                "Gain",
                util::db_to_gain(-6.0),
                FloatRange::Skewed {
                    min: util::db_to_gain(-48.0),
                    max: util::db_to_gain(6.0),
                    factor: FloatRange::gain_skew_factor(-48.0, 6.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(50.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(1))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),
        }
    }
}

impl ThreezParams {
    /// Reads every parameter for a block of `steps` samples, advancing the smoothers.
    pub fn snapshot(&self, steps: u32) -> dsp::SynthParams {
        dsp::SynthParams {
            oscs: std::array::from_fn(|i| self.oscs[i].snapshot(steps)),
            filter: self.filter.snapshot(steps),
            amp_env: self.amp_env.snapshot(),
            filter_env: self.filter_env.snapshot(),
            velocity_sensitivity: self.velocity_sensitivity.value(),
            bend_range: self.bend_range.value() as f32,
            gain: self.gain.smoothed.next_step(steps),
        }
    }
}
