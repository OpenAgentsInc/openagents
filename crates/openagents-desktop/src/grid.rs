//! Play and Watch share one window. Only an explicit Play session holds a
//! world identity; the spectator remains an independent read-only reader.
mod controls;
mod fixture;
mod panels;
pub(crate) mod store;

use crate::model::Intent;
use coder_mobile::verse_surface::{Command, GridSurface, Panel};
use controls::{Controls, Effect};
use rust_native::style::{Color, Space, Style, TextAlign, TextWeight};
use rust_native::surface::Viewport;
use rust_native::{Axis, Element, Node, TextRole};
use rust_native_desktop::backdrop::{Backdrop, Gpu, Look};
use rust_native_desktop::input::NativeInput;
use rust_native_desktop::{Rect, wgpu};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

/// The Verse page's node, which the world fills (#10116).
pub const WORLD: &str = "grid-world";
pub const FRAME: Duration = Duration::from_nanos(1_000_000_000 / 60);
pub type Shared = Rc<RefCell<Grid>>;

pub struct Grid {
    pub playing: bool,
    /// The Verse page shows ([`Layer`] loads the world only then).
    pub open: bool,
    pub surface: Option<GridSurface>,
    pub rect: Rect,
    controls: Controls,
    relay: String,
    root: PathBuf,
    fixture: bool,
    publication: Option<fixture::Publication>,
    visible: bool,
    focused: bool,
    suspended: bool,
    loading: Option<Receiver<Result<store::Launch, String>>>,
    connection: Option<Receiver<Result<Option<String>, String>>>,
    persistence: Option<Receiver<Result<(), String>>>,
    pending_persistence: VecDeque<Box<dyn FnOnce() -> Result<(), String> + Send>>,
    panel_commands: Vec<Command>,
    panel_page: usize,
    last_projection: u64,
    notice: Option<String>,
    pub graphics_available: bool,
    pub viewport: (f32, f32, f32),
    /// The Verse covers the whole window: no sidebar or title bar (#10116).
    /// The window sets it ([`crate::grid`]'s page offers the toggle).
    pub full: bool,
    /// How far an open board is scrolled, in points.
    panel_scroll: f32,
    /// Where a deck's slide shows the Grid behind it, in points (the
    /// Episode 289 title slide): the layer draws the world there, touring
    /// the plaza, whatever page is under the slide viewer.
    pub deck: Option<Rect>,
}

impl Grid {
    pub fn new(relay: String, root: PathBuf, fixture: bool) -> Shared {
        Rc::new(RefCell::new(Self {
            playing: false,
            open: false,
            surface: None,
            rect: Rect::default(),
            controls: Controls::default(),
            relay,
            root,
            fixture,
            publication: None,
            visible: true,
            focused: true,
            suspended: false,
            loading: None,
            connection: None,
            persistence: None,
            pending_persistence: VecDeque::new(),
            panel_commands: Vec::new(),
            panel_page: 0,
            last_projection: 0,
            notice: None,
            graphics_available: true,
            viewport: (1200.0, 840.0, 1.0),
            full: false,
            panel_scroll: 0.0,
            deck: None,
        }))
    }

    fn start(&mut self) {
        if self.playing || !self.graphics_available {
            return;
        }
        self.playing = true;
        self.notice = None;
        let (tx, rx) = mpsc::sync_channel(1);
        let root = if self.fixture {
            store::fixture_root()
        } else {
            self.root.clone()
        };
        let relay = self.relay.clone();
        let fixture = self.fixture;
        match std::thread::Builder::new()
            .name("grid-identity".into())
            .spawn(move || {
                let _ = tx.send(store::launch(&root, &relay, fixture));
            }) {
            Ok(_) => self.loading = Some(rx),
            Err(_) => self.offline("Couldn't start Play. Try again.".into()),
        }
    }

    fn offline(&mut self, reason: String) {
        self.notice = Some(reason);
        self.mount(store::Launch {
            publication: None,
            presence: None,
            gym: coder_mobile::BareGym {
                panel: true,
                results_panel: true,
                evals_panel: true,
                ..coder_mobile::BareGym::default()
            },
        });
    }

    fn mount(&mut self, launch: store::Launch) {
        self.publication = launch.publication;
        let viewport = Viewport::new(800, 500, self.viewport.2).expect("bounded initial viewport");
        match GridSurface::new(viewport, launch.presence, launch.gym).and_then(|mut surface| {
            surface.active(self.visible && self.focused && !self.suspended)?;
            Ok(surface)
        }) {
            Ok(surface) => {
                self.surface = Some(surface);
                self.controls.focused = self.focused;
            }
            Err(error) => {
                self.notice = Some(error);
                self.playing = false;
            }
        }
    }

    pub fn poll(&mut self) -> bool {
        if let Some(reply) = self.loading.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.loading = None;
            match reply {
                Ok(launch) => self.mount(launch),
                Err(error) => self.offline(error),
            }
        }
        if let Some(reply) = self.connection.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.connection = None;
            match reply {
                Ok(Some(code)) => {
                    let result = self
                        .surface
                        .as_mut()
                        .ok_or("Play is closed".to_owned())
                        .and_then(|surface| surface.command(Command::Configure(code.clone())));
                    match result {
                        Ok(()) => {
                            self.notice = None;
                            if !self.fixture {
                                self.persist(move || store::save_connection(&code));
                            }
                        }
                        Err(error) => self.notice = Some(error),
                    }
                }
                Err(error) => self.notice = Some(error),
                Ok(None) => {}
            }
        }
        if let Some(reply) = self.persistence.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.persistence = None;
            if let Err(error) = reply {
                self.notice = Some(error);
            }
            self.save_next();
        }
        if self.surface.as_ref().is_some_and(|s| s.panel().is_some()) {
            self.controls.clear();
        }
        use std::hash::{Hash, Hasher};
        let mut state = std::collections::hash_map::DefaultHasher::new();
        self.playing.hash(&mut state);
        self.notice.hash(&mut state);
        self.controls.focused.hash(&mut state);
        self.panel_page.hash(&mut state);
        self.panel_scroll.to_bits().hash(&mut state);
        self.full.hash(&mut state);
        if let Some(surface) = &self.surface {
            surface.status().hash(&mut state);
            surface.view_revision().hash(&mut state);
            surface.panel().map(|panel| panel as u8).hash(&mut state);
        }
        let revision = state.finish();
        let changed = revision != self.last_projection;
        self.last_projection = revision;
        changed
    }

    pub fn needs_tick(&self) -> bool {
        (self.playing && self.visible && self.focused && !self.suspended)
            || self.loading.is_some()
            || self.persistence.is_some()
            || self.connection.is_some()
    }

    fn persist(&mut self, action: impl FnOnce() -> Result<(), String> + Send + 'static) {
        self.pending_persistence.push_back(Box::new(action));
        self.save_next();
    }

    fn save_next(&mut self) {
        if self.persistence.is_some() {
            return;
        }
        let Some(action) = self.pending_persistence.pop_front() else {
            return;
        };
        let (tx, rx) = mpsc::sync_channel(1);
        self.persistence = Some(rx);
        if std::thread::Builder::new()
            .name("grid-preferences".into())
            .spawn(move || {
                let _ = tx.send(action());
            })
            .is_err()
        {
            self.persistence = None;
            self.notice = Some("Could not save the Grid setting".into());
        }
    }

    pub fn stop(&mut self) {
        if let Some(surface) = &mut self.surface {
            let _ = surface.active(false);
        }
        self.surface = None;
        self.publication = None;
        self.playing = false;
        self.loading = None;
        self.connection = None;
        self.controls.clear();
        self.panel_commands.clear();
        self.panel_page = 0;
        self.panel_scroll = 0.0;
    }

    pub fn visible(&mut self, visible: bool) {
        self.visible = visible;
        if !visible {
            self.controls.clear();
        }
        if let Some(surface) = &mut self.surface
            && let Err(error) = surface.active(visible && self.focused && !self.suspended)
        {
            self.notice = Some(error);
        }
    }

    /// The Verse page opened (`true`) or closed. Closing stops Play.
    pub fn set_open(&mut self, open: bool) {
        if self.open != open {
            self.open = open;
            if !open {
                self.stop();
            }
        }
    }

    pub fn suspend(&mut self, suspended: bool) {
        if self.suspended == suspended {
            return;
        }
        self.suspended = suspended;
        self.controls.clear();
        if let Some(surface) = &mut self.surface {
            let _ = surface.active(self.visible && self.focused && !suspended);
        }
    }

    pub fn capture(&self) -> bool {
        self.playing && self.visible && self.focused && !self.suspended && self.controls.capture()
    }
    pub fn capture_failed(&mut self) {
        self.controls.clear();
        self.notice = Some("Mouse capture is unavailable. Click the world to use keyboard movement; wheel zoom still works.".into());
    }
    pub fn graphics_failed(&mut self, error: &str) {
        self.stop();
        self.graphics_available = false;
        self.notice = Some(format!("The world renderer stopped: {error}"));
    }

    pub fn input(&mut self, event: NativeInput<'_>, now: Instant) -> bool {
        if let NativeInput::Focus(focused) = event {
            self.focused = focused;
            if let Some(surface) = &mut self.surface {
                let _ = surface.active(self.visible && focused && !self.suspended);
            }
        }
        if !self.playing || !self.visible || self.suspended {
            return false;
        }
        if let NativeInput::Key {
            code: "Escape",
            pressed: true,
            ..
        } = event
            && let Some(surface) = &mut self.surface
            && surface.panel().is_some()
        {
            let _ = surface.command(Command::Close);
            self.controls.clear();
            self.panel_scroll = 0.0;
            return true;
        }
        if self.surface.as_ref().is_some_and(|s| s.panel().is_some()) {
            self.controls.clear();
            // The wheel over the world scrolls the open board.
            if let NativeInput::Wheel { lines, x, y } = event
                && self.rect.contains(x, y)
                && lines.is_finite()
            {
                self.panel_scroll = (self.panel_scroll - lines * 40.0).clamp(0.0, 4000.0);
                return true;
            }
            return false;
        }
        let inside = match event {
            NativeInput::Button { x, y, .. }
            | NativeInput::Cursor { x, y }
            | NativeInput::Wheel { x, y, .. } => self.rect.contains(x, y),
            _ => self.focused,
        };
        if let NativeInput::Button { pressed: true, .. } = event
            && inside
        {
            self.controls.focused = true;
        }
        let (consumed, effect) = self.controls.event(event, inside, now);
        if let Some(surface) = &mut self.surface {
            let result = match effect {
                Some(Effect::Camera(action)) => surface.camera(action),
                Some(Effect::Click(point)) => surface
                    .click(point[0] - self.rect.x, point[1] - self.rect.y)
                    .map(|_| ()),
                _ => Ok(()),
            };
            if let Err(error) = result {
                self.notice = Some(error);
            }
            if surface.panel().is_some() {
                self.controls.clear();
            }
        }
        consumed
    }

    pub fn activate(&mut self, key: &str) {
        match key {
            "panel-close" => {
                if let Some(surface) = &mut self.surface {
                    let _ = surface.command(Command::Close);
                }
                self.controls.clear();
                self.panel_page = 0;
                self.panel_scroll = 0.0;
            }
            "panel-next" => {
                self.panel_page = self.panel_page.saturating_add(1);
                self.panel_scroll = 0.0;
            }
            "panel-previous" => {
                self.panel_page = self.panel_page.saturating_sub(1);
                self.panel_scroll = 0.0;
            }
            "play" => self.start(),
            "watch" => self.stop(),
            "connect"
                if self
                    .surface
                    .as_ref()
                    .is_some_and(|s| s.panel() == Some(Panel::Gym)) =>
            {
                if self.connection.is_none() {
                    let (tx, rx) = mpsc::sync_channel(1);
                    self.connection = Some(rx);
                    if std::thread::Builder::new()
                        .name("grid-connection-file".into())
                        .spawn(move || {
                            let _ = tx.send(store::pick_connection());
                        })
                        .is_err()
                    {
                        self.connection = None;
                        self.notice = Some("Could not open the connection chooser".into());
                    }
                }
            }
            "copy-key" => {
                if let Some(surface) = &self.surface {
                    let key = surface.identity_key();
                    let _ = std::thread::Builder::new()
                        .name("grid-copy-key".into())
                        .spawn(move || {
                            rust_native_desktop::input::copy(&key);
                        });
                }
            }
            _ => {
                if let Some(command) = key
                    .strip_prefix("panel-")
                    .and_then(|n| n.parse::<usize>().ok())
                    .and_then(|n| self.panel_commands.get(n))
                    .cloned()
                {
                    if matches!(
                        command,
                        Command::Run(_)
                            | Command::Recipe(_)
                            | Command::Back
                            | Command::Close
                            | Command::Results(
                                verse::gym_results::Action::Board { .. }
                                    | verse::gym_results::Action::Attempt { .. }
                                    | verse::gym_results::Action::Trace
                                    | verse::gym_results::Action::Back
                                    | verse::gym_results::Action::Tab { .. }
                            )
                    ) {
                        self.panel_page = 0;
                        self.panel_scroll = 0.0;
                    }
                    let notes = match command {
                        Command::Notes(on) => Some(on),
                        _ => None,
                    };
                    if let Some(surface) = &mut self.surface {
                        match surface.command(command) {
                            Ok(()) => {
                                self.controls.clear();
                                self.notice = None;
                                if let Some(on) = notes
                                    && !self.fixture
                                {
                                    let root = self.root.clone();
                                    self.persist(move || store::save_notes(&root, on));
                                }
                            }
                            Err(error) => self.notice = Some(error),
                        }
                    }
                }
            }
        }
    }

    /// The Verse page (#10116): the world fills the whole page, which the
    /// window's GPU layer draws behind it ([`Layer`] names this node), and
    /// the controls lie over it: Watch, Play, the status line, and Full
    /// screen at the top left, an open board in the middle, and the key
    /// hint, dim, at the bottom.
    pub fn view(&mut self) -> Node<Intent> {
        let mut watch = chip_button("watch", "Watch", !self.playing);
        let mut play = chip_button("play", "Play", self.playing);
        for button in [&mut watch, &mut play] {
            button.style.radius = Some(6);
        }
        if let Element::Button { enabled, .. } = &mut play.element {
            *enabled = self.graphics_available;
        }
        let status = if !self.graphics_available {
            Some("Play is unavailable on this graphics device.".to_owned())
        } else if !self.playing {
            Some("Watching the shared Grid".to_owned())
        } else if let Some(surface) = &self.surface {
            Some(format!(
                "{} · {}",
                surface.status(),
                if self.controls.focused {
                    "World controls active"
                } else {
                    "Click the world to move"
                }
            ))
        } else {
            Some("Starting the Grid…".to_owned())
        };
        let mut top = vec![watch, play];
        if let Some(status) = status {
            top.push(chip("grid-status", status, 0.9));
        }
        let mut gap = column("grid-top-gap", vec![]);
        gap.style.gap = None;
        top.push(gap);
        top.push(chip_button(
            "full",
            if self.full {
                "Exit full screen"
            } else {
                "Full screen"
            },
            false,
        ));
        let mut top = row("grid-top", top);
        if let Element::Stack { axis, .. } = &mut top.element {
            *axis = Axis::Horizontal;
        }
        top.style.gap = None;
        top.style.gap_points = Some(6);
        let mut children = vec![top];
        if let Some(notice) = &self.notice {
            children.push(alone(chip("grid-notice", notice.clone(), 1.0)));
        }
        self.panel_commands.clear();
        let mut middle = column("grid-middle", vec![]);
        let mut hint = None;
        if !self.playing {
            hint = Some("Watch the shared Grid from above. Choose Play to join it.");
        } else if let Some(surface) = &self.surface {
            let panel = surface.panel();
            if panel.is_none() {
                hint = Some(
                    "WASD move · right-drag look · left-drag orbit · Space jump · Shift sprint · wheel zoom and first person · Esc release mouse",
                );
            } else {
                let mut projection = panels::Projection::default();
                match panel {
                    Some(Panel::Gym) => {
                        if let Some(view) = surface.gym() {
                            projection.gym(view);
                        }
                    }
                    Some(Panel::Results) => {
                        if let Some(view) = surface.results() {
                            projection.results(view);
                        }
                    }
                    Some(Panel::Evals) => {
                        if let Some(view) = surface.evals() {
                            projection.evals(view, surface.league());
                        }
                    }
                    None => {}
                }
                // Bound retained nodes even when a publication contains many
                // tasks or the sidebar already holds 512 conversations.
                const PAGE: usize = 24;
                let pages = projection.nodes.len().div_ceil(PAGE).max(1);
                self.panel_page = self.panel_page.min(pages - 1);
                let mut board = Vec::new();
                if pages > 1 {
                    let mut navigation = vec![text(
                        "grid-panel-page",
                        format!("Board page {} of {}", self.panel_page + 1, pages),
                        TextRole::Status,
                    )];
                    if self.panel_page > 0 {
                        navigation.push(control("panel-previous", "Previous board page"));
                    }
                    if self.panel_page + 1 < pages {
                        navigation.push(control("panel-next", "Next board page"));
                    }
                    navigation.push(control("panel-close", "Close board"));
                    board.push(row("grid-panel-pages", navigation));
                }
                board.extend(
                    projection
                        .nodes
                        .into_iter()
                        .skip(self.panel_page * PAGE)
                        .take(PAGE),
                );
                self.panel_commands = projection.commands;
                // The board is a card over the world, scrolled by the wheel
                // over it ([`Grid::input`]).
                middle = column("grid-board", board);
                middle.style.background = Some(Color {
                    alpha: 235,
                    ..openagents_chat_app::visual::current().canvas
                });
                middle.style.border = Some(openagents_chat_app::visual::current().border);
                middle.style.radius = Some(10);
                middle.style.padding_points = Some([12, 14, 12, 14]);
                middle.style.viewport = Some(rust_native::style::Viewport {
                    // It fills what the page leaves; this is only the
                    // view's bound.
                    max_height: 4096,
                    offset: self.panel_scroll.round().clamp(0.0, f32::from(u16::MAX)) as u16,
                    fade: 12,
                });
            }
        }
        middle.style.fill_height = Some(true);
        children.push(middle);
        if let Some(hint) = hint {
            let mut hint = alone(chip("grid-controls", hint, 0.6));
            hint.style.align = Some(TextAlign::Center);
            children.push(hint);
        }
        let mut page = column(WORLD, children);
        page.style.gap = None;
        page.style.gap_points = Some(8);
        page.style.fill_height = Some(true);
        page.style.padding_points = Some([12, 12, 12, 12]);
        page
    }
}

/// A small dark chip over the world, with its text at `strength` of full
/// brightness: the status line full, the key hint dim.
fn chip(key: &str, value: impl Into<String>, strength: f32) -> Node<Intent> {
    let mut line = text(&format!("{key}-text"), value, TextRole::Status);
    line.style.text_size = Some(12);
    line.style.line_height = Some(16);
    line.style.foreground = Some(Color {
        alpha: (255.0 * strength.clamp(0.0, 1.0)).round() as u8,
        ..openagents_chat_app::visual::current().text
    });
    // A vertical stack is as wide as its text, where a row would take
    // the whole line.
    let mut chip = Node {
        key: key.into(),
        style: Style::default(),
        element: Element::Stack {
            axis: Axis::Vertical,
            children: vec![line],
        },
    };
    chip.style.intrinsic_width = Some(true);
    chip.style.background = Some(Color {
        alpha: (150.0 * strength.clamp(0.0, 1.0)).round().max(90.0) as u8,
        ..openagents_chat_app::visual::pick(
            Color::rgb(10, 10, 10),
            openagents_chat_app::visual::current().canvas,
        )
    });
    chip.style.radius = Some(6);
    chip.style.padding_points = Some([5, 10, 5, 10]);
    chip
}

/// A small button over the world; `on` marks the current mode.
fn chip_button(key: &str, label: &str, on: bool) -> Node<Intent> {
    let mut button = control(key, label);
    button.style.text_size = Some(12);
    button.style.line_height = Some(16);
    button.style.button_padding = Some([10, 5]);
    button.style.radius = Some(6);
    button.style.weight = Some(TextWeight::Medium);
    if on {
        button.style.background = Some(openagents_chat_app::visual::current().text);
        button.style.foreground = Some(openagents_chat_app::visual::pick(
            Color::rgb(20, 20, 20),
            openagents_chat_app::visual::current().on_text,
        ));
    } else {
        button.style.background = Some(Color {
            alpha: 170,
            ..openagents_chat_app::visual::pick(
                Color::rgb(10, 10, 10),
                openagents_chat_app::visual::current().canvas,
            )
        });
        button.style.foreground = Some(openagents_chat_app::visual::current().text);
        button.style.hover_background = Some(Color {
            alpha: 220,
            ..openagents_chat_app::visual::pick(
                Color::rgb(40, 40, 40),
                openagents_chat_app::visual::current().selected,
            )
        });
    }
    button
}

/// `node` on a row of its own, keeping its own width.
fn alone(node: Node<Intent>) -> Node<Intent> {
    let key = format!("{}-row", node.key);
    let mut row = column(&key, vec![node]);
    row.style.gap = None;
    row
}

pub fn control(key: &str, label: &str) -> Node<Intent> {
    Node {
        key: format!("grid-{key}"),
        style: Style::default(),
        element: Element::Button {
            shortcut: None,
            label: label.into(),
            icon: None,
            enabled: true,
            intent: Intent::Grid { key: key.into() },
        },
    }
}
pub fn text(key: &str, value: impl Into<String>, role: TextRole) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style::default(),
        element: Element::Text {
            value: value.into(),
            role,
        },
    }
}
pub fn column(key: &str, children: Vec<Node<Intent>>) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            gap: Some(Space::Sm),
            ..Style::default()
        },
        element: Element::Stack {
            axis: Axis::Vertical,
            children,
        },
    }
}
pub fn row(key: &str, children: Vec<Node<Intent>>) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            gap: Some(Space::Sm),
            ..Style::default()
        },
        element: Element::Stack {
            axis: Axis::Wrap,
            children,
        },
    }
}

/// Makes the spectator that Watch draws, each time the Verse page opens.
pub type Watcher = Box<dyn FnMut() -> crate::backdrop::GridBackdrop>;

/// The window's GPU layer for the Verse page (#10071).
///
/// Nothing of the world is loaded, connected, or drawn while another page
/// shows: the window's background is plain behind chat, Settings, and the
/// rest. Opening the Verse page makes a spectator with `watcher` and
/// connects it; leaving the page drops it with its relay connection and
/// GPU resources, and stops Play.
pub struct Layer {
    grid: Shared,
    watcher: Option<Watcher>,
    watch: Option<crate::backdrop::GridBackdrop>,
    /// Play's engine renderer on the window's device (#10606).
    player: Option<verse::grid_engine::GridEngine>,
    player_scale: f32,
    /// The glyph atlas revision the engine last uploaded.
    player_atlas: u64,
    started: Instant,
    last: Option<Instant>,
    playing: bool,
    visible: bool,
    open: bool,
}
impl Layer {
    /// `watcher` is `None` when the person asked for no world at all
    /// (`--no-backdrop`): the Verse page then shows only its controls.
    pub fn new(grid: Shared, watcher: Option<Watcher>) -> Self {
        Self {
            grid,
            watcher,
            watch: None,
            player: None,
            player_scale: 0.0,
            player_atlas: 0,
            started: Instant::now(),
            last: None,
            playing: false,
            visible: true,
            open: false,
        }
    }

    /// Whether the world is loaded: the Verse page shows.
    pub fn loaded(&self) -> bool {
        self.watch.is_some() || self.player.is_some()
    }

    /// Follows the page: loads the spectator when the Verse page opens and
    /// releases everything when it closes.
    fn follow(&mut self, now: Instant) {
        let open = {
            let grid = self.grid.borrow();
            grid.open || grid.deck.is_some()
        };
        if open == self.open {
            return;
        }
        self.open = open;
        self.player = None;
        self.last = None;
        self.playing = false;
        if open {
            self.watch = self.watcher.as_mut().map(|watcher| watcher());
            let shown = self.visible && !self.playing();
            if let Some(watch) = &mut self.watch {
                watch.shown(shown, now);
            }
        } else {
            self.watch = None;
        }
    }

    /// Whether Play draws: the person plays and no deck slide shows the
    /// Grid over the page (the slide viewer then takes every key).
    fn playing(&self) -> bool {
        let grid = self.grid.borrow();
        grid.playing && grid.deck.is_none()
    }
}
impl Backdrop for Layer {
    /// The Verse page's own node, which fills the content pane, or the
    /// whole window in full screen (#10116).
    fn surface(&self) -> Option<&str> {
        let grid = self.grid.borrow();
        (grid.open && !grid.full).then_some(WORLD)
    }
    /// A deck slide's place, when one shows the Grid behind it.
    fn rect(&self) -> Option<Rect> {
        self.grid.borrow().deck
    }
    fn look(&self) -> Option<Look> {
        if self.grid.borrow().deck.is_some() {
            // Behind a slide: sharp, under a 50% black overlay so the title reads.
            return Some(if self.watch.is_some() {
                Look {
                    dim: 0.5,
                    blur: 0.0,
                    scale: 1.0,
                }
            } else {
                Look {
                    dim: 1.0,
                    blur: 0.0,
                    scale: 0.25,
                }
            });
        }
        if !self.grid.borrow().open {
            // Another page: the plain background, as a window without a
            // backdrop has, with the smallest texture.
            Some(Look {
                dim: 1.0,
                blur: 0.0,
                scale: 0.25,
            })
        } else if self.watch.is_none() && !self.grid.borrow().playing {
            Some(Look {
                dim: 1.0,
                blur: 0.0,
                scale: 0.25,
            })
        } else {
            // The world is the page (#10116): Watch and Play both show it
            // sharp and undimmed; the controls over it carry their own
            // dark chips.
            Some(Look {
                dim: 0.0,
                blur: 0.0,
                scale: 1.0,
            })
        }
    }
    fn viewport(&mut self, rect: Rect, scale: f32) {
        let mut grid = self.grid.borrow_mut();
        if grid.deck.is_some() {
            // A slide's place, not the Verse page's.
            return;
        }
        grid.rect = rect;
        if rect.w <= 0.0 || rect.h <= 0.0 {
            return;
        }
        if let Some(surface) = &mut grid.surface {
            match Viewport::new(
                (rect.w * scale).round() as u32,
                (rect.h * scale).round() as u32,
                scale,
            )
            .and_then(|v| {
                surface
                    .resize(v)
                    .map_err(|_| rust_native::surface::SurfaceError::Viewport)
            }) {
                Ok(()) => {}
                Err(error) => grid.notice = Some(error.to_string()),
            }
        }
    }
    fn shown(&mut self, visible: bool, now: Instant) {
        self.visible = visible;
        self.last = None;
        self.grid.borrow_mut().visible(visible);
        let playing = self.playing();
        if let Some(watch) = &mut self.watch {
            watch.shown(visible && !playing, now);
        }
    }
    fn next_frame(&mut self, now: Instant) -> Option<Instant> {
        self.follow(now);
        if !self.open {
            return None;
        }
        let playing = self.playing();
        if let Some(watch) = &mut self.watch {
            watch.set_tour(self.grid.borrow().deck.is_some());
        }
        if playing != self.playing {
            self.playing = playing;
            self.player = None;
            self.last = None;
            if let Some(watch) = &mut self.watch {
                if playing {
                    watch.pause(now);
                } else {
                    watch.shown(self.visible, now);
                }
            }
        }
        if playing {
            (self.visible && self.grid.borrow().focused && !self.grid.borrow().suspended)
                .then(|| self.last.map_or(now, |last| last + FRAME))
        } else {
            self.watch.as_mut().and_then(|watch| watch.next_frame(now))
        }
    }
    fn draw(
        &mut self,
        gpu: &Gpu<'_>,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        now: Instant,
    ) -> Result<(), String> {
        self.follow(now);
        if !self.open {
            // Hidden under the plain background (`look`); nothing to draw.
            return Ok(());
        }
        let playing = self.playing();
        let mut grid = self.grid.borrow_mut();
        if playing && grid.surface.is_some() {
            let input = grid.controls.input();
            let surface = grid.surface.as_mut().expect("a player surface");
            let dt = surface
                .update(
                    now.saturating_duration_since(self.started).as_secs_f64(),
                    input,
                )?
                .unwrap_or(0.0);
            if self.player_scale != surface.scale() {
                self.player = None;
                self.player_scale = surface.scale();
            }
            let atlas = surface.atlas().revision();
            if self.player_atlas != atlas
                && let Some(engine) = &mut self.player
                && !engine.update_atlas(surface.atlas())
            {
                // The atlas grew: only a new engine takes it.
                self.player = None;
            }
            self.player_atlas = atlas;
            let engine = match &mut self.player {
                Some(engine) => {
                    if engine.size() != [size.0 as f32, size.1 as f32] {
                        engine.resize(size.0, size.1)?;
                    }
                    engine
                }
                None => self
                    .player
                    .insert(verse::grid_engine::GridEngine::on_device(
                        gpu.adapter,
                        gpu.device,
                        gpu.queue,
                        rust_native_desktop::backdrop::FORMAT,
                        verse::grid_engine::Content::grid()?,
                        surface.atlas(),
                        size.0,
                        size.1,
                    )?),
            };
            let frame = surface.engine_frame(dt);
            let drawn = engine.encode(
                encoder,
                target,
                frame.view,
                &frame.instances,
                &frame.ui,
                &frame.lighting,
            );
            if drawn.is_err() {
                // A lost device: the window opens a new one, and the next
                // frame a new engine on it.
                self.player = None;
            }
            drawn?;
            self.last = Some(now);
            Ok(())
        } else if !playing && let Some(watch) = &mut self.watch {
            watch.set_tour(grid.deck.is_some());
            drop(grid);
            watch.draw(gpu, encoder, target, size, now)
        } else {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Grid loading"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            self.last = Some(now);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn the_world_loads_only_on_the_verse_page_and_is_released_when_left() {
        let home = tempfile::tempdir().unwrap();
        let grid = Grid::new("ws://127.0.0.1:1".into(), home.path().into(), true);
        let made = Rc::new(Cell::new(0));
        let count = made.clone();
        let watcher: Watcher = Box::new(move || {
            count.set(count.get() + 1);
            crate::backdrop::GridBackdrop::new("ws://127.0.0.1:1", Box::new(|| false))
        });
        let mut layer = Layer::new(grid.clone(), Some(watcher));
        let now = Instant::now();
        // Chat and every other page: nothing made, no frame, the plain background.
        layer.shown(true, now);
        for _ in 0..3 {
            assert_eq!(layer.next_frame(now), None);
        }
        assert_eq!(made.get(), 0);
        assert!(!layer.loaded());
        assert_eq!(layer.surface(), None);
        assert_eq!(
            layer.look(),
            Some(Look {
                dim: 1.0,
                blur: 0.0,
                scale: 0.25
            })
        );
        // The Verse page: the spectator is made once, connects, and draws.
        grid.borrow_mut().set_open(true);
        assert!(layer.next_frame(now).is_some());
        assert!(layer.next_frame(now).is_some());
        assert_eq!(made.get(), 1);
        assert!(layer.loaded());
        assert!(layer.watch.as_ref().unwrap().connected());
        // The world is the page, sharp and undimmed, and fills it (#10116).
        assert_eq!(
            layer.look(),
            Some(Look {
                dim: 0.0,
                blur: 0.0,
                scale: 1.0
            })
        );
        assert_eq!(layer.surface(), Some(WORLD));
        grid.borrow_mut().full = true;
        assert_eq!(layer.surface(), None, "full screen: the whole window");
        grid.borrow_mut().full = false;
        // Left: everything is dropped, relay connection included, and Play stops.
        grid.borrow_mut().playing = true;
        grid.borrow_mut().set_open(false);
        assert!(!grid.borrow().playing);
        assert_eq!(layer.next_frame(now), None);
        assert!(!layer.loaded());
        assert!(layer.watch.is_none());
        // Opened again: a fresh spectator.
        grid.borrow_mut().set_open(true);
        assert!(layer.next_frame(now).is_some());
        assert_eq!(made.get(), 2);
    }

    #[test]
    fn without_a_world_the_verse_page_stays_plain() {
        let home = tempfile::tempdir().unwrap();
        let grid = Grid::new("ws://127.0.0.1:1".into(), home.path().into(), true);
        let mut layer = Layer::new(grid.clone(), None);
        grid.borrow_mut().set_open(true);
        assert_eq!(layer.next_frame(Instant::now()), None);
        assert!(!layer.loaded());
    }
}
