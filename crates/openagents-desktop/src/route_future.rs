//! The route map growing into the future: the Episode 289 deck's third
//! slide (`scene: routes-future`), drawn by the Map page's own view
//! ([`MapPage`]) fed a synthetic, growing model instead of live data.
//!
//! The model starts from today's map (the committed router, routes,
//! Coder, engines, and plugins) and adds a phase a year, after where
//! `docs/transcripts/200.md` says this goes:
//!
//! - 2027: plugins people sell as skills, and engines on spare local
//!   compute (Macs, clusters, idle GPUs) under Coder;
//! - 2028: a compute market, where requests buy inference and training
//!   from many providers;
//! - 2029: a market for skills, data, and work traces, paid per use;
//! - 2030: the agent network: autopilots forming coalitions and guilds,
//!   each with its own engines and plugins.
//!
//! Each phase grows in over a few seconds: what was there moves to its new
//! place in the radial tree while the new nodes come out of their parents.
//! Traffic flows the whole time: requests run from the router out to what
//! serves them, and payments run back from the plugins, engines, and data
//! that served them, more as the network grows. After the last year the
//! map rewinds to today and plays again. The year is the only text added.

use std::time::Instant;

use openagents_chat_app::route_map::layout::{Camera, Layout, Point};
use openagents_chat_app::route_map::{Edge, EdgeKind, Health, Kind, Map, Node};
use openagents_chat_app::visual;
use rust_native::style::{Color, TextAlign};
use rust_native_desktop::text::{Fonts, font};
use rust_native_desktop::{Frame, PxRect};

use crate::route_map::{MapPage, Pulse};

/// The years the simulation shows, one phase each; the first is today.
pub const YEARS: [u16; 5] = [2026, 2027, 2028, 2029, 2030];
/// How long each year shows, in seconds.
pub const PHASE: f32 = 7.0;
/// How long a year's growth takes, at the start of its phase, in seconds.
pub const GROW: f32 = 3.2;
/// How long the rewind to today takes, at the end of the last year.
pub const REWIND: f32 = 1.4;
/// One full loop, in seconds.
pub const LOOP: f32 = PHASE * YEARS.len() as f32;

/// A request on its way out.
const REQUEST: Color = Color::rgb(236, 240, 255);
/// A payment on its way back.
const PAYMENT: Color = Color::rgb(255, 206, 84);

/// The growing map and the Map page that draws it.
pub struct RouteFuture {
    page: MapPage,
    /// The layout after each phase has grown in.
    layouts: Vec<Layout>,
    /// The phase each node appears in.
    born: Vec<usize>,
    /// When the slide started showing.
    started: Option<Instant>,
    /// Seconds into the loop.
    seconds: f32,
    reduce_motion: bool,
    fonts: Fonts,
}

impl RouteFuture {
    /// The simulation from today's committed map. With `reduce_motion`, it
    /// holds still on the last year.
    pub fn new(reduce_motion: bool) -> Self {
        RouteFuture::growing(Map::committed(), reduce_motion)
    }

    /// The simulation growing from `today`.
    pub fn growing(today: Map, reduce_motion: bool) -> Self {
        let (map, born) = grow(today);
        let layouts = (0..YEARS.len())
            .map(|phase| {
                let count = born.iter().filter(|&&b| b <= phase).count();
                let mut snapshot = map.clone();
                snapshot.nodes.truncate(count);
                snapshot
                    .edges
                    .retain(|edge| edge.from < count && edge.to < count);
                Layout::of(&snapshot)
            })
            .collect();
        let mut future = RouteFuture {
            page: MapPage::presenting(map, reduce_motion),
            layouts,
            born,
            started: None,
            seconds: 0.0,
            reduce_motion,
            fonts: Fonts::new(),
        };
        future.seconds = future.still();
        future
    }

    /// Where a still picture sits: the last year, fully grown.
    fn still(&self) -> f32 {
        if self.reduce_motion {
            LOOP - REWIND - 0.5
        } else {
            0.0
        }
    }

    /// The model's map, every year's nodes in it.
    pub fn map(&self) -> &Map {
        self.page.map()
    }

    /// The phase each node appears in, by node index.
    pub fn born(&self) -> &[usize] {
        &self.born
    }

    /// Seconds into the loop.
    pub fn seconds(&self) -> f32 {
        self.seconds
    }

    /// The year showing.
    pub fn year(&self) -> u16 {
        YEARS[phase_at(self.seconds).0]
    }

    /// Starts the loop over the next time the slide shows.
    pub fn reset(&mut self) {
        self.started = None;
        self.seconds = self.still();
    }

    /// Moves the clock to `now`; the loop starts the first time.
    pub fn advance(&mut self, now: Instant) {
        if self.reduce_motion {
            return;
        }
        let started = *self.started.get_or_insert(now);
        self.seconds = now.saturating_duration_since(started).as_secs_f32() % LOOP;
    }

    /// Jumps to `seconds` into the loop.
    pub fn set_seconds(&mut self, seconds: f32) {
        self.seconds = seconds.rem_euclid(LOOP);
    }

    pub fn version(&self) -> u64 {
        self.page.version()
    }

    pub fn set_unit(&mut self, unit: f32) {
        self.page.set_unit(unit);
    }

    /// Paints the frame at the current time into `rect`, in pixels, with
    /// `unit` pixels a point.
    pub fn paint(&mut self, frame: &mut Frame, rect: PxRect, unit: f32) {
        let (layout, shown) = self.frame_at(self.seconds);
        let traffic = self.traffic(&layout, &shown, self.seconds);
        let size = (rect.w / unit.max(0.1), rect.h / unit.max(0.1));
        let camera = fit(&layout, &shown, size);
        self.page.set_unit(unit);
        self.page.set_frame(layout, shown, traffic, camera);
        self.page.paint(frame, rect);
        // The year, the only words: bottom left, large and quiet.
        let year = self.year().to_string();
        let size = 30.0 * unit;
        let paragraph = self.fonts.paragraph(
            &year,
            font(size, rust_native::layout::display::Weight::Semibold, false),
            None,
        );
        let line = size * rust_native_desktop::text::LINE_EM;
        self.fonts.draw(
            frame,
            &paragraph,
            rect.x + 22.0 * unit,
            rect.y + rect.h - line - 18.0 * unit,
            paragraph.width + 4.0,
            TextAlign::Start,
            1.0,
            Color {
                alpha: 220,
                ..visual::TEXT
            },
        );
    }

    /// Where every node sits `seconds` into the loop, and how far each has
    /// appeared.
    pub fn frame_at(&self, seconds: f32) -> (Layout, Vec<f32>) {
        let (phase, into) = phase_at(seconds);
        let n = self.born.len();
        let map = self.page.map();
        let last = YEARS.len() - 1;
        // Growing: from the year before's layout to this year's.
        let (from, to, t, rewinding) = if phase == last && into > PHASE - REWIND {
            let t = ease((into - (PHASE - REWIND)) / REWIND);
            (&self.layouts[last], &self.layouts[0], t, true)
        } else if phase == 0 {
            (&self.layouts[0], &self.layouts[0], 1.0, false)
        } else {
            let t = ease(into / GROW);
            (&self.layouts[phase - 1], &self.layouts[phase], t, false)
        };
        let mut positions = vec![Point::default(); n];
        let mut shown = vec![0.0_f32; n];
        let mut radii = vec![0.0_f32; n];
        for index in 0..n {
            let born = self.born[index];
            let (target, appear) = if rewinding {
                // Everything after today goes back into its parent.
                if born == 0 {
                    (to.positions[index], 1.0)
                } else {
                    (ancestor_at(map, &self.born, index, 0, to), 1.0 - t)
                }
            } else if born > phase {
                continue;
            } else {
                (to.positions[index], 1.0)
            };
            let (start, appear) = if rewinding {
                (from.positions[index], appear)
            } else if born == phase && phase > 0 {
                // New this year: out of its parent, staggered a little.
                let delay = 0.45 * hash(index as u64) as f32 / u32::MAX as f32;
                let own = ease(((into / GROW) - delay) / (1.0 - delay));
                shown[index] = own;
                let start = ancestor_at(map, &self.born, index, phase - 1, from);
                positions[index] = lerp(start, target, own);
                radii[index] = to.radii[index];
                continue;
            } else {
                (from.positions[index], appear)
            };
            positions[index] = lerp(start, target, t);
            shown[index] = appear;
            radii[index] = if index < to.radii.len() {
                to.radii[index]
            } else {
                from.radii[index]
            };
        }
        let spans = vec![(0.0, 0.0); n];
        (
            Layout {
                positions,
                radii,
                spans,
            },
            shown,
        )
    }

    /// The traffic `seconds` into the loop: requests from the router out
    /// to what serves them, and payments back from what was paid.
    pub fn traffic(&self, layout: &Layout, shown: &[f32], seconds: f32) -> Vec<Pulse> {
        let map = self.page.map();
        let leaves: Vec<usize> = (0..map.nodes.len())
            .filter(|&i| shown[i] >= 1.0 && map.nodes[i].parent.is_some())
            .filter(|&i| {
                matches!(
                    map.nodes[i].kind,
                    Kind::Plugin
                        | Kind::Engine
                        | Kind::Knowledge
                        | Kind::Answer
                        | Kind::Model
                        | Kind::Screen
                        | Kind::Coder
                )
            })
            .collect();
        if leaves.is_empty() {
            return Vec::new();
        }
        // More traffic as the network grows.
        let slots = (6 + leaves.len() / 4).min(150);
        let mut out = Vec::with_capacity(slots * 2);
        for slot in 0..slots {
            let period = 2.4 + 1.2 * unit(hash(slot as u64 * 7 + 1));
            let offset = period * unit(hash(slot as u64 * 13 + 5));
            let clock = seconds + offset;
            let cycle = (clock / period).floor() as u64;
            let p = clock / period - cycle as f32;
            let leaf = leaves[hash(slot as u64 * 1_000_003 + cycle) as usize % leaves.len()];
            let path = path_to(map, leaf);
            let paid = matches!(
                map.nodes[leaf].kind,
                Kind::Plugin | Kind::Engine | Kind::Knowledge
            );
            if p < 0.5 {
                out.push(Pulse {
                    at: along(layout, &path, p / 0.5),
                    color: REQUEST,
                    radius: 2.2,
                });
            } else if paid && p > 0.56 {
                out.push(Pulse {
                    at: along(layout, &path, 1.0 - (p - 0.56) / 0.44),
                    color: PAYMENT,
                    radius: 2.4,
                });
            }
        }
        out
    }
}

/// The phase showing `seconds` into the loop, and how far into it.
fn phase_at(seconds: f32) -> (usize, f32) {
    let seconds = seconds.rem_euclid(LOOP);
    let phase = ((seconds / PHASE) as usize).min(YEARS.len() - 1);
    (phase, seconds - phase as f32 * PHASE)
}

/// The camera fitting what shows, with room around it.
fn fit(layout: &Layout, shown: &[f32], size: (f32, f32)) -> Camera {
    let mut min = Point::new(f32::MAX, f32::MAX);
    let mut max = Point::new(f32::MIN, f32::MIN);
    for (index, p) in layout.positions.iter().enumerate() {
        if shown[index] <= 0.0 {
            continue;
        }
        let r = layout.radii[index];
        min = Point::new(min.x.min(p.x - r), min.y.min(p.y - r));
        max = Point::new(max.x.max(p.x + r), max.y.max(p.y + r));
    }
    if min.x > max.x {
        return Camera::default();
    }
    Camera::fit((min, max), size.0.max(1.0), size.1.max(1.0), 28.0)
}

/// Where `index` starts growing from: its nearest ancestor already there
/// in `phase`, at its place in `layout`.
fn ancestor_at(map: &Map, born: &[usize], index: usize, phase: usize, layout: &Layout) -> Point {
    let mut at = map.nodes[index].parent;
    while let Some(parent) = at {
        if born[parent] <= phase && parent < layout.positions.len() {
            return layout.positions[parent];
        }
        at = map.nodes[parent].parent;
    }
    Point::default()
}

/// The path from the router down to `leaf`.
fn path_to(map: &Map, leaf: usize) -> Vec<usize> {
    let mut path = vec![leaf];
    let mut at = map.nodes[leaf].parent;
    while let Some(parent) = at {
        path.push(parent);
        at = map.nodes[parent].parent;
    }
    path.reverse();
    path
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

/// Ease in and out.
fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A fixed scramble of `n`, so the model and its traffic are the same
/// every run.
fn hash(n: u64) -> u32 {
    let mut x = n.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03;
    x ^= x >> 31;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 29;
    (x >> 32) as u32
}

fn unit(n: u32) -> f32 {
    n as f32 / u32::MAX as f32
}

/// Appends a node of `kind` under `parent`, appearing in `phase`.
fn add(
    map: &mut Map,
    born: &mut Vec<usize>,
    phase: usize,
    id: String,
    kind: Kind,
    label: &str,
    parent: usize,
) -> usize {
    let depth = map.nodes[parent].depth + 1;
    let family = map.nodes[parent].family.clone();
    let index = map.nodes.len();
    map.nodes.push(Node {
        id,
        kind,
        label: label.to_string(),
        line: String::new(),
        parent: Some(parent),
        depth,
        weight: 0.3 + 0.5 * unit(hash(index as u64 + 77)),
        health: Health::Good,
        stage: None,
        family,
        gaps: Vec::new(),
        local: None,
        showcase: false,
    });
    let edge = match (map.nodes[parent].kind, kind) {
        (Kind::Front | Kind::Family, _) => EdgeKind::Groups,
        (Kind::Coder, Kind::Engine) => EdgeKind::HandsOff,
        (Kind::Coder, Kind::Plugin) => EdgeKind::Admits,
        _ => EdgeKind::ServedBy,
    };
    map.edges.push(Edge {
        from: parent,
        to: index,
        kind: edge,
    });
    born.push(phase);
    index
}

/// Today's map with every later year's nodes appended, phase by phase,
/// and the phase each node appears in.
fn grow(today: Map) -> (Map, Vec<usize>) {
    let mut map = today;
    // The future has no gaps to mark; today's are the Map page's to show.
    map.gaps.clear();
    for node in &mut map.nodes {
        node.gaps.clear();
    }
    let mut born = vec![0; map.nodes.len()];
    let front = map.find("front").unwrap_or(0);
    let coder = map.find("coder");
    let work = map.find("family:work");
    // 2027: skills people sell, and engines on spare local compute.
    if let Some(coder) = coder {
        for (n, name) in [
            "Mac mini",
            "Mac Studio",
            "Mac cluster",
            "Idle GPU",
            "Office rack",
        ]
        .iter()
        .enumerate()
        {
            add(
                &mut map,
                &mut born,
                1,
                format!("future:engine:{n}"),
                Kind::Engine,
                name,
                coder,
            );
        }
        for n in 0..14 {
            add(
                &mut map,
                &mut born,
                1,
                format!("future:skill:{n}"),
                Kind::Plugin,
                "Skill",
                coder,
            );
        }
    }
    if let Some(work) = work {
        let autopilot = add(
            &mut map,
            &mut born,
            1,
            "future:route:autopilot".into(),
            Kind::Route,
            "autopilot",
            work,
        );
        for n in 0..5 {
            add(
                &mut map,
                &mut born,
                1,
                format!("future:autopilot:{n}"),
                Kind::Plugin,
                "Skill",
                autopilot,
            );
        }
    }
    // 2028: a compute market.
    let compute = add(
        &mut map,
        &mut born,
        2,
        "future:family:compute".into(),
        Kind::Family,
        "Compute",
        front,
    );
    for (route, providers) in [("inference", 9), ("training", 6), ("compute.sell", 7)] {
        let index = add(
            &mut map,
            &mut born,
            2,
            format!("future:route:{route}"),
            Kind::Route,
            route,
            compute,
        );
        for n in 0..providers {
            add(
                &mut map,
                &mut born,
                2,
                format!("future:{route}:{n}"),
                Kind::Engine,
                "Provider",
                index,
            );
        }
    }
    // 2029: a market for skills, data, and work traces, paid per use.
    let market = add(
        &mut map,
        &mut born,
        3,
        "future:family:market".into(),
        Kind::Family,
        "Market",
        front,
    );
    for (route, kind, members) in [
        ("skills", Kind::Plugin, 12),
        ("data", Kind::Knowledge, 7),
        ("traces", Kind::Knowledge, 6),
        ("verify", Kind::Answer, 5),
    ] {
        let index = add(
            &mut map,
            &mut born,
            3,
            format!("future:route:{route}"),
            Kind::Route,
            route,
            market,
        );
        for n in 0..members {
            add(
                &mut map,
                &mut born,
                3,
                format!("future:{route}:{n}"),
                kind,
                "",
                index,
            );
        }
    }
    // 2030: the agent network: autopilots forming coalitions and guilds.
    let network = add(
        &mut map,
        &mut born,
        4,
        "future:family:network".into(),
        Kind::Family,
        "Network",
        front,
    );
    for (route, agents) in [("coalitions", 5), ("guilds", 4)] {
        let index = add(
            &mut map,
            &mut born,
            4,
            format!("future:route:{route}"),
            Kind::Route,
            route,
            network,
        );
        for n in 0..agents {
            let agent = add(
                &mut map,
                &mut born,
                4,
                format!("future:{route}:{n}"),
                Kind::Coder,
                "Autopilot",
                index,
            );
            for m in 0..4 {
                let kind = if m % 2 == 0 {
                    Kind::Plugin
                } else {
                    Kind::Engine
                };
                add(
                    &mut map,
                    &mut born,
                    4,
                    format!("future:{route}:{n}:{m}"),
                    kind,
                    "",
                    agent,
                );
            }
        }
    }
    (map, born)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_network_grows_every_year_from_todays_map() {
        let today = Map::committed();
        let future = RouteFuture::growing(today.clone(), false);
        let ids = |nodes: &[Node]| nodes.iter().map(|n| n.id.clone()).collect::<Vec<_>>();
        assert_eq!(
            ids(&future.map().nodes[..today.nodes.len()]),
            ids(&today.nodes)
        );
        let mut last = 0;
        for (phase, year) in YEARS.iter().enumerate() {
            let count = future.born().iter().filter(|&&b| b <= phase).count();
            assert!(count > last, "year {year} adds nodes");
            last = count;
        }
        for kind in [Kind::Family, Kind::Route, Kind::Plugin, Kind::Engine] {
            let added = (today.nodes.len()..future.map().nodes.len())
                .filter(|&i| future.map().nodes[i].kind == kind)
                .count();
            assert!(added > 0, "{kind:?}");
        }
    }

    #[test]
    fn today_is_the_live_layout_and_each_year_grows_in_then_rewinds() {
        let today = Map::committed();
        let live = Layout::of(&today);
        let future = RouteFuture::growing(today.clone(), false);
        let (layout, shown) = future.frame_at(1.0);
        assert_eq!(&layout.positions[..today.nodes.len()], &live.positions[..]);
        assert!(shown[today.nodes.len()..].iter().all(|&s| s == 0.0));
        // Mid growth of 2027: some of its nodes part way out.
        let (_, mid) = future.frame_at(PHASE + GROW * 0.5);
        let growing = future
            .born()
            .iter()
            .zip(&mid)
            .filter(|(b, s)| **b == 1 && **s > 0.0 && **s < 1.0)
            .count();
        assert!(growing > 0);
        let (_, grown) = future.frame_at(PHASE + GROW + 0.1);
        assert!(
            future
                .born()
                .iter()
                .zip(&grown)
                .all(|(b, s)| (*b <= 1) == (*s >= 1.0))
        );
        // The end of the loop is today again.
        let (end, shown) = future.frame_at(LOOP - 0.001);
        assert!(shown[today.nodes.len()..].iter().all(|&s| s < 0.01));
        for (a, b) in end.positions.iter().zip(&live.positions) {
            assert!(a.distance(*b) < 1.0);
        }
    }

    #[test]
    fn traffic_grows_with_the_network_and_payments_come_back() {
        let future = RouteFuture::growing(Map::committed(), false);
        let count = |seconds: f32| {
            let (layout, shown) = future.frame_at(seconds);
            future.traffic(&layout, &shown, seconds)
        };
        let today = count(3.0);
        let later = count(LOOP - REWIND - 1.0);
        assert!(later.len() > today.len(), "{} {}", today.len(), later.len());
        assert!(later.iter().any(|p| p.color == PAYMENT));
        assert!(later.iter().any(|p| p.color == REQUEST));
    }

    #[test]
    fn the_year_follows_the_clock_and_reduce_motion_holds_still() {
        let mut future = RouteFuture::growing(Map::committed(), false);
        let start = Instant::now();
        future.advance(start);
        assert_eq!(future.year(), 2026);
        future.advance(start + std::time::Duration::from_secs_f32(PHASE * 2.5));
        assert_eq!(future.year(), 2028);
        let mut still = RouteFuture::growing(Map::committed(), true);
        still.advance(start + std::time::Duration::from_secs(9));
        assert_eq!(still.year(), 2030);
    }
}
