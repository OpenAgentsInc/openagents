//! The chat beside the live route map on Episode 289's second slide
//! (`scene: routes`): a narrow column plays a scripted conversation, and
//! each message's way through the map lights up, from the router down
//! the route it takes to what serves it.
//!
//! The script names each message's route by the router's own id and the
//! node that serves it by the map's id; the way between them is the map's
//! own tree (the target's parents up to the router), so the light lands on
//! real things. Nothing here reads the message text to decide anything.
//!
//! Each exchange takes [`EXCHANGE`] seconds: the person's message appears,
//! a pulse runs from the router to the target, the target glows and the
//! reply appears, then the light fades. After the last exchange the column
//! holds for [`HOLD`] seconds and the conversation replays. Under "Reduce
//! motion" the whole conversation shows at once with the last way lit.

use std::time::Instant;

use openagents_chat_app::route_map::Map;
use openagents_chat_app::visual;
use rust_native::style::{Color, TextAlign};
use rust_native_desktop::text::{Fonts, font};
use rust_native_desktop::{Frame, PxRect};

use crate::route_map::RouteLight;

/// The column's share of the slide's width.
pub const COLUMN: f32 = 0.25;
/// One exchange, in seconds.
pub const EXCHANGE: f32 = 4.6;
/// How long the finished conversation holds before it replays, in seconds.
pub const HOLD: f32 = 4.0;

/// When, into an exchange, each part shows, in seconds.
const ASK_IN: f32 = 0.35;
const RUN_FROM: f32 = 0.45;
const RUN_FOR: f32 = 1.45;
const GLOW_FOR: f32 = 0.3;
const REPLY_AT: f32 = 2.0;
const REPLY_IN: f32 = 0.3;
const FADE_FROM: f32 = 3.8;

/// One scripted exchange.
#[derive(Clone, Copy, Debug)]
pub struct Exchange {
    /// What the person asks.
    pub ask: &'static str,
    /// The router's route id it takes.
    pub route: &'static str,
    /// The map's id of what serves it.
    pub serves: &'static str,
    /// The reply.
    pub reply: &'static str,
}

/// The conversation.
pub const SCRIPT: [Exchange; 8] = [
    Exchange {
        ask: "who are you?",
        route: "meta",
        serves: "answer:meta.who",
        reply: "I'm OpenAgents. I read each message and send it where it's best served.",
    },
    Exchange {
        ask: "what's in your essays?",
        route: "product.kb",
        serves: "knowledge:product",
        reply: "Two so far: The Return of the General Agent, and Test-Time Capabilities.",
    },
    Exchange {
        ask: "fix the failing test in my repo",
        route: "work.dispatch",
        serves: "engine:codex",
        reply: "Sending it to Coder on your computer, with Codex.",
    },
    Exchange {
        ask: "do it with Claude",
        route: "work.dispatch",
        serves: "engine:claude_code",
        reply: "Switched to Claude Code.",
    },
    Exchange {
        ask: "map this repo",
        route: "work.dispatch",
        serves: "plugin:crates/plugin-repo-map",
        reply: "Coder is drawing a project map.",
    },
    Exchange {
        ask: "what's new in the Gym?",
        route: "gym.news",
        serves: "answer:gym.news.lead",
        reply: "Here's what's new in the Gym this week.",
    },
    Exchange {
        ask: "send 1000 sats",
        route: "wallet",
        serves: "screen:wallet",
        reply: "Opening your wallet.",
    },
    Exchange {
        ask: "show the test-time capabilities deck",
        route: "presentation.open",
        serves: "deck:test-time-capabilities",
        reply: "Opening the deck.",
    },
];

/// One full play: every exchange, then the hold.
pub fn cycle() -> f32 {
    SCRIPT.len() as f32 * EXCHANGE + HOLD
}

/// The column on the left of `slide` and the map's place to its right.
pub fn split(slide: PxRect) -> (PxRect, PxRect) {
    let width = slide.w * COLUMN;
    (
        PxRect { w: width, ..slide },
        PxRect {
            x: slide.x + width,
            w: slide.w - width,
            ..slide
        },
    )
}

/// The way a message takes through `map`: the router, then each node
/// down to what serves it. `None` if the map has no such node or the way
/// doesn't pass the named route.
pub fn path(map: &Map, exchange: &Exchange) -> Option<Vec<usize>> {
    let target = map.find(exchange.serves)?;
    let mut path = vec![target];
    let mut at = map.nodes[target].parent;
    while let Some(parent) = at {
        path.push(parent);
        at = map.nodes[parent].parent;
    }
    path.reverse();
    let route = map.find(&format!("route:{}", exchange.route))?;
    path.contains(&route).then_some(path)
}

/// The scripted chat and its clock.
pub struct RouteChat {
    started: Option<Instant>,
    seconds: f32,
    reduce_motion: bool,
    fonts: Fonts,
}

/// Where the play is: the exchange showing and how far into it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Moment {
    pub exchange: usize,
    pub into: f32,
}

impl RouteChat {
    pub fn new(reduce_motion: bool) -> Self {
        RouteChat {
            started: None,
            seconds: 0.0,
            reduce_motion,
            fonts: Fonts::new(),
        }
    }

    /// Off its slide: the next visit plays from the start.
    pub fn reset(&mut self) {
        self.started = None;
        self.seconds = 0.0;
    }

    /// Moves the clock to `now`, the frame clock's time.
    pub fn advance(&mut self, now: Instant) {
        let started = *self.started.get_or_insert(now);
        self.seconds = now.saturating_duration_since(started).as_secs_f32();
    }

    /// Sets the clock directly, in seconds since the slide opened.
    pub fn set_seconds(&mut self, seconds: f32) {
        self.seconds = seconds.max(0.0);
    }

    /// Whether the chat moves (it holds still under "Reduce motion").
    pub fn playing(&self) -> bool {
        !self.reduce_motion
    }

    /// Where the play is now.
    pub fn moment(&self) -> Moment {
        if self.reduce_motion {
            return Moment {
                exchange: SCRIPT.len() - 1,
                into: REPLY_AT + REPLY_IN,
            };
        }
        let t = self.seconds % cycle();
        let exchange = ((t / EXCHANGE) as usize).min(SCRIPT.len() - 1);
        Moment {
            exchange,
            into: t - exchange as f32 * EXCHANGE,
        }
    }

    /// The way lit through `map` now, if any.
    pub fn light(&self, map: &Map) -> Option<RouteLight> {
        let Moment { exchange, into } = self.moment();
        let path = path(map, &SCRIPT[exchange])?;
        if self.reduce_motion {
            return Some(RouteLight {
                path,
                head: 1.0,
                glow: 1.0,
                fade: 1.0,
            });
        }
        if into < RUN_FROM || into >= EXCHANGE {
            return None;
        }
        let head = smooth((into - RUN_FROM) / RUN_FOR);
        let glow = smooth((into - RUN_FROM - RUN_FOR) / GLOW_FOR);
        let fade = 1.0 - smooth((into - FADE_FROM) / (EXCHANGE - FADE_FROM));
        if fade <= 0.0 {
            return None;
        }
        Some(RouteLight {
            path,
            head,
            glow,
            fade,
        })
    }

    /// The messages showing now, oldest first, each with how far it has
    /// appeared: (from the person, text, 0 to 1).
    pub fn messages(&self) -> Vec<(bool, &'static str, f32)> {
        let Moment { exchange, into } = self.moment();
        let mut shown = Vec::new();
        for (index, line) in SCRIPT.iter().enumerate().take(exchange + 1) {
            let (ask, reply) = if index < exchange {
                (1.0, 1.0)
            } else {
                (smooth(into / ASK_IN), smooth((into - REPLY_AT) / REPLY_IN))
            };
            if ask > 0.0 {
                shown.push((true, line.ask, ask));
            }
            if reply > 0.0 {
                shown.push((false, line.reply, reply));
            }
        }
        shown
    }

    /// Paints the column into `rect`, in pixels: the newest message at the
    /// bottom, older ones rising and dropping off the top.
    pub fn paint(&mut self, frame: &mut Frame, rect: PxRect, unit: f32) {
        use rust_native::layout::display::Weight;
        frame.fill(rect, 0.0, visual::SIDEBAR);
        frame.fill(
            PxRect {
                x: rect.x + rect.w - unit,
                w: unit,
                ..rect
            },
            0.0,
            visual::BORDER,
        );
        let clip = frame.clip_to(rect);
        let pad = 16.0 * unit;
        let inner = rect.w - 2.0 * pad;
        let bubble_pad = (11.0 * unit, 8.0 * unit);
        let most = inner * 0.86 - 2.0 * bubble_pad.0;
        let gap = 10.0 * unit;
        let mut bottom = rect.y + rect.h - pad;
        for (person, text, shown) in self.messages().into_iter().rev() {
            let paragraph =
                self.fonts
                    .paragraph(text, font(13.5 * unit, Weight::Regular, false), Some(most));
            let (w, h) = if person {
                (
                    paragraph.width + 2.0 * bubble_pad.0,
                    paragraph.height + 2.0 * bubble_pad.1,
                )
            } else {
                (paragraph.width, paragraph.height)
            };
            // A new message rises a little as it appears.
            let rise = (1.0 - shown) * 8.0 * unit;
            let top = bottom - h + rise;
            if bottom < rect.y {
                break;
            }
            let x = if person {
                rect.x + rect.w - pad - w
            } else {
                rect.x + pad
            };
            let opacity = shown.clamp(0.0, 1.0);
            let ink = |color: Color| Color {
                alpha: (f32::from(color.alpha) * opacity).round() as u8,
                ..color
            };
            if person {
                frame.fill(
                    PxRect { x, y: top, w, h },
                    12.0 * unit,
                    ink(Color {
                        alpha: 30,
                        ..visual::TEXT
                    }),
                );
                self.fonts.draw(
                    frame,
                    &paragraph,
                    x + bubble_pad.0,
                    top + bubble_pad.1,
                    paragraph.width + 1.0,
                    TextAlign::Start,
                    1.0,
                    ink(visual::TEXT),
                );
            } else {
                self.fonts.draw(
                    frame,
                    &paragraph,
                    x,
                    top,
                    most,
                    TextAlign::Start,
                    1.0,
                    ink(visual::MUTED),
                );
            }
            bottom = top - gap * if person { 1.6 } else { 1.0 };
        }
        frame.restore_clip(clip);
    }
}

/// Smoothstep on 0 to 1, clamped.
fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_lands_on_a_real_node_through_its_named_route() {
        let map = crate::route_map::build(Default::default());
        for exchange in &SCRIPT {
            let path = path(&map, exchange)
                .unwrap_or_else(|| panic!("{} has no way through the map", exchange.serves));
            assert_eq!(map.nodes[path[0]].id, "front");
            assert!(path.len() >= 3);
        }
    }

    #[test]
    fn an_exchange_asks_runs_glows_replies_and_fades() {
        let map = crate::route_map::build(Default::default());
        let mut chat = RouteChat::new(false);
        chat.set_seconds(0.2);
        assert!(chat.light(&map).is_none());
        assert_eq!(chat.messages().len(), 1);
        chat.set_seconds(1.0);
        let light = chat.light(&map).expect("lit mid run");
        assert!(light.head > 0.0 && light.head < 1.0);
        assert_eq!(light.glow, 0.0);
        chat.set_seconds(2.6);
        let light = chat.light(&map).unwrap();
        assert_eq!((light.head, light.glow, light.fade), (1.0, 1.0, 1.0));
        assert_eq!(chat.messages().len(), 2);
        chat.set_seconds(EXCHANGE + 0.1);
        assert!(chat.light(&map).is_none());
        assert_eq!(chat.moment().exchange, 1);
        // It replays after the hold.
        chat.set_seconds(cycle() + 0.1);
        assert_eq!(chat.moment().exchange, 0);
        assert_eq!(chat.messages().len(), 1);
    }

    #[test]
    fn reduce_motion_holds_the_whole_conversation_lit() {
        let map = crate::route_map::build(Default::default());
        let chat = RouteChat::new(true);
        assert_eq!(chat.messages().len(), 2 * SCRIPT.len());
        assert_eq!(chat.light(&map).unwrap().head, 1.0);
    }
}
