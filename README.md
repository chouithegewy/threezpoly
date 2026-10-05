# Threez Poly

A polyphonic subtractive synth in Rust, built on [nih-plug](https://github.com/robbert-vdh/nih-plug)
with a [vizia](https://github.com/vizia/vizia) GUI. Exports CLAP and VST3.

## Current features

- **3 oscillators**: sine, saw, square (with pulse width), triangle, noise. PolyBLEP anti-aliasing.
- **Per-oscillator tuning**: octave (±4), semitone (±12), fine (±100 cents), and key tracking
  (off pins the oscillator to C4 plus its tuning offset, for drones and fixed-pitch sources).
- **Unison**: 1–8 copies of every oscillator, detune up to ±100 cents and stereo spread. Copies
  start at random phases; at zero spread the voice stays mono and runs a single filter.
- **Thor-style filter routing**: each oscillator has **→ F1** / **→ F2** buttons choosing which
  per-voice filter slot(s) it feeds. **1 → 2** chains filter 1 into filter 2; otherwise both run
  in parallel into the amp. A **global filter** then shapes the sum of all voices.
- **Filters**: zero-delay-feedback SVF with LP 12, LP 24, BP and HP modes, or Off (pass-through).
  Per slot: cutoff, resonance up to self-oscillation, drive, envelope amount (±8 octaves) and
  keyboard tracking. Slots that receive no signal cost no CPU.
- **Signal-flow diagram** at the top of the editor that lights up the active routes.
- **Envelopes**: ADSR for the filter and the amplifier.
- **16 voices**: retrigger on repeated notes, steal the oldest released voice first.
- **Pitch bend** (range 0–24 semitones) and velocity sensitivity.

## Layout

```
crates/dsp/      host-agnostic engine (oscillators, filter, envelopes, voices). No plugin deps.
crates/plugin/   nih-plug wrapper: parameters, MIDI/event handling, vizia editor
xtask/           bundler (`cargo xtask bundle`)
```

All of the sound engine lives in `crates/dsp`, so the wrapper can be replaced (for example to drop
nih-plug's GPL `vst3-sys` dependency) without touching the DSP.

## Building

Linux needs a few X11/GL development packages for the GUI:

```
sudo apt install libx11-xcb-dev libxcb-icccm4-dev libxcb-dri2-0-dev libxcursor-dev libgl-dev
```

Then:

```
cargo test -p threezpoly_dsp          # engine unit tests
cargo run --release -p threezpoly_dsp --example cpu_worst_case   # CPU check, all voices + unison
cargo xtask bundle threezpoly --release
```

The bundles land in `target/bundled/` (`Threez Poly.clap` and `Threez Poly.vst3`). To install them:

```
ln -sfn "$PWD/target/bundled/Threez Poly.vst3" ~/.vst3/
ln -sfn "$PWD/target/bundled/Threez Poly.clap" ~/.clap/
```

### Validating

Steinberg's validator from the sibling SDK build works on the VST3 bundle:

```
../phatbooty/build/bin/Release/validator "target/bundled/Threez Poly.vst3"
```

## License

GPL-3.0-or-later (nih-plug's VST3 bindings are GPLv3).
