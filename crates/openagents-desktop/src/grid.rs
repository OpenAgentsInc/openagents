//! Play and Watch share one window. Only an explicit Play session holds a
//! world identity; the spectator remains an independent read-only reader.
mod controls;
mod fixture;
mod panels;
mod store;

use crate::model::Intent;
use coder_mobile::verse_surface::{Command, GridSurface, Panel};
use controls::{Controls, Effect};
use rust_native::style::{Space, Style};
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

pub const WORLD: &str = "grid-world";
pub const FRAME: Duration = Duration::from_nanos(1_000_000_000 / 60);
pub type Shared = Rc<RefCell<Grid>>;

pub struct Grid {
    pub playing: bool,
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
}

impl Grid {
    pub fn new(relay: String, root: PathBuf, fixture: bool) -> Shared {
        Rc::new(RefCell::new(Self {
            playing: false,
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
            Err(_) => self.offline("Could not start the world identity worker".into()),
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
            return true;
        }
        if self.surface.as_ref().is_some_and(|s| s.panel().is_some()) {
            self.controls.clear();
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
            }
            "panel-next" => self.panel_page = self.panel_page.saturating_add(1),
            "panel-previous" => self.panel_page = self.panel_page.saturating_sub(1),
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

    pub fn view(&mut self) -> Node<Intent> {
        let mut play = control("play", "Play");
        if let Element::Button { enabled, .. } = &mut play.element {
            *enabled = self.graphics_available;
        }
        let mut children = vec![row("grid-modes", vec![control("watch", "Watch"), play])];
        if !self.graphics_available {
            children.push(text(
                "grid-unavailable",
                "Play is unavailable on this graphics device.",
                TextRole::Status,
            ));
        }
        if let Some(notice) = &self.notice {
            children.push(text("grid-notice", notice, TextRole::Status));
        }
        self.panel_commands.clear();
        if !self.playing {
            children.push(text(
                "grid-watch-help",
                "Watch the shared Grid from above. Choose Play to join it.",
                TextRole::Body,
            ));
        } else if let Some(surface) = &self.surface {
            let panel = surface.panel();
            children.push(text(
                "grid-status",
                format!(
                    "{} · {}",
                    surface.status(),
                    if self.controls.focused {
                        "World controls active"
                    } else {
                        "Click the world to move"
                    }
                ),
                TextRole::Status,
            ));
            children.push(Node {
                key: "grid-viewport".into(),
                style: Style::default(),
                element: Element::Surface {
                    resource: WORLD.into(),
                    label: "Playable Grid".into(),
                },
            });
            if panel.is_none() {
                children.push(text("grid-controls", "WASD move · right-drag look · left-drag orbit · Space jump · Shift sprint · wheel zoom and first person · Esc release mouse", TextRole::Status));
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
                            projection.evals(view);
                        }
                    }
                    None => {}
                }
                // Bound retained nodes even when a publication contains many
                // tasks or the sidebar already holds 512 conversations.
                const PAGE: usize = 24;
                let pages = projection.nodes.len().div_ceil(PAGE).max(1);
                self.panel_page = self.panel_page.min(pages - 1);
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
                    children.push(row("grid-panel-pages", navigation));
                }
                children.extend(
                    projection
                        .nodes
                        .into_iter()
                        .skip(self.panel_page * PAGE)
                        .take(PAGE),
                );
                self.panel_commands = projection.commands;
            }
        } else {
            children.push(text(
                "grid-starting",
                "Starting the Grid…",
                TextRole::Status,
            ));
        }
        column("grid-page", children)
    }

    pub fn surface_height(&self) -> f32 {
        if self.surface.as_ref().is_some_and(|s| s.panel().is_some()) {
            160.0
        } else {
            (self.viewport.1 - 260.0).max(140.0)
        }
    }
}

pub fn control(key: &str, label: &str) -> Node<Intent> {
    Node {
        key: format!("grid-{key}"),
        style: Style::default(),
        element: Element::Button {
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

pub struct Layer {
    grid: Shared,
    watch: Option<crate::backdrop::GridBackdrop>,
    player: Option<verse::render::Layer>,
    player_scale: f32,
    started: Instant,
    last: Option<Instant>,
    playing: bool,
    visible: bool,
}
impl Layer {
    pub fn new(grid: Shared, watch: Option<crate::backdrop::GridBackdrop>) -> Self {
        Self {
            grid,
            watch,
            player: None,
            player_scale: 0.0,
            started: Instant::now(),
            last: None,
            playing: false,
            visible: true,
        }
    }
}
impl Backdrop for Layer {
    fn surface(&self) -> Option<&str> {
        self.grid.borrow().playing.then_some(WORLD)
    }
    fn look(&self) -> Option<Look> {
        if self.grid.borrow().playing {
            Some(Look {
                dim: 0.0,
                blur: 0.0,
                scale: 1.0,
            })
        } else if self.watch.is_none() {
            Some(Look {
                dim: 1.0,
                blur: 0.0,
                scale: 0.25,
            })
        } else {
            None
        }
    }
    fn viewport(&mut self, rect: Rect, scale: f32) {
        let mut grid = self.grid.borrow_mut();
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
        if let Some(watch) = &mut self.watch {
            watch.shown(visible && !self.grid.borrow().playing, now);
        }
    }
    fn next_frame(&mut self, now: Instant) -> Option<Instant> {
        let playing = self.grid.borrow().playing;
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
        let mut grid = self.grid.borrow_mut();
        if grid.playing && grid.surface.is_some() {
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
            let layer = match &mut self.player {
                Some(layer) => {
                    layer.resize(gpu.device, size.0, size.1)?;
                    layer
                }
                None => self.player.insert(verse::render::Layer::new(
                    gpu.adapter,
                    gpu.device,
                    gpu.queue,
                    rust_native_desktop::backdrop::FORMAT,
                    size,
                    &surface.world().world.mesh,
                    surface.atlas(),
                    surface.world().atmosphere(),
                    4,
                )?),
            };
            let frame = surface.frame(dt);
            layer.encode(
                gpu.device,
                gpu.queue,
                encoder,
                target,
                frame.view,
                &frame.mesh,
                &frame.ui,
            )?;
            self.last = Some(now);
            Ok(())
        } else if !grid.playing
            && let Some(watch) = &mut self.watch
        {
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
