//! The window: the model, presented as Rust Native views through
//! `rust-native-desktop`, with the code painted on its drawing surface.

use crate::worker::{Context, ScreenLock, Worker};
use openagents_desktop::chrome::{self, Page, State};
use openagents_desktop::model::{Intent, Model, Outcome, Request};
use openagents_desktop::qr::{self, Modules, QUIET_ZONE};
use openagents_desktop::screens::{CODE_SURFACE, Presenter, root};
use rust_native::ValidatedView;
use rust_native::style::Color;
use rust_native_desktop::{
    App, Frame, KeyBinding, PxRect, SplitLayout, Theme, Waker, WindowLayout,
};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// The largest the code draws, in points.
pub const CODE_SIDE: f32 = 360.0;

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
    screen_lock: Option<ScreenLock>,
    navigation: Option<State>,
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

impl DesktopApp {
    /// A window over `context`; the worker starts with the event loop.
    pub fn window(model: Model, context: Context) -> DesktopApp {
        DesktopApp::new(model, Runner::Pending(Some(context)), true, true)
    }

    /// A capture: requests run inline, one after another.
    pub fn inline(model: Model, context: Context) -> DesktopApp {
        DesktopApp::new(model, Runner::Inline(context), false, false)
    }

    /// A capture of the desktop shell with sample chats and an inline host.
    pub fn inline_shell(model: Model, context: Context) -> DesktopApp {
        DesktopApp::new(model, Runner::Inline(context), false, true)
    }

    fn new(mut model: Model, runner: Runner, live: bool, chrome: bool) -> DesktopApp {
        // A pairing code is shown only after the person opens its screen.
        if chrome && model.screen == openagents_desktop::model::Screen::Connect {
            model.screen = openagents_desktop::model::Screen::Home;
        }
        let mut app = DesktopApp {
            model,
            presenter: Presenter::new("openagents-desktop"),
            runner,
            modules: None,
            live,
            screen_lock: None,
            navigation: chrome.then(State::default),
        };
        app.present();
        app
    }

    pub fn model(&self) -> &Model {
        &self.model
    }

    fn present(&mut self) {
        let root = self.navigation.as_ref().map_or_else(
            || root(&self.model, unix_now()),
            |state| chrome::root(state, &self.model, unix_now()),
        );
        self.presenter.present(root);
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

    fn theme(&self) -> Theme {
        if self.navigation.is_none() {
            return Theme::default();
        }
        Theme {
            background: Color::rgb(9, 11, 14),
            text: Color::rgb(230, 232, 235),
            muted: Color::rgb(150, 155, 163),
            rule: Color::rgb(43, 47, 53),
            focus: Color::rgb(184, 207, 231),
            button_radius: 7.0,
            body: 13.0,
            heading: 26.0,
            status: 11.0,
            column: 620.0,
            ..Theme::default()
        }
    }

    fn window_layout(&self) -> WindowLayout {
        self.navigation
            .as_ref()
            .map_or(WindowLayout::Column, |state| {
                WindowLayout::Split(SplitLayout {
                    leading_width: state.sidebar_width,
                    min_leading_width: chrome::SIDEBAR_MIN,
                    max_leading_width: chrome::SIDEBAR_MAX,
                    min_content_width: 360.0,
                    collapsed: state.collapsed,
                    center_content: true,
                })
            })
    }

    fn resize_leading_pane(&mut self, width: f32, _: Instant) {
        if let Some(state) = &mut self.navigation {
            state.resize(width);
            self.present();
        }
    }

    fn key_bindings(&self) -> &'static [KeyBinding] {
        if self.navigation.is_none() {
            return &[];
        }
        &[
            KeyBinding {
                key: "b",
                shift: false,
                node: "shell-toggle-sidebar",
            },
            KeyBinding {
                key: "n",
                shift: false,
                node: "shell-new-chat",
            },
        ]
    }

    fn start(&mut self, waker: Waker) {
        crate::menubar::start(waker.clone());
        if self.live {
            self.screen_lock = Some(ScreenLock::start(waker.clone()));
        }
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
        if let Some(screen_lock) = &self.screen_lock {
            self.model.set_locked(screen_lock.locked());
        }
        crate::menubar::sync(&self.model)
            .into_iter()
            .for_each(|intent| self.activate(intent, now));
        let requests = self.model.tick(now);
        self.send(requests, now);
        self.present();
        Some(self.model.next_wake())
    }

    fn view(&self) -> &ValidatedView<Intent> {
        self.presenter.view()
    }

    fn activate(&mut self, intent: Intent, now: Instant) {
        if let Intent::Navigate { action } = intent {
            if let Some(state) = &mut self.navigation {
                state.activate(action);
                if state.page != Page::Computers
                    && self.model.screen == openagents_desktop::model::Screen::Connect
                {
                    let requests = self.model.activate(Intent::Back, now);
                    self.send(requests, now);
                    let requests = self.model.tick(now);
                    self.send(requests, now);
                }
            }
            self.present();
            return;
        }
        if let Some(state) = &mut self.navigation {
            state.page = Page::Computers;
        }
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
        if resource == chrome::MARK {
            return Some((64.0_f32.min(available), 64.0));
        }
        (resource == CODE_SURFACE).then(|| {
            let side = available.min(CODE_SIDE);
            (side, side)
        })
    }

    fn paint_surface(&mut self, resource: &str, frame: &mut Frame, rect: PxRect) {
        if resource == chrome::MARK {
            frame.fill(rect, rect.w * 0.25, Color::rgb(29, 32, 38));
            frame.stroke(rect, rect.w * 0.25, rect.w / 64.0, Color::rgb(58, 64, 73));
            let color = Color::rgb(220, 225, 233);
            let p = |x, y| (rect.x + rect.w * x, rect.y + rect.h * y);
            frame.stroke(
                PxRect {
                    x: p(0.19, 0.3).0,
                    y: p(0.19, 0.3).1,
                    w: rect.w * 0.25,
                    h: rect.h * 0.4,
                },
                rect.w * 0.125,
                rect.w / 32.0,
                color,
            );
            frame.line(p(0.52, 0.7), p(0.67, 0.3), rect.w / 32.0, color);
            frame.line(p(0.67, 0.3), p(0.82, 0.7), rect.w / 32.0, color);
            frame.line(p(0.58, 0.55), p(0.76, 0.55), rect.w / 32.0, color);
            return;
        }
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
            self.modules =
                qr::code_modules(&shown.text).map(|modules| (shown.text.clone(), modules));
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

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_desktop::chrome::Action;
    use openagents_desktop::fake::FakeHost;
    use openagents_desktop::model::{Agent, Screen};
    use std::time::Duration;

    fn preview() -> (DesktopApp, FakeHost, Instant) {
        let fake = FakeHost::new("Test computer", unix_now());
        let context = Context::new(
            Box::new(fake.clone()),
            Some(fake.clone()),
            None,
            None,
            std::env::temp_dir(),
        );
        let now = Instant::now();
        let mut app =
            DesktopApp::inline_shell(Model::new(now, Screen::Connect, Agent::Enabled), context);
        app.tick(now);
        (app, fake, now)
    }

    /// The window as `--fake-host` runs it (worker threads, the shell,
    /// the Phones page), on a simulated clock for two hours: once the
    /// in-process host first answers, the window never loses it and never
    /// says Coder is starting, waiting, or not answering, and a code shows
    /// whenever the code screen is open.
    #[test]
    fn the_fake_host_window_never_waits_for_coder() {
        let fake = FakeHost::new("Studio Mac", unix_now());
        let home = tempfile::tempdir().expect("a home");
        let context = Context::new(
            Box::new(fake.clone()),
            Some(fake.clone()),
            None,
            None,
            home.path().to_path_buf(),
        );
        let start = Instant::now();
        let mut app =
            DesktopApp::window(Model::new(start, Screen::Connect, Agent::Enabled), context);
        app.start(rust_native_desktop::Waker::none());
        // Lets the worker answer what the last tick sent.
        let settle = |app: &mut DesktopApp, now: Instant| {
            for _ in 0..3 {
                std::thread::sleep(Duration::from_millis(2));
                app.tick(now);
            }
        };
        settle(&mut app, start);
        for _ in 0..500 {
            if app.model.host.is_some() {
                break;
            }
            settle(&mut app, start);
        }
        assert!(app.model.host.is_some(), "the fake never answered");
        app.activate(
            Intent::Navigate {
                action: Action::Computers,
            },
            start,
        );
        let mut seconds = 0;
        while seconds < 2 * 60 * 60 {
            seconds += 5;
            let now = start + Duration::from_secs(seconds);
            match seconds % 1_800 {
                5 => app.click(Intent::ConnectAnother, now),
                300 => app.click(Intent::Back, now),
                // A phone the fake lets in, now and then.
                600 => {
                    use openagents_desktop::control::HostControl;
                    let invite = fake.clone().invite().expect("an invite");
                    fake.redeem(&invite.invitation, "").expect("a phone");
                }
                _ => {}
            }
            settle(&mut app, now);
            assert!(app.model.host.is_some(), "no answer at {seconds}s");
            for text in openagents_desktop::screens::words(&app.view().view().root) {
                for said in ["Waiting for Coder", "Starting Coder", "answering"] {
                    assert!(!text.contains(said), "{text:?} at {seconds}s");
                }
            }
            if app.model.screen == Screen::Connect && app.model.codes.held().is_none() {
                assert!(app.model.codes.shown().is_some() || app.model.codes.waiting());
            }
        }
        assert!(app.model.phones().len() >= 4);
    }

    #[test]
    fn leaving_pairing_for_a_chat_cancels_the_code() {
        let (mut app, fake, now) = preview();
        let tasks = app.model.tasks.clone();
        assert!(
            fake.open().is_empty(),
            "the chat shell does not mint a code"
        );
        app.click(
            Intent::Navigate {
                action: Action::Computers,
            },
            now,
        );
        app.click(Intent::ConnectAnother, now);
        assert!(app.model.codes.shown().is_some());
        assert_eq!(fake.open().len(), 1);
        let old = rust_native::Activation {
            instance: app.view().view().instance.clone(),
            revision: app.view().view().revision,
            node: "copy".into(),
        };
        app.click(
            Intent::Navigate {
                action: Action::SelectChat { id: 3 },
            },
            now,
        );
        assert!(app.model.codes.shown().is_none());
        assert!(fake.open().is_empty());
        assert!(app.view().activate(&old).is_err());
        assert_eq!(app.model.tasks, tasks, "sample chats submit no work");
    }

    #[test]
    fn collapsed_and_narrow_shells_keep_their_controls_inside_the_window() {
        let (mut app, _, now) = preview();
        app.resize_leading_pane(400.0, now);
        let (_, scene) = rust_native_desktop::capture(&mut app, 760.0, 540.0, 1.0);
        assert!(scene.unsupported.is_empty());
        let split = scene.split.expect("split window");
        assert!(split.leading.limit > 0.0);
        for hit in scene.hits.iter().filter(|hit| hit.clip.is_none()) {
            assert!(
                hit.rect.x >= 0.0 && hit.rect.x + hit.rect.w <= 760.0,
                "{hit:?}"
            );
            assert!(
                hit.rect.y >= 0.0 && hit.rect.y + hit.rect.h <= 540.0,
                "{hit:?}"
            );
        }
        app.click(
            Intent::Navigate {
                action: Action::ToggleSidebar,
            },
            now,
        );
        let (_, scene) = rust_native_desktop::capture(&mut app, 760.0, 540.0, 2.0);
        assert!(scene.unsupported.is_empty());
        assert!(scene.focus_order().contains(&"shell-toggle-sidebar"));
        assert!(
            scene
                .focus_order()
                .iter()
                .all(|key| !key.starts_with("sidebar-"))
        );
        app.click(
            Intent::Navigate {
                action: Action::NewChat,
            },
            now,
        );
        assert_eq!(
            app.navigation
                .as_ref()
                .and_then(State::selected)
                .expect("a chat")
                .title,
            "New chat"
        );
    }

    #[test]
    #[ignore = "reports shell interaction timing; run with --ignored --nocapture"]
    fn shell_paint_benchmark() {
        use rust_native_desktop::{layout, paint, text::Fonts};
        for (width, height, scale) in [
            (1200.0, 840.0, 1.0),
            (1271.0, 1428.0, 1.0),
            (1200.0, 840.0, 2.0),
        ] {
            for scenario in ["hover", "scroll", "resize"] {
                let (mut app, _, now) = preview();
                let mut fonts = Fonts::new();
                let mut layouts = Vec::new();
                let mut paints = Vec::new();
                let mut retained = paint::Retained::default();
                for iteration in 0..24 {
                    if scenario == "resize" {
                        app.resize_leading_pane(224.0 + (iteration % 12) as f32 * 15.0, now);
                    }
                    let interaction = layout::Interaction {
                        hover: Some(format!("sidebar-chat-{}", iteration % 3 + 1)),
                        leading_scroll: if scenario == "scroll" {
                            (iteration % 8) as f32 * 30.0
                        } else {
                            0.0
                        },
                        ..Default::default()
                    };
                    let start = Instant::now();
                    let scene = layout::lay_out_with_layout(
                        app.view().view(),
                        &app.theme(),
                        &mut fonts,
                        &|resource, available| app.surface_size(resource, available),
                        &interaction,
                        width,
                        height,
                        app.window_layout(),
                    );
                    let laid_out = Instant::now();
                    retained.update(
                        &scene,
                        ((width * scale) as usize, (height * scale) as usize),
                        scale,
                        0.0,
                        None,
                        &mut fonts,
                        &mut |resource, frame, rect| app.paint_surface(resource, frame, rect),
                    );
                    std::hint::black_box(&retained.frame().expect("a frame").pixels);
                    if iteration >= 4 {
                        layouts.push((laid_out - start).as_secs_f64() * 1000.0);
                        paints.push(laid_out.elapsed().as_secs_f64() * 1000.0);
                    }
                }
                layouts.sort_by(f64::total_cmp);
                paints.sort_by(f64::total_cmp);
                eprintln!(
                    "shell {scenario} {width}x{height} scale={scale}: layout p50={:.2}ms p95={:.2}ms, paint p50={:.2}ms p95={:.2}ms",
                    layouts[10], layouts[19], paints[10], paints[19]
                );
            }
        }
    }

    #[test]
    fn retained_shell_paint_matches_complete_frames_across_interactions() {
        use rust_native_desktop::{layout, paint, text::Fonts};
        for scale in [1.0, 2.0] {
            for opaque in [false, true] {
                let (mut app, _, now) = preview();
                let mut fonts = Fonts::new();
                let mut retained = paint::Retained::default();
                let background = opaque.then(|| app.theme().background);
                for step in 0..14 {
                    match step {
                        4 => app.click(
                            Intent::Navigate {
                                action: Action::SelectChat { id: 3 },
                            },
                            now,
                        ),
                        6 => app.click(
                            Intent::Navigate {
                                action: Action::NewChat,
                            },
                            now,
                        ),
                        7 => app.resize_leading_pane(400.0, now),
                        8 => app.click(
                            Intent::Navigate {
                                action: Action::ToggleSidebar,
                            },
                            now,
                        ),
                        9 => app.click(
                            Intent::Navigate {
                                action: Action::ToggleSidebar,
                            },
                            now,
                        ),
                        10 => app.click(
                            Intent::Navigate {
                                action: Action::Settings,
                            },
                            now,
                        ),
                        11 => app.click(
                            Intent::Navigate {
                                action: Action::Computers,
                            },
                            now,
                        ),
                        12 => app.click(Intent::ConnectAnother, now),
                        13 => {
                            app.tick(now + Duration::from_secs(61));
                        }
                        _ => {}
                    }
                    let width = if step >= 7 { 760.0 } else { 1200.0 };
                    let height = 540.0;
                    let interaction = layout::Interaction {
                        hover: Some(format!("sidebar-chat-{}", step % 3 + 1)),
                        focus: (step == 2).then(|| "shell-toggle-sidebar".into()),
                        pressed: (step == 3).then(|| "sidebar-chat-1".into()),
                        leading_scroll: if step == 5 { 160.0 } else { 0.0 },
                        ..Default::default()
                    };
                    let scene = layout::lay_out_with_layout(
                        app.view().view(),
                        &app.theme(),
                        &mut fonts,
                        &|resource, available| app.surface_size(resource, available),
                        &interaction,
                        width,
                        height,
                        app.window_layout(),
                    );
                    let size = ((width * scale) as usize, (height * scale) as usize);
                    retained.update(
                        &scene,
                        size,
                        scale,
                        0.0,
                        background,
                        &mut fonts,
                        &mut |resource, frame, rect| app.paint_surface(resource, frame, rect),
                    );
                    let mut expected = background.map_or_else(
                        || Frame::transparent(size.0, size.1),
                        |color| Frame::new(size.0, size.1, color),
                    );
                    paint::paint(
                        &scene,
                        &mut expected,
                        scale,
                        0.0,
                        &mut fonts,
                        &mut |resource, frame, rect| app.paint_surface(resource, frame, rect),
                    );
                    let actual = retained.frame().expect("a frame");
                    let mismatch = actual
                        .pixels
                        .iter()
                        .zip(&expected.pixels)
                        .position(|(a, b)| a != b);
                    assert!(
                        mismatch.is_none(),
                        "step={step}, scale={scale}, background={background:?}, mismatch={mismatch:?}"
                    );
                }
            }
        }
    }
}
