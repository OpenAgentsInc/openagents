//! The page's canvas: the demo chat as the graph's center unit, the parts it
//! uses around it as Unit-style units with pins, the links between them lit
//! only while data or sats cross them, a part's details on hover or tap,
//! and the composer's text field at the bottom.
//!
//! The chat is its own view, drawn cached, so the graph's motion doesn't
//! lay out the transcript again. Everything ticks at about 30 frames a
//! second and only while something moves, to leave the page's background
//! room on a slow computer.

use std::time::Duration;

use gpui::{
    AnyElement, AnyView, App, Bounds, Context, Entity, Focusable, FontWeight, Hsla, IntoElement,
    KeyBinding, MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement, PathBuilder, Pixels,
    Render, SharedString, StyleRefinement, Styled, Window, actions, canvas, div, fill, point,
    prelude::*, px, rgb, size, svg,
};
use openagents_web_chat_view::composer::{self, TextInput};
use openagents_web_chat_view::theme;
use openagents_web_chat_view::transcript::{footer_text, working_label};
use web_time::Instant;

use agent_graph::Accent;

use crate::chat::ChatView;
use crate::graph::{self, Graph};
use crate::scene::{Geometry, Node};

actions!(graph_card, [Hold]);

/// The composer's placeholder.
pub const PLACEHOLDER: &str = "Message OpenAgents";
/// What the page says about itself under the composer.
pub const DEMO_NOTE: &str = "A demo. Nothing here reads your accounts, sends mail, or moves money.";

/// The composer's keys, with Enter held: the field takes typing and sends
/// nothing.
#[cfg_attr(not(target_family = "wasm"), allow(dead_code))]
pub fn bind_keys(cx: &mut App) {
    composer::bind_keys(cx);
    // Bound after the composer's own Enter, so it wins.
    cx.bind_keys([KeyBinding::new("enter", Hold, Some("Composer"))]);
}

/// The time between ticks.
const TICK: Duration = Duration::from_millis(33);

/// `--color-text-{info,discovery,warning,success,danger}` in dark mode, and
/// the site's near-white, as the AgentGraph view colors its units.
fn accent(accent: Accent) -> Hsla {
    let hex = match accent {
        Accent::Core => 0xededed,
        Accent::Info => 0x66b5ff,
        Accent::Discovery => 0xad7bf9,
        Accent::Warning => 0xe25507,
        Accent::Success => 0x04b84c,
        Accent::Danger => 0xff6764,
    };
    rgb(hex).into()
}

/// Sats, in Bitcoin orange.
fn sats_color() -> Hsla {
    rgb(0xf7931a).into()
}

fn unit_bg() -> Hsla {
    gpui::hsla(0., 0., 0.07, 0.94)
}

fn f(p: Pixels) -> f32 {
    f32::from(p)
}

pub struct GraphCard {
    chat: Entity<ChatView>,
    composer: Entity<TextInput>,
    graph: Graph,
    still: bool,
    epoch: Instant,
    hovered: Option<Node>,
    selected: Option<Node>,
    /// The last frame's geometry, for pointer input, and the size it is for.
    geometry: Geometry,
    size: (f32, f32),
    working: bool,
    dots: i64,
}

impl GraphCard {
    pub fn new(still: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let chat = cx.new(|_| ChatView::new(still));
        let composer = cx.new(|cx| TextInput::new(PLACEHOLDER, cx));
        window.focus(&composer.focus_handle(cx), cx);
        if !still {
            cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(TICK).await;
                    if this.update(cx, |this, cx| this.tick(cx)).is_err() {
                        break;
                    }
                }
            })
            .detach();
        }
        let size = window.viewport_size();
        Self {
            chat,
            composer,
            graph: if still {
                Graph::settled()
            } else {
                Graph::new()
            },
            still,
            epoch: Instant::now(),
            hovered: None,
            selected: None,
            geometry: Geometry::new(f(size.width), f(size.height)),
            size: (f(size.width), f(size.height)),
            working: !still,
            dots: 0,
        }
    }

    fn now(&self) -> u64 {
        self.epoch.elapsed().as_millis() as u64
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let now = self.now();
        let tick = self.chat.update(cx, |chat, cx| chat.tick(cx));
        let mut changed = tick.reset || !tick.cues.is_empty();
        if tick.reset {
            self.graph.reset();
        }
        for cue in &tick.cues {
            self.graph.cue(cue, now);
        }
        changed |= self.graph.sweep(now);
        let working = self.chat.read(cx).working();
        let dots = crate::now_ms() / 350;
        if working != self.working || (working && dots != self.dots) {
            changed = true;
        }
        self.working = working;
        self.dots = dots;
        if changed {
            cx.notify();
        }
    }

    fn on_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let hovered = self.geometry.hit(f(event.position.x), f(event.position.y));
        if hovered != self.hovered {
            self.hovered = hovered;
            cx.notify();
        }
    }

    fn on_down(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let hit = self.geometry.hit(f(event.position.x), f(event.position.y));
        self.selected = match hit {
            Some(node) if self.selected != Some(node) => Some(node),
            _ => None,
        };
        cx.notify();
    }

    fn line(&self, node: Node, now: u64) -> String {
        let (line, _) = self.graph.lines[node.index()];
        if !self.geometry.narrow {
            return line.to_owned();
        }
        if node == Node::Wallet {
            return graph::sats(self.graph.balance(now));
        }
        crate::script::compact(line).to_owned()
    }

    fn unit(&self, node: Node, now: u64) -> AnyElement {
        let r = self.geometry.node(node);
        let color = accent(node.accent());
        let (_, busy) = self.graph.lines[node.index()];
        let active = self.graph.active(node, now);
        let pointed = self.hovered == Some(node) || self.selected == Some(node);
        let border = if active {
            color.opacity(0.85)
        } else if pointed {
            theme::border_strong().opacity(1.4)
        } else {
            theme::border_strong()
        };
        let pulse = if busy {
            0.6 + 0.4 * ((crate::now_ms() as f32 / 240.).sin() * 0.5 + 0.5)
        } else {
            1.0
        };
        let narrow = self.geometry.narrow;
        let line = self.line(node, now);
        let mut el = div()
            .absolute()
            .left(px(r.x))
            .top(px(r.y))
            .w(px(r.w))
            .h(px(r.h))
            .rounded(px(if narrow { 10. } else { 12. }))
            .bg(if pointed {
                gpui::hsla(0., 0., 0.1, 0.96)
            } else {
                unit_bg()
            })
            .border_1()
            .border_color(border)
            .overflow_hidden()
            .child(
                div()
                    .absolute()
                    .left(px(0.))
                    .top(px(if narrow { 10. } else { 14. }))
                    .w(px(3.))
                    .h(px(if narrow { 16. } else { 22. }))
                    .rounded(px(2.))
                    .bg(color),
            );
        let title_size = if narrow { 12. } else { 13.5 };
        let mut title_row = div()
            .absolute()
            .left(px(if narrow { 9. } else { 14. }))
            .right(px(if narrow { 6. } else { 12. }))
            .top(px(if narrow { 7. } else { 12. }))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(title_size))
                    .line_height(px(title_size + 5.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::text())
                    .child(if narrow { node.short() } else { node.title() }),
            );
        if node == Node::Wallet && !narrow {
            title_row = title_row.child(
                div()
                    .flex_none()
                    .font_family(theme::MONO)
                    .text_size(px(12.))
                    .text_color(sats_color())
                    .child(format!("{} sats", graph::sats(self.graph.balance(now)))),
            );
        }
        el = el.child(title_row).child(
            div()
                .absolute()
                .left(px(if narrow { 9. } else { 14. }))
                .right(px(if narrow { 6. } else { 12. }))
                .top(px(if narrow { 25. } else { 35. }))
                .truncate()
                .font_family(theme::MONO)
                .text_size(px(if narrow { 10. } else { 11.5 }))
                .line_height(px(if narrow { 14. } else { 16. }))
                .text_color(if busy || active {
                    color.opacity(pulse)
                } else if node == Node::Wallet && narrow {
                    sats_color()
                } else {
                    theme::text_muted()
                })
                .child(SharedString::from(line)),
        );
        el.into_any_element()
    }

    fn chat_card(&self, cx: &App) -> AnyElement {
        let r = self.geometry.chat;
        let chat = self.chat.read(cx);
        let status = if chat.working() {
            working_label().into_any_element()
        } else {
            footer_text("Waiting for you").into_any_element()
        };
        div()
            .absolute()
            .left(px(r.x))
            .top(px(r.y))
            .w(px(r.w))
            .h(px(r.h))
            .flex()
            .flex_col()
            .rounded(px(16.))
            .overflow_hidden()
            .bg(gpui::hsla(0., 0., 0.035, 0.9))
            .border_1()
            .border_color(theme::border_strong())
            .child(
                div()
                    .flex_none()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .px(px(16.))
                    .h(px(44.))
                    .border_b_1()
                    .border_color(theme::border())
                    .child(
                        svg()
                            .path("icons/openagents-logo.svg")
                            .size(px(14.))
                            .text_color(theme::text()),
                    )
                    .child(
                        div()
                            .text_size(px(13.5))
                            .font_weight(FontWeight::MEDIUM)
                            .child("Agent"),
                    )
                    .child(
                        div()
                            .px(px(6.))
                            .rounded(px(5.))
                            .bg(theme::element_hover())
                            .text_size(px(11.))
                            .line_height(px(18.))
                            .text_color(theme::text_muted())
                            .child("Demo"),
                    )
                    .child(div().flex_1())
                    .child(status),
            )
            .child(
                AnyView::from(self.chat.clone())
                    .cached(StyleRefinement::default().flex_1().w_full().min_h_0()),
            )
            .into_any_element()
    }

    fn popover(&self, node: Node, now: u64) -> AnyElement {
        let facts = node.facts();
        let tools = node.tools();
        let narrow = self.geometry.narrow;
        let w = if narrow { self.geometry.chat.w } else { 300. };
        let h = 118.
            + 22. * facts.len() as f32
            + if tools.is_empty() { 0. } else { 34. }
            + if narrow { 0. } else { 20. };
        let r = self.geometry.popover(node, w, h);
        let color = accent(node.accent());
        let (line, _) = self.graph.lines[node.index()];
        let mut facts_el = div().flex().flex_col().gap(px(2.));
        for (label, value) in facts {
            facts_el = facts_el.child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(8.))
                    .child(
                        div()
                            .w(px(110.))
                            .flex_none()
                            .text_color(theme::text_faint())
                            .child(*label),
                    )
                    .child(div().child(*value)),
            );
        }
        let now_line = if node == Node::Wallet {
            format!("{} sats. {line}", graph::sats(self.graph.balance(now)))
        } else {
            line.to_owned()
        };
        div()
            .absolute()
            .left(px(r.x))
            .top(px(r.y))
            .w(px(r.w))
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(14.))
            .rounded(px(12.))
            .bg(gpui::hsla(0., 0., 0.06, 0.98))
            .border_1()
            .border_color(color.opacity(0.6))
            .text_size(px(12.5))
            .line_height(px(18.))
            .text_color(theme::text())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .child(div().w(px(3.)).h(px(16.)).rounded(px(2.)).bg(color))
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(node.title()),
                    ),
            )
            .child(div().text_color(theme::text_muted()).child(node.about()))
            .child(facts_el)
            .when(!tools.is_empty(), |el| {
                el.child(div().flex().flex_row().flex_wrap().gap(px(6.)).children(
                    tools.iter().map(|tool| {
                        div()
                            .px(px(6.))
                            .rounded(px(5.))
                            .bg(theme::element_hover())
                            .font_family(theme::MONO)
                            .text_size(px(11.))
                            .child(*tool)
                    }),
                ))
            })
            .child(
                div()
                    .font_family(theme::MONO)
                    .text_size(px(11.5))
                    .text_color(color)
                    .child(SharedString::from(now_line)),
            )
            .into_any_element()
    }

    fn composer(&self) -> AnyElement {
        let r = self.geometry.composer;
        div()
            .absolute()
            .left(px(r.x))
            .top(px(r.y))
            .w(px(r.w))
            .h(px(r.h))
            .flex()
            .flex_row()
            .items_center()
            .px(px(20.))
            .rounded(px(r.h / 2.))
            .border_1()
            .border_color(theme::border_strong())
            .bg(gpui::hsla(0., 0., 0.055, 0.94))
            .key_context("GraphCardComposer")
            .on_action(|_: &Hold, _, _| {})
            .child(self.composer.clone())
            .into_any_element()
    }
}

/// A link on the page: its pieces and how lit it is.
struct Drawn {
    pieces: Vec<crate::scene::Cubic>,
    color: Hsla,
    lit: f32,
}

struct Dot {
    at: (f32, f32),
    r: f32,
    color: Hsla,
}

impl Render for GraphCard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        let (w, h) = (f(viewport.width), f(viewport.height));
        if (w, h) != self.size {
            self.geometry = Geometry::new(w, h);
            self.size = (w, h);
        }
        let now = if self.still { 0 } else { self.now() };
        let g = &self.geometry;
        let lit = self.graph.lit(now);
        let mut drawn: Vec<Drawn> = Vec::new();
        for (i, path) in g.links.iter().enumerate() {
            let (a, _) = crate::scene::LINKS[i];
            let node = match crate::scene::LINKS[i] {
                (_, crate::scene::End::Node(n)) => n,
                _ => Node::GitHub,
            };
            let mut color = accent(node.accent());
            if a == crate::scene::End::Node(Node::Wallet) {
                color = sats_color();
            }
            drawn.push(Drawn {
                pieces: path.pieces.clone(),
                color,
                lit: lit[i],
            });
        }
        // Lit links last, so they draw over the rest.
        drawn.sort_by(|a, b| a.lit.total_cmp(&b.lit));
        let mut dots = Vec::new();
        let mut labels = Vec::new();
        for (link, t, sats) in self.graph.moving(now) {
            let at = g.links[link].at(t);
            let (_, end) = crate::scene::LINKS[link];
            let color = match (sats, end) {
                (Some(_), _) => sats_color(),
                (None, crate::scene::End::Node(n)) => accent(n.accent()),
                _ => theme::text(),
            };
            dots.push(Dot {
                at,
                r: if sats.is_some() { 5.5 } else { 4. },
                color,
            });
            if let Some(amount) = sats {
                labels.push(
                    div()
                        .absolute()
                        .left(px(at.0 + 10.))
                        .top(px(at.1 - 22.))
                        .px(px(6.))
                        .rounded(px(5.))
                        .bg(gpui::hsla(0., 0., 0.05, 0.92))
                        .border_1()
                        .border_color(sats_color().opacity(0.6))
                        .font_family(theme::MONO)
                        .text_size(px(11.))
                        .line_height(px(17.))
                        .text_color(sats_color())
                        .child(format!("{amount} sats"))
                        .into_any_element(),
                );
            }
        }
        let mut pins = Vec::new();
        for (i, path) in g.links.iter().enumerate() {
            let on = lit[i] > 0.0;
            for p in [path.start(), path.end()] {
                pins.push((p, drawn_color(i), on));
            }
        }
        let links = canvas(
            move |_, _, _| (drawn, dots),
            move |bounds: Bounds<Pixels>, (drawn, dots): (Vec<Drawn>, Vec<Dot>), window, _| {
                let o = (f(bounds.origin.x), f(bounds.origin.y));
                let at = |p: (f32, f32)| point(px(o.0 + p.0), px(o.1 + p.1));
                for d in &drawn {
                    let width = 1.4 + 1.2 * d.lit;
                    let color = if d.lit > 0.0 {
                        let rest = gpui::hsla(0., 0., 1., 0.13);
                        mix(rest, d.color, d.lit)
                    } else {
                        gpui::hsla(0., 0., 1., 0.13)
                    };
                    let mut path = PathBuilder::stroke(px(width));
                    if let Some(first) = d.pieces.first() {
                        path.move_to(at(first.a));
                    }
                    for piece in &d.pieces {
                        path.cubic_bezier_to(at(piece.b), at(piece.c1), at(piece.c2));
                    }
                    if let Ok(path) = path.build() {
                        window.paint_path(path, color);
                    }
                }
                for d in &dots {
                    // A soft halo, then the dot.
                    let halo = d.r * 2.4;
                    window.paint_quad(
                        fill(
                            Bounds::new(
                                at((d.at.0 - halo, d.at.1 - halo)),
                                size(px(2. * halo), px(2. * halo)),
                            ),
                            d.color.opacity(0.18),
                        )
                        .corner_radii(px(halo)),
                    );
                    window.paint_quad(
                        fill(
                            Bounds::new(
                                at((d.at.0 - d.r, d.at.1 - d.r)),
                                size(px(2. * d.r), px(2. * d.r)),
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
            move |bounds: Bounds<Pixels>, pins: Vec<((f32, f32), Hsla, bool)>, window, _| {
                let o = (f(bounds.origin.x), f(bounds.origin.y));
                let r = 4.5;
                for (p, color, on) in &pins {
                    window.paint_quad(
                        fill(
                            Bounds::new(
                                point(px(o.0 + p.0 - r), px(o.1 + p.1 - r)),
                                size(px(2. * r), px(2. * r)),
                            ),
                            if *on { *color } else { rgb(0x0b0b0b).into() },
                        )
                        .corner_radii(px(r))
                        .border_widths(px(1.5))
                        .border_color(if *on {
                            *color
                        } else {
                            gpui::hsla(0., 0., 1., 0.3)
                        }),
                    );
                }
            },
        )
        .absolute()
        .size_full();

        let units: Vec<AnyElement> = Node::ALL.iter().map(|n| self.unit(*n, now)).collect();
        let shown = self.selected.or(self.hovered);
        let narrow = g.narrow;
        let note = (!narrow).then(|| {
            let c = g.composer;
            div()
                .absolute()
                .left(px(0.))
                .w(px(w))
                .top(px(c.bottom() + 6.))
                .flex()
                .justify_center()
                .text_size(px(11.5))
                .text_color(theme::text_faint())
                .child(DEMO_NOTE)
        });
        let mut root = div()
            .id("graph-card")
            .size_full()
            .relative()
            .font_family(theme::SANS)
            .text_color(theme::text())
            .text_size(px(theme::BODY))
            .on_mouse_move(cx.listener(Self::on_move))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_down))
            .child(links)
            .child(self.chat_card(cx))
            .children(units)
            .child(pins)
            .children(labels)
            .children(shown.map(|n| self.popover(n, now)))
            .child(self.composer())
            .children(note);
        root.text_style().font_fallbacks = Some(theme::fallbacks());
        if self.hovered.is_some() {
            root = root.cursor_pointer();
        }
        root
    }
}

/// A link's own color, for its pins.
fn drawn_color(link: usize) -> Hsla {
    match crate::scene::LINKS[link] {
        (crate::scene::End::Node(Node::Wallet), _) => sats_color(),
        (_, crate::scene::End::Node(n)) => accent(n.accent()),
        _ => theme::text(),
    }
}

fn mix(a: Hsla, b: Hsla, t: f32) -> Hsla {
    let a = a.to_rgb();
    let b = b.to_rgb();
    gpui::Rgba {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
    .into()
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_page_s_words_are_plain() {
        for text in [super::PLACEHOLDER, super::DEMO_NOTE] {
            assert!(oa_copy::violations(text, &[]).is_empty(), "{text}");
        }
    }
}
