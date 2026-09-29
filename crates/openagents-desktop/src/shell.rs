//! The window: the model, presented as Rust Native views through
//! `rust-native-desktop`, with the code painted on its drawing surface.

use crate::platform;
use crate::worker::{Context, Worker};
use openagents_desktop::model::{Intent, Model, Outcome, Request};
use openagents_desktop::qr::{self, Modules, QUIET_ZONE};
use openagents_desktop::screens::{CODE_SURFACE, Presenter, root};
use rust_native::ValidatedView;
use rust_native::style::Color;
use rust_native_desktop::{App, Frame, PxRect, Waker};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The largest the code draws, in points.
pub const CODE_SIDE: f32 = 280.0;
/// How often the screen lock is checked while a code may show.
const LOCK_POLL: Duration = Duration::from_secs(1);

/// Where requests go: a background worker in the window, or inline in a
/// capture.
enum Runner {
    Pending(Option<Context>),
    Background(Worker),
    Inline(Context),
}

pub struct DesktopApp {
    model: Model,
    presenter: Presenter,
    runner: Runner,
    /// The QR modules of the code on screen.
    modules: Option<(String, Modules)>,
    /// Whether this is a window (it checks the screen lock) or a capture.
    live: bool,
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

impl DesktopApp {
    /// A window over `context`; the worker starts with the event loop.
    pub fn window(model: Model, context: Context) -> DesktopApp {
        DesktopApp::new(model, Runner::Pending(Some(context)), true)
    }

    /// A capture: requests run inline, one after another.
    pub fn inline(model: Model, context: Context) -> DesktopApp {
        DesktopApp::new(model, Runner::Inline(context), false)
    }

    fn new(model: Model, runner: Runner, live: bool) -> DesktopApp {
        let mut app = DesktopApp {
            model,
            presenter: Presenter::new("openagents-desktop"),
            runner,
            modules: None,
            live,
        };
        app.present();
        app
    }

    pub fn model(&self) -> &Model {
        &self.model
    }

    fn present(&mut self) {
        self.presenter.present(root(&self.model, unix_now()));
    }

    /// Sends requests; inline, runs them and applies what comes back.
    fn send(&mut self, requests: Vec<Request>, now: Instant) {
        let mut queue: std::collections::VecDeque<Request> = requests.into();
        while let Some(request) = queue.pop_front() {
            match &mut self.runner {
                Runner::Background(worker) => worker.send(request),
                Runner::Inline(context) => {
                    if let Some(outcome) = context.run(request) {
                        queue.extend(self.model.outcome(outcome, now));
                    }
                }
                Runner::Pending(_) => {}
            }
        }
    }

    fn apply(&mut self, outcomes: Vec<Outcome>, now: Instant) {
        for outcome in outcomes {
            let requests = self.model.outcome(outcome, now);
            self.send(requests, now);
        }
    }

    /// Runs a click, as the window would after resolving it.
    pub fn click(&mut self, intent: Intent, now: Instant) {
        self.activate(intent, now);
        self.tick(now);
    }
}

impl App for DesktopApp {
    type Intent = Intent;

    fn title(&self) -> String {
        "OpenAgents".into()
    }

    fn start(&mut self, waker: Waker) {
        crate::menubar::start(waker.clone());
        if let Runner::Pending(context) = &mut self.runner
            && let Some(context) = context.take()
        {
            self.runner = Runner::Background(Worker::start(context, waker));
        }
    }

    fn tick(&mut self, now: Instant) -> Option<Instant> {
        if let Runner::Background(worker) = &self.runner {
            let outcomes = worker.outcomes();
            self.apply(outcomes, now);
        }
        if self.live {
            self.model.set_locked(platform::screen_locked());
        }
        crate::menubar::sync(&self.model)
            .into_iter()
            .for_each(|intent| self.activate(intent, now));
        let requests = self.model.tick(now);
        self.send(requests, now);
        self.present();
        let mut wake = self.model.next_wake();
        if self.live {
            wake = wake.min(now + LOCK_POLL);
        }
        Some(wake)
    }

    fn view(&self) -> &ValidatedView<Intent> {
        self.presenter.view()
    }

    fn activate(&mut self, intent: Intent, now: Instant) {
        let requests = self.model.activate(intent, now);
        self.send(requests, now);
        self.present();
    }

    fn shown(&mut self, visible: bool, now: Instant) {
        self.model.shown(visible, now);
        let requests = self.model.tick(now);
        self.send(requests, now);
        self.present();
    }

    fn input(&mut self, now: Instant) {
        self.model.input(now);
    }

    fn surface_size(&self, resource: &str, available: f32) -> Option<(f32, f32)> {
        (resource == CODE_SURFACE).then(|| {
            let side = available.min(CODE_SIDE);
            (side, side)
        })
    }

    fn paint_surface(&mut self, resource: &str, frame: &mut Frame, rect: PxRect) {
        if resource != CODE_SURFACE {
            return;
        }
        let Some(shown) = self.model.codes.shown() else {
            return;
        };
        if self
            .modules
            .as_ref()
            .is_none_or(|(text, _)| *text != shown.text)
        {
            self.modules = qr::modules(&shown.text).map(|modules| (shown.text.clone(), modules));
        }
        let Some((_, modules)) = &self.modules else {
            return;
        };
        paint_code(frame, rect, modules);
    }
}

/// Paints `modules` black on a white rounded square filling `rect`, with
/// the quiet zone, on whole pixels so every module is sharp.
pub fn paint_code(frame: &mut Frame, rect: PxRect, modules: &Modules) {
    let white = Color::rgb(255, 255, 255);
    let black = Color::rgb(0, 0, 0);
    frame.fill(rect, rect.w * 0.04, white);
    let count = modules.size + 2 * QUIET_ZONE;
    let module = (rect.w.min(rect.h) / count as f32).floor().max(1.0);
    let side = module * modules.size as f32;
    let x0 = (rect.x + (rect.w - side) / 2.0).round();
    let y0 = (rect.y + (rect.h - side) / 2.0).round();
    for y in 0..modules.size {
        for x in 0..modules.size {
            if modules.get(x, y) {
                frame.fill(
                    PxRect {
                        x: x0 + x as f32 * module,
                        y: y0 + y as f32 * module,
                        w: module,
                        h: module,
                    },
                    0.0,
                    black,
                );
            }
        }
    }
}
