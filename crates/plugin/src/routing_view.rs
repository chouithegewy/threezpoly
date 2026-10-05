//! A Thor-style signal flow diagram: oscillators into the two filter slots, through the amp and the
//! global filter to the output. It is rebuilt whenever the routing changes, so automation and preset
//! changes show up too.

use nih_plug::prelude::Enum;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use std::sync::Arc;
use threezpoly_dsp::NUM_OSCS;

use crate::params::{FilterMode, ThreezParams, Waveform};

/// Everything the diagram shows. Comparing this is how the view knows when to redraw.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoutingState {
    osc_level_on: [bool; NUM_OSCS],
    waveform: [Waveform; NUM_OSCS],
    to_filter1: [bool; NUM_OSCS],
    to_filter2: [bool; NUM_OSCS],
    filter1: FilterMode,
    filter2: FilterMode,
    serial: bool,
    global: FilterMode,
    unison: i32,
}

impl Data for RoutingState {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl RoutingState {
    pub fn read(p: &Arc<ThreezParams>) -> Self {
        Self {
            osc_level_on: std::array::from_fn(|i| p.oscs[i].level.value() > 0.0),
            waveform: std::array::from_fn(|i| p.oscs[i].waveform.value()),
            to_filter1: std::array::from_fn(|i| p.oscs[i].to_filter1.value()),
            to_filter2: std::array::from_fn(|i| p.oscs[i].to_filter2.value()),
            filter1: p.filter1.mode.value(),
            filter2: p.filter2.mode.value(),
            serial: p.filter1_to_filter2.value(),
            global: p.global_filter.mode.value(),
            unison: p.unison.voices.value(),
        }
    }

    fn osc_active(&self, i: usize) -> bool {
        self.osc_level_on[i] && (self.to_filter1[i] || self.to_filter2[i])
    }

    fn filter1_active(&self) -> bool {
        (0..NUM_OSCS).any(|i| self.osc_level_on[i] && self.to_filter1[i])
    }

    fn filter2_active(&self) -> bool {
        (0..NUM_OSCS).any(|i| self.osc_level_on[i] && self.to_filter2[i])
            || (self.serial && self.filter1_active())
    }

    fn amp_active(&self) -> bool {
        (self.filter1_active() && !self.serial) || self.filter2_active()
    }
}

/// A node's rectangle as fractions of the diagram's width and height.
#[derive(Clone, Copy)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

fn osc_rect(i: usize) -> Rect {
    Rect { x: 0.02, y: 0.06 + i as f32 * 0.31, w: 0.17, h: 0.24 }
}
const FILTER1: Rect = Rect { x: 0.31, y: 0.08, w: 0.16, h: 0.30 };
const FILTER2: Rect = Rect { x: 0.31, y: 0.62, w: 0.16, h: 0.30 };
const AMP: Rect = Rect { x: 0.57, y: 0.35, w: 0.09, h: 0.30 };
const GLOBAL: Rect = Rect { x: 0.73, y: 0.35, w: 0.15, h: 0.30 };
const OUT: Rect = Rect { x: 0.92, y: 0.38, w: 0.06, h: 0.24 };

fn mode_name(mode: FilterMode) -> &'static str {
    FilterMode::variants()[mode.to_index()]
}

/// Builds the diagram. Must be called with a lens to the plugin's params.
pub fn build<L>(cx: &mut Context, params: L)
where
    L: Lens<Target = Arc<ThreezParams>>,
{
    ZStack::new(cx, |cx| {
        Binding::new(cx, params.map(RoutingState::read), |cx, state| {
            let s = state.get(cx);

            RoutingCanvas { state: s }
                .build(cx, |_| {})
                .position_type(PositionType::SelfDirected)
                .width(Stretch(1.0))
                .height(Stretch(1.0));

            for i in 0..NUM_OSCS {
                let wave = Waveform::variants()[s.waveform[i].to_index()].to_uppercase();
                let text = if s.unison > 1 {
                    format!("OSC {} · {wave} ×{}", i + 1, s.unison)
                } else {
                    format!("OSC {} · {wave}", i + 1)
                };
                node_label(cx, &text, osc_rect(i), s.osc_active(i));
            }
            let slot = |n: u8, mode: FilterMode| match mode {
                FilterMode::Off => format!("F{n} · thru"),
                m => format!("F{n} · {}", mode_name(m)),
            };
            node_label(cx, &slot(1, s.filter1), FILTER1, s.filter1_active());
            node_label(cx, &slot(2, s.filter2), FILTER2, s.filter2_active());
            node_label(cx, "AMP", AMP, s.amp_active());
            let global = match s.global {
                FilterMode::Off => "GLOBAL · thru".to_string(),
                m => format!("GLOBAL · {}", mode_name(m)),
            };
            node_label(cx, &global, GLOBAL, s.amp_active());
            node_label(cx, "OUT", OUT, s.amp_active());
        });
    })
    .class("routing");
}

fn node_label(cx: &mut Context, text: &str, r: Rect, active: bool) {
    Label::new(cx, text)
        .class("node-label")
        .toggle_class("active", active)
        .position_type(PositionType::SelfDirected)
        .left(Percentage(r.x * 100.0))
        .top(Percentage(r.y * 100.0))
        .width(Percentage(r.w * 100.0))
        .height(Percentage(r.h * 100.0))
        .child_space(Stretch(1.0))
        .hoverable(false);
}

struct RoutingCanvas {
    state: RoutingState,
}

const ACCENT: (u8, u8, u8) = (0xff, 0x8a, 0x3d);
const DIM: (u8, u8, u8) = (0x4a, 0x4d, 0x57);
const BOX_FILL: (u8, u8, u8) = (0x26, 0x28, 0x2f);

fn color((r, g, b): (u8, u8, u8), alpha: f32) -> vg::Color {
    vg::Color::rgba(r, g, b, (alpha * 255.0) as u8)
}

impl View for RoutingCanvas {
    fn element(&self) -> Option<&'static str> {
        Some("routing-canvas")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let b = cx.bounds();
        if b.w == 0.0 || b.h == 0.0 {
            return;
        }
        let px = cx.logical_to_physical(1.0);
        let opacity = cx.opacity();
        let s = &self.state;

        // Fractional coordinates to pixels
        let pt = |x: f32, y: f32| (b.x + x * b.w, b.y + y * b.h);
        let left_at = |r: Rect, f: f32| pt(r.x, r.y + r.h * f);
        let right_mid = |r: Rect| pt(r.x + r.w, r.y + r.h * 0.5);

        let wire = |canvas: &mut Canvas, from: (f32, f32), to: (f32, f32), active: bool| {
            let mut path = vg::Path::new();
            path.move_to(from.0, from.1);
            let mid = (from.0 + to.0) * 0.5;
            path.bezier_to(mid, from.1, mid, to.1, to.0, to.1);
            let mut paint = vg::Paint::color(color(if active { ACCENT } else { DIM }, opacity));
            paint.set_line_width(if active { 2.0 } else { 1.5 } * px);
            canvas.stroke_path(&path, &paint);

            // A dot where the wire enters the next node
            let mut dot = vg::Path::new();
            dot.circle(to.0, to.1, 2.5 * px);
            canvas.fill_path(&dot, &vg::Paint::color(color(if active { ACCENT } else { DIM }, opacity)));
        };

        // Wires first, so the boxes sit on top of their ends
        for i in 0..NUM_OSCS {
            let spot = 0.3 + 0.2 * i as f32;
            if s.to_filter1[i] {
                wire(canvas, right_mid(osc_rect(i)), left_at(FILTER1, spot), s.osc_active(i));
            }
            if s.to_filter2[i] {
                wire(canvas, right_mid(osc_rect(i)), left_at(FILTER2, spot), s.osc_active(i));
            }
        }
        if s.serial {
            // Straight down from filter 1 into filter 2
            let x = FILTER1.x + FILTER1.w * 0.5;
            let mut path = vg::Path::new();
            let (x0, y0) = pt(x, FILTER1.y + FILTER1.h);
            let (_, y1) = pt(x, FILTER2.y);
            path.move_to(x0, y0);
            path.line_to(x0, y1);
            let active = s.filter1_active();
            let mut paint = vg::Paint::color(color(if active { ACCENT } else { DIM }, opacity));
            paint.set_line_width(2.0 * px);
            canvas.stroke_path(&path, &paint);
        } else {
            wire(canvas, right_mid(FILTER1), left_at(AMP, 0.35), s.filter1_active());
        }
        wire(canvas, right_mid(FILTER2), left_at(AMP, 0.65), s.filter2_active());
        wire(canvas, right_mid(AMP), left_at(GLOBAL, 0.5), s.amp_active());
        wire(canvas, right_mid(GLOBAL), left_at(OUT, 0.5), s.amp_active());

        let node = |canvas: &mut Canvas, r: Rect, active: bool| {
            let (x, y) = pt(r.x, r.y);
            let mut path = vg::Path::new();
            path.rounded_rect(x, y, r.w * b.w, r.h * b.h, 4.0 * px);
            canvas.fill_path(&path, &vg::Paint::color(color(BOX_FILL, opacity)));
            let mut stroke = vg::Paint::color(color(if active { ACCENT } else { DIM }, opacity));
            stroke.set_line_width(1.0 * px);
            canvas.stroke_path(&path, &stroke);
        };
        for i in 0..NUM_OSCS {
            node(canvas, osc_rect(i), s.osc_active(i));
        }
        node(canvas, FILTER1, s.filter1_active());
        node(canvas, FILTER2, s.filter2_active());
        node(canvas, AMP, s.amp_active());
        node(canvas, GLOBAL, s.amp_active());
        node(canvas, OUT, s.amp_active());
    }
}
