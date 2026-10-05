use nih_plug::prelude::*;
use std::sync::Arc;
use threezpoly_dsp::Synth;

mod editor;
mod params;
mod routing_view;

use params::ThreezParams;

/// Upper bound on how many samples are rendered between note events and parameter reads.
const MAX_BLOCK_SIZE: usize = 64;

pub struct ThreezPoly {
    params: Arc<ThreezParams>,
    synth: Synth,
}

impl Default for ThreezPoly {
    fn default() -> Self {
        Self {
            params: Arc::new(ThreezParams::default()),
            synth: Synth::new(44_100.0),
        }
    }
}

impl ThreezPoly {
    fn handle_event(&mut self, event: NoteEvent<()>) {
        match event {
            NoteEvent::NoteOn { voice_id, channel, note, velocity, .. } => {
                self.synth.note_on(note, channel, voice_id, velocity)
            }
            NoteEvent::NoteOff { voice_id, channel, note, .. } => {
                self.synth.note_off(note, channel, voice_id)
            }
            NoteEvent::Choke { voice_id, channel, note, .. } => {
                self.synth.choke(note, channel, voice_id)
            }
            NoteEvent::MidiPitchBend { value, .. } => self.synth.set_pitch_bend(value * 2.0 - 1.0),
            _ => (),
        }
    }
}

impl Plugin for ThreezPoly {
    const NAME: &'static str = "Threez Poly";
    const VENDOR: &'static str = "vstthreez";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    // MidiCCs rather than Basic so pitch bend comes through
    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(self.params.clone(), self.params.editor_state.clone())
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.synth.set_sample_rate(buffer_config.sample_rate);
        true
    }

    fn reset(&mut self) {
        self.synth.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let num_samples = buffer.samples();
        let [left, right] = buffer.as_slice() else {
            return ProcessStatus::Error("expected a stereo output");
        };

        // Split the buffer at every note event, and into chunks of at most MAX_BLOCK_SIZE so
        // parameter changes and pitch bend are picked up quickly.
        let mut next_event = context.next_event();
        let mut block_start = 0;
        while block_start < num_samples {
            let mut block_end = (block_start + MAX_BLOCK_SIZE).min(num_samples);
            while let Some(event) = next_event {
                let timing = event.timing() as usize;
                if timing <= block_start {
                    self.handle_event(event);
                    next_event = context.next_event();
                } else {
                    block_end = block_end.min(timing);
                    break;
                }
            }

            let params = self.params.snapshot((block_end - block_start) as u32);
            self.synth.render(
                &params,
                &mut left[block_start..block_end],
                &mut right[block_start..block_end],
            );

            // Lets hosts with per-voice modulation (e.g. Bitwig via CLAP) know a voice ended
            self.synth.drain_terminated(|t| {
                context.send_event(NoteEvent::VoiceTerminated {
                    timing: block_end as u32 - 1,
                    voice_id: t.voice_id,
                    channel: t.channel,
                    note: t.note,
                })
            });

            block_start = block_end;
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for ThreezPoly {
    const CLAP_ID: &'static str = "com.vstthreez.threez-poly";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Polyphonic subtractive synthesizer");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] =
        &[ClapFeature::Instrument, ClapFeature::Synthesizer, ClapFeature::Stereo];
}

impl Vst3Plugin for ThreezPoly {
    const VST3_CLASS_ID: [u8; 16] = *b"ThreezPolySynth1";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth, Vst3SubCategory::Stereo];
}

nih_export_clap!(ThreezPoly);
nih_export_vst3!(ThreezPoly);
