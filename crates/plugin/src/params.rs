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

/// The stable `#[id]`s keep saved values valid if variants are added or reordered.
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    #[id = "off"]
    #[name = "Off"]
    Off,
    #[id = "lp12"]
    #[name = "LP 12"]
    LowPass12,
    #[id = "lp24"]
    #[name = "LP 24"]
    LowPass24,
    #[id = "bp"]
    #[name = "BP"]
    BandPass,
    #[id = "hp"]
    #[name = "HP"]
    HighPass,
}

impl From<FilterMode> for dsp::FilterMode {
    fn from(m: FilterMode) -> Self {
        match m {
            FilterMode::Off => dsp::FilterMode::Off,
            FilterMode::LowPass12 => dsp::FilterMode::LowPass12,
            FilterMode::LowPass24 => dsp::FilterMode::LowPass24,
            FilterMode::BandPass => dsp::FilterMode::BandPass,
            FilterMode::HighPass => dsp::FilterMode::HighPass,
        }
    }
}

impl From<dsp::FilterMode> for FilterMode {
    fn from(m: dsp::FilterMode) -> Self {
        match m {
            dsp::FilterMode::Off => FilterMode::Off,
            dsp::FilterMode::LowPass12 => FilterMode::LowPass12,
            dsp::FilterMode::LowPass24 => FilterMode::LowPass24,
            dsp::FilterMode::BandPass => FilterMode::BandPass,
            dsp::FilterMode::HighPass => FilterMode::HighPass,
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

    /// No ID prefix so filter 1 keeps the IDs from before there were two slots.
    #[nested(group = "Filter 1")]
    pub filter1: FilterParams,

    #[nested(id_prefix = "f2", group = "Filter 2")]
    pub filter2: FilterParams,

    #[id = "f1tof2"]
    pub filter1_to_filter2: BoolParam,

    #[nested(group = "Global Filter")]
    pub global_filter: GlobalFilterParams,

    #[nested(id_prefix = "fenv", group = "Filter Envelope")]
    pub filter_env: EnvParams,

    #[nested(id_prefix = "aenv", group = "Amp Envelope")]
    pub amp_env: EnvParams,

    #[nested(group = "Unison")]
    pub unison: UnisonParams,

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
    #[id = "tof1"]
    pub to_filter1: BoolParam,
    #[id = "tof2"]
    pub to_filter2: BoolParam,
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
pub struct GlobalFilterParams {
    #[id = "gmode"]
    pub mode: EnumParam<FilterMode>,
    #[id = "gcutoff"]
    pub cutoff: FloatParam,
    #[id = "gres"]
    pub resonance: FloatParam,
    #[id = "gdrive"]
    pub drive: FloatParam,
}

#[derive(Params)]
pub struct UnisonParams {
    #[id = "uvoices"]
    pub voices: IntParam,
    #[id = "udetune"]
    pub detune: FloatParam,
    #[id = "uspread"]
    pub spread: FloatParam,
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

fn cutoff(name: impl Into<String>, default: f32) -> FloatParam {
    FloatParam::new(
        name,
        default,
        FloatRange::Skewed { min: 20.0, max: 20_000.0, factor: FloatRange::skew_factor(-2.2) },
    )
    .with_smoother(SmoothingStyle::Logarithmic(20.0))
    .with_value_to_string(formatters::v2s_f32_hz_then_khz(1))
    .with_string_to_value(formatters::s2v_f32_hz_then_khz())
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
            to_filter1: BoolParam::new(format!("Osc {n} to Filter 1"), defaults.to_filter1),
            to_filter2: BoolParam::new(format!("Osc {n} to Filter 2"), defaults.to_filter2),
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
            to_filter1: self.to_filter1.value(),
            to_filter2: self.to_filter2.value(),
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

impl FilterParams {
    fn new(prefix: &str, d: dsp::FilterSettings) -> Self {
        Self {
            mode: EnumParam::new(format!("{prefix} Mode"), d.mode.into()),
            cutoff: cutoff(format!("{prefix} Cutoff"), d.cutoff),
            resonance: percent(format!("{prefix} Resonance"), d.resonance)
                .with_smoother(SmoothingStyle::Linear(10.0)),
            drive: percent(format!("{prefix} Drive"), d.drive).with_smoother(SmoothingStyle::Linear(10.0)),
            env_amount: FloatParam::new(
                format!("{prefix} Env Amount"),
                d.env_amount,
                FloatRange::Linear { min: -8.0, max: 8.0 },
            )
            .with_unit(" oct")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),
            key_track: percent(format!("{prefix} Key Track"), d.key_track),
        }
    }
}

impl Default for GlobalFilterParams {
    fn default() -> Self {
        let d = dsp::GlobalFilterSettings::default();
        Self {
            mode: EnumParam::new("Global Filter Mode", d.mode.into()),
            cutoff: cutoff("Global Filter Cutoff", d.cutoff),
            resonance: percent("Global Filter Resonance", d.resonance)
                .with_smoother(SmoothingStyle::Linear(10.0)),
            drive: percent("Global Filter Drive", d.drive).with_smoother(SmoothingStyle::Linear(10.0)),
        }
    }
}

impl GlobalFilterParams {
    pub fn snapshot(&self, steps: u32) -> dsp::GlobalFilterSettings {
        dsp::GlobalFilterSettings {
            mode: self.mode.value().into(),
            cutoff: self.cutoff.smoothed.next_step(steps),
            resonance: self.resonance.smoothed.next_step(steps),
            drive: self.drive.smoothed.next_step(steps),
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

impl Default for UnisonParams {
    fn default() -> Self {
        let d = dsp::UnisonSettings::default();
        Self {
            voices: IntParam::new("Unison", d.voices as i32, IntRange::Linear { min: 1, max: dsp::MAX_UNISON as i32 }),
            detune: FloatParam::new(
                "Unison Detune",
                d.detune_cents,
                FloatRange::Skewed { min: 0.0, max: 100.0, factor: FloatRange::skew_factor(-1.0) },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_unit(" ct")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            spread: percent("Unison Spread", d.spread).with_smoother(SmoothingStyle::Linear(20.0)),
        }
    }
}

impl UnisonParams {
    pub fn snapshot(&self, steps: u32) -> dsp::UnisonSettings {
        dsp::UnisonSettings {
            voices: self.voices.value() as usize,
            detune_cents: self.detune.smoothed.next_step(steps),
            spread: self.spread.smoothed.next_step(steps),
        }
    }
}

impl Default for ThreezParams {
    fn default() -> Self {
        let d = dsp::SynthParams::default();
        Self {
            editor_state: editor::default_state(),
            oscs: std::array::from_fn(OscParams::new),
            filter1: FilterParams::new("Filter 1", d.filter1),
            filter2: FilterParams::new("Filter 2", d.filter2),
            filter1_to_filter2: BoolParam::new("Filter 1 to Filter 2", d.filter1_to_filter2),
            global_filter: GlobalFilterParams::default(),
            filter_env: EnvParams::new("Filter Env", d.filter_env),
            amp_env: EnvParams::new("Amp Env", d.amp_env),
            unison: UnisonParams::default(),
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
            filter1: self.filter1.snapshot(steps),
            filter2: self.filter2.snapshot(steps),
            filter1_to_filter2: self.filter1_to_filter2.value(),
            global_filter: self.global_filter.snapshot(steps),
            amp_env: self.amp_env.snapshot(),
            filter_env: self.filter_env.snapshot(),
            unison: self.unison.snapshot(steps),
            velocity_sensitivity: self.velocity_sensitivity.value(),
            bend_range: self.bend_range.value() as f32,
            gain: self.gain.smoothed.next_step(steps),
        }
    }
}
