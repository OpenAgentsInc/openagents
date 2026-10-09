//! The Map page (#10085): OpenAgents' composition as one zoomable graph,
//! from [`openagents_chat_app::route_map`], with an inspector, a Gaps
//! panel, a legend, and an outline for keyboards and screen readers.
//!
//! The graph is painted into one surface, [`RESOURCE`], at the window's
//! scale: edges, a circle per node in its kind's color with its health as
//! the ring, a red marker on every gap, and labels thinned by zoom
//! ([`layout::detail`]). The camera pans with a drag or two-finger scroll,
//! zooms with a pinch, Cmd or Ctrl and the wheel, the toolbar, or Cmd +
//! and −, and eases between views on the frame clock unless "Reduce
//! motion" is on, when it moves at once. A double-click zooms into a node.
//! Arrows move between nodes, Tab steps through them in the outline's
//! order, Enter zooms into the selection, and Esc steps out to its parent.
//!
//! Beside the surface, normal Rust Native views: the inspector (what a
//! node is, why the router sends things there, its numbers with their
//! records, its members, its gaps, and next steps), the Gaps panel, and
//! the outline. A next step never runs by itself: it becomes an
//! [`Effect`] the window carries out after the tap (a chat with the
//! message in the composer, unsent; a command copied; a page opened;
//! Settings' Coder page).
//!
//! The page holds nothing when it isn't open: the shell builds it when the
//! Map opens and drops it when the person leaves, so idle cost elsewhere
//! is unchanged.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use openagents_chat_app::route_map::layout::{self, Camera, Detail, Direction, Layout, Point};
use openagents_chat_app::route_map::{
    EdgeKind, FAMILIES, Filter, Health, Kind, Local, Map, NextStep, family_label,
};
use openagents_chat_app::visual;
use rust_native::style::{Color, Space, Style, TextAlign, TextWeight};
use rust_native::{Axis, Element, Node, TextRole};
use rust_native_desktop::input::SurfaceInput;
use rust_native_desktop::text::{Fonts, font};
use rust_native_desktop::{Frame, PxRect};

use crate::model::Intent;

/// The surface the graph paints into.
pub const RESOURCE: &str = "route-map";
/// How long the camera eases to a new view.
pub const EASE: Duration = Duration::from_millis(260);
/// How often the host asks for a frame while the camera moves.
pub const FRAME: Duration = Duration::from_millis(16);
/// Two clicks this close in time and place are a double-click.
const DOUBLE: Duration = Duration::from_millis(400);
/// A press that moves less than this is a click, not a drag, in points.
const SLOP: f32 = 4.0;
/// The most rows one page of the Gaps panel shows.
pub const GAPS_PER_PAGE: usize = 6;
/// The most members the inspector lists before "and N more".
const MEMBERS_SHOWN: usize = 8;
/// The most records the inspector lists for a plugin.
const RECORDS_SHOWN: usize = 8;
/// The toolbar's height, in points.
const TOOLBAR: f32 = 40.0;
/// What sits above and below the page in the window, in points: the
/// titlebar, the content's footer, and their margins.
const CHROME: f32 = 92.0;

pub use crate::map_action::{Action, Panel};

/// What the window does for a next step, after the tap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Start a new chat with this message in the composer, unsent.
    Chat(String),
    /// Copy a command.
    Copy(String),
    /// Open a web page.
    Open(String),
    /// Open Settings' Coder page.
    Settings,
}

/// The kinds the kind filter cycles through: everything, then each member
/// kind alone.
const KIND_CYCLE: [Option<Kind>; 6] = [
    None,
    Some(Kind::Answer),
    Some(Kind::Knowledge),
    Some(Kind::Plugin),
    Some(Kind::Engine),
    Some(Kind::Screen),
];

/// The Map page.
pub struct MapPage {
    map: Map,
    layout: Layout,
    camera: Camera,
    /// An eased move: from, to, and when it started.
    motion: Option<(Camera, Camera, Instant)>,
    /// The surface has been fitted once.
    fitted: bool,
    /// The person moved the camera since it was last fitted; until they
    /// do, a resize fits the map again.
    moved: bool,
    /// The surface's size in points as last laid out or painted.
    size: (f32, f32),
    /// Pixels a point.
    unit: f32,
    selected: Option<usize>,
    hover: Option<usize>,
    panel: Panel,
    filter: Filter,
    kind_at: usize,
    expanded: BTreeSet<usize>,
    legend: bool,
    gaps_page: usize,
    /// Whether keys go to the map (after a click on it, or a selection
    /// from the keyboard); Esc with nothing selected gives them back.
    focused: bool,
    press: Option<Press>,
    last_click: Option<(Instant, Point)>,
    reduce_motion: bool,
    version: u64,
    fonts: Fonts,
    effects: Vec<Effect>,
    /// A line the page shows after a step: "Copied …".
    notice: Option<String>,
    /// What the local inputs were when the map was built, so the window
    /// rebuilds it only when they change.
    local_key: String,
    /// The width last given to the surface, in points.
    surface_width: std::cell::Cell<f32>,
    /// The window's height in points.
    window_height: f32,
    /// How far the side panel is scrolled, in points.
    side_offset: f32,
    /// The surface's left and right edges in the window, in points, as
    /// last painted: the side panel is to their right.
    surface_right: f32,
    /// Shown in a deck slide ([`MapPage::presenting`]): no hint or legend,
    /// and the selection's details on a card over the graph.
    presenting: bool,
    /// How far each node has appeared, 0 to 1, while a scene drives the
    /// map ([`MapPage::set_frame`]); `None` shows every node whole.
    shown: Option<Vec<f32>>,
    /// The traffic a scene draws on the edges.
    traffic: Vec<Pulse>,
    /// A message's way through the map, lit while a slide's chat plays
    /// ([`MapPage::set_light`]).
    light: Option<RouteLight>,
}

/// One message's way through the map: from the router down to what serves
/// it, lit up to a moving head, the target glowing once the head arrives.
#[derive(Clone, Debug, PartialEq)]
pub struct RouteLight {
    /// The nodes it passes, the router first and the target last.
    pub path: Vec<usize>,
    /// How far along the path the head is, 0 to 1, by distance.
    pub head: f32,
    /// How bright the target glows, 0 to 1.
    pub glow: f32,
    /// The whole light's opacity, 0 to 1, as it fades.
    pub fade: f32,
    /// Nothing serves it yet: the way lights in the gap's red, and the
    /// target, dim, gets dashed rings instead of a glow.
    pub missing: bool,
}

/// A dot of traffic on the map: a request on its way out, or a payment on
/// its way back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pulse {
    /// Where it is, in world units.
    pub at: Point,
    pub color: Color,
    /// Its radius, in points.
    pub radius: f32,
    /// A ring around it: a bonus share (`scene: routes-live`).
    pub ring: bool,
}

#[derive(Clone, Copy, Debug)]
struct Press {
    at: Point,
    last: Point,
    moved: bool,
}

impl MapPage {
    /// The page over `map`, with nothing selected, on the Gaps panel.
    pub fn new(map: Map, reduce_motion: bool) -> Self {
        let layout = Layout::of(&map);
        let front = map.find("front");
        let mut expanded = BTreeSet::new();
        expanded.extend(front);
        for (index, node) in map.nodes.iter().enumerate() {
            if node.kind == Kind::Family {
                expanded.insert(index);
            }
        }
        MapPage {
            map,
            layout,
            camera: Camera::default(),
            motion: None,
            fitted: false,
            moved: false,
            size: (0.0, 0.0),
            unit: 1.0,
            selected: None,
            hover: None,
            panel: Panel::Gaps,
            filter: Filter::default(),
            kind_at: 0,
            expanded,
            legend: true,
            gaps_page: 0,
            focused: true,
            press: None,
            last_click: None,
            reduce_motion,
            version: 1,
            fonts: Fonts::new(),
            effects: Vec::new(),
            notice: None,
            local_key: String::new(),
            surface_width: std::cell::Cell::new(0.0),
            window_height: 840.0,
            side_offset: 0.0,
            surface_right: f32::MAX,
            presenting: false,
            shown: None,
            traffic: Vec::new(),
            light: None,
        }
    }

    /// The page as a deck's slide shows it (`scene: routes`): the same
    /// graph and controls, with no hint or legend over it, and the
    /// selection's details on a card in the graph's corner, since the
    /// slide has no side panel.
    pub fn presenting(map: Map, reduce_motion: bool) -> Self {
        let mut page = MapPage::new(map, reduce_motion);
        page.presenting = true;
        page.legend = false;
        page
    }

    /// Whether a press on the graph is held: a click or a drag under way.
    pub fn pressed(&self) -> bool {
        self.press.is_some()
    }

    /// Whether the graph is being dragged.
    pub fn dragging(&self) -> bool {
        self.press.is_some_and(|press| press.moved)
    }

    /// Draws one frame of a scene over this map (`scene: routes-future`):
    /// where each node sits now, how far each has appeared (0 hides it,
    /// 1 is whole), the traffic on the edges, and the camera.
    pub fn set_frame(
        &mut self,
        layout: Layout,
        shown: Vec<f32>,
        traffic: Vec<Pulse>,
        camera: Camera,
    ) {
        self.layout = layout;
        self.shown = Some(shown);
        self.traffic = traffic;
        self.camera = camera;
        self.motion = None;
        self.fitted = true;
        self.moved = true;
        self.changed();
    }

    /// Lights a message's way through the map, or clears it, leaving the
    /// camera, the selection, and the input as they are.
    pub fn set_light(&mut self, light: Option<RouteLight>) {
        if self.light != light {
            self.light = light;
            self.changed();
        }
    }

    /// The way lit through the map, if any.
    pub fn light(&self) -> Option<&RouteLight> {
        self.light.as_ref()
    }

    /// How many of the lit path's nodes the head has reached, by distance
    /// along it in the map's own units.
    fn light_reached(&self) -> usize {
        let Some(light) = &self.light else {
            return 0;
        };
        let at = |i: usize| self.layout.positions[light.path[i]];
        let lengths: Vec<f32> = (1..light.path.len())
            .map(|i| at(i - 1).distance(at(i)))
            .collect();
        let total: f32 = lengths.iter().sum();
        let head = light.head.clamp(0.0, 1.0) * total;
        let mut walked = 0.0;
        let mut reached = 1;
        for length in lengths {
            walked += length;
            if walked <= head + 0.01 {
                reached += 1;
            } else {
                break;
            }
        }
        reached.min(light.path.len())
    }

    /// The camera that fits `bounds` in the surface as last painted.
    pub fn fit_bounds(&self, bounds: (Point, Point), margin: f32) -> Camera {
        let (w, h) = self.size;
        Camera::fit(bounds, w.max(1.0), h.max(1.0), margin)
    }

    /// The surface's size in points as last painted.
    pub fn size(&self) -> (f32, f32) {
        self.size
    }

    /// The window's height in points, which the map's height follows.
    pub fn set_window_height(&mut self, height: f32) {
        if height.is_finite() && height > 0.0 {
            self.window_height = height;
        }
    }

    fn surface_height(&self) -> f32 {
        // The toolbar wraps to a second row in a narrow pane.
        let row = self.surface_width.get() + side_width(self.surface_width.get()) + 12.0;
        let toolbar = if row > 1.0 && row < 640.0 {
            2.0 * TOOLBAR - 8.0
        } else {
            TOOLBAR
        };
        (self.window_height - CHROME - toolbar).max(160.0)
    }

    /// A wheel at `x` (window points): scrolls the side panel when the
    /// pointer is over it. Returns whether it did.
    pub fn wheel_side(&mut self, x: f32, lines: f32) -> bool {
        if x < self.surface_right || !lines.is_finite() {
            return false;
        }
        self.side_offset = (self.side_offset - lines * 40.0).clamp(0.0, 4000.0);
        self.changed();
        true
    }

    /// What the local inputs were when the map was built.
    pub fn local_key(&self) -> &str {
        &self.local_key
    }

    pub fn set_local_key(&mut self, key: String) {
        self.local_key = key;
    }

    /// Replaces the map with a fresh build (new local counts, readings, or
    /// records), keeping the view: positions depend only on the tree.
    pub fn refresh(&mut self, map: Map) {
        let selected = self.selected.map(|i| self.map.nodes[i].id.clone());
        self.layout = Layout::of(&map);
        self.selected = selected.and_then(|id| map.find(&id));
        self.map = map;
        self.changed();
    }

    pub fn map(&self) -> &Map {
        &self.map
    }

    pub fn camera(&self) -> Camera {
        self.camera
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn panel(&self) -> Panel {
        self.panel
    }

    pub fn filter(&self) -> &Filter {
        &self.filter
    }

    pub fn focused(&self) -> bool {
        self.focused
    }

    pub fn set_reduce_motion(&mut self, reduce: bool) {
        self.reduce_motion = reduce;
    }

    /// Changes whenever the painting would.
    pub fn version(&self) -> u64 {
        self.version
    }

    fn changed(&mut self) {
        self.version = self.version.wrapping_add(1);
    }

    /// The window's scale, in pixels a point.
    pub fn set_unit(&mut self, unit: f32) {
        if unit.is_finite() && unit > 0.0 && unit != self.unit {
            self.unit = unit;
            self.changed();
        }
    }

    /// The effects the last actions asked for, oldest first.
    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }

    /// The surface's size for a row `available` points wide in a window
    /// `height` points tall: the side panel keeps a third of the row, at
    /// least 220 and at most 320 points.
    pub fn surface_size(&self, available: f32) -> (f32, f32) {
        let h = self.surface_height();
        // The layout asks again with the width it gave the surface: that
        // width is the answer, not a row to split again.
        if (available - self.surface_width.get()).abs() < 0.5 {
            return (available, h);
        }
        let width = (available - side_width(available) - 12.0).max(120.0);
        self.surface_width.set(width);
        (width, h)
    }

    fn visible(&self, node: usize) -> bool {
        self.filter.admits(&self.map, node)
    }

    /// Whether the camera is easing, so the host asks for frames.
    pub fn animating(&self) -> bool {
        self.motion.is_some()
    }

    /// When the host should next call [`MapPage::tick`].
    pub fn next_wake(&self, now: Instant) -> Option<Instant> {
        self.animating().then(|| now + FRAME)
    }

    /// Advances an eased move to `now`. Returns whether anything changed.
    pub fn tick(&mut self, now: Instant) -> bool {
        let Some((from, to, started)) = self.motion else {
            return false;
        };
        let t = now.saturating_duration_since(started).as_secs_f32() / EASE.as_secs_f32();
        if t >= 1.0 {
            self.camera = to;
            self.motion = None;
        } else {
            self.camera = from.toward(&to, crate::slides::ease_out(t));
        }
        self.changed();
        true
    }

    /// Moves the camera to `to`: eased over [`EASE`], or at once under
    /// "Reduce motion".
    fn go(&mut self, to: Camera, now: Instant) {
        self.moved = true;
        if self.reduce_motion {
            self.camera = to;
            self.motion = None;
        } else {
            self.motion = Some((self.camera, to, now));
        }
        self.changed();
    }

    fn fit_camera(&self) -> Camera {
        let (w, h) = self.size;
        Camera::fit(self.layout.bounds(), w.max(1.0), h.max(1.0), 24.0)
    }

    /// Fits the whole map in view.
    pub fn fit(&mut self, now: Instant) {
        let to = self.fit_camera();
        self.go(to, now);
        self.moved = false;
    }

    /// Zooms by `factor` about the surface's center.
    pub fn zoom(&mut self, factor: f32, now: Instant) {
        let (w, h) = self.size;
        let mut to = self.motion.map_or(self.camera, |(_, to, _)| to);
        to.zoom_at(factor, Point::new(w / 2.0, h / 2.0), w, h);
        self.go(to, now);
    }

    /// Brings `node` and its members into view: fitted around them, no
    /// further out than now unless they don't fit, and at least close
    /// enough to name a node with no members.
    pub fn zoom_into(&mut self, node: usize, now: Instant) {
        let (w, h) = (self.size.0.max(1.0), self.size.1.max(1.0));
        let mut members = self.map.children(node);
        members.push(node);
        let mut min = Point::new(f32::MAX, f32::MAX);
        let mut max = Point::new(f32::MIN, f32::MIN);
        for &m in &members {
            let (p, r) = (self.layout.positions[m], self.layout.radii[m] + 60.0);
            min = Point::new(min.x.min(p.x - r), min.y.min(p.y - r));
            max = Point::new(max.x.max(p.x + r), max.y.max(p.y + r));
        }
        let mut to = Camera::fit((min, max), w, h, 32.0);
        if members.len() == 1 {
            to.zoom = self.camera.zoom.clamp(1.6, layout::MAX_ZOOM);
        } else {
            // Close enough that neighboring members are about 20 points
            // apart, so their names don't pile up.
            let mut spacing = f32::MAX;
            for (i, &a) in members.iter().enumerate() {
                for &b in &members[i + 1..] {
                    let d = self.layout.positions[a].distance(self.layout.positions[b]);
                    if d > 0.01 {
                        spacing = spacing.min(d);
                    }
                }
            }
            let needed = if spacing < f32::MAX {
                20.0 / spacing
            } else {
                0.0
            };
            to.zoom = to.zoom.max(needed).min(2.4);
        }
        self.go(to, now);
    }

    /// Selects `node`, shows it in the inspector, and opens it and its
    /// ancestors in the outline.
    pub fn select(&mut self, node: Option<usize>) {
        self.selected = node;
        if let Some(node) = node {
            self.panel = Panel::Inspector;
            self.expanded.insert(node);
            let mut at = self.map.nodes[node].parent;
            while let Some(parent) = at {
                self.expanded.insert(parent);
                at = self.map.nodes[parent].parent;
            }
        }
        self.changed();
    }

    /// Runs a control's action.
    pub fn act(&mut self, action: Action, now: Instant) {
        self.notice = None;
        self.side_offset = 0.0;
        match action {
            Action::Fit => self.fit(now),
            Action::ZoomIn => self.zoom(layout::ZOOM_STEP, now),
            Action::ZoomOut => self.zoom(1.0 / layout::ZOOM_STEP, now),
            Action::Panel { panel } => self.panel = panel,
            Action::Select { node } if node < self.map.nodes.len() => {
                self.select(Some(node));
                self.zoom_into(node, now);
            }
            Action::Select { .. } => {}
            Action::Expand { node } => {
                if !self.expanded.remove(&node) {
                    self.expanded.insert(node);
                }
            }
            Action::Family { family } => {
                self.filter.family = family.filter(|f| FAMILIES.contains(&f.as_str()));
                self.gaps_page = 0;
            }
            Action::Families => {
                let present: Vec<&str> = FAMILIES
                    .into_iter()
                    .filter(|f| self.map.find(&format!("family:{f}")).is_some())
                    .collect();
                let at = self
                    .filter
                    .family
                    .as_deref()
                    .and_then(|f| present.iter().position(|p| *p == f));
                self.filter.family = match at {
                    None => present.first().map(|f| (*f).to_string()),
                    Some(at) => present.get(at + 1).map(|f| (*f).to_string()),
                };
                self.gaps_page = 0;
            }
            Action::Kinds => {
                self.kind_at = (self.kind_at + 1) % KIND_CYCLE.len();
                self.filter.kinds = KIND_CYCLE[self.kind_at].into_iter().collect();
                self.gaps_page = 0;
            }
            Action::GapsOnly => {
                self.filter.gaps_only = !self.filter.gaps_only;
                self.gaps_page = 0;
            }
            Action::UnmeasuredOnly => {
                self.filter.unmeasured_only = !self.filter.unmeasured_only;
                self.gaps_page = 0;
            }
            Action::Legend => self.legend = !self.legend,
            Action::GapsPage { page } => self.gaps_page = page,
            Action::Step { node, step } if node < self.map.nodes.len() => {
                if let Some(step) = self.map.inspect(node).steps.get(step).cloned() {
                    self.run(&step);
                }
            }
            Action::Step { .. } => {}
            Action::GapStep { gap } => {
                if let Some(step) = self.map.gaps.get(gap).map(|g| g.step.clone()) {
                    self.run(&step);
                }
            }
            Action::Link { node, field } if node < self.map.nodes.len() => {
                if let Some(link) = self
                    .map
                    .inspect(node)
                    .fields
                    .get(field)
                    .and_then(|f| f.link.clone())
                {
                    self.effects.push(Effect::Open(link.target.url()));
                }
            }
            Action::Link { .. } => {}
        }
        self.changed();
    }

    fn run(&mut self, step: &NextStep) {
        match step {
            NextStep::Chat { message, .. } => self.effects.push(Effect::Chat(message.clone())),
            NextStep::Command { command, doc, .. } => {
                self.effects.push(Effect::Copy(command.clone()));
                self.notice = Some(format!("Copied the command. How it works: {doc}"));
            }
            NextStep::Issue { url, .. } => self.effects.push(Effect::Open(url.clone())),
            NextStep::SignIn { .. } => self.effects.push(Effect::Settings),
        }
    }

    /// Pointer input on [`RESOURCE`], in points relative to it.
    pub fn input(&mut self, event: SurfaceInput, now: Instant) -> bool {
        let (w, h) = self.size;
        match event {
            SurfaceInput::Down { x, y, .. } => {
                self.focused = true;
                let at = Point::new(x, y);
                self.press = Some(Press {
                    at,
                    last: at,
                    moved: false,
                });
                true
            }
            SurfaceInput::Move { x, y } => {
                let at = Point::new(x, y);
                if let Some(press) = &mut self.press {
                    if press.moved || press.at.distance(at) > SLOP {
                        press.moved = true;
                        let (dx, dy) = (at.x - press.last.x, at.y - press.last.y);
                        press.last = at;
                        self.motion = None;
                        self.moved = true;
                        self.camera.pan(dx, dy);
                        self.changed();
                    }
                    return true;
                }
                let hover = layout::hit(&self.layout, &self.camera, at, w, h, &|i| self.visible(i));
                if hover != self.hover {
                    self.hover = hover;
                    self.changed();
                }
                true
            }
            SurfaceInput::Up { x, y } => {
                let Some(press) = self.press.take() else {
                    return false;
                };
                if press.moved {
                    return true;
                }
                let at = Point::new(x, y);
                let hit = layout::hit(&self.layout, &self.camera, at, w, h, &|i| self.visible(i));
                let double = self.last_click.is_some_and(|(when, where_)| {
                    now.saturating_duration_since(when) <= DOUBLE && where_.distance(at) <= 6.0
                });
                self.last_click = Some((now, at));
                if double {
                    self.last_click = None;
                    match hit {
                        Some(node) => self.zoom_into(node, now),
                        None => {
                            let mut to = self.camera;
                            to.zoom_at(layout::ZOOM_STEP * layout::ZOOM_STEP, at, w, h);
                            self.go(to, now);
                        }
                    }
                } else {
                    self.select(hit);
                }
                true
            }
            SurfaceInput::Wheel { dx, dy, .. } => {
                self.motion = None;
                self.moved = true;
                self.camera.pan(dx, dy);
                self.changed();
                true
            }
            SurfaceInput::Zoom { x, y, factor } => {
                if factor.is_finite() && factor > 0.0 {
                    self.motion = None;
                    self.moved = true;
                    self.camera.zoom_at(factor, Point::new(x, y), w, h);
                    self.changed();
                }
                true
            }
        }
    }

    /// Answers a key while the page shows. Returns whether the map took
    /// it. Cmd or Ctrl with `=`/`+`, `-`, or `0` zoom and fit whenever the
    /// page shows; arrows, Tab, Enter, and Esc only while the map has the
    /// keys.
    pub fn key(&mut self, key: &str, command: bool, shift: bool, now: Instant) -> bool {
        if command {
            match key {
                "=" | "+" => self.zoom(layout::ZOOM_STEP, now),
                "-" => self.zoom(1.0 / layout::ZOOM_STEP, now),
                "0" => self.fit(now),
                _ => return false,
            }
            return true;
        }
        if !self.focused {
            return false;
        }
        let visible = |i: usize| self.filter.admits(&self.map, i);
        let from = self.selected.or_else(|| self.map.find("front"));
        match key {
            "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown" => {
                let direction = match key {
                    "ArrowLeft" => Direction::Left,
                    "ArrowRight" => Direction::Right,
                    "ArrowUp" => Direction::Up,
                    _ => Direction::Down,
                };
                if let Some(to) =
                    from.and_then(|from| layout::step(&self.layout, from, direction, &visible))
                {
                    self.select(Some(to));
                    self.follow(to, now);
                }
            }
            "Tab" => {
                let order: Vec<usize> = self
                    .map
                    .outline()
                    .into_iter()
                    .filter(|&i| visible(i))
                    .collect();
                if order.is_empty() {
                    return true;
                }
                let at = from.and_then(|f| order.iter().position(|&i| i == f));
                let next = match (at, shift) {
                    (None, _) => 0,
                    (Some(at), false) => (at + 1) % order.len(),
                    (Some(at), true) => (at + order.len() - 1) % order.len(),
                };
                self.select(Some(order[next]));
                self.follow(order[next], now);
            }
            "Enter" | "Space" | " " => {
                if let Some(node) = self.selected {
                    self.zoom_into(node, now);
                }
            }
            "Escape" => match self.selected {
                Some(node) => {
                    let parent = self.map.nodes[node].parent;
                    self.select(parent);
                    if let Some(parent) = parent {
                        self.follow(parent, now);
                    } else {
                        self.fit(now);
                    }
                }
                None => {
                    self.focused = false;
                    self.changed();
                }
            },
            _ => return false,
        }
        true
    }

    /// Keeps `node` on screen: moves the camera only when it is outside
    /// the middle of the view.
    fn follow(&mut self, node: usize, now: Instant) {
        let (w, h) = self.size;
        let p = self.camera.to_screen(self.layout.positions[node], w, h);
        if p.x < w * 0.15 || p.x > w * 0.85 || p.y < h * 0.15 || p.y > h * 0.85 {
            let to = Camera {
                center: self.layout.positions[node],
                zoom: self.camera.zoom,
            };
            self.go(to, now);
        }
    }

    /// Paints the map into `rect` of `frame`, in pixels.
    pub fn paint(&mut self, frame: &mut Frame, rect: PxRect) {
        let unit = self.unit.max(0.1);
        let size = (rect.w / unit, rect.h / unit);
        if size != self.size {
            self.size = size;
            if self.fitted && !self.moved && self.motion.is_none() {
                self.camera = self.fit_camera();
            }
        }
        self.surface_right = (rect.x + rect.w) / unit;
        if !self.fitted && size.0 > 1.0 && size.1 > 1.0 {
            self.camera = self.fit_camera();
            self.fitted = true;
        }
        let clip = frame.clip_to(rect);
        frame.fill(rect, 0.0, visual::current().canvas);
        let (w, h) = self.size;
        let camera = self.camera;
        let px = |p: Point| {
            let s = camera.to_screen(p, w, h);
            (rect.x + s.x * unit, rect.y + s.y * unit)
        };
        let on_screen = |(x, y): (f32, f32), r: f32| {
            x + r >= rect.x
                && x - r <= rect.x + rect.w
                && y + r >= rect.y
                && y - r <= rect.y + rect.h
        };
        let lit = self.lit();
        let appeared = self.shown.clone();
        let appear = |i: usize| {
            appeared
                .as_ref()
                .map_or(1.0, |a| a.get(i).copied().unwrap_or(1.0))
        };
        // Edges: the tree under every node, then the other links, fainter.
        for edge in &self.map.edges {
            let (a, b) = (edge.from, edge.to);
            let grown = appear(a).min(appear(b));
            if grown <= 0.0 {
                continue;
            }
            let shown = self.visible(a) && self.visible(b);
            let (pa, pb) = (px(self.layout.positions[a]), px(self.layout.positions[b]));
            if !segment_visible(pa, pb, rect) {
                continue;
            }
            let bright = lit.contains(&a) && lit.contains(&b);
            let mut color = visual::map::current().edge;
            color.alpha = match (edge.kind, bright, shown) {
                (_, _, false) => 10,
                (_, true, true) => 140,
                (
                    EdgeKind::Groups
                    | EdgeKind::ServedBy
                    | EdgeKind::HandsOff
                    | EdgeKind::Admits
                    | EdgeKind::Opens,
                    false,
                    true,
                ) => 40,
                (EdgeKind::Tests, false, true) => 18,
            };
            color.alpha = (f32::from(color.alpha) * grown).round() as u8;
            if edge.kind == EdgeKind::Tests && !bright {
                dashed(frame, pa, pb, unit, color);
            } else {
                thin_line(frame, pa, pb, unit * if bright { 1.6 } else { 1.0 }, color);
            }
        }
        // Nodes.
        for index in 0..self.map.nodes.len() {
            let node = &self.map.nodes[index];
            let grown = appear(index);
            if grown <= 0.0 {
                continue;
            }
            let center = px(self.layout.positions[index]);
            let r = (self.layout.radii[index] * camera.zoom).max(2.5) * unit * grown;
            if !on_screen(center, r + 12.0 * unit) {
                continue;
            }
            let shown = self.visible(index);
            let mut fill = node.kind.color();
            if !shown {
                fill.alpha = 46;
            }
            fill.alpha = (f32::from(fill.alpha) * grown.min(1.0)).round() as u8;
            let disc = PxRect {
                x: center.0 - r,
                y: center.1 - r,
                w: 2.0 * r,
                h: 2.0 * r,
            };
            frame.fill(disc, r, fill);
            if shown {
                match node.health {
                    Health::Good => {}
                    Health::Weak => frame.stroke(
                        grow(disc, 2.5 * unit),
                        r + 2.5 * unit,
                        2.0 * unit,
                        visual::map::current().weak,
                    ),
                    Health::Unmeasured => {
                        dashed_ring(frame, center, r + 2.5 * unit, unit, visual::current().muted)
                    }
                }
            }
            if Some(index) == self.selected {
                frame.stroke(
                    grow(disc, 5.0 * unit),
                    r + 5.0 * unit,
                    2.0 * unit,
                    visual::current().text,
                );
            } else if Some(index) == self.hover {
                frame.stroke(
                    grow(disc, 4.0 * unit),
                    r + 4.0 * unit,
                    1.0 * unit,
                    visual::current().muted,
                );
            }
            if shown && !node.gaps.is_empty() {
                let m = (3.5 * unit).max(r * 0.38);
                let at = (center.0 + r * 0.72, center.1 - r * 0.72);
                frame.fill(
                    PxRect {
                        x: at.0 - m,
                        y: at.1 - m,
                        w: 2.0 * m,
                        h: 2.0 * m,
                    },
                    m,
                    visual::map::current().gap,
                );
            }
        }
        self.paint_light(frame, &px, unit);
        // Traffic, over the edges it travels: a soft glow and a bright core.
        for pulse in &self.traffic {
            let center = px(pulse.at);
            let r = pulse.radius * unit;
            if !on_screen(center, r * 3.0) {
                continue;
            }
            let glow = r * 2.6;
            frame.fill(
                PxRect {
                    x: center.0 - glow,
                    y: center.1 - glow,
                    w: 2.0 * glow,
                    h: 2.0 * glow,
                },
                glow,
                Color {
                    alpha: 46,
                    ..pulse.color
                },
            );
            frame.fill(
                PxRect {
                    x: center.0 - r,
                    y: center.1 - r,
                    w: 2.0 * r,
                    h: 2.0 * r,
                },
                r,
                pulse.color,
            );
            if pulse.ring {
                let ring = r * 2.4;
                frame.stroke(
                    PxRect {
                        x: center.0 - ring,
                        y: center.1 - ring,
                        w: 2.0 * ring,
                        h: 2.0 * ring,
                    },
                    ring,
                    (0.9 * unit).max(1.0),
                    pulse.color,
                );
            }
        }
        // Labels, thinned by zoom; a selection's neighbors are always named.
        // Each label sits outside its node, away from the front, so the
        // rings read like a radial tree; one that would overlap a label
        // already placed waits for a closer zoom. The selection and the
        // shallowest nodes place first. A scene's frame labels only what
        // is lit: its slide carries its own words.
        let mut order: Vec<usize> = if self.shown.is_some() {
            lit.iter().copied().collect()
        } else {
            (0..self.map.nodes.len()).collect()
        };
        order.sort_by_key(|&i| {
            let focus = Some(i) == self.selected || Some(i) == self.hover;
            (!focus, !lit.contains(&i), self.map.nodes[i].depth, i)
        });
        let mut placed: Vec<PxRect> = Vec::new();
        for index in order {
            if (!self.visible(index) && Some(index) != self.selected) || appear(index) < 0.999 {
                continue;
            }
            let node = &self.map.nodes[index];
            let focus = Some(index) == self.selected || Some(index) == self.hover;
            let detail = if focus {
                Detail::Evidence
            } else if lit.contains(&index) {
                match layout::detail(node.kind, node.depth, camera.zoom) {
                    Detail::Evidence => Detail::Evidence,
                    _ => Detail::Name,
                }
            } else {
                layout::detail(node.kind, node.depth, camera.zoom)
            };
            if detail == Detail::None {
                continue;
            }
            let center = px(self.layout.positions[index]);
            let r = (self.layout.radii[index] * camera.zoom).max(2.5) * unit;
            if !on_screen(center, r + 200.0 * unit) {
                continue;
            }
            let size = match node.kind {
                Kind::Front => 13.0,
                Kind::Family | Kind::Route | Kind::Coder => 12.0,
                _ => 11.0,
            } * unit;
            let weight = if matches!(node.kind, Kind::Front | Kind::Family) {
                rust_native::layout::display::Weight::Semibold
            } else {
                rust_native::layout::display::Weight::Medium
            };
            let mut label = node.label.clone();
            if node.showcase {
                label.push_str(" · example to copy");
            }
            let width = 190.0 * unit;
            let name = self
                .fonts
                .paragraph(&label, font(size, weight, false), Some(width));
            let evidence = (detail == Detail::Evidence)
                .then(|| self.map.evidence(index))
                .flatten()
                .map(|line| {
                    self.fonts.paragraph(
                        &line,
                        font(
                            10.5 * unit,
                            rust_native::layout::display::Weight::Regular,
                            false,
                        ),
                        Some(width),
                    )
                });
            let block = name.height + evidence.as_ref().map_or(0.0, |e| e.height + 2.0 * unit);
            let world = self.layout.positions[index];
            let length = (world.x * world.x + world.y * world.y).sqrt();
            let (cos, sin) = if node.depth == 0 || length < 1.0 {
                (0.0, 1.0)
            } else {
                (world.x / length, world.y / length)
            };
            let gap = r + 5.0 * unit;
            let (x, y, align) = if cos > 0.4 {
                (
                    center.0 + cos * gap,
                    center.1 + sin * gap - block / 2.0,
                    TextAlign::Start,
                )
            } else if cos < -0.4 {
                (
                    center.0 + cos * gap - width,
                    center.1 + sin * gap - block / 2.0,
                    TextAlign::End,
                )
            } else if sin >= 0.0 {
                (center.0 - width / 2.0, center.1 + gap, TextAlign::Center)
            } else {
                (
                    center.0 - width / 2.0,
                    center.1 - gap - block,
                    TextAlign::Center,
                )
            };
            let left = match align {
                TextAlign::Start => x,
                TextAlign::End => x + width - name.width,
                TextAlign::Center => x + (width - name.width) / 2.0,
            };
            // Keep the label inside the surface: slide it in from an edge.
            let shift = (rect.x + 4.0 * unit - left).max(0.0)
                - (left + name.width - (rect.x + rect.w - 4.0 * unit)).max(0.0);
            let x = x + shift;
            let taken = PxRect {
                x: left + shift,
                y,
                w: name.width,
                h: block,
            };
            if !focus && placed.iter().any(|p| overlaps(*p, taken)) {
                continue;
            }
            placed.push(taken);
            let ink = if focus {
                visual::current().text
            } else {
                Color {
                    alpha: 225,
                    ..visual::current().text
                }
            };
            self.fonts.draw(frame, &name, x, y, width, align, 1.0, ink);
            if let Some(evidence) = evidence {
                self.fonts.draw(
                    frame,
                    &evidence,
                    x,
                    y + name.height + 2.0 * unit,
                    width,
                    align,
                    1.0,
                    visual::current().muted,
                );
            }
        }
        if self.legend {
            self.paint_legend(frame, rect, unit);
        }
        if self.presenting {
            self.paint_card(frame, rect, unit);
            frame.restore_clip(clip);
            return;
        }
        // The hint, top left.
        let hint = if self.focused {
            "Drag to move · pinch or Cmd+scroll to zoom · arrows, Tab, Esc"
        } else {
            "Click the map to use the keys"
        };
        let small = 11.0 * unit;
        let paragraph = self.fonts.paragraph(
            hint,
            font(small, rust_native::layout::display::Weight::Regular, false),
            Some(rect.w - 24.0 * unit),
        );
        self.fonts.draw(
            frame,
            &paragraph,
            rect.x + 12.0 * unit,
            rect.y + 10.0 * unit,
            rect.w - 24.0 * unit,
            TextAlign::Start,
            1.0,
            visual::current().faint,
        );
        frame.restore_clip(clip);
    }

    /// A message's way: the edges behind the head bright in the target's
    /// color over a soft wash, a ring on each node passed, the head as a
    /// moving pulse, and the target glowing once it arrives.
    fn paint_light(&self, frame: &mut Frame, px: &dyn Fn(Point) -> (f32, f32), unit: f32) {
        let Some(light) = &self.light else {
            return;
        };
        let fade = light.fade.clamp(0.0, 1.0);
        if fade <= 0.0 || light.path.is_empty() {
            return;
        }
        let target = *light.path.last().unwrap_or(&0);
        let tint = if light.missing {
            visual::map::current().gap
        } else {
            self.map.nodes[target].kind.color()
        };
        let alpha = |a: f32| Color {
            alpha: (a * fade).round().clamp(0.0, 255.0) as u8,
            ..tint
        };
        let points: Vec<(f32, f32)> = light
            .path
            .iter()
            .map(|&i| px(self.layout.positions[i]))
            .collect();
        let world = |i: usize| self.layout.positions[light.path[i]];
        let lengths: Vec<f32> = (1..light.path.len())
            .map(|i| world(i - 1).distance(world(i)))
            .collect();
        let total: f32 = lengths.iter().sum();
        let mut left = light.head.clamp(0.0, 1.0) * total;
        let mut head = points[0];
        for (i, length) in lengths.iter().enumerate() {
            if left <= 0.0 {
                break;
            }
            let (a, b) = (points[i], points[i + 1]);
            let t = if *length > 0.0 {
                (left / length).min(1.0)
            } else {
                1.0
            };
            let end = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            thin_line(frame, a, end, 9.0 * unit, alpha(34.0));
            thin_line(frame, a, end, 3.0 * unit, alpha(235.0));
            head = end;
            left -= length;
        }
        let radius = |i: usize| (self.layout.radii[i] * self.camera.zoom).max(2.5) * unit;
        for (step, &index) in light.path.iter().enumerate().take(self.light_reached()) {
            let (x, y) = points[step];
            let r = radius(index) + 3.5 * unit;
            frame.stroke(
                PxRect {
                    x: x - r,
                    y: y - r,
                    w: 2.0 * r,
                    h: 2.0 * r,
                },
                r,
                1.5 * unit,
                alpha(200.0),
            );
        }
        let glow = light.glow.clamp(0.0, 1.0);
        if glow > 0.0 && light.missing {
            // A gap: the target dimmed, with dashed rings around it.
            let (x, y) = *points.last().unwrap_or(&head);
            let base = radius(target);
            let r = base + 2.0 * unit;
            frame.fill(
                PxRect {
                    x: x - r,
                    y: y - r,
                    w: 2.0 * r,
                    h: 2.0 * r,
                },
                r,
                Color {
                    alpha: (170.0 * glow * fade) as u8,
                    ..visual::current().canvas
                },
            );
            for (grow_by, a) in [(6.0, 255.0), (14.0, 150.0), (23.0, 80.0)] {
                dashed_ring(
                    frame,
                    (x, y),
                    base + grow_by * unit * glow,
                    unit * 1.4,
                    alpha(a * glow),
                );
            }
        } else if glow > 0.0 {
            let (x, y) = *points.last().unwrap_or(&head);
            let base = radius(target);
            for (grow_by, a) in [(22.0, 26.0), (13.0, 48.0), (6.0, 90.0)] {
                let r = base + grow_by * unit * glow;
                frame.fill(
                    PxRect {
                        x: x - r,
                        y: y - r,
                        w: 2.0 * r,
                        h: 2.0 * r,
                    },
                    r,
                    alpha(a * glow),
                );
            }
            let r = base + 5.0 * unit;
            frame.stroke(
                PxRect {
                    x: x - r,
                    y: y - r,
                    w: 2.0 * r,
                    h: 2.0 * r,
                },
                r,
                2.0 * unit,
                alpha(255.0 * glow),
            );
        }
        if light.head < 1.0 {
            let (x, y) = head;
            for (r, a) in [(13.0, 40.0), (7.0, 110.0), (4.0, 255.0)] {
                let r = r * unit;
                let color = if a >= 255.0 {
                    Color {
                        alpha: (255.0 * fade) as u8,
                        ..visual::current().text
                    }
                } else {
                    alpha(a)
                };
                frame.fill(
                    PxRect {
                        x: x - r,
                        y: y - r,
                        w: 2.0 * r,
                        h: 2.0 * r,
                    },
                    r,
                    color,
                );
            }
        }
    }

    /// The selection's details on a card in the graph's top right corner,
    /// for a deck slide, which has no side panel: what the inspector
    /// leads with (the name, kind, what it is, why the router sends things
    /// there) and its first facts.
    fn paint_card(&mut self, frame: &mut Frame, rect: PxRect, unit: f32) {
        use rust_native::layout::display::Weight;
        let Some(node) = self.selected else {
            return;
        };
        let inspector = self.map.inspect(node);
        let pad = 14.0 * unit;
        let width = (300.0 * unit).min(rect.w * 0.42);
        let inner = width - 2.0 * pad;
        let mut lines = vec![
            (
                inspector.title.clone(),
                15.0,
                Weight::Semibold,
                visual::current().text,
            ),
            (
                inspector.kind.label().to_string(),
                11.0,
                Weight::Medium,
                inspector.kind.color(),
            ),
            (
                inspector.line.clone(),
                12.0,
                Weight::Regular,
                visual::current().muted,
            ),
        ];
        if let Some(why) = &inspector.why {
            lines.push((why.clone(), 12.0, Weight::Regular, visual::current().muted));
        }
        for field in inspector.fields.iter().take(6) {
            lines.push((
                format!("{}  {}", field.label, field.value),
                11.5,
                Weight::Regular,
                visual::current().text,
            ));
        }
        let mut paragraphs = Vec::new();
        let mut height = 2.0 * pad;
        for (value, size, weight, color) in lines {
            let paragraph =
                self.fonts
                    .paragraph(&value, font(size * unit, weight, false), Some(inner));
            if height + paragraph.height > rect.h - 24.0 * unit {
                break;
            }
            height += paragraph.height + 6.0 * unit;
            paragraphs.push((paragraph, color));
        }
        let card = PxRect {
            x: rect.x + rect.w - width - 12.0 * unit,
            y: rect.y + 12.0 * unit,
            w: width,
            h: height - 6.0 * unit,
        };
        frame.fill(
            card,
            10.0 * unit,
            Color {
                alpha: 235,
                ..visual::current().sidebar
            },
        );
        frame.stroke(card, 10.0 * unit, unit, visual::current().border);
        let mut y = card.y + pad;
        for (paragraph, color) in paragraphs {
            self.fonts.draw(
                frame,
                &paragraph,
                card.x + pad,
                y,
                inner,
                TextAlign::Start,
                1.0,
                color,
            );
            y += paragraph.height + 6.0 * unit;
        }
    }

    fn paint_legend(&mut self, frame: &mut Frame, rect: PxRect, unit: f32) {
        let rows: Vec<(Color, &str, bool)> = Kind::ALL
            .iter()
            .map(|k| (k.color(), k.label(), false))
            .chain([
                (visual::map::current().weak, "Ring: weak", true),
                (visual::current().muted, "Dashed: not measured", true),
                (visual::map::current().gap, "Red dot: a gap", false),
            ])
            .collect();
        let line = 16.0 * unit;
        let column = 150.0 * unit;
        let per = rows.len().div_ceil(2);
        let box_w = 2.0 * column + 8.0 * unit;
        let box_h = line * per as f32 + 14.0 * unit;
        if box_h > rect.h * 0.6 || box_w > rect.w * 0.9 {
            return;
        }
        let panel = PxRect {
            x: rect.x + 10.0 * unit,
            y: rect.y + rect.h - box_h - 10.0 * unit,
            w: box_w,
            h: box_h,
        };
        frame.fill(
            panel,
            8.0 * unit,
            Color {
                alpha: 230,
                ..visual::current().sidebar
            },
        );
        frame.stroke(panel, 8.0 * unit, unit, visual::current().border);
        for (row, (color, label, ring)) in rows.into_iter().enumerate() {
            let cy = panel.y + 7.0 * unit + line * (row % per) as f32 + line / 2.0;
            let cx = panel.x + 14.0 * unit + column * (row / per) as f32;
            let r = 4.5 * unit;
            let disc = PxRect {
                x: cx - r,
                y: cy - r,
                w: 2.0 * r,
                h: 2.0 * r,
            };
            if ring {
                if label.starts_with("Dashed") {
                    dashed_ring(frame, (cx, cy), r, unit, color);
                } else {
                    frame.stroke(disc, r, 1.5 * unit, color);
                }
            } else {
                frame.fill(disc, r, color);
            }
            let size = 11.0 * unit;
            let paragraph = self.fonts.paragraph(
                label,
                font(size, rust_native::layout::display::Weight::Regular, false),
                None,
            );
            let text_h = size * rust_native_desktop::text::LINE_EM;
            self.fonts.draw(
                frame,
                &paragraph,
                cx + 12.0 * unit,
                cy - text_h / 2.0,
                column - 26.0 * unit,
                TextAlign::Start,
                1.0,
                visual::current().muted,
            );
        }
    }

    /// The selected node, its path to the front, and its children: what a
    /// selection lights.
    fn lit(&self) -> BTreeSet<usize> {
        let mut lit = BTreeSet::new();
        if let Some(light) = self.light.as_ref().filter(|light| light.fade > 0.25) {
            lit.extend(light.path.iter().take(self.light_reached()));
        }
        if let Some(node) = self.selected.or(self.hover) {
            lit.insert(node);
            let mut at = self.map.nodes[node].parent;
            while let Some(parent) = at {
                lit.insert(parent);
                at = self.map.nodes[parent].parent;
            }
            lit.extend(self.map.children(node));
            for edge in &self.map.edges {
                if edge.from == node {
                    lit.insert(edge.to);
                }
            }
        }
        lit
    }

    /// What the surface paints, as rows a screen reader reads: every node
    /// by its name, kind, state, and gaps, in the outline's order, with
    /// the bounds of those on screen so a screen reader can select them.
    pub fn access_content(&self) -> rust_native_desktop::access::Content {
        use std::sync::Arc;
        let (w, h) = self.size;
        let mut rows = Vec::new();
        let mut bounds = std::collections::HashMap::new();
        for index in self.map.outline() {
            let key = format!("route-map-node-{index}");
            rows.push(Arc::new(Node {
                key: key.clone(),
                style: Style::default(),
                element: Element::Button {
                    shortcut: None,
                    label: self.map.accessible_name(index),
                    enabled: true,
                    icon: None,
                    intent: (),
                },
            }));
            let p = self.camera.to_screen(self.layout.positions[index], w, h);
            let r = (self.layout.radii[index] * self.camera.zoom).max(6.0);
            if p.x + r >= 0.0 && p.x - r <= w && p.y + r >= 0.0 && p.y - r <= h {
                bounds.insert(
                    key,
                    rust_native_desktop::layout::Rect {
                        x: p.x - r,
                        y: p.y - r,
                        w: 2.0 * r,
                        h: 2.0 * r,
                    },
                );
            }
        }
        for kind in Kind::ALL {
            rows.push(Arc::new(Node {
                key: format!("route-map-legend-{}", kind.word()),
                style: Style::default(),
                element: Element::Text {
                    value: format!("Legend: {} has its own color", kind.label()),
                    role: TextRole::Status,
                },
            }));
        }
        rust_native_desktop::access::Content {
            rows,
            bounds,
            conversation: false,
        }
    }

    /// The page's views: the toolbar, then the surface beside the panel.
    pub fn view(&self) -> Node<Intent> {
        let mut toolbar = vec![
            button("map-fit", "Fit", Action::Fit, false),
            button("map-zoom-out", "Zoom out", Action::ZoomOut, false),
            button("map-zoom-in", "Zoom in", Action::ZoomIn, false),
            button(
                "map-families",
                &format!(
                    "Family: {}",
                    self.filter.family.as_deref().map_or("all", family_label)
                ),
                Action::Families,
                self.filter.family.is_some(),
            ),
        ];
        toolbar.push(button(
            "map-kinds",
            &match KIND_CYCLE[self.kind_at] {
                None => "Kind: all".to_string(),
                Some(kind) => format!("Kind: {}", kind.label()),
            },
            Action::Kinds,
            self.kind_at != 0,
        ));
        toolbar.push(button(
            "map-gaps-only",
            "Gaps only",
            Action::GapsOnly,
            self.filter.gaps_only,
        ));
        toolbar.push(button(
            "map-unmeasured-only",
            "Not measured",
            Action::UnmeasuredOnly,
            self.filter.unmeasured_only,
        ));
        toolbar.push(button("map-legend", "Legend", Action::Legend, self.legend));
        let mut toolbar = stack("map-toolbar", Axis::Wrap, toolbar);
        toolbar.style.gap_points = Some(4);
        let surface = Node {
            key: "route-map-surface".into(),
            style: Style::default(),
            element: Element::Surface {
                resource: RESOURCE.into(),
                label: format!(
                    "The route map: {} routes, {} nodes, {} gaps",
                    self.map
                        .nodes
                        .iter()
                        .filter(|n| n.kind == Kind::Route)
                        .count(),
                    self.map.nodes.len(),
                    self.map.gaps.len()
                ),
            },
        };
        let mut side = stack("map-side", Axis::Vertical, self.side());
        side.style.gap_points = Some(6);
        // As tall as the map, top-aligned beside it, and scrolled by the
        // wheel over it when it holds more.
        let height = self
            .surface_height()
            .round()
            .clamp(1.0, f32::from(u16::MAX)) as u16;
        side.style.min_height = Some(height);
        side.style.viewport = Some(rust_native::style::Viewport {
            max_height: height,
            offset: self.side_offset.round().max(0.0) as u16,
            fade: 12,
        });
        let mut row = stack("map-row", Axis::Horizontal, vec![surface, side]);
        row.style.gap_points = Some(12);
        let mut page = stack("map-page", Axis::Vertical, vec![toolbar, row]);
        page.style.gap_points = Some(8);
        page.style.padding_points = Some([4, 12, 0, 12]);
        page
    }

    fn side(&self) -> Vec<Node<Intent>> {
        let gaps = self.map.gaps_where(&self.filter);
        let mut tabs = stack(
            "map-tabs",
            Axis::Horizontal,
            vec![
                button(
                    "map-tab-inspector",
                    "Details",
                    Action::Panel {
                        panel: Panel::Inspector,
                    },
                    self.panel == Panel::Inspector,
                ),
                button(
                    "map-tab-gaps",
                    &format!("Gaps ({})", gaps.len()),
                    Action::Panel { panel: Panel::Gaps },
                    self.panel == Panel::Gaps,
                ),
                button(
                    "map-tab-outline",
                    "Outline",
                    Action::Panel {
                        panel: Panel::Outline,
                    },
                    self.panel == Panel::Outline,
                ),
            ],
        );
        tabs.style.gap_points = Some(4);
        let mut out = vec![tabs];
        if let Some(notice) = &self.notice {
            out.push(text("map-notice", notice, TextRole::Status));
        }
        match self.panel {
            Panel::Inspector => out.extend(self.inspector()),
            Panel::Gaps => out.extend(self.gaps_panel(&gaps)),
            Panel::Outline => out.extend(self.outline()),
        }
        out
    }

    fn inspector(&self) -> Vec<Node<Intent>> {
        let Some(node) = self.selected else {
            return vec![text(
                "map-inspector-empty",
                "Select a node to see what it is, why the router sends things there, its numbers, and what to fill in.",
                TextRole::Body,
            )];
        };
        let inspector = self.map.inspect(node);
        let n = &self.map.nodes[node];
        let mut out = vec![text(
            "map-inspector-title",
            &inspector.title,
            TextRole::Heading,
        )];
        let mut state = vec![inspector.kind.label().to_string()];
        if let Some(stage) = n.stage {
            state.push(stage.words().to_string());
        } else if matches!(n.kind, Kind::Route | Kind::Engine) {
            state.push(n.health.words().to_string());
        }
        if n.showcase {
            state.push("The example to copy".into());
        }
        out.push(text(
            "map-inspector-kind",
            &state.join(" · "),
            TextRole::Status,
        ));
        if inspector.why.is_none() {
            out.push(text("map-inspector-line", &inspector.line, TextRole::Body));
        }
        if let Some(why) = &inspector.why {
            out.push(text(
                "map-inspector-why-label",
                "Why the router sends things here",
                TextRole::Status,
            ));
            out.push(text("map-inspector-why", why, TextRole::Body));
        }
        let mut records = 0;
        for (index, field) in inspector.fields.iter().enumerate() {
            let record = matches!(field.label.as_str(), "Result" | "Check" | "Validation");
            if record {
                records += 1;
                if records > RECORDS_SHOWN {
                    continue;
                }
            }
            let line = format!("{}: {}", field.label, field.value);
            if field.link.is_some() {
                out.push(link(
                    &format!("map-field-{index}"),
                    &line,
                    Action::Link { node, field: index },
                ));
            } else {
                out.push(text(&format!("map-field-{index}"), &line, TextRole::Body));
            }
        }
        if records > RECORDS_SHOWN {
            out.push(text(
                "map-records-more",
                &format!("and {} more records", records - RECORDS_SHOWN),
                TextRole::Status,
            ));
        }
        for &gap in &inspector.gaps {
            let g = &self.map.gaps[gap];
            out.push(text(
                &format!("map-inspector-gap-{gap}"),
                &format!("Gap: {}. {}", g.title, g.detail),
                TextRole::Body,
            ));
        }
        for (index, step) in inspector.steps.iter().enumerate() {
            out.push(button(
                &format!("map-step-{index}"),
                step.label(),
                Action::Step { node, step: index },
                true,
            ));
        }
        if !inspector.members.is_empty() {
            out.push(text("map-members-label", "Members", TextRole::Status));
            let mut members = inspector.members.clone();
            members.sort_by_key(|(child, _)| match self.map.nodes[*child].kind {
                Kind::Coder => 0,
                Kind::Model => 1,
                Kind::Knowledge => 2,
                Kind::Plugin => 3,
                Kind::Engine => 4,
                Kind::Screen => 5,
                _ => 6,
            });
            for (child, label) in members.iter().take(MEMBERS_SHOWN) {
                out.push(link(
                    &format!("map-member-{child}"),
                    &format!("{} · {}", label, self.map.nodes[*child].kind.label()),
                    Action::Select { node: *child },
                ));
            }
            if inspector.members.len() > MEMBERS_SHOWN {
                out.push(text(
                    "map-members-more",
                    &format!(
                        "and {} more in the outline",
                        inspector.members.len() - MEMBERS_SHOWN
                    ),
                    TextRole::Status,
                ));
            }
        }
        out
    }

    fn gaps_panel(&self, gaps: &[usize]) -> Vec<Node<Intent>> {
        if gaps.is_empty() {
            return vec![text(
                "map-gaps-empty",
                "No gaps match the filter.",
                TextRole::Body,
            )];
        }
        let pages = gaps.len().div_ceil(GAPS_PER_PAGE);
        let page = self.gaps_page.min(pages - 1);
        let mut out = Vec::new();
        for &gap in gaps.iter().skip(page * GAPS_PER_PAGE).take(GAPS_PER_PAGE) {
            let g = &self.map.gaps[gap];
            out.push(link(
                &format!("map-gap-{gap}"),
                &g.title,
                Action::Select { node: g.node },
            ));
            out.push(text(
                &format!("map-gap-{gap}-detail"),
                &g.detail,
                TextRole::Status,
            ));
            out.push(button(
                &format!("map-gap-{gap}-step"),
                g.step.label(),
                Action::GapStep { gap },
                true,
            ));
        }
        if pages > 1 {
            let mut nav = vec![];
            if page > 0 {
                nav.push(button(
                    "map-gaps-previous",
                    "Previous",
                    Action::GapsPage { page: page - 1 },
                    false,
                ));
            }
            nav.push(text(
                "map-gaps-page",
                &format!("{} of {pages}", page + 1),
                TextRole::Status,
            ));
            if page + 1 < pages {
                nav.push(button(
                    "map-gaps-next",
                    "Next",
                    Action::GapsPage { page: page + 1 },
                    false,
                ));
            }
            let mut nav = stack("map-gaps-pages", Axis::Horizontal, nav);
            nav.style.gap_points = Some(6);
            out.push(nav);
        }
        out
    }

    fn outline(&self) -> Vec<Node<Intent>> {
        let mut out = Vec::new();
        for index in self.map.outline() {
            let node = &self.map.nodes[index];
            let mut ancestor = node.parent;
            let mut shown = true;
            while let Some(parent) = ancestor {
                if !self.expanded.contains(&parent) {
                    shown = false;
                    break;
                }
                ancestor = self.map.nodes[parent].parent;
            }
            if !shown || !self.visible(index) && Some(index) != self.selected {
                continue;
            }
            let indent = "   ".repeat(usize::from(node.depth));
            let children = self.map.children(index).len();
            let mut row = vec![link(
                &format!("map-outline-{index}"),
                &format!("{indent}{}", self.map.accessible_name(index)),
                Action::Select { node: index },
            )];
            if children > 0 && node.depth > 0 {
                row.push(button(
                    &format!("map-outline-{index}-expand"),
                    if self.expanded.contains(&index) {
                        "Hide members"
                    } else {
                        "Show members"
                    },
                    Action::Expand { node: index },
                    false,
                ));
            }
            let mut row = stack(&format!("map-outline-row-{index}"), Axis::Horizontal, row);
            row.style.gap_points = Some(4);
            out.push(row);
            if out.len() >= 240 {
                out.push(text(
                    "map-outline-more",
                    "Narrow the filter to see more.",
                    TextRole::Status,
                ));
                break;
            }
        }
        out
    }
}

/// The side panel's width for a row `available` points wide.
#[must_use]
pub fn side_width(available: f32) -> f32 {
    (available * 0.36).clamp(220.0, 320.0)
}

fn overlaps(a: PxRect, b: PxRect) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
}

fn grow(rect: PxRect, by: f32) -> PxRect {
    PxRect {
        x: rect.x - by,
        y: rect.y - by,
        w: rect.w + 2.0 * by,
        h: rect.h + 2.0 * by,
    }
}

/// Whether the segment from `a` to `b` may cross `rect`.
fn segment_visible(a: (f32, f32), b: (f32, f32), rect: PxRect) -> bool {
    !(a.0.max(b.0) < rect.x
        || a.0.min(b.0) > rect.x + rect.w
        || a.1.max(b.1) < rect.y
        || a.1.min(b.1) > rect.y + rect.h)
}

/// An antialiased line about `width` pixels wide, drawn along its length
/// (cost in proportion to its length, not its bounding box), clipped to
/// the frame.
fn thin_line(frame: &mut Frame, a: (f32, f32), b: (f32, f32), width: f32, color: Color) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length = (dx * dx + dy * dy).sqrt();
    if length < 0.5 {
        return;
    }
    let steps = length.ceil() as usize;
    let (nx, ny) = (-dy / length, dx / length);
    let half = (width / 2.0).max(0.5);
    let (fw, fh) = (frame.width as f32, frame.height as f32);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let (x, y) = (a.0 + dx * t, a.1 + dy * t);
        if x < -2.0 || y < -2.0 || x > fw + 2.0 || y > fh + 2.0 {
            continue;
        }
        // Blend the pixels across the line's width at this step.
        let reach = half.ceil() as i64 + 1;
        for k in -reach..=reach {
            let (px, py) = (x + nx * k as f32, y + ny * k as f32);
            let coverage = (half + 0.5 - (k as f32).abs()).clamp(0.0, 1.0);
            if coverage > 0.0 {
                frame.blend(px.floor() as i64, py.floor() as i64, color, coverage / 1.4);
            }
        }
    }
}

/// A dashed line: four on, four off, in points.
fn dashed(frame: &mut Frame, a: (f32, f32), b: (f32, f32), unit: f32, color: Color) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length = (dx * dx + dy * dy).sqrt();
    let dash = 4.0 * unit;
    let mut at = 0.0;
    while at < length {
        let end = (at + dash).min(length);
        let p = |d: f32| (a.0 + dx * d / length, a.1 + dy * d / length);
        thin_line(frame, p(at), p(end), unit, color);
        at += 2.0 * dash;
    }
}

/// A dashed circle of radius `r` around `center`.
fn dashed_ring(frame: &mut Frame, center: (f32, f32), r: f32, unit: f32, color: Color) {
    let segments = ((r * std::f32::consts::TAU) / (5.0 * unit)).max(8.0) as usize;
    for i in (0..segments).step_by(2) {
        let a0 = std::f32::consts::TAU * i as f32 / segments as f32;
        let a1 = std::f32::consts::TAU * (i + 1) as f32 / segments as f32;
        thin_line(
            frame,
            (center.0 + r * a0.cos(), center.1 + r * a0.sin()),
            (center.0 + r * a1.cos(), center.1 + r * a1.sin()),
            1.2 * unit,
            color,
        );
    }
}

fn stack(key: &str, axis: Axis, children: Vec<Node<Intent>>) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            gap: Some(Space::None),
            ..Style::default()
        },
        element: Element::Stack { axis, children },
    }
}

fn text(key: &str, value: &str, role: TextRole) -> Node<Intent> {
    let mut node = Node {
        key: key.into(),
        style: Style::default(),
        element: Element::Text {
            value: value.into(),
            role,
        },
    };
    node.style.text_size = Some(match role {
        TextRole::Heading => 15,
        TextRole::Status => 11,
        _ => 12,
    });
    node.style.line_height = Some(match role {
        TextRole::Heading => 20,
        TextRole::Status => 15,
        _ => 17,
    });
    node.style.foreground = Some(match role {
        TextRole::Status => visual::current().muted,
        _ => visual::current().text,
    });
    node
}

/// A small button; `on` draws it selected.
fn button(key: &str, label: &str, action: Action, on: bool) -> Node<Intent> {
    let mut node = Node {
        key: key.into(),
        style: Style::default(),
        element: Element::Button {
            shortcut: None,
            label: label.into(),
            enabled: true,
            icon: None,
            intent: Intent::Map { action },
        },
    };
    node.style.background = Some(if on {
        visual::current().selected
    } else {
        visual::pick(Color::rgb(20, 20, 20), visual::current().sidebar)
    });
    node.style.foreground = Some(if on {
        visual::current().text
    } else {
        visual::current().muted
    });
    node.style.hover_background = Some(visual::current().selected);
    node.style.hover_foreground = Some(visual::current().text);
    node.style.radius = Some(6);
    node.style.text_size = Some(12);
    node.style.line_height = Some(16);
    node.style.weight = Some(TextWeight::Medium);
    node.style.button_padding = Some([8, 4]);
    node.style.min_height = Some(26);
    node
}

/// A full-width text button that reads as a link: a record or a member.
fn link(key: &str, label: &str, action: Action) -> Node<Intent> {
    let mut node = button(key, label, action, false);
    node.style.align = Some(TextAlign::Start);
    node.style.background = Some(Color {
        alpha: 0,
        ..visual::current().canvas
    });
    node.style.foreground = Some(visual::current().text);
    node.style.weight = Some(TextWeight::Normal);
    node
}

/// The map from the committed sources and records, with what this
/// computer knows.
#[must_use]
pub fn build(local: Local) -> Map {
    Map::build(
        openagents_chat_app::route_map::sources::Sources::committed(),
        openagents_chat_app::route_map::records::Records::committed(),
        local,
    )
}

#[cfg(test)]
#[path = "route_map_tests.rs"]
mod tests;
