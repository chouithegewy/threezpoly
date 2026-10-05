use nih_plug::prelude::{Editor, Param};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::*;
use nih_plug_vizia::{assets, create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::Arc;

use crate::params::{EnvParams, FilterParams, ThreezParams};
use crate::routing_view;

#[derive(Lens)]
struct Data {
    params: Arc<ThreezParams>,
}

impl Model for Data {}

pub(crate) fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (1000, 790))
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

            routing_view::build(cx, Data::params);

            HStack::new(cx, |cx| {
                for i in 0..threezpoly_dsp::NUM_OSCS {
                    osc_panel(cx, i);
                }
            })
            .class("row");

            HStack::new(cx, |cx| {
                filter_panel(cx, "FILTER 1", |p| &p.filter1, true);
                filter_panel(cx, "FILTER 2", |p| &p.filter2, false);
                env_panel(cx, "FILTER ENV", |p| &p.filter_env);
                env_panel(cx, "AMP ENV", |p| &p.amp_env);
            })
            .class("row");

            HStack::new(cx, |cx| {
                panel(cx, "UNISON", |cx| {
                    pair(cx, |cx| {
                        control(cx, "Voices", |p| &p.unison.voices);
                        control(cx, "Detune", |p| &p.unison.detune);
                    });
                    control(cx, "Spread", |p| &p.unison.spread);
                });
                panel(cx, "GLOBAL FILTER", |cx| {
                    pair(cx, |cx| {
                        control(cx, "Mode", |p| &p.global_filter.mode);
                        control(cx, "Drive", |p| &p.global_filter.drive);
                    });
                    pair(cx, |cx| {
                        control(cx, "Cutoff", |p| &p.global_filter.cutoff);
                        control(cx, "Resonance", |p| &p.global_filter.resonance);
                    });
                });
                panel(cx, "MASTER", |cx| {
                    pair(cx, |cx| {
                        control(cx, "Gain", |p| &p.gain);
                        control(cx, "Velocity", |p| &p.velocity_sensitivity);
                    });
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
    panel_with_header(cx, title, |_| {}, content);
}

/// A panel whose title row also holds some controls on the right.
fn panel_with_header(
    cx: &mut Context,
    title: &str,
    header: impl FnOnce(&mut Context),
    content: impl FnOnce(&mut Context),
) {
    VStack::new(cx, |cx| {
        HStack::new(cx, |cx| {
            Label::new(cx, title).class("panel-title");
            header(cx);
        })
        .class("panel-header");
        content(cx);
    })
    .class("panel");
}

/// Two controls side by side.
fn pair(cx: &mut Context, content: impl FnOnce(&mut Context)) {
    HStack::new(cx, content).class("pair");
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

/// A small toggle with a short label, like Thor's routing buttons.
fn toggle<P, F>(cx: &mut Context, label: &str, map: F)
where
    P: Param + 'static,
    F: Fn(&Arc<ThreezParams>) -> &P + Copy + 'static,
{
    ParamButton::new(cx, Data::params, map).with_label(label).class("route");
}

fn osc_panel(cx: &mut Context, i: usize) {
    panel_with_header(
        cx,
        &format!("OSC {}", i + 1),
        move |cx| {
            toggle(cx, "→ F1", move |p| &p.oscs[i].to_filter1);
            toggle(cx, "→ F2", move |p| &p.oscs[i].to_filter2);
        },
        move |cx| {
            pair(cx, |cx| {
                control(cx, "Wave", move |p| &p.oscs[i].waveform);
                control(cx, "Level", move |p| &p.oscs[i].level);
            });
            pair(cx, |cx| {
                control(cx, "Octave", move |p| &p.oscs[i].octave);
                control(cx, "Semi", move |p| &p.oscs[i].semitone);
            });
            pair(cx, |cx| {
                control(cx, "Fine", move |p| &p.oscs[i].fine);
                control(cx, "Pulse Width", move |p| &p.oscs[i].pulse_width);
            });
            control(cx, "Key Track", move |p| &p.oscs[i].key_track);
        },
    );
}

fn filter_panel(
    cx: &mut Context,
    title: &str,
    filter: fn(&Arc<ThreezParams>) -> &FilterParams,
    with_series_switch: bool,
) {
    panel_with_header(
        cx,
        title,
        move |cx| {
            if with_series_switch {
                toggle(cx, "1 → 2", |p| &p.filter1_to_filter2);
            }
        },
        move |cx| {
            pair(cx, |cx| {
                control(cx, "Mode", move |p| &filter(p).mode);
                control(cx, "Drive", move |p| &filter(p).drive);
            });
            pair(cx, |cx| {
                control(cx, "Cutoff", move |p| &filter(p).cutoff);
                control(cx, "Resonance", move |p| &filter(p).resonance);
            });
            pair(cx, |cx| {
                control(cx, "Env Amount", move |p| &filter(p).env_amount);
                control(cx, "Key Track", move |p| &filter(p).key_track);
            });
        },
    );
}

fn env_panel(cx: &mut Context, title: &str, env: fn(&Arc<ThreezParams>) -> &EnvParams) {
    panel(cx, title, move |cx| {
        pair(cx, |cx| {
            control(cx, "Attack", move |p| &env(p).attack);
            control(cx, "Decay", move |p| &env(p).decay);
        });
        pair(cx, |cx| {
            control(cx, "Sustain", move |p| &env(p).sustain);
            control(cx, "Release", move |p| &env(p).release);
        });
    });
}
