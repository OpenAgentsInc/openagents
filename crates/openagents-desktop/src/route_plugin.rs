//! A person makes a plugin, on the live route map: the Episode 289 deck's
//! third slide (`scene: routes-plugin`). The chat column from the slide
//! before ([`crate::route_chat`]) stays on the left and the conversation
//! goes on:
//!
//! 1. The person asks for something no plugin serves yet. The way runs
//!    from the router down `capability.missing` and the route shows as a
//!    gap: lit in the gap's red, dimmed, with dashed rings.
//! 2. We offer to build it as a plugin, and the authoring route
//!    (`eval.author`, the Gym's interview) lights as the interview asks
//!    what the plugin is for and then drafts its tests
//!    (`ext_eval::author::Stage`: tool, quality, tests).
//! 3. The person approves and the tests run (`eval.run`): the new plugin
//!    grows onto the map under Coder, linked to the route that tests it,
//!    and the reply reads as the Gym's result does ("passed 6 of 6 with
//!    it, 2 of 6 without", Better).
//! 4. XP, as NIP-XP's rules give it: other trainers' checks of the result
//!    (`eval-check`, 25 to the original evaluator), then Coder adopting
//!    the plugin for everyone (`eval-adopt`, 200 to its author). A small
//!    counter at the top of the column counts it up.
//! 5. Others use it: faint bubbles from other people, each followed by a
//!    request running from the router to the plugin, again and again, a
//!    uses counter ticking up, and gold payment dots running back, the
//!    same dots as the future slide's.
//!
//! Then it holds on that last state with the traffic still flowing. The
//! script names each step's route by the router's own id and what serves
//! it by the map's id; the plugin node is the one thing made up. Nothing
//! here reads a message's text to decide anything. The play starts over
//! each time the slide opens, and under "Reduce motion" it shows the last
//! state, still.

use std::time::Instant;

use openagents_chat_app::route_map::layout::{Camera, Layout, Point};
use openagents_chat_app::route_map::{Edge, EdgeKind, Health, Kind, Map, Node, Stage};
use openagents_chat_app::visual;
use rust_native::layout::display::Weight;
use rust_native::style::{Color, TextAlign};
use rust_native_desktop::text::{Fonts, font};
use rust_native_desktop::{Frame, PxRect};

use crate::route_chat::{self, Line, Who, smooth};
use crate::route_future::{payment, request};
use crate::route_map::{MapPage, Pulse, RouteLight};

/// The map's id of the plugin the person makes, the one node not in the
/// committed map.
pub const PLUGIN: &str = "plugin:notes-to-tickets";
/// Its name.
pub const PLUGIN_NAME: &str = "Notes to tickets";

/// When, into a step, each part shows, in seconds (as on the slide
/// before).
const ASK_IN: f32 = 0.35;
const RUN_FROM: f32 = 0.45;
const RUN_FOR: f32 = 1.45;
const GLOW_FOR: f32 = 0.3;
const REPLY_AT: f32 = 2.0;
const REPLY_IN: f32 = 0.3;
/// How long before a step's end its light starts to fade.
const FADE_FOR: f32 = 0.8;
/// When, into its step, the plugin grows in, and for how long.
const GROW_FROM: f32 = 0.3;
const GROW_FOR: f32 = 1.3;
/// How long the XP counter takes to count up an award, and how long the
/// award's "+N XP" shows.
const COUNT_FOR: f32 = 0.8;
const TOAST_FOR: f32 = 2.2;
/// How often another person's message arrives once others use it.
pub const OTHER_EVERY: f32 = 1.2;
/// How bright the way to the plugin stays while others use it.
const IN_USE: f32 = 0.55;
/// The traffic once others use it: one stream per other person's
/// message, then more streams as it spreads.
const STREAMS: usize = 11;
/// The share of a stream's cycle its request takes to arrive.
const OUT: f32 = 0.45;
/// When the payment for a request leaves the plugin.
const BACK: f32 = 0.52;

/// One step of the story.
#[derive(Clone, Copy, Debug)]
pub struct Step {
    /// What the person says, if they say anything: some steps are only
    /// our news.
    pub ask: Option<&'static str>,
    /// The router's route id the step takes.
    pub route: &'static str,
    /// The map's id of what serves it.
    pub serves: &'static str,
    /// The reply.
    pub reply: &'static str,
    /// Nothing serves it yet: it lights as a gap.
    pub missing: bool,
    /// The plugin grows onto the map in this step.
    pub grows: bool,
    /// The XP the person earns as the reply lands.
    pub xp: u32,
    /// How long the step takes, in seconds.
    pub length: f32,
}

/// The story.
pub const STORY: [Step; 6] = [
    Step {
        ask: Some("turn my meeting notes into Linear tickets"),
        route: "capability.missing",
        serves: "route:capability.missing",
        reply: "We can't do that yet. Want to build it as a plugin?",
        missing: true,
        grows: false,
        xp: 0,
        length: 5.0,
    },
    Step {
        ask: Some("yes"),
        route: "eval.author",
        serves: "route:eval.author",
        reply: "What should it do, and what shouldn't it?",
        missing: false,
        grows: false,
        xp: 0,
        length: 4.2,
    },
    Step {
        ask: Some("one ticket per action item, nothing made up"),
        route: "eval.author",
        serves: "route:eval.author",
        reply: "Six tests drafted, to run with it and without. Look good?",
        missing: false,
        grows: false,
        xp: 0,
        length: 4.4,
    },
    Step {
        ask: Some("looks good, run it"),
        route: "eval.run",
        serves: PLUGIN,
        reply: "Passed 6 of 6 with it, 2 of 6 without. Better.",
        missing: false,
        grows: true,
        xp: 0,
        length: 5.4,
    },
    Step {
        ask: None,
        route: "eval.check",
        serves: "route:eval.check",
        reply: "Three trainers ran your tests again and confirmed it.",
        missing: false,
        grows: false,
        xp: 25,
        length: 4.4,
    },
    Step {
        ask: None,
        route: "work.dispatch",
        serves: PLUGIN,
        reply: "Coder now uses Notes to tickets for everyone.",
        missing: false,
        grows: false,
        xp: 200,
        length: 4.6,
    },
];

/// The way other people's requests take to the plugin.
const IN_USE_ROUTE: &str = "work.dispatch";

/// Other people's messages once Coder uses the plugin for everyone, each
/// with its avatar.
pub const OTHERS: [(&str, u8); 5] = [
    ("tickets from today's standup", 0),
    ("file the action items in these notes", 1),
    ("make issues from our retro", 2),
    ("turn the sync notes into tickets", 3),
    ("tickets for everything we agreed", 4),
];

/// When the story's steps are done and others start using the plugin.
pub fn others_from() -> f32 {
    STORY.iter().map(|step| step.length).sum()
}

/// When the last other person's message has arrived; it holds after,
/// the traffic still flowing.
pub fn end() -> f32 {
    others_from() + OTHERS.len() as f32 * OTHER_EVERY
}

/// The time a still picture shows: the last state, with traffic on the
/// way and the counters up.
fn still() -> f32 {
    end() + 6.0
}

/// When each step starts.
fn starts() -> [f32; STORY.len()] {
    let mut at = 0.0;
    let mut starts = [0.0; STORY.len()];
    for (index, step) in STORY.iter().enumerate() {
        starts[index] = at;
        at += step.length;
    }
    starts
}

/// The live route map with the person's plugin, playing the story.
pub struct RoutePlugin {
    page: MapPage,
    /// Where every node sits before the plugin and after it, by index;
    /// the plugin, last, starts at Coder.
    before: Layout,
    after: Layout,
    /// The plugin's index.
    plugin: usize,
    started: Option<Instant>,
    seconds: f32,
    reduce_motion: bool,
    fonts: Fonts,
}

impl RoutePlugin {
    /// The story over `today`, the live map the slide before showed.
    /// With `reduce_motion`, it holds still on the last state.
    pub fn new(today: Map, reduce_motion: bool) -> Self {
        let mut before = Layout::of(&today);
        let (map, plugin) = with_plugin(today);
        let after = Layout::of(&map);
        let parent = map.nodes[plugin].parent.unwrap_or(0);
        before.positions.push(before.positions[parent]);
        before.radii.push(after.radii[plugin]);
        before.spans.push(after.spans[plugin]);
        let mut story = RoutePlugin {
            page: MapPage::presenting(map, reduce_motion),
            before,
            after,
            plugin,
            started: None,
            seconds: 0.0,
            reduce_motion,
            fonts: Fonts::new(),
        };
        story.seconds = story.rest();
        story
    }

    /// Where the clock rests: the start, or the still picture.
    fn rest(&self) -> f32 {
        if self.reduce_motion { still() } else { 0.0 }
    }

    /// The map with the plugin in it.
    pub fn map(&self) -> &Map {
        self.page.map()
    }

    /// The plugin's index in [`RoutePlugin::map`].
    pub fn plugin(&self) -> usize {
        self.plugin
    }

    /// Seconds since the slide opened.
    pub fn seconds(&self) -> f32 {
        self.seconds
    }

    /// Whether it moves (it holds still under "Reduce motion").
    pub fn playing(&self) -> bool {
        !self.reduce_motion
    }

    /// Off its slide: the next visit plays from the start.
    pub fn reset(&mut self) {
        self.started = None;
        self.seconds = self.rest();
    }

    /// Moves the clock to `now`; the play starts the first time.
    pub fn advance(&mut self, now: Instant) {
        if self.reduce_motion {
            return;
        }
        let started = *self.started.get_or_insert(now);
        self.seconds = now.saturating_duration_since(started).as_secs_f32();
    }

    /// Sets the clock directly, in seconds since the slide opened.
    pub fn set_seconds(&mut self, seconds: f32) {
        self.seconds = seconds.max(0.0);
    }

    pub fn version(&self) -> u64 {
        self.page.version()
    }

    /// The step showing `seconds` after the slide opened, and how far
    /// into it; `None` once others use the plugin.
    fn step_at(seconds: f32) -> Option<(usize, f32)> {
        let starts = starts();
        (seconds < others_from()).then(|| {
            let index = starts
                .iter()
                .rposition(|&start| start <= seconds)
                .unwrap_or(0);
            (index, seconds - starts[index])
        })
    }

    /// How far the plugin has grown in, 0 to 1.
    pub fn grown(&self) -> f32 {
        let starts = starts();
        let Some(index) = STORY.iter().position(|step| step.grows) else {
            return 1.0;
        };
        smooth((self.seconds - starts[index] - GROW_FROM) / GROW_FOR)
    }

    /// The way lit through the map now, if any.
    pub fn light(&self) -> Option<RouteLight> {
        let map = self.page.map();
        let Some((index, into)) = Self::step_at(self.seconds) else {
            // Others use it: the way to the plugin stays softly lit.
            let path = route_chat::way(map, IN_USE_ROUTE, PLUGIN)?;
            return Some(RouteLight {
                path,
                head: 1.0,
                glow: 0.7,
                fade: IN_USE,
                missing: false,
            });
        };
        let step = &STORY[index];
        let path = route_chat::way(map, step.route, step.serves)?;
        if into < RUN_FROM {
            return None;
        }
        let head = smooth((into - RUN_FROM) / RUN_FOR);
        let glow = smooth((into - RUN_FROM - RUN_FOR) / GLOW_FOR);
        // The last step's way stays softly lit into the others' use.
        let floor = if index + 1 == STORY.len() {
            IN_USE
        } else {
            0.0
        };
        let fading = smooth((into - (step.length - FADE_FOR)) / FADE_FOR);
        let fade = 1.0 - (1.0 - floor) * fading;
        (fade > 0.0).then_some(RouteLight {
            path,
            head,
            glow,
            fade,
            missing: step.missing,
        })
    }

    /// The column's lines now, oldest first: the slide before's whole
    /// conversation, the story so far, and others' messages.
    pub fn lines(&self) -> Vec<Line> {
        let mut lines = Vec::new();
        for exchange in &route_chat::SCRIPT {
            lines.push(Line {
                who: Who::Person,
                text: exchange.ask,
                shown: 1.0,
            });
            lines.push(Line {
                who: Who::Us,
                text: exchange.reply,
                shown: 1.0,
            });
        }
        let starts = starts();
        for (index, step) in STORY.iter().enumerate() {
            let into = self.seconds - starts[index];
            if into < 0.0 {
                break;
            }
            if let Some(ask) = step.ask {
                let shown = smooth(into / ASK_IN);
                if shown > 0.0 {
                    lines.push(Line {
                        who: Who::Person,
                        text: ask,
                        shown,
                    });
                }
            }
            let shown = smooth((into - REPLY_AT) / REPLY_IN);
            if shown > 0.0 {
                lines.push(Line {
                    who: Who::Us,
                    text: step.reply,
                    shown,
                });
            }
        }
        for (k, (text, avatar)) in OTHERS.iter().enumerate() {
            let shown = smooth((self.seconds - other_at(k)) / ASK_IN);
            if shown > 0.0 {
                lines.push(Line {
                    who: Who::Other(*avatar),
                    text,
                    shown,
                });
            }
        }
        lines
    }

    /// The person's XP now, counting up as each award lands.
    pub fn xp(&self) -> f32 {
        awards()
            .map(|(at, xp)| xp as f32 * smooth((self.seconds - at) / COUNT_FOR))
            .sum()
    }

    /// The award showing as a "+N XP" toast now, and how far through its
    /// showing it is, 0 to 1.
    fn toast(&self) -> Option<(u32, f32)> {
        awards().find_map(|(at, xp)| {
            let t = (self.seconds - at) / TOAST_FOR;
            (0.0..1.0).contains(&t).then_some((xp, t))
        })
    }

    /// How many requests others have sent the plugin so far.
    pub fn uses(&self) -> u32 {
        (0..STREAMS)
            .map(|stream| {
                let (start, period) = stream_clock(stream);
                let first = start + OUT * period;
                if self.seconds < first {
                    0
                } else {
                    ((self.seconds - first) / period) as u32 + 1
                }
            })
            .sum()
    }

    /// The traffic now: requests from the router to the plugin, and gold
    /// payments back.
    pub fn traffic(&self, layout: &Layout) -> Vec<Pulse> {
        let Some(path) = route_chat::way(self.page.map(), IN_USE_ROUTE, PLUGIN) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for stream in 0..STREAMS {
            let (start, period) = stream_clock(stream);
            if self.seconds < start {
                continue;
            }
            let p = ((self.seconds - start) / period).fract();
            if p < OUT {
                out.push(Pulse {
                    at: along(layout, &path, p / OUT),
                    color: request(),
                    radius: 2.6,
                    ring: false,
                });
            } else if p > BACK {
                out.push(Pulse {
                    at: along(layout, &path, 1.0 - (p - BACK) / (1.0 - BACK)),
                    color: payment(),
                    radius: 2.8,
                    ring: false,
                });
            }
        }
        out
    }

    /// Where every node sits now, and how far each has appeared.
    pub fn frame(&self) -> (Layout, Vec<f32>) {
        let grown = self.grown();
        let mut layout = self.after.clone();
        for (index, at) in layout.positions.iter_mut().enumerate() {
            *at = lerp(
                self.before.positions[index],
                self.after.positions[index],
                grown,
            );
        }
        let mut shown = vec![1.0; layout.positions.len()];
        shown[self.plugin] = grown;
        (layout, shown)
    }

    /// Paints the column into `column` and the map into `map`, in pixels,
    /// with `unit` pixels a point.
    pub fn paint(&mut self, frame: &mut Frame, column: PxRect, map: PxRect, unit: f32) {
        let (layout, shown) = self.frame();
        let size = (map.w / unit.max(0.1), map.h / unit.max(0.1));
        let camera = Camera::fit(self.after.bounds(), size.0.max(1.0), size.1.max(1.0), 24.0);
        let traffic = self.traffic(&layout);
        let light = self.light();
        self.page.set_unit(unit);
        self.page.set_frame(layout, shown, traffic, camera);
        self.page.set_light(light);
        self.page.paint(frame, map);
        let lines = self.lines();
        let xp = self.xp();
        let uses = self.uses();
        let header = if xp > 0.0 || uses > 0 {
            40.0 * unit
        } else {
            0.0
        };
        let below = PxRect {
            y: column.y + header,
            h: column.h - header,
            ..column
        };
        route_chat::paint_column(&mut self.fonts, frame, below, unit, &lines);
        if header > 0.0 {
            self.paint_counters(
                frame,
                PxRect {
                    h: header,
                    ..column
                },
                unit,
                xp,
                uses,
            );
        }
    }

    /// The counters at the top of the column: the person's XP, with the
    /// latest award rising beside it, and the plugin's uses.
    fn paint_counters(&mut self, frame: &mut Frame, rect: PxRect, unit: f32, xp: f32, uses: u32) {
        frame.fill(rect, 0.0, visual::current().sidebar);
        frame.fill(
            PxRect {
                y: rect.y + rect.h - unit,
                h: unit,
                ..rect
            },
            0.0,
            visual::current().border,
        );
        frame.fill(
            PxRect {
                x: rect.x + rect.w - unit,
                w: unit,
                ..rect
            },
            0.0,
            visual::current().border,
        );
        let pad = 16.0 * unit;
        let size = 13.0 * unit;
        let line = size * rust_native_desktop::text::LINE_EM;
        let y = rect.y + (rect.h - line) / 2.0;
        let xp_text = format!("{} XP", xp.round() as u32);
        let paragraph = self
            .fonts
            .paragraph(&xp_text, font(size, Weight::Semibold, false), None);
        let xp_width = paragraph.width;
        if xp > 0.0 {
            self.fonts.draw(
                frame,
                &paragraph,
                rect.x + pad,
                y,
                xp_width + 2.0,
                TextAlign::Start,
                1.0,
                visual::current().text,
            );
        }
        if let Some((award, t)) = self.toast() {
            let text = format!("+{award} XP");
            let paragraph = self
                .fonts
                .paragraph(&text, font(size, Weight::Semibold, false), None);
            let opacity = smooth(t / 0.15) * (1.0 - smooth((t - 0.7) / 0.3));
            self.fonts.draw(
                frame,
                &paragraph,
                rect.x + pad + xp_width + 10.0 * unit,
                y - 6.0 * unit * t,
                paragraph.width + 2.0,
                TextAlign::Start,
                1.0,
                Color {
                    alpha: (255.0 * opacity) as u8,
                    ..payment()
                },
            );
        }
        if uses > 0 {
            let text = format!("{uses} uses");
            let paragraph = self
                .fonts
                .paragraph(&text, font(size, Weight::Medium, false), None);
            self.fonts.draw(
                frame,
                &paragraph,
                rect.x + rect.w - pad - paragraph.width,
                y,
                paragraph.width + 2.0,
                TextAlign::Start,
                1.0,
                visual::current().muted,
            );
        }
    }
}

/// When the `k`th other person's message arrives.
fn other_at(k: usize) -> f32 {
    others_from() + 0.3 + k as f32 * OTHER_EVERY
}

/// When a stream of others' requests starts, and its period: one with
/// each other person's message, then more as the plugin spreads.
fn stream_clock(stream: usize) -> (f32, f32) {
    let start = if stream < OTHERS.len() {
        other_at(stream) + ASK_IN
    } else {
        other_at(OTHERS.len() - 1) + 0.5 + (stream - OTHERS.len()) as f32 * 0.65
    };
    (start, 2.3 + 0.37 * (stream % 5) as f32)
}

/// The XP awards: when each lands, and how much.
fn awards() -> impl Iterator<Item = (f32, u32)> {
    let starts = starts();
    STORY
        .iter()
        .enumerate()
        .filter(|(_, step)| step.xp > 0)
        .map(move |(index, step)| (starts[index] + REPLY_AT, step.xp))
}

/// `today` with the person's plugin under Coder, linked to the Gym's test
/// route as the map links every plugin, and its index.
fn with_plugin(mut map: Map) -> (Map, usize) {
    let parent = map.find("coder").or_else(|| map.find("front")).unwrap_or(0);
    let index = map.nodes.len();
    let (depth, family) = (
        map.nodes[parent].depth + 1,
        map.nodes[parent].family.clone(),
    );
    map.nodes.push(Node {
        id: PLUGIN.to_string(),
        kind: Kind::Plugin,
        label: PLUGIN_NAME.to_string(),
        line: "Turns meeting notes into one ticket per action item.".to_string(),
        parent: Some(parent),
        depth,
        weight: 0.65,
        health: Health::Good,
        stage: Some(Stage::Adopted),
        family,
        gaps: Vec::new(),
        local: None,
        showcase: false,
    });
    map.edges.push(Edge {
        from: parent,
        to: index,
        kind: EdgeKind::Admits,
    });
    if let Some(run) = map.find("route:eval.run") {
        map.edges.push(Edge {
            from: run,
            to: index,
            kind: EdgeKind::Tests,
        });
    }
    (map, index)
}

/// The point `t` of the way along `path`, each hop taking the same time.
fn along(layout: &Layout, path: &[usize], t: f32) -> Point {
    if path.len() < 2 {
        return layout.positions[path[0]];
    }
    let hops = (path.len() - 1) as f32;
    let at = (t.clamp(0.0, 1.0) * hops).min(hops - 0.0001);
    let hop = at.floor() as usize;
    lerp(
        layout.positions[path[hop]],
        layout.positions[path[hop + 1]],
        at - hop as f32,
    )
}

fn lerp(a: Point, b: Point, t: f32) -> Point {
    Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn story() -> RoutePlugin {
        RoutePlugin::new(crate::route_map::build(Default::default()), false)
    }

    #[test]
    fn every_step_lands_on_a_real_node_through_its_named_route() {
        let story = story();
        let map = story.map();
        for step in &STORY {
            let path = route_chat::way(map, step.route, step.serves)
                .unwrap_or_else(|| panic!("{} has no way through the map", step.serves));
            assert_eq!(map.nodes[path[0]].id, "front");
            let route = map.find(&format!("route:{}", step.route)).unwrap();
            assert!(path.contains(&route), "{}", step.serves);
        }
        // Only the plugin is made up: under Coder, tested by the Gym's run
        // route, and the first step is the gap no plugin serves.
        assert_eq!(
            map.nodes.len(),
            crate::route_map::build(Default::default()).nodes.len() + 1
        );
        let plugin = &map.nodes[story.plugin()];
        assert_eq!(plugin.kind, Kind::Plugin);
        assert_eq!(map.nodes[plugin.parent.unwrap()].id, "coder");
        assert!(STORY[0].missing && STORY[0].route == "capability.missing");
        let run = route_chat::way(map, "eval.run", PLUGIN).unwrap();
        assert_eq!(map.nodes[run[run.len() - 2]].id, "route:eval.run");
        assert!(!map.nodes[story.plugin()].id.is_empty());
    }

    #[test]
    fn the_story_finds_the_gap_makes_the_plugin_earns_xp_and_others_use_it() {
        let mut story = story();
        let starts = starts();
        // The gap: the way down capability.missing lit as missing.
        story.set_seconds(1.2);
        let light = story.light().expect("lit");
        assert!(light.missing);
        let target = *light.path.last().unwrap();
        assert_eq!(story.map().nodes[target].id, "route:capability.missing");
        assert_eq!(story.grown(), 0.0);
        assert_eq!(story.frame().1[story.plugin()], 0.0);
        // The interview: the authoring route.
        story.set_seconds(starts[1] + 2.5);
        let light = story.light().unwrap();
        assert!(!light.missing);
        let target = *light.path.last().unwrap();
        assert_eq!(story.map().nodes[target].id, "route:eval.author");
        // The run: the plugin grows in and the test route lights to it.
        story.set_seconds(starts[3] + 1.0);
        let mid = story.grown();
        assert!(mid > 0.0 && mid < 1.0, "{mid}");
        story.set_seconds(starts[3] + 2.6);
        assert_eq!(story.grown(), 1.0);
        let light = story.light().unwrap();
        assert_eq!(*light.path.last().unwrap(), story.plugin());
        assert_eq!(story.xp(), 0.0);
        // XP: a check's award, then the adoption's.
        story.set_seconds(starts[4] + REPLY_AT + 1.0);
        assert_eq!(story.xp(), 25.0);
        story.set_seconds(starts[5] + REPLY_AT + 1.0);
        assert_eq!(story.xp(), 225.0);
        assert_eq!(story.uses(), 0);
        // Others use it: bubbles, requests, uses, and payments back.
        story.set_seconds(end() + 3.0);
        let others = story
            .lines()
            .iter()
            .filter(|line| matches!(line.who, Who::Other(_)))
            .count();
        assert_eq!(others, OTHERS.len());
        let uses = story.uses();
        assert!(uses >= OTHERS.len() as u32, "{uses}");
        let (layout, _) = story.frame();
        let traffic = story.traffic(&layout);
        assert!(traffic.iter().any(|p| p.color == payment()));
        assert!(traffic.iter().any(|p| p.color == request()));
        assert!(story.light().is_some(), "the way to it stays lit");
        // It holds there with the traffic still flowing.
        story.set_seconds(end() + 40.0);
        assert!(story.uses() > uses);
        let later = story.traffic(&story.frame().0);
        assert_ne!(traffic, later);
        assert_eq!(story.xp(), 225.0);
    }

    #[test]
    fn it_replays_and_reduce_motion_shows_the_last_state() {
        let mut story = story();
        let start = Instant::now();
        story.advance(start);
        story.advance(start + std::time::Duration::from_secs(50));
        assert!(story.seconds() > end());
        story.reset();
        story.advance(start + std::time::Duration::from_secs(60));
        assert_eq!(story.seconds(), 0.0);
        let still = RoutePlugin::new(crate::route_map::build(Default::default()), true);
        assert!(!still.playing());
        assert_eq!(still.grown(), 1.0);
        assert_eq!(still.xp(), 225.0);
        assert!(still.uses() > 0);
        assert_eq!(
            still.lines().len(),
            2 * route_chat::SCRIPT.len()
                + STORY.iter().filter(|s| s.ask.is_some()).count()
                + STORY.len()
                + OTHERS.len()
        );
    }
}
