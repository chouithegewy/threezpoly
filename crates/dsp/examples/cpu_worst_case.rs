use std::time::Instant;
use threezpoly_dsp::{
    FilterMode, FilterSettings, GlobalFilterSettings, Synth, SynthParams, UnisonSettings, MAX_UNISON,
    MAX_VOICES,
};

fn main() {
    let sr = 48_000.0;
    let mut params = SynthParams::default();
    for osc in &mut params.oscs {
        osc.level = 0.6;
        osc.to_filter1 = true;
        osc.to_filter2 = true;
    }
    // Both slots running, chained, plus the global filter
    params.filter2 = FilterSettings { mode: FilterMode::BandPass, ..params.filter1 };
    params.filter1_to_filter2 = true;
    params.global_filter = GlobalFilterSettings { mode: FilterMode::LowPass24, ..Default::default() };
    params.unison = UnisonSettings { voices: MAX_UNISON, detune_cents: 25.0, spread: 1.0 };
    params.amp_env.sustain = 1.0;

    let mut synth = Synth::new(sr);
    for n in 0..MAX_VOICES as u8 {
        synth.note_on(36 + n * 3, 0, None, 1.0);
    }

    let (mut l, mut r) = ([0.0; 64], [0.0; 64]);
    let seconds = 5.0;
    let blocks = (seconds * sr / 64.0) as usize;
    let start = Instant::now();
    for _ in 0..blocks {
        synth.render(&params, &mut l, &mut r);
    }
    let elapsed = start.elapsed().as_secs_f32();
    println!(
        "{} voices x {} unison x 3 osc, both filter slots + global: {:.1}% of one core ({:.2}s to render {seconds}s)",
        MAX_VOICES,
        MAX_UNISON,
        100.0 * elapsed / seconds,
        elapsed
    );
}
