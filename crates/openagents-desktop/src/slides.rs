//! The slide viewer: a deck from `openagents_deck` shown as an overlay
//! above the current page.
//!
//! [`Slides`] wraps an [`openagents_deck::Viewer`] with the overlay's own
//! state: the open and close animation, full screen, and the controls
//! (previous, next, the slide counter, full screen, and close). The
//! animation runs on the frame clock: the host passes each frame's
//! `Instant` to [`Slides::tick`], and nothing sleeps. Opening scales the
//! viewer from 0.96 to 1 and fades it from 0 to 1 over [`OPEN`], eased
//! out; closing runs the same curve backward. The chrome is gradations of
//! white on black, or of black on white in Coder Light (#11028); the
//! slides keep the deck's own look. Under "Reduce motion" the viewer opens and
//! closes at once.
//!
//! The host shows [`Slides::node`] as the window's overlay, laid over the
//! whole window ([`rust_native_desktop::OverlayPlacement::Cover`]), and
//! routes the [`RESOURCE`] surface's painting and input here.
//!
//! A slide may name a live scene. `scene: grid` asks the host to draw the
//! Verse's Grid behind it ([`Slides::scene`]). `scene: routes` shows the
//! Map page's graph ([`MapPage`]) in the slide's place, built from the
//! host's live data ([`Slides::routes_local`]) and as interactive as on
//! the Map page: the mouse goes to the map (drag to pan, pinch or Cmd and
//! the wheel to zoom, click to select and see the node's details, double
//! click to zoom in), and Tab, Enter, Esc with a selection, and Cmd + − 0
//! go to it too. The arrow and page keys still change slides, except while
//! the map is being dragged. Beside it, a narrow column plays a scripted
//! chat and lights each message's way through the map ([`RouteChat`]).
//! `scene: routes-plugin` goes on with that chat over the same map: a
//! request no plugin serves, the plugin made with the Gym's interview, its
//! XP, and others using it ([`RoutePlugin`]). `scene: routes-future` shows
//! the same view fed a growing model instead ([`RouteFuture`]), on the
//! frame clock. `scene: routes-live` shows that view fed today's real
//! payments, plugin calls, payouts, and runs from the public flow stream
//! ([`RouteLive`]), and says so when the stream can't be reached. `scene: essays` and `scene: download` show link cards in
//! the slide's place ([`Embeds`]): two essays as GitHub file previews, and
//! openagents.com/download in a browser window. A click on a card opens its
//! link in the browser ([`Slides::take_link`]); a click elsewhere on the
//! slide goes on as usual.

use crate::route_chat::RouteChat;
use crate::route_future::RouteFuture;
use crate::route_live::{FlowSource, RouteLive};
use crate::route_map::MapPage;
use crate::route_plugin::RoutePlugin;
use crate::slide_embeds::Embeds;
use openagents_deck::{Outcome, UnknownDeck, Viewer};
use rust_native::style::{Color, Style, TextAlign};
use rust_native::{Element, Node};
use rust_native_desktop::input::SurfaceInput;
use rust_native_desktop::text::{Fonts, font};
use rust_native_desktop::{Frame, PxRect};
use std::time::{Duration, Instant};

/// How long the viewer takes to open, and to close.
pub const OPEN: Duration = Duration::from_millis(240);
/// The scale the viewer opens from.
pub const START_SCALE: f32 = 0.96;
/// The surface resource the viewer paints into.
pub const RESOURCE: &str = "slides-viewer";
/// The overlay's node key, which is also the window's modal root while
/// the viewer shows.
pub const NODE: &str = "slides-overlay";
/// How often the host asks for a frame while the viewer animates.
pub const FRAME: Duration = Duration::from_millis(16);
/// How often the host asks for a frame while the future map plays.
pub const FUTURE_FRAME: Duration = Duration::from_millis(33);
/// The scene that shows the live route map alone, full slide, to look
/// around before the chat starts.
pub const MAP: &str = "map";
/// The scene that shows the live route map with the scripted chat beside
/// it.
pub const ROUTES: &str = "routes";
/// The scene that goes on with the chat as a person makes a plugin.
pub const ROUTES_PLUGIN: &str = "routes-plugin";
/// The scene that shows the route map growing into the future.
pub const ROUTES_FUTURE: &str = "routes-future";
/// The scene that shows today's real traffic on the route map.
pub const ROUTES_LIVE: &str = "routes-live";

/// The margin around the viewer's card, in points, when not full screen.
const MARGIN: f32 = 40.0;
/// The height of the control bar under the slide, in points.
const BAR: f32 = 48.0;

/// Where the viewer is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Opening,
    Open,
    Closing,
    /// Fully closed: the host removes the overlay.
    Closed,
}

/// A control on the viewer's bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Previous,
    Next,
    FullScreen,
    Close,
}

/// The slide viewer overlay.
pub struct Slides {
    viewer: Viewer,
    deck_id: String,
    phase: Phase,
    /// When the running animation started, and how far open it was then.
    started: Instant,
    from: f32,
    /// How far open the viewer is, 0 to 1, as of the last tick.
    progress: f32,
    fullscreen: bool,
    /// Open and close at once, without the animation.
    reduce_motion: bool,
    version: u64,
    fonts: Fonts,
    /// The controls' rectangles as last painted, relative to the surface,
    /// in points.
    controls: Vec<(Control, PxRect)>,
    /// The surface's size in points as last painted.
    size: (f32, f32),
    /// Pixels a point.
    unit: f32,
    /// The host can draw a slide's live scene (the Grid) behind it.
    scene_host: bool,
    /// The slide's place in the window as last painted, in points.
    slide_at: Option<PxRect>,
    /// The live route map a `scene: routes` slide shows, once the host
    /// gave it the data, with what that data was.
    routes: Option<(MapPage, String)>,
    /// The growing map a `scene: routes-future` slide shows, once shown.
    future: Option<RouteFuture>,
    /// Today's traffic a `scene: routes-live` slide shows, once shown.
    live: Option<RouteLive>,
    /// Where that traffic comes from; `None` is [`FlowSource::from_env`].
    flow: Option<FlowSource>,
    /// The scripted chat beside the live route map, once shown.
    chat: Option<RouteChat>,
    /// The plugin story on the live route map, once shown.
    plugin: Option<RoutePlugin>,
    /// The link cards a `scene: essays` or `scene: download` slide shows.
    embeds: Option<Embeds>,
    /// A card's link a click asked to open, until the host takes it.
    link: Option<String>,
}

/// Ease-out: fast at first, settling at the end.
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

impl Slides {
    /// Opens the deck filed under `deck_id`, starting the open animation
    /// at `now`; with `reduce_motion`, fully open at once.
    pub fn open(deck_id: &str, now: Instant, reduce_motion: bool) -> Result<Slides, UnknownDeck> {
        let viewer = Viewer::open(deck_id)?;
        Ok(Slides {
            viewer,
            deck_id: deck_id.to_string(),
            phase: if reduce_motion {
                Phase::Open
            } else {
                Phase::Opening
            },
            started: now,
            from: 0.0,
            progress: if reduce_motion { 1.0 } else { 0.0 },
            fullscreen: false,
            reduce_motion,
            version: 1,
            fonts: Fonts::new(),
            controls: Vec::new(),
            size: (0.0, 0.0),
            unit: 1.0,
            scene_host: false,
            slide_at: None,
            routes: None,
            future: None,
            live: None,
            flow: None,
            chat: None,
            plugin: None,
            embeds: None,
            link: None,
        })
    }

    /// Whether the showing slide is the live route map, alone or with
    /// the chat, so the host hands it the data ([`Slides::routes_local`]).
    /// Both slides share one map, so the view carries over.
    pub fn wants_routes(&self) -> bool {
        matches!(self.viewer.scene(), Some(MAP | ROUTES))
    }

    /// Whether the showing slide plays the scripted chat beside the map;
    /// the `map` slide has the map alone.
    pub fn on_chat(&self) -> bool {
        self.viewer.scene() == Some(ROUTES)
    }

    /// The data the live route map is built from: this computer's route
    /// counts and engines. Builds the map the first time and rebuilds it
    /// when the data changes, keeping the view.
    pub fn routes_local(&mut self, local: openagents_chat_app::route_map::Local) {
        let key = format!("{:?}|{:?}", local.routes, local.engines);
        match &mut self.routes {
            Some((page, was)) => {
                if *was == key {
                    return;
                }
                page.refresh(crate::route_map::build(local));
                *was = key;
            }
            None => {
                let mut page =
                    MapPage::presenting(crate::route_map::build(local), self.reduce_motion);
                page.set_unit(self.unit);
                self.routes = Some((page, key));
            }
        }
        self.changed();
    }

    /// The live route map, while the slide that shows it is up.
    pub fn routes(&self) -> Option<&MapPage> {
        self.routes
            .as_ref()
            .filter(|_| self.wants_routes())
            .map(|(page, _)| page)
    }

    fn routes_mut(&mut self) -> Option<&mut MapPage> {
        if !self.wants_routes() || self.phase != Phase::Open {
            return None;
        }
        self.routes.as_mut().map(|(page, _)| page)
    }

    /// The scripted chat beside the live route map, once its slide has
    /// shown.
    pub fn chat(&self) -> Option<&RouteChat> {
        self.chat.as_ref()
    }

    /// The growing map, once its slide has shown.
    pub fn future(&self) -> Option<&RouteFuture> {
        self.future.as_ref()
    }

    fn on_future(&self) -> bool {
        self.viewer.scene() == Some(ROUTES_FUTURE)
    }

    /// Today's traffic, once its slide has shown.
    pub fn live(&self) -> Option<&RouteLive> {
        self.live.as_ref()
    }

    fn on_live(&self) -> bool {
        self.viewer.scene() == Some(ROUTES_LIVE)
    }

    /// Where the `routes-live` scene's events come from: a fixture for a
    /// capture or a test, or another stream. Set before its slide shows.
    pub fn set_flow_source(&mut self, source: FlowSource) {
        self.flow = Some(source);
        self.live = None;
    }

    /// Today's traffic, connected the first time its slide shows.
    fn live_scene(&mut self) -> &mut RouteLive {
        let reduce = self.reduce_motion;
        let flow = &self.flow;
        self.live.get_or_insert_with(|| {
            RouteLive::new(flow.clone().unwrap_or_else(FlowSource::from_env), reduce)
        })
    }

    /// The plugin story, once its slide has shown.
    pub fn plugin(&self) -> Option<&RoutePlugin> {
        self.plugin.as_ref()
    }

    fn on_plugin(&self) -> bool {
        self.viewer.scene() == Some(ROUTES_PLUGIN)
    }

    /// The plugin story, made the first time over the live map the slide
    /// before showed (or the committed map, if it never showed).
    fn plugin_story(&mut self) -> &mut RoutePlugin {
        let reduce = self.reduce_motion;
        let routes = &self.routes;
        self.plugin.get_or_insert_with(|| {
            let today = routes.as_ref().map_or_else(
                || crate::route_map::build(Default::default()),
                |(page, _)| page.map().clone(),
            );
            RoutePlugin::new(today, reduce)
        })
    }

    /// The link-card scene the showing slide asks for, if any.
    fn embed_scene(&self) -> Option<&'static str> {
        match self.viewer.scene() {
            Some(crate::slide_embeds::ESSAYS) => Some(crate::slide_embeds::ESSAYS),
            Some(crate::slide_embeds::DOWNLOAD) => Some(crate::slide_embeds::DOWNLOAD),
            _ => None,
        }
    }

    /// The link cards, once a slide with them has shown.
    pub fn embeds(&self) -> Option<&Embeds> {
        self.embeds.as_ref()
    }

    /// The link a click on a card asked to open, once: the host opens it
    /// in the browser.
    pub fn take_link(&mut self) -> Option<String> {
        self.link.take()
    }

    /// The overlay's node: one surface over the whole window.
    pub fn node<I>(&self) -> Node<I> {
        Node {
            key: NODE.into(),
            style: Style {
                fill_height: Some(true),
                ..Style::default()
            },
            element: Element::Surface {
                resource: RESOURCE.into(),
                label: self.viewer.label(),
            },
        }
    }

    /// The window's scale, in pixels a point.
    pub fn set_unit(&mut self, unit: f32) {
        if unit.is_finite() && unit > 0.0 && unit != self.unit {
            self.unit = unit;
            self.changed();
        }
    }

    /// Says the host can draw a slide's live scene behind it
    /// ([`Slides::scene`]).
    pub fn set_scene_host(&mut self, on: bool) {
        if self.scene_host != on {
            self.scene_host = on;
            self.changed();
        }
    }

    /// Where the host draws the showing slide's live scene, in the
    /// window's points: the slide's place while it is fully open and asks
    /// for one (the Episode 289 title slide asks for the Grid).
    pub fn scene(&self) -> Option<rust_native_desktop::Rect> {
        let live =
            self.scene_host && self.phase == Phase::Open && self.viewer.scene() == Some("grid");
        let at = self.slide_at.filter(|_| live)?;
        Some(rust_native_desktop::Rect {
            x: at.x,
            y: at.y,
            w: at.w,
            h: at.h,
        })
    }

    /// When the host should next call [`Slides::tick`]: a frame from
    /// `now` while animating, otherwise never.
    pub fn next_wake(&self, now: Instant) -> Option<Instant> {
        if self.animating() {
            return Some(now + FRAME);
        }
        if self.phase != Phase::Open {
            return None;
        }
        if let Some(scene) = self.embed_scene()
            && self
                .embeds
                .as_ref()
                .is_none_or(|embeds| embeds.pending(scene))
        {
            return Some(now + FRAME);
        }
        if (self.on_future() || self.on_chat() || self.on_plugin() || self.on_live())
            && !self.reduce_motion
        {
            return Some(now + FUTURE_FRAME);
        }
        if self.on_live() {
            // Still, but the stream's news still shows.
            return Some(now + Duration::from_millis(250));
        }
        self.routes().and_then(|page| page.next_wake(now))
    }

    pub fn deck_id(&self) -> &str {
        &self.deck_id
    }

    pub fn viewer(&self) -> &Viewer {
        &self.viewer
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// Whether the viewer shows the slide edge to edge.
    pub fn fullscreen(&self) -> bool {
        self.fullscreen
    }

    /// Whether an animation is running, so the host asks for frames.
    pub fn animating(&self) -> bool {
        matches!(self.phase, Phase::Opening | Phase::Closing)
    }

    /// Whether the overlay is gone and the host should drop it.
    pub fn closed(&self) -> bool {
        self.phase == Phase::Closed
    }

    /// How far open the viewer is, 0 to 1, before easing.
    pub fn progress(&self) -> f32 {
        self.progress
    }

    /// The eased openness: the opacity, 0 to 1.
    pub fn opacity(&self) -> f32 {
        ease_out(self.progress)
    }

    /// The viewer's scale: 0.96 closed, 1 open.
    pub fn scale(&self) -> f32 {
        START_SCALE + (1.0 - START_SCALE) * self.opacity()
    }

    /// The slide counter, such as "3 / 11".
    pub fn counter(&self) -> String {
        format!("{} / {}", self.viewer.index() + 1, self.viewer.deck().len())
    }

    /// Changes whenever the painting would.
    pub fn version(&self) -> u64 {
        let routes = self.routes().map_or(0, MapPage::version);
        let future = self.future.as_ref().map_or(0, RouteFuture::version);
        let live = self.live.as_ref().map_or(0, RouteLive::version);
        let plugin = self.plugin.as_ref().map_or(0, RoutePlugin::version);
        let embeds = self.embeds.as_ref().map_or(0, Embeds::version);
        self.version
            .wrapping_add(self.viewer.version())
            .wrapping_add(routes)
            .wrapping_add(future)
            .wrapping_add(live)
            .wrapping_add(plugin)
            .wrapping_add(embeds)
    }

    fn changed(&mut self) {
        self.version = self.version.wrapping_add(1);
    }

    /// Advances the animation to `now`, the frame clock's time. Returns
    /// whether anything changed.
    pub fn tick(&mut self, now: Instant) -> bool {
        let mut moved = false;
        if self.on_future() && self.phase != Phase::Closed {
            let reduce = self.reduce_motion;
            let future = self.future.get_or_insert_with(|| RouteFuture::new(reduce));
            future.advance(now);
            moved = true;
        } else if let Some(future) = &mut self.future {
            // Off its slide: the next visit plays from today.
            future.reset();
        }
        if self.on_live() && self.phase != Phase::Closed {
            self.live_scene().advance(now);
            moved = true;
        }
        if self.on_plugin() && self.phase != Phase::Closed {
            let story = self.plugin_story();
            story.advance(now);
            moved |= story.playing();
        } else if let Some(story) = &mut self.plugin {
            // Off its slide: the next visit plays the story from the start.
            story.reset();
        }
        match self.embed_scene() {
            Some(scene) if self.phase != Phase::Closed => {
                let reduce = self.reduce_motion;
                let embeds = self.embeds.get_or_insert_with(|| Embeds::new(reduce));
                moved |= embeds.show(scene, now);
            }
            _ => {
                if let Some(embeds) = &mut self.embeds {
                    // Off its slide: the next visit comes in again.
                    embeds.reset();
                }
            }
        }
        if self.on_chat() && self.phase != Phase::Closed {
            let reduce = self.reduce_motion;
            let chat = self.chat.get_or_insert_with(|| RouteChat::new(reduce));
            chat.advance(now);
            moved |= chat.playing();
            if let Some((page, _)) = &mut self.routes {
                page.set_light(chat.light(page.map()));
            }
        } else {
            if let Some(chat) = &mut self.chat {
                // Off its slide: the next visit starts the conversation over.
                chat.reset();
            }
            if let Some((page, _)) = &mut self.routes {
                // The map alone lights no route.
                page.set_light(None);
            }
        }
        if let Some(page) = self.routes_mut() {
            moved |= page.tick(now);
        }
        if moved {
            self.changed();
        }
        if !self.animating() {
            return moved;
        }
        let t = now.saturating_duration_since(self.started).as_secs_f32() / OPEN.as_secs_f32();
        let t = t.clamp(0.0, 1.0);
        match self.phase {
            Phase::Opening => {
                self.progress = self.from + (1.0 - self.from) * t;
                if t >= 1.0 {
                    self.progress = 1.0;
                    self.phase = Phase::Open;
                }
            }
            Phase::Closing => {
                self.progress = self.from * (1.0 - t);
                if t >= 1.0 {
                    self.progress = 0.0;
                    self.phase = Phase::Closed;
                }
            }
            Phase::Open | Phase::Closed => {}
        }
        self.changed();
        true
    }

    /// Starts the close animation at `now`, from wherever the viewer is.
    pub fn close(&mut self, now: Instant) {
        if matches!(self.phase, Phase::Closing | Phase::Closed) {
            return;
        }
        if self.reduce_motion {
            self.progress = 0.0;
            self.phase = Phase::Closed;
            self.fullscreen = false;
            self.changed();
            return;
        }
        self.tick(now);
        // A viewer still opening closes over only the distance it opened,
        // at the same speed.
        let elapsed = OPEN.mul_f32(1.0 - self.progress);
        self.from = 1.0;
        self.started = now.checked_sub(elapsed).unwrap_or(now);
        self.phase = Phase::Closing;
        self.fullscreen = false;
        self.changed();
    }

    /// Goes full screen, or leaves it.
    pub fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
        self.changed();
    }

    /// Answers a key. Returns whether the viewer took it; while open it
    /// takes every plain key, so none reaches the page under it, and
    /// leaves the window's command keys (quit, close) alone.
    pub fn key(&mut self, key: &str, command: bool, now: Instant) -> bool {
        if matches!(self.phase, Phase::Closing | Phase::Closed) {
            return false;
        }
        if command {
            let taken = self.viewer.key(key, true) != Outcome::Ignored;
            if taken {
                self.changed();
            }
            return taken;
        }
        if let Some(taken) = self.map_key(key, command, now) {
            self.changed();
            return taken;
        }
        let viewer = &self.viewer;
        let settled = !viewer.overview() && !viewer.notes() && !viewer.black();
        match key {
            "f" | "F" => {
                self.toggle_fullscreen();
                return true;
            }
            // Esc steps back: out of full screen first, then closed.
            "Escape" if self.fullscreen && settled => {
                self.fullscreen = false;
                self.changed();
                return true;
            }
            _ => {}
        }
        match self.viewer.key(key, false) {
            Outcome::Close | Outcome::Quit => self.close(now),
            Outcome::ToggleFullscreen => self.toggle_fullscreen(),
            Outcome::Handled | Outcome::Ignored => {}
        }
        self.changed();
        true
    }

    /// A key on the live route map's slide that the map takes, if it is
    /// one: the arrow and page keys wait while the map is dragged (so a
    /// slide doesn't change under the hand), Cmd + − 0 zoom and fit, Tab
    /// steps through the nodes, and Enter and Esc zoom into and step out
    /// of a selection. Everything else changes slides as usual.
    fn map_key(&mut self, key: &str, command: bool, now: Instant) -> Option<bool> {
        let page = self.routes_mut()?;
        let navigation = matches!(
            key,
            "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown" | "PageUp" | "PageDown" | " "
        );
        if page.pressed() && navigation {
            return Some(true);
        }
        let taken = match (key, command) {
            ("=" | "+" | "-" | "0", true) | ("Tab", false) => true,
            ("Enter" | "Escape", false) => page.selected().is_some(),
            _ => false,
        };
        if !taken {
            return None;
        }
        page.key(key, command, false, now);
        Some(true)
    }

    /// Runs a control.
    pub fn control(&mut self, control: Control, now: Instant) {
        match control {
            Control::Previous => {
                self.viewer_key("ArrowLeft");
            }
            Control::Next => {
                self.viewer_key("ArrowRight");
            }
            Control::FullScreen => self.toggle_fullscreen(),
            Control::Close => self.close(now),
        }
    }

    fn viewer_key(&mut self, key: &str) {
        self.viewer.key(key, false);
        self.changed();
    }

    /// Pointer input on the [`RESOURCE`] surface, in points relative to
    /// it. The viewer takes all of it while it shows.
    pub fn input(&mut self, event: SurfaceInput, now: Instant) -> bool {
        if self.closed() {
            return false;
        }
        if self.map_input(event, now) {
            self.changed();
            return true;
        }
        if self.embed_input(event) {
            self.changed();
            return true;
        }
        if let SurfaceInput::Down { x, y, .. } = event
            && !matches!(self.phase, Phase::Closing)
        {
            let (width, height) = self.size;
            self.click(x, y, width, height, now);
        }
        true
    }

    /// Pointer input on the live route map's slide that goes to the map:
    /// anything over the slide, and the rest of a press that began there.
    fn map_input(&mut self, event: SurfaceInput, now: Instant) -> bool {
        let (width, height) = self.size;
        let slide = Layout::of(width, height, self.fullscreen, 1.0).slide;
        let slide = if self.on_chat() {
            crate::route_chat::split(slide).1
        } else {
            slide
        };
        let Some(page) = self.routes_mut() else {
            return false;
        };
        let (x, y) = match event {
            SurfaceInput::Down { x, y, .. }
            | SurfaceInput::Move { x, y }
            | SurfaceInput::Up { x, y }
            | SurfaceInput::Wheel { x, y, .. }
            | SurfaceInput::Zoom { x, y, .. } => (x, y),
        };
        let over = contains(slide, x, y);
        let held =
            page.pressed() && matches!(event, SurfaceInput::Move { .. } | SurfaceInput::Up { .. });
        if !over && !held {
            return false;
        }
        let (dx, dy) = (slide.x, slide.y);
        let local = match event {
            SurfaceInput::Down { x, y, shift } => SurfaceInput::Down {
                x: x - dx,
                y: y - dy,
                shift,
            },
            SurfaceInput::Move { x, y } => SurfaceInput::Move {
                x: x - dx,
                y: y - dy,
            },
            SurfaceInput::Up { x, y } => SurfaceInput::Up {
                x: x - dx,
                y: y - dy,
            },
            SurfaceInput::Wheel {
                x,
                y,
                dx: wx,
                dy: wy,
            } => SurfaceInput::Wheel {
                x: x - dx,
                y: y - dy,
                dx: wx,
                dy: wy,
            },
            SurfaceInput::Zoom { x, y, factor } => SurfaceInput::Zoom {
                x: x - dx,
                y: y - dy,
                factor,
            },
        };
        page.input(local, now);
        true
    }

    /// Pointer input on a link-card slide: the pointer over a card
    /// brightens it, and a press on one asks the host to open its link.
    /// Anything else goes on as on any slide.
    fn embed_input(&mut self, event: SurfaceInput) -> bool {
        let Some(scene) = self.embed_scene() else {
            return false;
        };
        if self.phase != Phase::Open {
            return false;
        }
        let (width, height) = self.size;
        let slide = Layout::of(width, height, self.fullscreen, 1.0).slide;
        let (x, y) = match event {
            SurfaceInput::Down { x, y, .. }
            | SurfaceInput::Move { x, y }
            | SurfaceInput::Up { x, y }
            | SurfaceInput::Wheel { x, y, .. }
            | SurfaceInput::Zoom { x, y, .. } => (x, y),
        };
        let over = crate::slide_embeds::cards(scene, slide)
            .iter()
            .position(|card| contains(*card, x, y));
        let Some(embeds) = self.embeds.as_mut() else {
            return false;
        };
        match event {
            SurfaceInput::Move { .. } => embeds.set_hover(over),
            SurfaceInput::Down { .. } => match over {
                Some(index) => {
                    self.link = crate::slide_embeds::url(scene, index);
                    true
                }
                None => false,
            },
            _ => false,
        }
    }

    /// A click at `x`, `y` relative to the `width` by `height` surface, in
    /// points. A click on a control runs it; a click on the slide goes
    /// on; a click outside the card closes the viewer.
    pub fn click(&mut self, x: f32, y: f32, width: f32, height: f32, now: Instant) {
        if let Some((control, _)) = self
            .controls
            .iter()
            .find(|(_, rect)| contains(*rect, x, y))
            .copied()
        {
            self.control(control, now);
            return;
        }
        let layout = Layout::of(width, height, self.fullscreen, 1.0);
        if contains(layout.slide, x, y) {
            self.viewer_key("ArrowRight");
        } else if !contains(layout.card, x, y) {
            self.close(now);
        }
    }

    /// The controls' rectangles, relative to the surface, as last painted.
    pub fn controls(&self) -> &[(Control, PxRect)] {
        &self.controls
    }

    /// Paints the overlay into `rect` of `frame`, in pixels, at the
    /// window's scale ([`Slides::set_unit`]).
    pub fn paint(&mut self, frame: &mut Frame, rect: PxRect) {
        let unit = self.unit.max(0.1);
        self.size = (rect.w / unit, rect.h / unit);
        let opacity = self.opacity();
        if opacity <= 0.0 {
            self.controls.clear();
            return;
        }
        if opacity >= 1.0 {
            self.paint_opaque(frame, rect, unit);
            return;
        }
        // Mid-animation: paint into a copy of what is under the overlay,
        // then fade that over the page.
        let (w, h) = (rect.w.ceil() as usize, rect.h.ceil() as usize);
        let (x0, y0) = (rect.x.max(0.0) as usize, rect.y.max(0.0) as usize);
        let mut layer = Frame::new(w.max(1), h.max(1), Color::rgb(0, 0, 0));
        copy(frame, x0, y0, &mut layer, 0, 0, w, h);
        let local = PxRect {
            x: 0.0,
            y: 0.0,
            w: rect.w,
            h: rect.h,
        };
        self.paint_opaque(&mut layer, local, unit);
        blend(frame, x0, y0, &layer, w, h, opacity);
    }

    fn paint_opaque(&mut self, frame: &mut Frame, rect: PxRect, unit: f32) {
        let scale = self.scale();
        let unit = unit.max(0.1);
        let layout = Layout::of(rect.w / unit, rect.h / unit, self.fullscreen, scale);
        let px = |r: PxRect| PxRect {
            x: rect.x + r.x * unit,
            y: rect.y + r.y * unit,
            w: r.w * unit,
            h: r.h * unit,
        };
        // The scrim over the page.
        frame.fill(
            rect,
            0.0,
            Color {
                alpha: if self.fullscreen { 255 } else { 214 },
                ..Color::rgb(0, 0, 0)
            },
        );
        if !self.fullscreen {
            frame.fill(
                px(layout.card),
                14.0 * unit * scale,
                tone(|p| p.surface_raised),
            );
            frame.stroke(
                px(layout.card),
                14.0 * unit * scale,
                unit,
                tone(|p| p.stroke_subtle),
            );
        }
        self.slide_at = Some(PxRect {
            x: rect.x / unit + layout.slide.x,
            y: rect.y / unit + layout.slide.y,
            w: layout.slide.w,
            h: layout.slide.h,
        });
        let live = self.scene_host && self.phase == Phase::Open;
        self.viewer.set_live_scene(live);
        self.viewer.layout(px(layout.slide));
        self.viewer.paint(frame, px(layout.slide));
        if self.wants_routes()
            && !self.on_chat()
            && let Some((page, _)) = &mut self.routes
        {
            // The map alone, full slide.
            page.set_light(None);
            page.set_unit(unit);
            page.paint(frame, px(layout.slide));
        } else if self.on_chat()
            && let Some((page, _)) = &mut self.routes
        {
            let (column, map) = crate::route_chat::split(px(layout.slide));
            let reduce = self.reduce_motion;
            let chat = self.chat.get_or_insert_with(|| RouteChat::new(reduce));
            page.set_light(chat.light(page.map()));
            page.set_unit(unit);
            page.paint(frame, map);
            chat.paint(frame, column, unit);
        } else if self.on_plugin() {
            let (column, map) = crate::route_chat::split(px(layout.slide));
            self.plugin_story().paint(frame, column, map, unit);
        } else if let Some(scene) = self.embed_scene() {
            let reduce = self.reduce_motion;
            self.embeds
                .get_or_insert_with(|| Embeds::new(reduce))
                .paint(frame, px(layout.slide), scene);
        } else if self.on_live() {
            self.live_scene().paint(frame, px(layout.slide), unit);
        } else if self.on_future() {
            let reduce = self.reduce_motion;
            self.future
                .get_or_insert_with(|| RouteFuture::new(reduce))
                .paint(frame, px(layout.slide), unit);
        }
        self.controls.clear();
        let Some(bar) = layout.bar else {
            return;
        };
        let size = 13.0 * scale;
        let counter = self.counter();
        let buttons = [
            (Control::Previous, "Previous", bar.x),
            (Control::Next, "Next", bar.x + 92.0 * scale),
            (
                Control::FullScreen,
                "Full screen",
                bar.x + bar.w - 196.0 * scale,
            ),
            (Control::Close, "Close", bar.x + bar.w - 84.0 * scale),
        ];
        for (control, label, x) in buttons {
            let width = match control {
                Control::FullScreen => 104.0,
                _ => 84.0,
            } * scale;
            let button = PxRect {
                x,
                y: bar.y + (bar.h - 30.0 * scale) / 2.0,
                w: width,
                h: 30.0 * scale,
            };
            frame.fill(
                px(button),
                8.0 * unit * scale,
                tone(|p| p.control_on_overlay),
            );
            frame.stroke(px(button), 8.0 * unit * scale, unit, tone(|p| p.stroke));
            self.text(frame, px(button), label, size * unit, tone(|p| p.content));
            self.controls.push((control, button));
        }
        self.text(
            frame,
            px(bar),
            &counter,
            size * unit,
            tone(|p| p.content_secondary),
        );
    }

    fn text(&mut self, frame: &mut Frame, rect: PxRect, value: &str, size: f32, color: Color) {
        let paragraph = self.fonts.paragraph(
            value,
            font(size, rust_native::layout::display::Weight::Medium, false),
            None,
        );
        let line = size * rust_native_desktop::text::LINE_EM;
        self.fonts.draw(
            frame,
            &paragraph,
            rect.x,
            rect.y + ((rect.h - line) / 2.0).round(),
            rect.w,
            TextAlign::Center,
            1.0,
            color,
        );
    }
}

/// A role of the shared token table in the look the app paints with: the
/// viewer's chrome (its card, buttons, and counter) is Coder Noir or Coder
/// Light, as the web's (#11120). The slides themselves and the scrim stay
/// as the deck paints them.
fn tone(role: impl Fn(&oa_tokens::Palette) -> oa_tokens::Rgba8) -> Color {
    openagents_chat_app::visual::role(role(openagents_chat_app::visual::palette()))
}

fn contains(rect: PxRect, x: f32, y: f32) -> bool {
    x >= rect.x && x < rect.x + rect.w && y >= rect.y && y < rect.y + rect.h
}

/// Where the card, the slide, and the control bar sit in a `width` by
/// `height` surface, in points, at `scale` about the center.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub card: PxRect,
    pub slide: PxRect,
    pub bar: Option<PxRect>,
}

impl Layout {
    pub fn of(width: f32, height: f32, fullscreen: bool, scale: f32) -> Layout {
        let whole = PxRect {
            x: 0.0,
            y: 0.0,
            w: width,
            h: height,
        };
        if fullscreen {
            return Layout {
                card: whole,
                slide: whole,
                bar: None,
            };
        }
        let aspect = openagents_deck::compose::WIDTH / openagents_deck::compose::HEIGHT;
        let room_w = (width - 2.0 * MARGIN).max(1.0);
        let room_h = (height - 2.0 * MARGIN - BAR).max(1.0);
        let slide_w = room_w.min(room_h * aspect);
        let slide_h = slide_w / aspect;
        let card_w = slide_w;
        let card_h = slide_h + BAR;
        let (cx, cy) = (width / 2.0, height / 2.0);
        let at = |x: f32, y: f32, w: f32, h: f32| PxRect {
            x: cx + (x - cx) * scale,
            y: cy + (y - cy) * scale,
            w: w * scale,
            h: h * scale,
        };
        let left = cx - card_w / 2.0;
        let top = cy - card_h / 2.0;
        Layout {
            card: at(left, top, card_w, card_h),
            slide: at(left, top, slide_w, slide_h),
            bar: Some(at(left + 12.0, top + slide_h, card_w - 24.0, BAR)),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn copy(
    src: &Frame,
    sx: usize,
    sy: usize,
    dst: &mut Frame,
    dx: usize,
    dy: usize,
    w: usize,
    h: usize,
) {
    for row in 0..h {
        let (ys, yd) = (sy + row, dy + row);
        if ys >= src.height || yd >= dst.height {
            break;
        }
        let n = w
            .min(src.width.saturating_sub(sx))
            .min(dst.width.saturating_sub(dx));
        let s = (ys * src.width + sx) * 4;
        let d = (yd * dst.width + dx) * 4;
        dst.pixels[d..d + n * 4].copy_from_slice(&src.pixels[s..s + n * 4]);
    }
}

/// Lays `layer` over `frame` at `x`, `y` with `opacity`.
fn blend(frame: &mut Frame, x: usize, y: usize, layer: &Frame, w: usize, h: usize, opacity: f32) {
    let a = (opacity.clamp(0.0, 1.0) * 256.0) as u32;
    for row in 0..h.min(layer.height) {
        let yd = y + row;
        if yd >= frame.height {
            break;
        }
        let n = w.min(layer.width).min(frame.width.saturating_sub(x));
        for col in 0..n {
            let s = (row * layer.width + col) * 4;
            let d = (yd * frame.width + x + col) * 4;
            for c in 0..4 {
                let (under, over) = (frame.pixels[d + c] as u32, layer.pixels[s + c] as u32);
                frame.pixels[d + c] = ((under * (256 - a) + over * a) >> 8) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DECK: &str = "three-devdays-later";

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    /// Opening progresses over frames, scaling from 0.96 and fading from
    /// 0, eased out, and ends fully open at 240 ms.
    #[test]
    fn opening_animates_over_frames_to_fully_open() {
        let start = Instant::now();
        let mut slides = Slides::open(DECK, start, false).expect("the deck opens");
        assert_eq!(slides.phase(), Phase::Opening);
        assert!(slides.animating());
        assert_eq!(slides.opacity(), 0.0);
        assert!((slides.scale() - 0.96).abs() < 1e-6);
        let mut last = 0.0;
        for frame in 1..15 {
            slides.tick(start + ms(frame * 16));
            assert!(slides.opacity() > last, "frame {frame} moves on");
            assert!(slides.opacity() < 1.0);
            last = slides.opacity();
        }
        slides.tick(start + ms(120));
        assert!(
            slides.opacity() > 0.5,
            "ease-out is past halfway at half time"
        );
        slides.tick(start + ms(240));
        assert_eq!(slides.phase(), Phase::Open);
        assert!(!slides.animating());
        assert_eq!(slides.opacity(), 1.0);
        assert_eq!(slides.scale(), 1.0);
    }

    /// Arrows and space move through the deck; the buttons do the same;
    /// the counter follows.
    #[test]
    fn navigation_moves_the_counter() {
        let now = Instant::now();
        let mut slides = Slides::open(DECK, now, false).expect("the deck opens");
        let total = slides.viewer().deck().len();
        assert_eq!(slides.counter(), format!("1 / {total}"));
        assert!(slides.key("ArrowRight", false, now));
        assert!(slides.key("Space", false, now));
        assert_eq!(slides.counter(), format!("3 / {total}"));
        slides.key("ArrowLeft", false, now);
        assert_eq!(slides.counter(), format!("2 / {total}"));
        slides.control(Control::Next, now);
        assert_eq!(slides.counter(), format!("3 / {total}"));
        slides.control(Control::Previous, now);
        slides.control(Control::Previous, now);
        slides.control(Control::Previous, now);
        assert_eq!(slides.counter(), format!("1 / {total}"));
    }

    /// `F` toggles full screen; Esc leaves full screen, then closes.
    #[test]
    fn f_toggles_full_screen_and_escape_steps_back_then_closes() {
        let start = Instant::now();
        let mut slides = Slides::open(DECK, start, false).expect("the deck opens");
        slides.tick(start + OPEN);
        slides.key("F", false, start + OPEN);
        assert!(slides.fullscreen());
        slides.key("f", false, start + OPEN);
        assert!(!slides.fullscreen());
        slides.key("f", false, start + OPEN);
        assert!(slides.fullscreen());
        slides.key("Escape", false, start + OPEN);
        assert!(!slides.fullscreen());
        assert_eq!(
            slides.phase(),
            Phase::Open,
            "Esc from full screen returns to the viewer"
        );
        slides.key("Escape", false, start + OPEN);
        assert_eq!(
            slides.phase(),
            Phase::Closing,
            "Esc from the viewer closes it"
        );
    }

    /// Closing runs the open animation backward, then the viewer is
    /// closed and takes no more keys.
    #[test]
    fn closing_animates_then_ends() {
        let start = Instant::now();
        let mut slides = Slides::open(DECK, start, false).expect("the deck opens");
        slides.tick(start + OPEN);
        let at = start + ms(300);
        slides.control(Control::Close, at);
        assert!(slides.animating());
        slides.tick(at + ms(16));
        let early = slides.opacity();
        assert!(early < 1.0 && early > 0.5);
        slides.tick(at + ms(120));
        assert!(slides.opacity() < early);
        assert!(slides.scale() < 1.0 && slides.scale() > 0.96);
        assert!(!slides.closed());
        slides.tick(at + OPEN);
        assert!(slides.closed());
        assert_eq!(slides.opacity(), 0.0);
        assert!(!slides.key("ArrowRight", false, at + OPEN));
    }

    /// Painting mid-open fades over the page; fully open paints the
    /// controls; full screen paints the slide edge to edge with no bar.
    #[test]
    fn paints_dark_with_controls_and_edge_to_edge_full_screen() {
        let start = Instant::now();
        let mut slides = Slides::open(DECK, start, false).expect("the deck opens");
        let rect = PxRect {
            x: 0.0,
            y: 0.0,
            w: 960.0,
            h: 600.0,
        };
        let page = Color::rgb(200, 30, 30);
        slides.tick(start + ms(60));
        let mut frame = Frame::new(960, 600, page);
        slides.paint(&mut frame, rect);
        let corner = &frame.pixels[0..3];
        assert!(
            corner[0] < 200 && corner[0] > 0,
            "mid-open fades over the page: {corner:?}"
        );
        slides.tick(start + OPEN);
        let mut frame = Frame::new(960, 600, page);
        slides.paint(&mut frame, rect);
        let names: Vec<Control> = slides.controls().iter().map(|(c, _)| *c).collect();
        assert_eq!(
            names,
            [
                Control::Previous,
                Control::Next,
                Control::FullScreen,
                Control::Close
            ]
        );
        let (_, close) = slides.controls()[3];
        slides.click(close.x + 2.0, close.y + 2.0, 960.0, 600.0, start + OPEN);
        assert_eq!(slides.phase(), Phase::Closing);
        let mut slides = Slides::open(DECK, start, false).expect("the deck opens");
        slides.tick(start + OPEN);
        slides.toggle_fullscreen();
        let mut frame = Frame::new(960, 600, page);
        slides.paint(&mut frame, rect);
        assert!(slides.controls().is_empty());
        let layout = Layout::of(960.0, 600.0, true, 1.0);
        assert_eq!(layout.slide, rect);
        let corner = &frame.pixels[0..3];
        assert_ne!(corner, [200, 30, 30], "full screen covers the page");
    }
    /// Under "Reduce motion" the viewer opens and closes at once.
    #[test]
    fn reduced_motion_opens_and_closes_at_once() {
        let now = Instant::now();
        let mut slides = Slides::open(DECK, now, true).expect("the deck opens");
        assert_eq!(slides.phase(), Phase::Open);
        assert!(!slides.animating());
        assert_eq!(slides.opacity(), 1.0);
        assert_eq!(slides.scale(), 1.0);
        assert_eq!(slides.next_wake(now), None);
        slides.key("Escape", false, now);
        assert!(slides.closed(), "closing is instant too");
        assert_eq!(slides.opacity(), 0.0);
    }

    /// The window's command keys (quit, close) pass through; the viewer's
    /// own command keys (zoom) do not; plain keys never reach the page.
    #[test]
    fn command_keys_pass_through_and_plain_keys_are_taken() {
        let now = Instant::now();
        let mut slides = Slides::open(DECK, now, true).expect("the deck opens");
        assert!(!slides.key("q", true, now));
        assert!(!slides.key("w", true, now));
        assert!(slides.key("=", true, now));
        assert!(slides.key("x", false, now));
        assert_eq!(slides.phase(), Phase::Open);
    }

    /// Esc with the overview open closes the overview, not full screen.
    #[test]
    fn escape_closes_the_overview_before_full_screen() {
        let now = Instant::now();
        let mut slides = Slides::open(DECK, now, true).expect("the deck opens");
        slides.key("f", false, now);
        slides.key("o", false, now);
        assert!(slides.viewer().overview());
        slides.key("Escape", false, now);
        assert!(!slides.viewer().overview());
        assert!(slides.fullscreen());
        slides.key("Escape", false, now);
        assert!(!slides.fullscreen());
        assert_eq!(slides.phase(), Phase::Open);
    }

    /// While animating the viewer asks for frames; once open it does not.
    #[test]
    fn asks_for_frames_only_while_animating() {
        let start = Instant::now();
        let mut slides = Slides::open(DECK, start, false).expect("the deck opens");
        assert_eq!(slides.next_wake(start), Some(start + FRAME));
        let before = slides.version();
        slides.tick(start + ms(16));
        assert_ne!(slides.version(), before, "each frame repaints");
        slides.tick(start + OPEN);
        assert_eq!(slides.next_wake(start + OPEN), None);
    }
}
