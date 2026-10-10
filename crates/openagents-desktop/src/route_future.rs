//! The route map growing into the future: the Episode 289 deck's third
//! slide (`scene: routes-future`), drawn by the Map page's own view
//! ([`MapPage`]) fed a synthetic, growing model instead of live data.
//!
//! The model starts from today's map (the committed router, routes,
//! Coder, engines, and plugins) and grows month by month, October 2026 to
//! December 2030, after where `docs/transcripts/200.md` says this goes:
//!
//! - skills people sell as plugins, and engines on spare local compute
//!   (Macs, clusters, idle GPUs) under Coder, from the first months;
//! - a compute market, where requests buy inference, training, storage,
//!   and batch work from many providers (from mid 2027);
//! - a market for skills, data, work traces, checks, and evals, paid per
//!   use (from 2028);
//! - the agent network: autopilots forming coalitions, guilds, and crews,
//!   each with its own engines and plugins (from 2029);
//! - and around our router, other agent universes, each with its own
//!   router-like hub, families, autopilots, engines, and plugins, and
//!   other hubs (markets and exchanges), all linked to ours and to their
//!   neighbors.
//!
//! Something new appears nearly every month: new nodes grow out of their
//! parents while what was there eases to its new place. Traffic flows the
//! whole time: requests run out from a router to what serves them, and
//! payments run back from the plugins, engines, and data that served
//! them, more as the network grows. The run plays once from when the
//! slide opens and holds on December 2030 with the traffic still
//! flowing. The month and year is the only text.

use std::time::Instant;

use openagents_chat_app::route_map::layout::{Camera, Layout, Point};
use openagents_chat_app::route_map::{Edge, EdgeKind, Health, Kind, Map, Node};
use openagents_chat_app::visual;
use rust_native::style::{Color, TextAlign};
use rust_native_desktop::text::{Fonts, font};
use rust_native_desktop::{Frame, PxRect};

use crate::route_map::{MapPage, Pulse};

/// The months the simulation steps through, after October 2026 (month 0,
/// today): the last is December 2030.
pub const MONTHS: usize = 50;
/// How long each month shows, in seconds.
pub const MONTH: f32 = 0.75;
/// How long a month's growth takes, at its start, in seconds.
pub const GROW: f32 = 0.6;
/// When the last month has fully grown in, in seconds; it holds after.
pub const END: f32 = MONTHS as f32 * MONTH + GROW;

/// The first month's place in its year (October) and its year.
const FIRST_MONTH: usize = 9;
const FIRST_YEAR: usize = 2026;
const MONTH_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// A request on its way out, on the dark canvas.
pub const REQUEST: Color = Color::rgb(236, 240, 255);
/// A payment on its way back, on the dark canvas.
pub const PAYMENT: Color = Color::rgb(255, 206, 84);
/// A request on the light canvas: Coder Light's accent.
pub const REQUEST_LIGHT: Color = visual::Visual::LIGHT.accent;
/// A payment on the light canvas: a deeper gold that reads on white.
pub const PAYMENT_LIGHT: Color = Color::rgb(196, 130, 0);

/// A request on its way out, in the scheme the app paints with.
#[must_use]
pub fn request() -> Color {
    visual::pick(REQUEST, REQUEST_LIGHT)
}

/// A payment on its way back, in the scheme the app paints with.
#[must_use]
pub fn payment() -> Color {
    visual::pick(PAYMENT, PAYMENT_LIGHT)
}

/// Whether `color` is a payment's, in either scheme.
#[must_use]
pub fn is_payment(color: Color) -> bool {
    color == PAYMENT || color == PAYMENT_LIGHT
}

/// How a cluster around ours is drawn smaller than ours.
const SATELLITE_SCALE: f32 = 0.42;

/// The growing map and the Map page that draws it.
pub struct RouteFuture {
    page: MapPage,
    /// The layout once each month has grown in.
    layouts: Vec<Layout>,
    /// The month each node appears in.
    born: Vec<usize>,
    /// Each node's cluster root: our router or another hub.
    roots: Vec<usize>,
    /// Our router.
    front: usize,
    /// When the slide started showing.
    started: Option<Instant>,
    /// Seconds since the slide opened.
    seconds: f32,
    reduce_motion: bool,
    fonts: Fonts,
}

impl RouteFuture {
    /// The simulation from today's committed map. With `reduce_motion`, it
    /// holds still on the last month.
    pub fn new(reduce_motion: bool) -> Self {
        RouteFuture::growing(Map::committed(), reduce_motion)
    }

    /// The simulation growing from `today`.
    pub fn growing(today: Map, reduce_motion: bool) -> Self {
        let Grown {
            map,
            born,
            satellites,
        } = grow(today);
        let front = map.find("front").unwrap_or(0);
        let roots = roots(&map);
        let layouts = layouts(&map, &born, &roots, front, &satellites);
        let mut future = RouteFuture {
            page: MapPage::presenting(map, reduce_motion),
            layouts,
            born,
            roots,
            front,
            started: None,
            seconds: 0.0,
            reduce_motion,
            fonts: Fonts::new(),
        };
        future.seconds = future.still();
        future
    }

    /// Where a still picture sits: the last month, fully grown.
    fn still(&self) -> f32 {
        if self.reduce_motion { END } else { 0.0 }
    }

    /// The model's map, every month's nodes in it.
    pub fn map(&self) -> &Map {
        self.page.map()
    }

    /// The month each node appears in, by node index.
    pub fn born(&self) -> &[usize] {
        &self.born
    }

    /// Seconds since the slide opened.
    pub fn seconds(&self) -> f32 {
        self.seconds
    }

    /// The month showing, from 0 (October 2026) to [`MONTHS`].
    pub fn month(&self) -> usize {
        month_at(self.seconds).0
    }

    /// The month showing, as the slide writes it: "Nov 2026".
    pub fn label(&self) -> String {
        label(self.month())
    }

    /// Whether the run has finished and holds on its last month.
    pub fn done(&self) -> bool {
        self.seconds >= END
    }

    /// Plays from the start the next time the slide shows.
    pub fn reset(&mut self) {
        self.started = None;
        self.seconds = self.still();
    }

    /// Moves the clock to `now`; the run starts the first time.
    pub fn advance(&mut self, now: Instant) {
        if self.reduce_motion {
            return;
        }
        let started = *self.started.get_or_insert(now);
        self.seconds = now.saturating_duration_since(started).as_secs_f32();
    }

    /// Jumps to `seconds` after the slide opened.
    pub fn set_seconds(&mut self, seconds: f32) {
        self.seconds = seconds.max(0.0);
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
        let (mut layout, shown) = self.frame_at(self.seconds);
        let size = (rect.w / unit.max(0.1), rect.h / unit.max(0.1));
        let camera = fit(&layout, &shown, size);
        // As the camera pulls back, the nodes grow a little so they read.
        let boost = (0.55 / camera.zoom.max(0.01)).sqrt().clamp(1.0, 2.2);
        for r in &mut layout.radii {
            *r *= boost;
        }
        let traffic = self.traffic(&layout, &shown, self.seconds);
        self.page.set_unit(unit);
        self.page.set_frame(layout, shown, traffic, camera);
        self.page.paint(frame, rect);
        // The month, the only words: bottom left, large and quiet.
        let text = self.label();
        let size = 30.0 * unit;
        let paragraph = self.fonts.paragraph(
            &text,
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
                ..visual::current().text
            },
        );
    }

    /// Where every node sits `seconds` after the slide opened, and how far
    /// each has appeared.
    pub fn frame_at(&self, seconds: f32) -> (Layout, Vec<f32>) {
        let (month, into) = month_at(seconds);
        let n = self.born.len();
        let map = self.page.map();
        let to = &self.layouts[month];
        let from = &self.layouts[month.saturating_sub(1)];
        let t = if month == 0 { 1.0 } else { ease(into / GROW) };
        let mut positions = vec![Point::default(); n];
        let mut shown = vec![0.0_f32; n];
        for index in 0..n {
            let born = self.born[index];
            if born > month {
                continue;
            }
            if born == month && month > 0 {
                // New this month: out of its parent, staggered a little.
                let delay = 0.4 * unit(hash(index as u64));
                let own = ease(((into / GROW) - delay) / (1.0 - delay));
                let target = to.positions[index];
                let start = ancestor_at(map, &self.born, index, month - 1, from).unwrap_or(target);
                positions[index] = lerp(start, target, own);
                shown[index] = own;
            } else {
                positions[index] = lerp(from.positions[index], to.positions[index], t);
                shown[index] = 1.0;
            }
        }
        (
            Layout {
                positions,
                radii: to.radii.clone(),
                spans: vec![(0.0, 0.0); n],
            },
            shown,
        )
    }

    /// The traffic `seconds` after the slide opened: requests from a
    /// router out to what serves them (ours reaching the other clusters
    /// over their links, and each hub serving its own), and payments back
    /// from what was paid.
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
        let slots = (8 + leaves.len() / 3).min(240);
        let mut out = Vec::with_capacity(slots);
        for slot in 0..slots {
            let period = 2.2 + 1.2 * unit(hash(slot as u64 * 7 + 1));
            let offset = period * unit(hash(slot as u64 * 13 + 5));
            let clock = seconds + offset;
            let cycle = (clock / period).floor() as u64;
            let p = clock / period - cycle as f32;
            let pick = hash(slot as u64 * 1_000_003 + cycle);
            let leaf = leaves[pick as usize % leaves.len()];
            let mut path = path_to(map, leaf);
            // Half the requests to another cluster come from ours.
            if self.roots[leaf] != self.front && pick % 2 == 0 {
                path.insert(0, self.front);
            }
            let paid = matches!(
                map.nodes[leaf].kind,
                Kind::Plugin | Kind::Engine | Kind::Knowledge
            );
            if p < 0.5 {
                out.push(Pulse {
                    at: along(layout, &path, p / 0.5),
                    color: request(),
                    radius: 2.2,
                    ring: false,
                });
            } else if paid && p > 0.56 {
                out.push(Pulse {
                    at: along(layout, &path, 1.0 - (p - 0.56) / 0.44),
                    color: payment(),
                    radius: 2.4,
                    ring: false,
                });
            }
        }
        out
    }
}

/// The month showing `seconds` after the slide opened, and how far into
/// it; after the last month, it holds there, fully grown.
fn month_at(seconds: f32) -> (usize, f32) {
    let seconds = seconds.max(0.0);
    let month = ((seconds / MONTH) as usize).min(MONTHS);
    (month, seconds - month as f32 * MONTH)
}

/// Month `month` after October 2026, as the slide writes it.
fn label(month: usize) -> String {
    let at = FIRST_MONTH + month;
    format!("{} {}", MONTH_NAMES[at % 12], FIRST_YEAR + at / 12)
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
    // Room at the bottom left for the month.
    Camera::fit((min, max), size.0.max(1.0), size.1.max(1.0), 34.0)
}

/// Each node's cluster root.
fn roots(map: &Map) -> Vec<usize> {
    let mut roots = vec![0; map.nodes.len()];
    for index in 0..map.nodes.len() {
        roots[index] = map.nodes[index]
            .parent
            .map_or(index, |parent| roots[parent]);
    }
    roots
}

/// The layout once each month has grown in: our router's tree laid out as
/// the Map page lays it out, spread a little as it fills, and every other
/// cluster laid out the same way, smaller, on a ring around ours.
fn layouts(
    map: &Map,
    born: &[usize],
    roots: &[usize],
    front: usize,
    satellites: &[Satellite],
) -> Vec<Layout> {
    let n = map.nodes.len();
    let mut template = map.clone();
    template.nodes.clear();
    template.edges.clear();
    template.gaps.clear();
    // One cluster's nodes shown in `month`, laid out around its root at
    // the origin: positions by node index.
    let others: Vec<usize> = satellites.iter().map(|s| s.root).collect();
    let ours = |i: usize| !others.contains(&roots[i]);
    let cluster = |root: usize, month: usize| -> Vec<(usize, Point)> {
        let members: Vec<usize> = (0..n)
            .filter(|&i| born[i] <= month)
            .filter(|&i| {
                if root == front {
                    ours(i)
                } else {
                    roots[i] == root
                }
            })
            .collect();
        let mut sub = template.clone();
        let mut at = vec![usize::MAX; n];
        for (k, &i) in members.iter().enumerate() {
            at[i] = k;
            let mut node = map.nodes[i].clone();
            node.parent = node.parent.map(|p| at[p]);
            sub.nodes.push(node);
        }
        let layout = Layout::of(&sub);
        members
            .iter()
            .zip(layout.positions)
            .map(|(&i, p)| (i, p))
            .collect()
    };
    // How far a cluster reaches from its root, by its final layout.
    let reach = |placed: &[(usize, Point)], scale: f32| {
        placed
            .iter()
            .map(|(_, p)| (p.x * p.x + p.y * p.y).sqrt() * scale)
            .fold(0.0_f32, f32::max)
    };
    let today_leaves = leaves(map, born, &ours, 0).max(1) as f32;
    let radii: Vec<f32> = map
        .nodes
        .iter()
        .map(|node| openagents_chat_app::route_map::layout::radius(node.kind, node.weight))
        .collect();
    (0..=MONTHS)
        .map(|month| {
            let mut positions = vec![Point::default(); n];
            // Ours, spread as it fills so its leaves keep room.
            let filled = leaves(map, born, &ours, month) as f32 / today_leaves;
            let spread = filled.sqrt().clamp(1.0, 1.7);
            let ours = cluster(front, month);
            let ours_reach = reach(&ours, spread);
            for (i, p) in ours {
                positions[i] = Point::new(p.x * spread, p.y * spread);
            }
            // The others on a ring around ours, wider than tall, for the
            // slide; each at its place on the ring from when it appears.
            // The others' reach as they are this month, so the picture
            // stays tight while they are small and opens as they grow.
            let satellite_reach = satellites
                .iter()
                .filter(|s| s.born <= month)
                .map(|s| reach(&cluster(s.root, month), SATELLITE_SCALE))
                .fold(150.0_f32, f32::max);
            let ring = ours_reach + satellite_reach + 140.0;
            for satellite in satellites {
                let center = Point::new(
                    satellite.angle.cos() * ring * 1.55,
                    satellite.angle.sin() * ring,
                );
                if satellite.born > month {
                    positions[satellite.root] = center;
                    continue;
                }
                for (i, p) in cluster(satellite.root, month) {
                    positions[i] = Point::new(
                        center.x + p.x * SATELLITE_SCALE,
                        center.y + p.y * SATELLITE_SCALE,
                    );
                }
            }
            Layout {
                positions,
                radii: radii.clone(),
                spans: vec![(0.0, 0.0); n],
            }
        })
        .collect()
}

/// How many leaves the nodes `member` takes have in `month`.
fn leaves(map: &Map, born: &[usize], member: &dyn Fn(usize) -> bool, month: usize) -> usize {
    let mut parents = vec![false; map.nodes.len()];
    for (i, node) in map.nodes.iter().enumerate() {
        if born[i] <= month
            && let Some(parent) = node.parent
        {
            parents[parent] = true;
        }
    }
    (0..map.nodes.len())
        .filter(|&i| member(i) && born[i] <= month && !parents[i])
        .count()
}

/// Where `index` grows out of: its nearest ancestor already there in
/// `month`, at its place in `layout`. A new cluster's root grows in place.
fn ancestor_at(
    map: &Map,
    born: &[usize],
    index: usize,
    month: usize,
    layout: &Layout,
) -> Option<Point> {
    let mut at = map.nodes[index].parent;
    while let Some(parent) = at {
        if born[parent] <= month {
            return Some(layout.positions[parent]);
        }
        at = map.nodes[parent].parent;
    }
    None
}

/// The path from the leaf's cluster root down to `leaf`.
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

/// The `k`th of `count` months spread evenly from `from` to `to`.
fn spread(k: usize, count: usize, from: usize, to: usize) -> usize {
    if count <= 1 || to <= from {
        return from.min(MONTHS);
    }
    (from + k * (to - from) / (count - 1)).min(MONTHS)
}

/// A cluster around ours: its root, when it appears, and its place on the
/// ring, in radians.
struct Satellite {
    root: usize,
    born: usize,
    angle: f32,
}

/// The grown map, the month each node appears in, and the clusters
/// around ours.
struct Grown {
    map: Map,
    born: Vec<usize>,
    satellites: Vec<Satellite>,
}

/// Builds the growing map.
struct Grower {
    map: Map,
    born: Vec<usize>,
}

impl Grower {
    fn node(
        &mut self,
        month: usize,
        id: String,
        kind: Kind,
        label: &str,
        parent: Option<usize>,
    ) -> usize {
        let index = self.map.nodes.len();
        let (depth, family) = match parent {
            Some(parent) => (
                self.map.nodes[parent].depth + 1,
                self.map.nodes[parent].family.clone(),
            ),
            None => (0, None),
        };
        let weight = if parent.is_none() {
            0.25
        } else {
            0.3 + 0.5 * unit(hash(index as u64 + 77))
        };
        self.map.nodes.push(Node {
            id,
            kind,
            label: label.to_string(),
            line: String::new(),
            parent,
            depth,
            weight,
            health: Health::Good,
            stage: None,
            family,
            gaps: Vec::new(),
            local: None,
            showcase: false,
        });
        if let Some(parent) = parent {
            let edge = match (self.map.nodes[parent].kind, kind) {
                (Kind::Front | Kind::Family, _) => EdgeKind::Groups,
                (Kind::Coder, Kind::Engine) => EdgeKind::HandsOff,
                (Kind::Coder, Kind::Plugin) => EdgeKind::Admits,
                _ => EdgeKind::ServedBy,
            };
            self.map.edges.push(Edge {
                from: parent,
                to: index,
                kind: edge,
            });
        }
        // A child never shows before its parent.
        let month = parent.map_or(month, |p| month.max(self.born[p]));
        self.born.push(month.min(MONTHS));
        index
    }

    fn add(&mut self, month: usize, id: String, kind: Kind, label: &str, parent: usize) -> usize {
        self.node(month, id, kind, label, Some(parent))
    }

    /// `count` members of `kind` under `parent`, spread from `from` to
    /// `to`.
    fn members(
        &mut self,
        parent: usize,
        prefix: &str,
        kinds: &[Kind],
        label: &str,
        count: usize,
        (from, to): (usize, usize),
    ) {
        for k in 0..count {
            let kind = kinds[k % kinds.len()];
            self.add(
                spread(k, count, from, to),
                format!("{prefix}:{k}"),
                kind,
                label,
                parent,
            );
        }
    }

    /// Another agent universe: a router-like hub with its own families,
    /// autopilots, engines, and plugins, growing from `start`.
    fn universe(&mut self, n: usize, start: usize) -> usize {
        let hub = self.node(
            start,
            format!("future:universe:{n}"),
            Kind::Front,
            "Agent universe",
            None,
        );
        let families = 2 + (hash(n as u64 + 900) % 2) as usize;
        for f in 0..families {
            let opened = start + f * 3;
            if opened > MONTHS {
                break;
            }
            let family = self.add(
                opened,
                format!("future:universe:{n}:{f}"),
                Kind::Family,
                "Family",
                hub,
            );
            let pilot = self.add(
                opened + 1,
                format!("future:universe:{n}:{f}:autopilot"),
                Kind::Coder,
                "Autopilot",
                family,
            );
            let count = (MONTHS.saturating_sub(opened + 1)) / 4 + 2;
            let kinds: &[Kind] = if (n + f) % 2 == 0 {
                &[Kind::Plugin, Kind::Engine, Kind::Plugin]
            } else {
                &[Kind::Engine, Kind::Plugin]
            };
            self.members(
                pilot,
                &format!("future:universe:{n}:{f}:m"),
                kinds,
                "",
                count,
                (opened + 1, MONTHS),
            );
            // A route of its own with knowledge or prepared answers.
            if f == 0 {
                let route = self.add(
                    opened + 2,
                    format!("future:universe:{n}:route"),
                    Kind::Route,
                    "Route",
                    family,
                );
                let count = (MONTHS.saturating_sub(opened + 2)) / 6 + 2;
                self.members(
                    route,
                    &format!("future:universe:{n}:route:m"),
                    &[Kind::Knowledge, Kind::Answer],
                    "",
                    count,
                    (opened + 2, MONTHS),
                );
            }
        }
        hub
    }

    /// Another hub: a market or exchange of routes with many providers,
    /// growing from `start`.
    fn hub(&mut self, n: usize, start: usize) -> usize {
        let hub = self.node(start, format!("future:hub:{n}"), Kind::Front, "Hub", None);
        let routes: [(usize, &[Kind]); 4] = [
            (0, &[Kind::Engine]),
            (1, &[Kind::Plugin]),
            (4, &[Kind::Knowledge, Kind::Engine]),
            (8, &[Kind::Plugin, Kind::Answer]),
        ];
        for (r, (after, kinds)) in routes.into_iter().enumerate() {
            let opened = start + after;
            if opened > MONTHS {
                break;
            }
            let route = self.add(
                opened,
                format!("future:hub:{n}:{r}"),
                Kind::Route,
                "Route",
                hub,
            );
            let count = (MONTHS.saturating_sub(opened)) / 3 + 2;
            self.members(
                route,
                &format!("future:hub:{n}:{r}:m"),
                kinds,
                "Provider",
                count,
                (opened, MONTHS),
            );
        }
        hub
    }
}

/// Today's map with every later month's nodes appended, and the month
/// each node appears in, with the clusters around ours.
fn grow(today: Map) -> Grown {
    let mut map = today;
    // The future has no gaps to mark; today's are the Map page's to show.
    map.gaps.clear();
    for node in &mut map.nodes {
        node.gaps.clear();
    }
    let born = vec![0; map.nodes.len()];
    let front = map.find("front").unwrap_or(0);
    let coder = map.find("coder");
    let work = map.find("family:work");
    let mut g = Grower { map, born };
    // Skills people sell, and engines on spare local compute, from the
    // first months.
    if let Some(coder) = coder {
        for (n, name) in [
            "Mac mini",
            "Mac Studio",
            "Mac cluster",
            "Idle GPU",
            "Office rack",
            "Gaming PC",
            "Home server",
            "GPU cluster",
        ]
        .iter()
        .enumerate()
        {
            g.add(
                spread(n, 8, 1, 22),
                format!("future:engine:{n}"),
                Kind::Engine,
                name,
                coder,
            );
        }
        g.members(coder, "future:skill", &[Kind::Plugin], "Skill", 22, (1, 48));
    }
    if let Some(work) = work {
        let autopilot = g.add(
            2,
            "future:route:autopilot".into(),
            Kind::Route,
            "autopilot",
            work,
        );
        g.members(
            autopilot,
            "future:autopilot",
            &[Kind::Plugin],
            "Skill",
            10,
            (3, 44),
        );
    }
    // A compute market, from mid 2027.
    let compute = g.add(
        9,
        "future:family:compute".into(),
        Kind::Family,
        "Compute",
        front,
    );
    for (route, opened, providers) in [
        ("inference", 9, 14),
        ("compute.sell", 10, 10),
        ("training", 13, 9),
        ("storage", 21, 6),
        ("batch", 31, 5),
    ] {
        let index = g.add(
            opened,
            format!("future:route:{route}"),
            Kind::Route,
            route,
            compute,
        );
        g.members(
            index,
            &format!("future:{route}"),
            &[Kind::Engine],
            "Provider",
            providers,
            (opened, MONTHS),
        );
    }
    // A market for skills, data, work traces, checks, and evals, paid per
    // use, from 2028.
    let market = g.add(
        16,
        "future:family:market".into(),
        Kind::Family,
        "Market",
        front,
    );
    for (route, opened, kind, members) in [
        ("skills", 16, Kind::Plugin, 12),
        ("data", 18, Kind::Knowledge, 8),
        ("traces", 21, Kind::Knowledge, 7),
        ("verify", 25, Kind::Answer, 5),
        ("evals", 33, Kind::Answer, 4),
    ] {
        let index = g.add(
            opened,
            format!("future:route:{route}"),
            Kind::Route,
            route,
            market,
        );
        g.members(
            index,
            &format!("future:{route}"),
            &[kind],
            "",
            members,
            (opened, MONTHS),
        );
    }
    // The agent network: autopilots forming coalitions, guilds, and
    // crews, from 2029.
    let network = g.add(
        27,
        "future:family:network".into(),
        Kind::Family,
        "Network",
        front,
    );
    for (route, opened, agents) in [("coalitions", 27, 4), ("guilds", 30, 3), ("crews", 36, 3)] {
        let index = g.add(
            opened,
            format!("future:route:{route}"),
            Kind::Route,
            route,
            network,
        );
        for n in 0..agents {
            let joined = spread(n, agents, opened, MONTHS - 4);
            let agent = g.add(
                joined,
                format!("future:{route}:{n}"),
                Kind::Coder,
                "Autopilot",
                index,
            );
            g.members(
                agent,
                &format!("future:{route}:{n}"),
                &[Kind::Plugin, Kind::Engine],
                "",
                3,
                (joined, (joined + 8).min(MONTHS)),
            );
        }
    }
    // Other agent universes and other hubs, each on its own place on the
    // ring around ours, linked to ours and to the neighbors on the ring.
    // Their order on the ring alternates sides so the picture stays
    // balanced as they appear.
    let others: [(bool, usize, usize); 10] = [
        (true, 6, 0),
        (false, 10, 5),
        (true, 14, 9),
        (true, 19, 4),
        (false, 23, 1),
        (true, 28, 6),
        (true, 33, 2),
        (false, 37, 7),
        (true, 41, 3),
        (true, 45, 8),
    ];
    let mut satellites = Vec::new();
    let (mut universes, mut hubs) = (0, 0);
    for (universe, start, slot) in others {
        let root = if universe {
            universes += 1;
            g.universe(universes, start)
        } else {
            hubs += 1;
            g.hub(hubs, start)
        };
        satellites.push(Satellite {
            root,
            born: start,
            angle: slot as f32 * std::f32::consts::TAU / others.len() as f32,
        });
    }
    let mut links = Vec::new();
    for s in &satellites {
        links.push((front, s.root));
    }
    let mut ring: Vec<&Satellite> = satellites.iter().collect();
    ring.sort_by(|a, b| a.angle.total_cmp(&b.angle));
    for k in 0..ring.len() {
        links.push((ring[k].root, ring[(k + 1) % ring.len()].root));
    }
    for (from, to) in links {
        g.map.edges.push(Edge {
            from,
            to,
            kind: EdgeKind::ServedBy,
        });
    }
    Grown {
        map: g.map,
        born: g.born,
        satellites,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_network_grows_nearly_every_month_from_todays_map() {
        let today = Map::committed();
        let future = RouteFuture::growing(today.clone(), false);
        let map = future.map();
        let ids = |nodes: &[Node]| nodes.iter().map(|n| n.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&map.nodes[..today.nodes.len()]), ids(&today.nodes));
        let mut quiet = 0;
        for month in 1..=MONTHS {
            if !future.born().contains(&month) {
                quiet += 1;
            }
        }
        assert!(quiet <= 2, "{quiet} months add nothing");
        for kind in [Kind::Family, Kind::Route, Kind::Plugin, Kind::Engine] {
            let added = (today.nodes.len()..map.nodes.len())
                .filter(|&i| map.nodes[i].kind == kind)
                .count();
            assert!(added > 10, "{kind:?} {added}");
        }
        // Other universes and hubs: clusters of their own, linked to ours.
        let hubs = (0..map.nodes.len())
            .filter(|&i| map.nodes[i].parent.is_none() && map.nodes[i].kind == Kind::Front)
            .count();
        assert!(hubs >= 8, "{hubs}");
        assert!(map.nodes.len() > today.nodes.len() * 3);
        // A child never shows before its parent.
        for (i, node) in map.nodes.iter().enumerate() {
            if let Some(parent) = node.parent {
                assert!(future.born()[parent] <= future.born()[i], "{}", node.id);
            }
        }
    }

    #[test]
    fn today_is_the_live_layout_and_each_month_grows_in() {
        let today = Map::committed();
        let live = Layout::of(&today);
        let future = RouteFuture::growing(today.clone(), false);
        let (layout, shown) = future.frame_at(0.1);
        assert_eq!(&layout.positions[..today.nodes.len()], &live.positions[..]);
        assert!(shown[today.nodes.len()..].iter().all(|&s| s == 0.0));
        // Mid growth of a month: some of its nodes part way out.
        let month = 6;
        let (_, mid) = future.frame_at(month as f32 * MONTH + GROW * 0.6);
        let growing = future
            .born()
            .iter()
            .zip(&mid)
            .filter(|(b, s)| **b == month && **s > 0.0 && **s < 1.0)
            .count();
        assert!(growing > 0);
        let (_, grown) = future.frame_at(month as f32 * MONTH + GROW + 0.01);
        assert!(
            future
                .born()
                .iter()
                .zip(&grown)
                .all(|(b, s)| (*b <= month) == (*s >= 1.0))
        );
    }

    #[test]
    fn it_plays_once_and_holds_on_the_last_month() {
        let future = RouteFuture::growing(Map::committed(), false);
        let (end, shown) = future.frame_at(END);
        assert!(shown.iter().all(|&s| s >= 1.0));
        let (later, _) = future.frame_at(END + 120.0);
        assert_eq!(end.positions, later.positions);
        assert_eq!(label(MONTHS), "Dec 2030");
        assert_eq!(label(0), "Oct 2026");
        assert_eq!(label(1), "Nov 2026");
        assert_eq!(label(3), "Jan 2027");
        assert!((35.0..=45.0).contains(&END), "{END}");
    }

    #[test]
    fn the_clusters_do_not_overlap_at_the_end() {
        let future = RouteFuture::growing(Map::committed(), false);
        let (layout, _) = future.frame_at(END);
        let map = future.map();
        let roots = roots(map);
        let mut extents: Vec<(usize, Point, Point)> = Vec::new();
        for (i, p) in layout.positions.iter().enumerate() {
            let r = layout.radii[i];
            match extents.iter_mut().find(|(root, _, _)| *root == roots[i]) {
                Some((_, min, max)) => {
                    *min = Point::new(min.x.min(p.x - r), min.y.min(p.y - r));
                    *max = Point::new(max.x.max(p.x + r), max.y.max(p.y + r));
                }
                None => extents.push((
                    roots[i],
                    Point::new(p.x - r, p.y - r),
                    Point::new(p.x + r, p.y + r),
                )),
            }
        }
        // Each cluster's center sits outside every other's box.
        for (a, amin, amax) in &extents {
            let center = layout.positions[*a];
            for (b, bmin, bmax) in &extents {
                if a == b {
                    continue;
                }
                let inside = center.x > bmin.x
                    && center.x < bmax.x
                    && center.y > bmin.y
                    && center.y < bmax.y;
                assert!(!inside, "{a} in {b} ({amin:?} {amax:?})");
            }
        }
    }

    #[test]
    fn traffic_grows_with_the_network_and_payments_come_back() {
        let future = RouteFuture::growing(Map::committed(), false);
        let count = |seconds: f32| {
            let (layout, shown) = future.frame_at(seconds);
            future.traffic(&layout, &shown, seconds)
        };
        let today = count(0.3);
        let later = count(END + 3.0);
        // Today's committed map already carries the GitHub route (#11167),
        // so the grown network is a little under three times today's.
        assert!(
            later.len() > today.len() * 5 / 2,
            "{} {}",
            today.len(),
            later.len()
        );
        assert!(later.iter().any(|p| p.color == payment()));
        assert!(later.iter().any(|p| p.color == request()));
        // Still flowing after the run ends.
        assert_ne!(count(END + 3.0), count(END + 4.0));
    }

    #[test]
    fn the_month_follows_the_clock_and_reduce_motion_holds_still() {
        let mut future = RouteFuture::growing(Map::committed(), false);
        let start = Instant::now();
        future.advance(start);
        assert_eq!(future.label(), "Oct 2026");
        future.advance(start + std::time::Duration::from_secs_f32(MONTH * 15.5));
        assert_eq!(future.label(), "Jan 2028");
        future.advance(start + std::time::Duration::from_secs(300));
        assert_eq!(future.label(), "Dec 2030");
        assert!(future.done());
        future.reset();
        future.advance(start + std::time::Duration::from_secs(400));
        assert_eq!(future.label(), "Oct 2026");
        let mut still = RouteFuture::growing(Map::committed(), true);
        still.advance(start + std::time::Duration::from_secs(9));
        assert_eq!(still.label(), "Dec 2030");
    }
}
