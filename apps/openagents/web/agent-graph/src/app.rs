//! The graph view: units, pins and links drawn on a dark canvas you can pan,
//! zoom and drag, with the selected unit's details in a side panel.
//!
//! Links and moving data are painted on a `canvas` under the units; units
//! are absolutely placed boxes whose text GPUI lays out at the current zoom,
//! so it stays crisp; pins are painted over them. Pointer input is handled
//! once, on the graph area, against the layout's boxes.

use agent_graph::layout::{self, HEADER, Layout, PIN_ROW, Rect, Side};
use agent_graph::{Detail, GraphSpec, Unit, words};
use gpui::{
    AnyElement, Bounds, Context, FontWeight, Hsla, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, PathBuilder, Pixels, Render, ScrollWheelEvent,
    SharedString, Styled, Window, canvas, div, fill, point, prelude::*, px, size,
};
use web_time::{SystemTime, UNIX_EPOCH};

use crate::theme;

const PANEL_W: f32 = 300.0;
/// Below this width the panel moves under the graph.
const NARROW: f32 = 760.0;
const MIN_ZOOM: f32 = 0.2;
const MAX_ZOOM: f32 = 2.5;
/// Unit's pin radius (`PIN_RADIUS`).
const PIN_R: f32 = 5.0;
/// The dot grid's spacing, in graph units.
const GRID: f32 = 32.0;
/// How far a press may move and still count as a click, in pixels.
const CLICK_SLOP: f32 = 4.0;

enum Drag {
    None,
    Pan { from: (f32, f32), pan: (f32, f32) },
    Unit { unit: usize, last: (f32, f32) },
}

pub struct GraphView {
    spec: GraphSpec,
    layout: Layout,
    zoom: f32,
    /// Where the graph's origin is on the screen.
    pan: (f32, f32),
    fitted: bool,
    selected: Option<usize>,
    hovered: Option<usize>,
    focused: Option<usize>,
    drag: Drag,
    down_at: (f32, f32),
    moved: bool,
    events: Vec<agent_graph::events::Event>,
    still: bool,
    /// The graph area's size, from the last frame.
    area: (f32, f32),
}

fn f(p: Pixels) -> f32 {
    f32::from(p)
}

impl GraphView {
    pub fn new(spec: GraphSpec, still: bool, events: Vec<agent_graph::events::Event>) -> Self {
        let layout = layout::layout(&spec);
        Self {
            spec,
            layout,
            zoom: 1.0,
            pan: (0.0, 0.0),
            fitted: false,
            selected: None,
            hovered: None,
            focused: None,
            drag: Drag::None,
            down_at: (0.0, 0.0),
            moved: false,
            events,
            still,
            area: (0.0, 0.0),
        }
    }

    pub(crate) fn keyboard(&mut self, key: &str, unit: usize) -> bool {
        match key {
            "Enter" | " " => self.selected = Some(unit),
            "ArrowLeft" => self.pan.0 += 40.0,
            "ArrowRight" => self.pan.0 -= 40.0,
            "ArrowUp" => self.pan.1 += 40.0,
            "ArrowDown" => self.pan.1 -= 40.0,
            "+" | "=" => self.zoom_at(1.2, self.area.0 / 2.0, self.area.1 / 2.0),
            "-" => self.zoom_at(1.0 / 1.2, self.area.0 / 2.0, self.area.1 / 2.0),
            _ => return false,
        }
        true
    }

    pub(crate) fn focus_unit(&mut self, unit: Option<usize>) {
        self.focused = unit;
        if let Some(unit) = unit {
            let b = self.layout.boxes[unit];
            self.pan = (
                self.area.0 / 2.0 - (b.x + b.w / 2.0) * self.zoom,
                self.area.1 / 2.0 - (b.y + b.h / 2.0) * self.zoom,
            );
        }
    }

    pub(crate) fn tab_order(&self) -> Vec<usize> {
        let mut order: Vec<_> = (0..self.spec.units.len()).collect();
        order.sort_by(|&a, &b| {
            let a = self.layout.boxes[a];
            let b = self.layout.boxes[b];
            a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y))
        });
        order
    }

    fn to_graph(&self, x: f32, y: f32) -> (f32, f32) {
        ((x - self.pan.0) / self.zoom, (y - self.pan.1) / self.zoom)
    }

    fn to_screen(&self, x: f32, y: f32) -> (f32, f32) {
        (self.pan.0 + x * self.zoom, self.pan.1 + y * self.zoom)
    }

    /// Fits the whole graph in the area, at most at zoom 1, but no smaller
    /// than `least`, so words stay readable.
    fn fit_at_least(&mut self, least: f32) {
        self.fit();
        if self.zoom < least {
            let (w, h) = self.area;
            self.zoom_at(least / self.zoom, w / 2.0, h / 2.0);
        }
    }

    /// Fits the whole graph in the area, at most at zoom 1.
    fn fit(&mut self) {
        let (w, h) = self.area;
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let b = self.layout.bounds();
        let margin = 48.0;
        let zoom = ((w - 2.0 * margin) / b.w)
            .min((h - 2.0 * margin - 40.0) / b.h)
            .clamp(MIN_ZOOM, 1.0);
        self.zoom = zoom;
        self.pan = (
            w / 2.0 - (b.x + b.w / 2.0) * zoom,
            h / 2.0 + 20.0 - (b.y + b.h / 2.0) * zoom,
        );
    }

    /// Zooms by `factor` keeping the graph point under (x, y) still.
    fn zoom_at(&mut self, factor: f32, x: f32, y: f32) {
        let zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let k = zoom / self.zoom;
        self.pan = (x - (x - self.pan.0) * k, y - (y - self.pan.1) * k);
        self.zoom = zoom;
    }

    /// The top-level unit a unit sits in.
    fn root(&self, unit: usize) -> usize {
        let id = self.spec.root_of(&self.spec.units[unit].id);
        self.spec
            .units
            .iter()
            .position(|u| u.id == id)
            .unwrap_or(unit)
    }

    fn on_down(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let (x, y) = (f(event.position.x), f(event.position.y));
        let (gx, gy) = self.to_graph(x, y);
        self.down_at = (x, y);
        self.moved = false;
        match self.layout.hit(gx, gy) {
            Some(unit) => {
                self.selected = Some(unit);
                self.drag = Drag::Unit {
                    unit: self.root(unit),
                    last: (x, y),
                };
            }
            None => {
                self.drag = Drag::Pan {
                    from: (x, y),
                    pan: self.pan,
                };
            }
        }
        cx.notify();
    }

    fn on_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let (x, y) = (f(event.position.x), f(event.position.y));
        if event.pressed_button != Some(MouseButton::Left) {
            self.drag = Drag::None;
        }
        if (x - self.down_at.0).abs() + (y - self.down_at.1).abs() > CLICK_SLOP {
            self.moved = true;
        }
        match self.drag {
            Drag::Pan { from, pan } => {
                self.pan = (pan.0 + x - from.0, pan.1 + y - from.1);
                cx.notify();
            }
            Drag::Unit { unit, last } => {
                let (dx, dy) = ((x - last.0) / self.zoom, (y - last.1) / self.zoom);
                self.layout.translate(&self.spec, unit, dx, dy);
                self.drag = Drag::Unit { unit, last: (x, y) };
                cx.notify();
            }
            Drag::None => {
                let (gx, gy) = self.to_graph(x, y);
                let hovered = self.layout.hit(gx, gy);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    cx.notify();
                }
            }
        }
    }

    fn on_up(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.drag, Drag::Pan { .. }) && !self.moved {
            self.selected = None;
        }
        self.drag = Drag::None;
        cx.notify();
    }

    fn on_wheel(&mut self, event: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let delta = event.delta.pixel_delta(px(16.0));
        let (x, y) = (f(event.position.x), f(event.position.y));
        if event.modifiers.shift {
            self.pan = (self.pan.0 - f(delta.x) - f(delta.y), self.pan.1);
        } else {
            self.zoom_at((f(delta.y) * 0.0025).exp(), x, y);
        }
        cx.notify();
    }

    /// Whether a link touches the selected or hovered unit, or a unit inside it.
    fn lit(&self, unit: &str) -> bool {
        [self.selected, self.hovered]
            .into_iter()
            .flatten()
            .any(|at| {
                let id = &self.spec.units[at].id;
                unit == id || self.spec.root_of(unit) == id
            })
    }
}

/// A point on the cubic from `a` to `b` with controls `c1`, `c2`, at `t`.
fn cubic(a: (f32, f32), c1: (f32, f32), c2: (f32, f32), b: (f32, f32), t: f32) -> (f32, f32) {
    let u = 1.0 - t;
    let (w0, w1, w2, w3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    (
        w0 * a.0 + w1 * c1.0 + w2 * c2.0 + w3 * b.0,
        w0 * a.1 + w1 * c1.1 + w2 * c2.1 + w3 * b.1,
    )
}

/// A link's curve on screen: from an output pin to an input pin, bowing out
/// sideways; a link against the flow loops under both units.
struct Curve {
    a: (f32, f32),
    c1: (f32, f32),
    c2: (f32, f32),
    b: (f32, f32),
    color: Hsla,
    width: f32,
    lit: bool,
}

struct Dot {
    at: (f32, f32),
    r: f32,
    color: Hsla,
}

struct PinMark {
    at: (f32, f32),
    r: f32,
    color: Hsla,
    filled: bool,
}

struct Scene {
    grid: Vec<(f32, f32)>,
    grid_r: f32,
    curves: Vec<Curve>,
    dots: Vec<Dot>,
}

impl GraphView {
    fn scene(&self, t: u64) -> (Scene, Vec<PinMark>) {
        let z = self.zoom;
        let (w, h) = self.area;
        let mut grid = vec![];
        let step = if z < 0.5 { GRID * 2.0 } else { GRID } * z;
        if step > 6.0 {
            let x0 = self.pan.0.rem_euclid(step);
            let y0 = self.pan.1.rem_euclid(step);
            let mut x = x0;
            while x < w {
                let mut y = y0;
                while y < h {
                    grid.push((x, y));
                    y += step;
                }
                x += step;
            }
        }
        let mut curves = vec![];
        let mut dots = vec![];
        let mut linked: Vec<(usize, String, Side)> = vec![];
        let index = |id: &str| self.spec.units.iter().position(|u| u.id == id);
        let bounds = self.layout.bounds();
        for (at, link) in self.spec.links.iter().enumerate() {
            let (Some(from), Some(to)) = (index(&link.from.unit), index(&link.to.unit)) else {
                continue;
            };
            let (Some((x1, y1, _)), Some((x2, y2, _))) = (
                self.layout.pin(&self.spec, from, &link.from.pin),
                self.layout.pin(&self.spec, to, &link.to.pin),
            ) else {
                continue;
            };
            linked.push((from, link.from.pin.clone(), Side::Output));
            linked.push((to, link.to.pin.clone(), Side::Input));
            let a = self.to_screen(x1, y1);
            let b = self.to_screen(x2, y2);
            let (c1, c2) = if self.layout.back[at] {
                let under = self.to_screen(0.0, bounds.y + bounds.h + 56.0).1;
                ((a.0 + 160.0 * z, under), (b.0 - 160.0 * z, under))
            } else {
                let dx = ((b.0 - a.0).abs() * 0.5).max(48.0 * z);
                ((a.0 + dx, a.1), (b.0 - dx, b.1))
            };
            let lit = self.lit(&link.from.unit) || self.lit(&link.to.unit);
            let accent = theme::accent(self.spec.units[from].accent);
            let color = if lit {
                accent
            } else {
                Hsla { a: 0.42, ..accent }
            };
            let width = if lit { 2.4 } else { 1.6 } * z.clamp(0.6, 1.4);
            if !self.still {
                for event in &self.events {
                    if let Some(phase) = agent_graph::events::phase(event.at, t) {
                        if event.links(&self.spec).contains(&at) {
                            dots.push(Dot {
                                at: cubic(a, c1, c2, b, phase),
                                r: 3.4 * z.clamp(0.6, 1.4),
                                color: accent,
                            });
                        }
                    }
                }
            }
            curves.push(Curve {
                a,
                c1,
                c2,
                b,
                color,
                width,
                lit,
            });
        }
        // Lit links last, so they draw over the rest.
        curves.sort_by_key(|c| c.lit);
        let mut pins = vec![];
        for (at, unit) in self.spec.units.iter().enumerate() {
            let accent = theme::accent(unit.accent);
            for pin in unit.inputs.iter().chain(&unit.outputs) {
                if let Some((x, y, side)) = self.layout.pin(&self.spec, at, &pin.id) {
                    let filled = linked
                        .iter()
                        .any(|(u, p, s)| *u == at && *p == pin.id && *s == side);
                    pins.push(PinMark {
                        at: self.to_screen(x, y),
                        r: PIN_R * z.clamp(0.5, 1.6),
                        color: accent,
                        filled,
                    });
                }
            }
        }
        (
            Scene {
                grid,
                grid_r: (1.1 * z).clamp(0.6, 1.4),
                curves,
                dots,
            },
            pins,
        )
    }

    fn unit_box(&self, at: usize, unit: &Unit, b: Rect) -> AnyElement {
        let z = self.zoom;
        let (x, y) = self.to_screen(b.x, b.y);
        let (w, h) = (b.w * z, b.h * z);
        let accent = theme::accent(unit.accent);
        let group = self.spec.children(Some(&unit.id)).next().is_some();
        let selected = self.selected == Some(at);
        let hovered = self.hovered == Some(at) || self.focused == Some(at);
        let border = if selected {
            accent
        } else if hovered {
            theme::border_strong()
        } else {
            theme::border()
        };
        let detail = z >= 0.45;
        let mut el = div()
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(w))
            .h(px(h))
            .rounded(px(12.0 * z))
            .bg(if group { theme::group() } else { theme::unit() })
            .border_1()
            .border_color(border)
            .child(
                div()
                    .absolute()
                    .left(px(0.0))
                    .top(px(14.0 * z))
                    .w(px((3.0 * z).max(2.0)))
                    .h(px(22.0 * z))
                    .rounded(px(2.0 * z))
                    .bg(accent),
            )
            .child(
                div()
                    .absolute()
                    .left(px(14.0 * z))
                    .right(px(12.0 * z))
                    .top(px(9.0 * z))
                    .truncate()
                    .text_size(px(13.5 * z))
                    .line_height(px(18.0 * z))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::text())
                    .child(SharedString::from(unit.title.clone())),
            );
        if detail && !unit.line.is_empty() {
            el = el.child(
                div()
                    .absolute()
                    .left(px(14.0 * z))
                    .right(px(12.0 * z))
                    .top(px(29.0 * z))
                    .truncate()
                    .font_family(theme::MONO)
                    .text_size(px(11.0 * z))
                    .line_height(px(14.0 * z))
                    .text_color(theme::text_muted())
                    .child(SharedString::from(unit.line.clone())),
            );
        }
        if detail {
            for (i, pin) in unit.inputs.iter().enumerate() {
                el = el.child(pin_label(&pin.name, i, z, false));
            }
            for (i, pin) in unit.outputs.iter().enumerate() {
                el = el.child(pin_label(&pin.name, i, z, true));
            }
        }
        if detail && !unit.tags.is_empty() {
            el = el.child(
                div()
                    .absolute()
                    .right(px(6.0 * z))
                    .top(px(-17.0 * z))
                    .flex()
                    .gap(px(4.0 * z))
                    .children(unit.tags.iter().take(4).map(|tag| {
                        div()
                            .px(px(5.0 * z))
                            .rounded(px(4.0 * z))
                            .bg(theme::chip())
                            .font_family(theme::MONO)
                            .text_size(px(9.5 * z))
                            .line_height(px(14.0 * z))
                            .text_color(theme::text_muted())
                            .child(SharedString::from(tag.clone()))
                    })),
            );
        }
        el.into_any_element()
    }

    fn controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let button = |label: &'static str| {
            div()
                .id(label)
                .px(px(10.0))
                .h(px(28.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(7.0))
                .bg(theme::panel())
                .border_1()
                .border_color(theme::border())
                .text_size(px(13.0))
                .text_color(theme::text())
                .cursor_pointer()
                .hover(|s| s.bg(theme::chip()))
                .child(label)
        };
        let center = (self.area.0 / 2.0, self.area.1 / 2.0);
        div()
            .absolute()
            .right(px(12.0))
            // The page's toolbar sits at the top left; on a narrow screen it
            // can take the whole top, so the zoom controls go to the foot.
            .top(px(if self.area.0 < NARROW {
                self.area.1 - 40.0
            } else {
                12.0
            }))
            .flex()
            .gap(px(6.0))
            .child(
                div()
                    .h(px(28.0))
                    .px(px(8.0))
                    .flex()
                    .items_center()
                    .font_family(theme::MONO)
                    .text_size(px(12.0))
                    .text_color(theme::text_muted())
                    .child(format!("{:.0}%", self.zoom * 100.0)),
            )
            .child(button("−").on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.zoom_at(1.0 / 1.25, center.0, center.1);
                    cx.notify();
                }),
            ))
            .child(button("+").on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.zoom_at(1.25, center.0, center.1);
                    cx.notify();
                }),
            ))
            .child(button(words::FIT).on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.fit();
                    cx.notify();
                }),
            ))
    }

    fn panel(&self, narrow: bool) -> impl IntoElement {
        let mut panel = div()
            .id("panel")
            .flex()
            .flex_col()
            .gap(px(14.0))
            .p(px(18.0))
            .bg(theme::panel())
            .border_color(theme::border())
            .overflow_y_scroll()
            .text_color(theme::text())
            .text_size(px(13.5))
            .line_height(px(20.0));
        panel = if narrow {
            panel.w_full().flex_1().border_t_1()
        } else {
            panel.w(px(PANEL_W)).h_full().border_l_1()
        };
        let Some(at) = self.selected else {
            return panel
                .child(heading(&self.spec.title, theme::text()))
                .child(muted(&self.spec.description))
                .child(muted(words::HINT));
        };
        let unit = &self.spec.units[at];
        let accent = theme::accent(unit.accent);
        panel = panel.child(
            div()
                .flex()
                .gap(px(10.0))
                .child(div().w(px(3.0)).h(px(40.0)).rounded(px(2.0)).bg(accent))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(heading(&unit.title, theme::text()))
                        .child(mono(&unit.line)),
                ),
        );
        if !unit.tags.is_empty() {
            panel = panel.child(chips(&unit.tags));
        }
        let index = |id: &str| {
            self.spec
                .unit(id)
                .map_or(id.to_owned(), |u| u.title.clone())
        };
        let takes: Vec<String> = unit
            .inputs
            .iter()
            .map(|pin| {
                let from: Vec<String> = self
                    .spec
                    .links
                    .iter()
                    .filter(|l| l.to.unit == unit.id && l.to.pin == pin.id)
                    .map(|l| index(&l.from.unit))
                    .collect();
                if from.is_empty() {
                    pin.name.clone()
                } else {
                    format!("{} {} {}", pin.name, words::FROM, from.join(", "))
                }
            })
            .collect();
        let sends: Vec<String> = unit
            .outputs
            .iter()
            .map(|pin| {
                let to: Vec<String> = self
                    .spec
                    .links
                    .iter()
                    .filter(|l| l.from.unit == unit.id && l.from.pin == pin.id)
                    .map(|l| index(&l.to.unit))
                    .collect();
                if to.is_empty() {
                    pin.name.clone()
                } else {
                    format!("{} {} {}", pin.name, words::TO, to.join(", "))
                }
            })
            .collect();
        for detail in &unit.details {
            panel = panel.child(detail_block(detail));
        }
        let inside: Vec<String> = self
            .spec
            .children(Some(&unit.id))
            .map(|u| u.title.clone())
            .collect();
        if !inside.is_empty() {
            panel = panel.child(list(Some(words::INSIDE), &inside));
        }
        if !takes.is_empty() {
            panel = panel.child(list(Some(words::TAKES), &takes));
        }
        if !sends.is_empty() {
            panel = panel.child(list(Some(words::SENDS), &sends));
        }
        panel
    }
}

fn pin_label(name: &str, row: usize, z: f32, output: bool) -> impl IntoElement {
    let top = (HEADER + PIN_ROW * row as f32 + 3.0) * z;
    let label = div()
        .absolute()
        .top(px(top))
        .font_family(theme::MONO)
        .text_size(px(10.5 * z))
        .line_height(px(16.0 * z))
        .text_color(theme::text_faint())
        .child(SharedString::from(name.to_owned()));
    if output {
        label.right(px(12.0 * z))
    } else {
        label.left(px(12.0 * z))
    }
}

fn heading(text: &str, color: Hsla) -> impl IntoElement {
    div()
        .text_size(px(16.0))
        .line_height(px(22.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color)
        .child(SharedString::from(text.to_owned()))
}

fn muted(text: &str) -> impl IntoElement {
    div()
        .text_color(theme::text_muted())
        .child(SharedString::from(text.to_owned()))
}

fn mono(text: &str) -> impl IntoElement {
    div()
        .font_family(theme::MONO)
        .text_size(px(12.0))
        .text_color(theme::text_muted())
        .child(SharedString::from(text.to_owned()))
}

fn label(text: &str) -> impl IntoElement {
    div()
        .text_size(px(12.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme::text_muted())
        .child(SharedString::from(text.to_owned()))
}

fn chips(items: &[String]) -> impl IntoElement {
    div()
        .flex()
        .flex_wrap()
        .gap(px(6.0))
        .children(items.iter().map(|item| {
            div()
                .px(px(7.0))
                .rounded(px(5.0))
                .bg(theme::chip())
                .font_family(theme::MONO)
                .text_size(px(12.0))
                .line_height(px(20.0))
                .child(SharedString::from(item.clone()))
        }))
}

fn list(title: Option<&str>, items: &[String]) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .children(title.map(label))
        .children(items.iter().map(|item| {
            div()
                .flex()
                .gap(px(8.0))
                .child(div().text_color(theme::text_faint()).child("•"))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .child(SharedString::from(item.clone())),
                )
        }))
}

fn detail_block(detail: &Detail) -> AnyElement {
    match detail {
        Detail::Text { text } => div()
            .child(SharedString::from(text.clone()))
            .into_any_element(),
        Detail::Quote { text } => div()
            .pl(px(10.0))
            .border_l_2()
            .border_color(theme::border())
            .text_color(theme::text_muted())
            .child(SharedString::from(text.clone()))
            .into_any_element(),
        Detail::Fact { label: l, value } => div()
            .flex()
            .flex_col()
            .child(label(l))
            .child(SharedString::from(value.clone()))
            .into_any_element(),
        Detail::Chips { label: l, items } => div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .children(l.as_deref().map(label))
            .child(chips(items))
            .into_any_element(),
        Detail::List { label: l, items } => list(l.as_deref(), items).into_any_element(),
    }
}

impl Render for GraphView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        #[cfg(target_family = "wasm")]
        crate::accessibility::sync(&self.spec, self.selected);
        let viewport = window.viewport_size();
        let (vw, vh) = (f(viewport.width), f(viewport.height));
        let narrow = vw < NARROW;
        self.area = if narrow {
            (vw, (vh * 0.62).round())
        } else {
            ((vw - PANEL_W).max(1.0), vh)
        };
        if !self.fitted && self.area.0 > 1.0 && self.area.1 > 1.0 {
            self.fit_at_least(0.5);
            self.fitted = true;
        }
        let t = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let (scene, pins) = self.scene(t);
        if !self.still && self.events.iter().any(|e| e.at.saturating_add(1000) > t) {
            window.request_animation_frame();
        }

        // Parents before children, so children draw over them.
        let mut order: Vec<usize> = (0..self.spec.units.len()).collect();
        order.sort_by_key(|&at| self.layout.depth[at]);
        let (aw, ah) = self.area;
        let units: Vec<AnyElement> = order
            .into_iter()
            .filter(|&at| {
                let b = self.layout.boxes[at];
                let (x, y) = self.to_screen(b.x, b.y);
                x < aw && y < ah && x + b.w * self.zoom > 0.0 && y + b.h * self.zoom > 0.0
            })
            .map(|at| self.unit_box(at, &self.spec.units[at], self.layout.boxes[at]))
            .collect();

        let links = canvas(
            move |_, _, _| scene,
            move |bounds: Bounds<Pixels>, scene: Scene, window, _| {
                let o = (f(bounds.origin.x), f(bounds.origin.y));
                let at = |p: (f32, f32)| point(px(o.0 + p.0), px(o.1 + p.1));
                for &(x, y) in &scene.grid {
                    let r = scene.grid_r;
                    window.paint_quad(
                        fill(
                            Bounds::new(at((x - r, y - r)), size(px(2.0 * r), px(2.0 * r))),
                            theme::grid(),
                        )
                        .corner_radii(px(r)),
                    );
                }
                for c in &scene.curves {
                    let mut path = PathBuilder::stroke(px(c.width));
                    path.move_to(at(c.a));
                    path.cubic_bezier_to(at(c.b), at(c.c1), at(c.c2));
                    if let Ok(path) = path.build() {
                        window.paint_path(path, c.color);
                    }
                }
                for d in &scene.dots {
                    window.paint_quad(
                        fill(
                            Bounds::new(
                                at((d.at.0 - d.r, d.at.1 - d.r)),
                                size(px(2.0 * d.r), px(2.0 * d.r)),
                            ),
                            d.color,
                        )
                        .corner_radii(px(d.r)),
                    );
                }
            },
        )
        .absolute()
        .size_full();

        let pins = canvas(
            move |_, _, _| pins,
            move |bounds: Bounds<Pixels>, pins: Vec<PinMark>, window, _| {
                let o = (f(bounds.origin.x), f(bounds.origin.y));
                for p in &pins {
                    let side = px(2.0 * p.r);
                    window.paint_quad(
                        fill(
                            Bounds::new(
                                point(px(o.0 + p.at.0 - p.r), px(o.1 + p.at.1 - p.r)),
                                size(side, side),
                            ),
                            if p.filled { p.color } else { theme::bg() },
                        )
                        .corner_radii(px(p.r))
                        .border_widths(px(1.5))
                        .border_color(p.color),
                    );
                }
            },
        )
        .absolute()
        .size_full();

        let grabbing = !matches!(self.drag, Drag::None);
        let mut area = div()
            .id("graph")
            .relative()
            .overflow_hidden()
            .bg(theme::bg())
            .font_family(theme::SANS)
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_down))
            .on_mouse_move(cx.listener(Self::on_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_up))
            .on_scroll_wheel(cx.listener(Self::on_wheel))
            .child(links)
            .children(units)
            .child(pins)
            .child(self.controls(cx));
        area = if grabbing {
            area.cursor_grabbing()
        } else if self.hovered.is_some() {
            area.cursor_pointer()
        } else {
            area.cursor_grab()
        };
        area = if narrow {
            area.w_full().h(px(self.area.1))
        } else {
            area.flex_1().h_full()
        };

        let root = div()
            .size_full()
            .flex()
            .bg(theme::bg())
            .font_family(theme::SANS)
            .child(area)
            .child(self.panel(narrow));
        if narrow {
            root.flex_col()
        } else {
            root.flex_row()
        }
    }
}

#[cfg(test)]
mod keyboard_tests {
    use super::*;

    #[test]
    fn keyboard_mirror_follows_layout_and_controls_graph() {
        let mut spec = GraphSpec::new("test", "Test", "");
        for id in ["a", "b", "c"] {
            spec.units
                .push(Unit::new(id, id, "", agent_graph::Accent::Info));
        }
        let mut view = GraphView::new(spec, true, vec![]);
        view.area = (800.0, 600.0);
        view.layout.boxes[0].x = 300.0;
        view.layout.boxes[1].x = 0.0;
        view.layout.boxes[2].x = 0.0;
        view.layout.boxes[1].y = 100.0;
        view.layout.boxes[2].y = 0.0;
        assert_eq!(view.tab_order(), vec![2, 1, 0]);
        view.focus_unit(Some(2));
        assert_eq!(view.focused, Some(2));
        assert_eq!(view.selected, None);
        assert!(view.keyboard("Enter", 2));
        assert_eq!(view.selected, Some(2));
        let pan = view.pan;
        for key in ["ArrowLeft", "ArrowUp", "ArrowRight", "ArrowDown"] {
            assert!(view.keyboard(key, 2));
        }
        assert_eq!(view.pan, pan);
        assert!(view.keyboard("+", 2));
        assert!(view.zoom > 1.0);
        assert!(view.keyboard("-", 2));
        assert!((view.zoom - 1.0).abs() < 0.001);
        assert!(!view.keyboard("Tab", 2));
        view.focus_unit(None);
        assert_eq!(view.focused, None);
    }
}
