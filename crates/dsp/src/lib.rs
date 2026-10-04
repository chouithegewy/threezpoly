//! Host-agnostic DSP core for Threez Poly.
//!
//! Nothing in this crate knows about plugin formats or hosts: the plugin crate turns host
//! parameters and MIDI into [`SynthParams`] and calls into [`Synth`]. Keeping it that way means the
//! wrapper layer can be swapped without touching the sound engine.

pub mod envelope;
pub mod filter;
pub mod osc;
pub mod synth;
pub mod voice;

pub use envelope::EnvSettings;
pub use filter::{FilterMode, FilterSettings};
pub use osc::{OscSettings, Waveform};
pub use synth::{Synth, SynthParams, TerminatedVoice, MAX_VOICES, NUM_OSCS};

/// Converts a (fractional) MIDI note number to a frequency in Hz, with A4 (note 69) at 440 Hz.
#[inline]
pub fn note_to_freq(note: f32) -> f32 {
    440.0 * ((note - 69.0) / 12.0).exp2()
}
