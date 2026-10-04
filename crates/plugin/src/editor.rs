use nih_plug::prelude::{Editor, Param};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::*;
use nih_plug_vizia::{assets, create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::Arc;

use crate::params::{EnvParams, ThreezParams};

#[derive(Lens)]
struct Data {
    params: Arc<ThreezParams>,
}

impl Model for Data {}

pub(crate) fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (900, 560))
}

pub(crate) fn create(
    params: Arc<ThreezParams>,
    editor_state: Arc<ViziaState>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::Custom, move |cx, _| {
        assets::register_noto_sans_light(cx);
        assets::register_noto_sans_bold(cx);
        if let Err(err) = cx.add_stylesheet(include_style!("src/style.css")) {
            nih_plug::nih_error!("Failed to load stylesheet: {err:?}");
        }

        Data { params: params.clone() }.build(cx);

        VStack::new(cx, |cx| {
            Label::new(cx, "THREEZ POLY").class("title");

            HStack::new(cx, |cx| {
                for i in 0..threezpoly_dsp::NUM_OSCS {
                    osc_panel(cx, i);
                }
            })
            .class("row");

            HStack::new(cx, |cx| {
                panel(cx, "FILTER", |cx| {
                    control(cx, "Mode", |p| &p.filter.mode);
                    control(cx, "Cutoff", |p| &p.filter.cutoff);
                    control(cx, "Resonance", |p| &p.filter.resonance);
                    control(cx, "Drive", |p| &p.filter.drive);
                    control(cx, "Env Amount", |p| &p.filter.env_amount);
                    control(cx, "Key Track", |p| &p.filter.key_track);
                });
                env_panel(cx, "FILTER ENV", |p| &p.filter_env);
                env_panel(cx, "AMP ENV", |p| &p.amp_env);
                panel(cx, "MASTER", |cx| {
                    control(cx, "Gain", |p| &p.gain);
                    control(cx, "Velocity", |p| &p.velocity_sensitivity);
                    control(cx, "Bend Range", |p| &p.bend_range);
                });
            })
            .class("row");
        })
        .class("root");

        ResizeHandle::new(cx);
    })
}

fn panel(cx: &mut Context, title: &str, content: impl FnOnce(&mut Context)) {
    VStack::new(cx, |cx| {
        Label::new(cx, title).class("panel-title");
        content(cx);
    })
    .class("panel");
}

/// A label above a slider.
fn control<P, F>(cx: &mut Context, label: &str, map: F)
where
    P: Param + 'static,
    F: Fn(&Arc<ThreezParams>) -> &P + Copy + 'static,
{
    VStack::new(cx, |cx| {
        Label::new(cx, label).class("label");
        ParamSlider::new(cx, Data::params, map);
    })
    .class("control");
}

fn osc_panel(cx: &mut Context, i: usize) {
    panel(cx, &format!("OSC {}", i + 1), move |cx| {
        control(cx, "Wave", move |p| &p.oscs[i].waveform);
        HStack::new(cx, |cx| {
            control(cx, "Octave", move |p| &p.oscs[i].octave);
            control(cx, "Semi", move |p| &p.oscs[i].semitone);
        })
        .class("pair");
        control(cx, "Fine", move |p| &p.oscs[i].fine);
        control(cx, "Level", move |p| &p.oscs[i].level);
        HStack::new(cx, |cx| {
            control(cx, "Pulse Width", move |p| &p.oscs[i].pulse_width);
            control(cx, "Key Track", move |p| &p.oscs[i].key_track);
        })
        .class("pair");
    });
}

fn env_panel(cx: &mut Context, title: &str, env: fn(&Arc<ThreezParams>) -> &EnvParams) {
    panel(cx, title, move |cx| {
        control(cx, "Attack", move |p| &env(p).attack);
        control(cx, "Decay", move |p| &env(p).decay);
        control(cx, "Sustain", move |p| &env(p).sustain);
        control(cx, "Release", move |p| &env(p).release);
    });
}
