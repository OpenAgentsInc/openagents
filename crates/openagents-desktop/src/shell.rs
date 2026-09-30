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
    fixture: bool,
    screen_lock: Option<ScreenLock>,
    navigation: Option<State>,
    chat: Option<openagents_desktop::chat::Panel>,
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

    /// An isolated chat acceptance window with requests answered inline.
    #[cfg(test)]
    pub fn inline_chat(model: Model, context: Context) -> DesktopApp {
        let mut app = DesktopApp::new(model, Runner::Inline(context), false, true);
        app.navigation = Some(State::empty());
        app.chat = Some(openagents_desktop::chat::Panel::new(Instant::now()));
        app.present();
        app
    }

    /// Synthetic long content for repeatable measurements; no host or home access.
    pub fn performance_fixture(
        rows: usize,
        chats: usize,
        now: Instant,
    ) -> (DesktopApp, openagents_chat::service::Snapshot) {
        use openagents_chat::{
            basic_chats::Summary,
            basic_coder::Turn,
            service::{Command, Snapshot},
        };
        let fake = openagents_desktop::fake::FakeHost::new("Fixture computer", unix_now());
        let context = Context::new(
            Box::new(fake.clone()),
            Some(fake),
            None,
            None,
            std::env::temp_dir(),
        );
        let mut app = DesktopApp::new(
            Model::new(
                now,
                openagents_desktop::model::Screen::Home,
                openagents_desktop::model::Agent::Enabled,
            ),
            Runner::Inline(context),
            false,
            true,
        );
        app.navigation = Some(State::empty());
        let mut panel = openagents_desktop::chat::Panel::new(now);
        let Request::Chat {
            ticket,
            command: Command::Create { chat },
        } = panel.new_chat()
        else {
            unreachable!()
        };
        let snapshot = Snapshot {
            chat: Some(chat.clone()), total:rows,
            chats:(0..chats).map(|index| Summary{id:if index==0 {chat.clone()} else {format!("{:032x}",index)},title:format!("Saved conversation {index}"),started:1,updated:1,coder:None,archived:false}).collect(),
            turns:(0..rows).map(|index| if index%2==0 {Turn::user(format!("Question {index}: explain the next step."))} else {Turn::assistant(format!("Reply {index} with **bold**, *italic*, and `inline code`.\n\n- First item\n- Second item\n\n```rust\nlet answer = 42;\n```"),None)}).collect(),
            ..Snapshot::default()
        };
        panel.outcome(ticket, Ok(snapshot.clone()));
        app.chat = Some(panel);
        app.present();
        (app, snapshot)
    }

    /// Advance a synthetic stream using the shared snapshot contract.
    pub fn performance_stream(
        &mut self,
        snapshot: openagents_chat::service::Snapshot,
        now: Instant,
    ) {
        if let Some(panel) = &mut self.chat
            && let Some(Request::Chat { ticket, .. }) = panel.tick(now)
        {
            panel.outcome(ticket, Ok(snapshot));
            self.present();
        }
    }
    pub fn performance_scroll(&mut self, lines: f32, now: Instant) {
        self.surface_input(
            openagents_desktop::chat::TRANSCRIPT,
            rust_native_desktop::input::SurfaceInput::Wheel {
                x: 100.0,
                y: 100.0,
                dx: 0.0,
                dy: lines,
            },
            now,
        );
    }
    pub fn performance_idle(&mut self) {
        self.present();
    }
    pub fn performance_counts(&self) -> (usize, usize, usize) {
        self.chat.as_ref().map_or((0, 0, 0), |panel| {
            (
                panel.transcript.rows(),
                panel.transcript.visible_rows(),
                panel.transcript.relaid,
            )
        })
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
        let fixture = match &runner {
            Runner::Pending(Some(context)) | Runner::Inline(context) => context.is_fixture(),
            _ => false,
        };
        let mut app = DesktopApp {
            fixture,
            model,
            presenter: Presenter::new("openagents-desktop"),
            runner,
            modules: None,
            live,
            screen_lock: None,
            navigation: chrome.then(|| {
                if live {
                    State::empty()
                } else {
                    State::default()
                }
            }),
            chat: (live && chrome).then(|| openagents_desktop::chat::Panel::new(Instant::now())),
        };
        app.present();
        app
    }

    pub fn model(&self) -> &Model {
        &self.model
    }

    fn present(&mut self) {
        if let (Some(chat), Some(state)) = (&mut self.chat, &mut self.navigation) {
            chat.sync_sidebar(state);
        }
        let mut root = self.navigation.as_ref().map_or_else(
            || root(&self.model, unix_now()),
            |state| chrome::root(state, &self.model, unix_now()),
        );
        if self.model.nearby().is_none()
            && self
                .navigation
                .as_ref()
                .is_some_and(|state| matches!(state.page, Page::Chat(_)))
            && let Some(chat) = &mut self.chat
            && let rust_native::Element::Stack { children, .. } = &mut root.element
            && let Some(content) = children.get_mut(1)
            && let rust_native::Element::Stack { children, .. } = &mut content.element
        {
            children[1] = chat.body();
            children[2] = chat.footer();
        }
        self.presenter.present(root);
        if let Some(chat) = &mut self.chat {
            chat.mounted(self.presenter.view());
        }
    }

    /// Sends requests; inline, runs them and applies what comes back.
    fn send(&mut self, requests: Vec<Request>, now: Instant) {
        let mut queue: std::collections::VecDeque<Request> = requests.into();
        while let Some(request) = queue.pop_front() {
            match &mut self.runner {
                Runner::Background(worker) => worker.send(request),
                Runner::Inline(context) => {
                    if let Some(outcome) = context.run(request) {
                        if let Outcome::Chat { ticket, result } = outcome {
                            if let Some(chat) = &mut self.chat {
                                chat.outcome(ticket, result);
                            }
                        } else {
                            queue.extend(self.model.outcome(outcome, now));
                        }
                    }
                }
                Runner::Pending(_) => {}
            }
        }
    }

    fn apply(&mut self, outcomes: Vec<Outcome>, now: Instant) {
        for outcome in outcomes {
            if let Outcome::Chat { ticket, result } = outcome {
                if let Some(chat) = &mut self.chat {
                    chat.outcome(ticket, result);
                }
                continue;
            }
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
        if self.fixture {
            "OpenAgents — offline chat fixture".into()
        } else {
            "OpenAgents".into()
        }
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
        if let Some(chat) = &mut self.chat {
            chat.start(waker.clone());
        }
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
        if let Some(request) = self.chat.as_mut().and_then(|chat| chat.tick(now)) {
            self.send(vec![request], now);
        }
        self.present();
        Some(
            self.model.next_wake().min(
                self.chat
                    .as_ref()
                    .map_or(self.model.next_wake(), |chat| chat.next_wake(now)),
            ),
        )
    }

    fn view(&self) -> &ValidatedView<Intent> {
        self.presenter.view()
    }

    fn activate(&mut self, intent: Intent, now: Instant) {
        if let Intent::Chat { action } = intent {
            let request = self
                .chat
                .as_mut()
                .and_then(|chat| chat.action(action, self.presenter.view(), now));
            if let Some(request) = request {
                self.send(vec![request], now);
            }
            let requests = self
                .chat
                .as_mut()
                .map_or_else(Vec::new, |chat| chat.take_requests());
            self.send(requests, now);
            if let Some(screen) = self.chat.as_mut().and_then(|chat| chat.take_navigation()) {
                use openagents_chat::router::Screen;
                let action = match screen {
                    Screen::Computers => Some(chrome::Action::Computers),
                    Screen::Keys => Some(chrome::Action::Settings),
                    Screen::VerseGym => Some(chrome::Action::Grid),
                    _ => None,
                };
                if let Some(action) = action {
                    self.activate(Intent::Navigate { action }, now);
                } else if let Some(chat) = &mut self.chat {
                    chat.navigation_notice("Open this destination on your phone.".into());
                }
            }
            self.present();
            return;
        }
        if let Intent::Navigate { action } = intent {
            if matches!(
                action,
                chrome::Action::Grid | chrome::Action::Computers | chrome::Action::Settings
            ) && let Some(chat) = &mut self.chat
            {
                chat.input(rust_native_desktop::input::TextInput::FocusLost, now);
            }
            let mut chat_request = None;
            if let Some(chat) = &mut self.chat {
                chat_request = match &action {
                    chrome::Action::NewChat => Some(chat.new_chat()),
                    chrome::Action::SelectChat { id } => chat.select_numeric(*id),
                    _ => None,
                };
            }
            if let Some(state) = &mut self.navigation {
                if chat_request.is_some() {
                    state.page = Page::Chat(0);
                } else {
                    state.activate(action);
                }
                if state.page != Page::Computers
                    && self.model.screen == openagents_desktop::model::Screen::Connect
                {
                    let requests = self.model.activate(Intent::Back, now);
                    self.send(requests, now);
                    let requests = self.model.tick(now);
                    self.send(requests, now);
                }
            }
            if let Some(request) = chat_request {
                self.send(vec![request], now);
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

    fn text_input(
        &mut self,
        event: rust_native_desktop::input::TextInput<'_>,
        now: Instant,
    ) -> bool {
        if !self
            .navigation
            .as_ref()
            .is_some_and(|state| matches!(state.page, Page::Chat(_)))
        {
            return false;
        }
        let Some(chat) = &mut self.chat else {
            return false;
        };
        let action = chat.input(event, now);
        if action == rust_native_desktop::composer::field::Action::Send {
            let request = chat.action(
                openagents_desktop::chat_action::Action::Send,
                self.presenter.view(),
                now,
            );
            if let Some(request) = request {
                self.send(vec![request], now);
            }
        }
        if action != rust_native_desktop::composer::field::Action::Unhandled {
            self.present();
            return true;
        }
        false
    }

    fn surface_version(&self, resource: &str) -> Option<u64> {
        if resource == chrome::MARK {
            return Some(0);
        }
        self.chat.as_ref().and_then(|chat| chat.version(resource))
    }

    fn surface_input(
        &mut self,
        resource: &str,
        event: rust_native_desktop::input::SurfaceInput,
        now: Instant,
    ) -> bool {
        let handled = self
            .chat
            .as_mut()
            .is_some_and(|chat| chat.surface(resource, event, now));
        if handled {
            let keys = self
                .chat
                .as_mut()
                .map_or_else(Vec::new, |chat| chat.take_activated());
            for key in keys {
                self.activate(
                    Intent::Chat {
                        action: openagents_desktop::chat_action::Action::Card { key },
                    },
                    now,
                );
            }
            self.present();
        }
        handled
    }

    fn viewport(&mut self, width: f32, height: f32, scale: f32) {
        if let Some(chat) = &mut self.chat
            && chat.viewport != (width, height, scale)
        {
            chat.viewport = (width, height, scale);
            self.present();
        }
    }

    fn ime_cursor(&self) -> Option<(f64, f64)> {
        if !self
            .navigation
            .as_ref()
            .is_some_and(|state| matches!(state.page, Page::Chat(_)))
        {
            return None;
        }
        self.chat.as_ref().and_then(|chat| chat.cursor())
    }

    fn surface_size(&self, resource: &str, available: f32) -> Option<(f32, f32)> {
        if let Some(size) = self
            .chat
            .as_ref()
            .and_then(|chat| chat.size(resource, available))
        {
            return Some(size);
        }
        if resource == chrome::MARK {
            return Some((64.0_f32.min(available), 64.0));
        }
        (resource == CODE_SURFACE).then(|| {
            let side = available.min(CODE_SIDE);
            (side, side)
        })
    }

    fn paint_surface(&mut self, resource: &str, frame: &mut Frame, rect: PxRect) {
        if self
            .chat
            .as_mut()
            .is_some_and(|chat| chat.paint(resource, frame, rect))
        {
            return;
        }
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

    pub(super) fn chat_fixture(rows: usize) -> (DesktopApp, Instant) {
        use openagents_chat::{
            basic_coder::Turn,
            service::{Command, Snapshot},
        };
        let fake = FakeHost::new("Test computer", unix_now());
        let context = Context::new(
            Box::new(fake.clone()),
            Some(fake),
            None,
            None,
            std::env::temp_dir(),
        );
        let now = Instant::now();
        let mut app =
            DesktopApp::inline_chat(Model::new(now, Screen::Connect, Agent::Enabled), context);
        let panel = app.chat.as_mut().unwrap();
        let Request::Chat {
            ticket,
            command: Command::Create { chat },
        } = panel.new_chat()
        else {
            panic!("create")
        };
        panel.outcome(ticket, Ok(Snapshot { chat: Some(chat), total: rows,
            turns: (0..rows).map(|i| if i % 2 == 0 { Turn::user(format!("Question {i}: how does this work?")) } else { Turn::assistant(format!("Reply {i} with **bold**, *italic*, and `inline code`.\n\n- First item\n- Second item\n\n```rust\nlet answer = 42;\n```"), None) }).collect(), ..Snapshot::default() }));
        app.present();
        (app, now)
    }

    #[test]
    fn composer_submits_once_and_keeps_a_draft_when_the_host_refuses() {
        use rust_native_desktop::input::TextInput;
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
            DesktopApp::inline_chat(Model::new(now, Screen::Connect, Agent::Enabled), context);
        app.activate(
            Intent::Navigate {
                action: Action::NewChat,
            },
            now,
        );
        assert!(app.text_input(TextInput::Commit("Hello"), now));
        assert_eq!(app.chat.as_ref().unwrap().draft(), "Hello");
        assert!(app.text_input(
            TextInput::Key {
                key: "Enter",
                text: Some("\r"),
                command: false,
                alt: false,
                shift: false
            },
            now
        ));
        assert_eq!(app.chat.as_ref().unwrap().draft(), "");
        app.activate(
            Intent::Chat {
                action: openagents_desktop::chat_action::Action::Stop,
            },
            now,
        );
        fake.set_down(true);
        assert!(app.text_input(TextInput::Commit("Keep this draft"), now));
        assert!(app.text_input(
            TextInput::Key {
                key: "Enter",
                text: Some("\r"),
                command: false,
                alt: false,
                shift: false
            },
            now
        ));
        assert_eq!(app.chat.as_ref().unwrap().draft(), "Keep this draft");
    }

    #[test]
    #[ignore = "reports idle tick cost for a 1,000-line draft; run with --ignored --nocapture"]
    fn composer_idle_benchmark() {
        use rust_native_desktop::input::TextInput;
        let (mut app, now) = chat_fixture(0);
        app.text_input(TextInput::Commit(&"draft line\n".repeat(1000)), now);
        app.viewport(1200.0, 840.0, 2.0);
        rust_native_desktop::capture(&mut app, 1200.0, 840.0, 2.0);
        let before = app.view().view().revision;
        let start = Instant::now();
        for _ in 0..1000 {
            app.tick(now);
        }
        let average = start.elapsed().as_secs_f64() / 1000.0;
        assert_eq!(app.view().view().revision, before);
        // One idle tick per second. This measures callback work, not process CPU.
        eprintln!(
            "composer 1000-line idle tick average={:.3}ms; callback CPU at 1Hz={:.4}%",
            average * 1000.0,
            average * 100.0
        );
        assert!(average < 0.01, "idle callback exceeds 1% of one CPU at 1Hz");
    }

    #[test]
    #[ignore = "public hosted inference with a scratch identity; paints real streamed snapshots"]
    fn live_hosted_reply_reaches_desktop_painter() {
        use openagents_chat::service::{self, Command};
        use rust_native_desktop::input::TextInput;
        let _ = rustls::crypto::ring::default_provider().install_default();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let mut chats = crate::chat_test_host::chats(&runtime, temp.path());
        let (mut app, now) = chat_fixture(0);
        let mut panel = openagents_desktop::chat::Panel::new(now);
        let Request::Chat { ticket, command } = panel.new_chat() else {
            panic!("create")
        };
        let first = service::apply(&mut chats, command, unix_now()).unwrap();
        let first_id = first.chat.clone().unwrap();
        panel.outcome(ticket, Ok(first));
        app.chat = Some(panel);
        app.present();
        app.text_input(TextInput::Commit("In about 150 words, explain how a Nostr relay handles signed events and subscriptions."), now);
        let request = app
            .chat
            .as_mut()
            .unwrap()
            .action(
                openagents_desktop::chat_action::Action::Send,
                app.presenter.view(),
                now,
            )
            .unwrap();
        let Request::Chat { ticket, command } = request else {
            panic!("send")
        };
        let snapshot = service::apply(&mut chats, command, unix_now()).unwrap();
        app.chat.as_mut().unwrap().outcome(ticket, Ok(snapshot));
        app.present();
        let captures = std::env::var_os("OPENAGENTS_CHAT_CAPTURE_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| temp.path().join("captures"));
        std::fs::create_dir_all(&captures).unwrap();
        let start = Instant::now();
        let mut first_words = None;
        let mut painted_states = 0;
        let mut previous = String::new();
        let mut switched = false;
        loop {
            let now = Instant::now();
            if let Some(Request::Chat { ticket, command }) = app.chat.as_mut().unwrap().tick(now) {
                let snapshot = service::apply(&mut chats, command, unix_now()).unwrap();
                assert!(snapshot.failure.is_none(), "{:?}", snapshot.failure);
                if !snapshot.partial.is_empty() && snapshot.partial != previous {
                    first_words.get_or_insert(start.elapsed());
                    previous = snapshot.partial.clone();
                    painted_states += 1;
                }
                let busy = snapshot.busy;
                app.chat.as_mut().unwrap().outcome(ticket, Ok(snapshot));
                app.present();
                if !previous.is_empty() && busy && !switched {
                    let Request::Chat { ticket, command } = app.chat.as_mut().unwrap().new_chat()
                    else {
                        panic!("second chat")
                    };
                    let other = service::apply(&mut chats, command, unix_now()).unwrap();
                    app.chat.as_mut().unwrap().outcome(ticket, Ok(other));
                    app.present();
                    rust_native_desktop::capture(&mut app, 1200.0, 840.0, 2.0);
                    assert_eq!(
                        app.chat.as_ref().unwrap().transcript.rows(),
                        1,
                        "new chat contains only its welcome"
                    );
                    let number = app
                        .navigation
                        .as_ref()
                        .unwrap()
                        .chats
                        .iter()
                        .find(|chat| chat.title.starts_with("In about 150 words"))
                        .unwrap()
                        .id;
                    let Request::Chat { ticket, command } =
                        app.chat.as_mut().unwrap().select_numeric(number).unwrap()
                    else {
                        panic!("return")
                    };
                    let returned = service::apply(&mut chats, command, unix_now()).unwrap();
                    assert_eq!(returned.chat.as_deref(), Some(first_id.as_str()));
                    app.chat.as_mut().unwrap().outcome(ticket, Ok(returned));
                    app.present();
                    switched = true;
                }
                if !previous.is_empty() {
                    let (frame, _) = rust_native_desktop::capture(&mut app, 1200.0, 840.0, 2.0);
                    std::fs::write(
                        captures.join(if busy {
                            "streaming.png"
                        } else {
                            "completed.png"
                        }),
                        frame.png().unwrap(),
                    )
                    .unwrap();
                }
                if !busy {
                    assert!(chats.turns(&first_id).last().is_some_and(|turn| turn.role
                        == openagents_chat::basic_coder::Role::Assistant
                        && !turn.text.is_empty()));
                    break;
                }
            }
            assert!(
                start.elapsed() < Duration::from_secs(60),
                "live reply timed out"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(painted_states >= 2, "no incremental live reply was painted");
        assert!(
            switched,
            "reply completed before a chat switch could be tested"
        );
        eprintln!(
            "desktop live first words={first_words:?}, total={:?}, painted states={painted_states}",
            start.elapsed()
        );
        let request = "9".repeat(32);
        let followup = service::apply(
            &mut chats,
            Command::Send {
                chat: first_id.clone(),
                request,
                text: "Now summarize your previous explanation in one sentence.".into(),
            },
            unix_now(),
        )
        .unwrap();
        assert_eq!(followup.turns.len(), 3);
        let follow_start = Instant::now();
        while chats.busy(&first_id) {
            chats.settle(unix_now());
            assert!(follow_start.elapsed() < Duration::from_secs(60));
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(chats.turns(&first_id).len(), 4);
        assert!(matches!(
            chats.tail(&first_id),
            openagents_chat::basic_chats::Tail::None
        ));
        eprintln!(
            "desktop live contextual follow-up total={:?}",
            follow_start.elapsed()
        );
        // All records disappear with the temporary store; no owner list is involved.
    }

    #[test]
    fn pending_send_acknowledgements_do_not_clear_another_conversations_draft() {
        use openagents_chat::{
            basic_coder::Turn,
            service::{Command, Snapshot},
        };
        use rust_native_desktop::input::TextInput;
        let (mut app, now) = chat_fixture(0);
        app.text_input(TextInput::Commit("First draft"), now);
        let Request::Chat {
            ticket: first_ticket,
            command:
                Command::Send {
                    chat: first_id,
                    request: first_request,
                    ..
                },
        } = app
            .chat
            .as_mut()
            .unwrap()
            .action(
                openagents_desktop::chat_action::Action::Send,
                app.presenter.view(),
                now,
            )
            .unwrap()
        else {
            panic!("first send")
        };
        let Request::Chat {
            ticket,
            command: Command::Create { chat: second_id },
        } = app.chat.as_mut().unwrap().new_chat()
        else {
            panic!("second chat")
        };
        app.chat.as_mut().unwrap().outcome(
            ticket,
            Ok(Snapshot {
                chat: Some(second_id.clone()),
                ..Snapshot::default()
            }),
        );
        app.present();
        app.text_input(TextInput::Commit("Second draft"), now);
        let Request::Chat {
            ticket: second_ticket,
            command:
                Command::Send {
                    request: second_request,
                    ..
                },
        } = app
            .chat
            .as_mut()
            .unwrap()
            .action(
                openagents_desktop::chat_action::Action::Send,
                app.presenter.view(),
                now,
            )
            .unwrap()
        else {
            panic!("independent second send")
        };
        let mut first = Turn::user("First draft");
        first.request = Some(first_request);
        app.chat.as_mut().unwrap().outcome(
            first_ticket,
            Ok(Snapshot {
                chat: Some(first_id),
                turns: vec![first],
                total: 1,
                ..Snapshot::default()
            }),
        );
        app.present();
        assert_eq!(app.chat.as_ref().unwrap().draft(), "Second draft");
        let mut second = Turn::user("Second draft");
        second.request = Some(second_request);
        app.chat.as_mut().unwrap().outcome(
            second_ticket,
            Ok(Snapshot {
                chat: Some(second_id),
                turns: vec![second],
                total: 1,
                ..Snapshot::default()
            }),
        );
        assert_eq!(app.chat.as_ref().unwrap().draft(), "");
    }

    #[test]
    fn fresh_store_opens_an_editable_chat_without_setup() {
        use openagents_chat::service::{Command, Snapshot};
        use rust_native_desktop::input::TextInput;
        let (mut app, now) = chat_fixture(0);
        let mut panel = openagents_desktop::chat::Panel::new(now);
        let Request::Chat {
            ticket,
            command: Command::List {},
        } = panel.tick(now).unwrap()
        else {
            panic!("list")
        };
        panel.outcome(ticket, Ok(Snapshot::default()));
        let Request::Chat {
            ticket,
            command: Command::Create { chat },
        } = panel.tick(now + Duration::from_secs(2)).unwrap()
        else {
            panic!("fresh chat")
        };
        panel.outcome(
            ticket,
            Ok(Snapshot {
                chat: Some(chat),
                ..Snapshot::default()
            }),
        );
        app.chat = Some(panel);
        app.present();
        assert!(app.text_input(TextInput::Commit("Ready to chat"), now));
        assert_eq!(app.chat.as_ref().unwrap().draft(), "Ready to chat");
    }

    #[test]
    fn transcript_click_cancels_marked_composer_text() {
        use rust_native_desktop::input::{SurfaceInput, TextInput};
        let (mut app, now) = chat_fixture(30);
        assert!(app.text_input(TextInput::Commit("saved draft"), now));
        assert!(app.text_input(
            TextInput::Preedit {
                text: "候補",
                selection: Some((0, 6)),
            },
            now
        ));
        assert!(app.surface_input(
            openagents_desktop::chat::TRANSCRIPT,
            SurfaceInput::Down {
                x: 20.0,
                y: 30.0,
                shift: false
            },
            now
        ));
        assert_eq!(app.chat.as_ref().unwrap().draft(), "saved draft");
        app.present();
        assert!(!app.text_input(TextInput::Commit("late candidate"), now));
        assert_eq!(app.chat.as_ref().unwrap().draft(), "saved draft");
    }

    #[test]
    fn idle_chat_pointer_and_draft_edit_leave_transcript_unchanged() {
        use rust_native_desktop::input::{SurfaceInput, TextInput};
        let (mut app, now) = chat_fixture(30);
        let before = app.surface_version(openagents_desktop::chat::TRANSCRIPT);
        for _ in 0..20 {
            assert!(!app.surface_input(
                openagents_desktop::chat::TRANSCRIPT,
                SurfaceInput::Move { x: 20.0, y: 30.0 },
                now
            ));
            assert!(!app.surface_input(
                openagents_desktop::chat::COMPOSER,
                SurfaceInput::Move { x: 20.0, y: 30.0 },
                now
            ));
        }
        assert!(app.text_input(
            TextInput::Key {
                key: "h",
                text: Some("h"),
                command: false,
                alt: false,
                shift: false
            },
            now
        ));
        assert_eq!(
            app.surface_version(openagents_desktop::chat::TRANSCRIPT),
            before
        );
    }

    #[test]
    #[ignore = "reports end-to-end chat interaction timing; run with --ignored --nocapture"]
    fn chat_paint_benchmark() {
        use rust_native_desktop::{
            input::{SurfaceInput, TextInput},
            layout, paint,
            text::Fonts,
        };
        for (width, height, scale) in [
            (1200.0, 840.0, 1.0),
            (1200.0, 840.0, 2.0),
            (1414.0, 891.0, 2.2),
        ] {
            for scenario in ["hover", "typing", "scroll"] {
                let cold = Instant::now();
                let (mut app, now) = chat_fixture(3300);
                app.viewport(width, height, scale);
                let mut fonts = Fonts::new();
                let mut retained = paint::Retained::default();
                let mut timings = Vec::new();
                let mut uploads = Vec::new();
                for iteration in 0..24 {
                    let start = Instant::now();
                    if scenario == "typing" {
                        app.text_input(
                            TextInput::Key {
                                key: "h",
                                text: Some("h"),
                                command: false,
                                alt: false,
                                shift: false,
                            },
                            now,
                        );
                    }
                    if scenario == "scroll" && iteration > 0 {
                        let before = app.surface_version(openagents_desktop::chat::TRANSCRIPT);
                        assert!(app.surface_input(
                            openagents_desktop::chat::TRANSCRIPT,
                            SurfaceInput::Wheel {
                                x: 100.0,
                                y: 100.0,
                                dx: 0.0,
                                dy: 80.0
                            },
                            now
                        ));
                        assert_ne!(
                            before,
                            app.surface_version(openagents_desktop::chat::TRANSCRIPT),
                            "scroll changes drawing revision"
                        );
                    }
                    app.present();
                    let interaction = layout::Interaction {
                        hover: (scenario == "hover").then(|| {
                            if iteration % 2 == 0 {
                                "sidebar-grid".into()
                            } else {
                                "sidebar-new-chat".into()
                            }
                        }),
                        ..Default::default()
                    };
                    let mut scene = layout::lay_out_with_layout(
                        app.view().view(),
                        &app.theme(),
                        &mut fonts,
                        &|resource, available| app.surface_size(resource, available),
                        &interaction,
                        width,
                        height,
                        app.window_layout(),
                    );
                    for op in &mut scene.ops {
                        if let layout::Op::Surface {
                            resource, version, ..
                        } = op
                        {
                            *version = app.surface_version(resource);
                        }
                    }
                    let damage = retained.update(
                        &scene,
                        ((width * scale) as usize, (height * scale) as usize),
                        scale,
                        0.0,
                        None,
                        &mut fonts,
                        &mut |resource, frame, rect| app.paint_surface(resource, frame, rect),
                    );
                    if iteration == 0 {
                        assert_eq!(app.chat.as_ref().unwrap().transcript.rows(), 3300);
                        eprintln!(
                            "rows height={}",
                            app.chat.as_ref().unwrap().transcript.height()
                        );
                        eprintln!(
                            "chat {scenario} {width}x{height} scale={scale}: cold3300={:.2}ms",
                            cold.elapsed().as_secs_f64() * 1000.0
                        );
                    }
                    if iteration >= 4 {
                        timings.push(start.elapsed().as_secs_f64() * 1000.0);
                        uploads.push(damage.iter().map(|rect| rect.w * rect.h * 4.0).sum::<f32>());
                    }
                    std::hint::black_box(retained.frame());
                }
                timings.sort_by(f64::total_cmp);
                uploads.sort_by(f32::total_cmp);
                eprintln!(
                    "chat {scenario} {width}x{height} scale={scale}: input+projection+layout+paint p50={:.2}ms p95={:.2}ms, upload p95={:.0}bytes",
                    timings[10], timings[19], uploads[19]
                );
            }
        }
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

#[cfg(test)]
mod card_fixtures {
    use super::*;
    use openagents_chat::{
        basic_coder::Turn,
        router::Meta,
        service::{Command, Snapshot},
    };
    use rust_native_desktop::input::SurfaceInput;

    #[test]
    fn cards_mount_paint_and_admit_only_their_current_buttons() {
        let mut app = super::tests::chat_fixture(0).0;
        let now = Instant::now();
        let create = app.chat.as_mut().unwrap().new_chat();
        app.send(vec![create], now);
        app.present();
        rust_native_desktop::capture(&mut app, 1200.0, 840.0, 2.0);
        let panel = app.chat.as_mut().unwrap();
        let bounds = panel
            .transcript
            .control_bounds("coder-suggest-meta.who")
            .unwrap();
        let x = bounds.x + bounds.w / 2.0;
        let y = bounds.y + bounds.h / 2.0;
        assert!(app.surface_input(
            openagents_desktop::chat::TRANSCRIPT,
            SurfaceInput::Down { x, y, shift: false },
            now
        ));
        assert!(app.surface_input(
            openagents_desktop::chat::TRANSCRIPT,
            SurfaceInput::Up { x, y },
            now
        ));
        assert_eq!(app.chat.as_ref().unwrap().draft(), "");
        let captures =
            std::env::var_os("OPENAGENTS_CARD_CAPTURE_DIR").map(std::path::PathBuf::from);
        for (name, fixture) in [
            (
                "tool",
                include_str!("../../coder/fixtures/nip-cj/router-card-tool.json"),
            ),
            (
                "result",
                include_str!("../../coder/fixtures/nip-cj/router-card-result.json"),
            ),
            (
                "news",
                include_str!("../../coder/fixtures/nip-cj/router-card-news.json"),
            ),
            (
                "check",
                include_str!("../../coder/fixtures/nip-cj/router-card-check.json"),
            ),
            (
                "draft",
                include_str!("../../coder/fixtures/nip-cj/router-card-draft.json"),
            ),
            (
                "credit",
                include_str!("../../coder/fixtures/nip-cj/router-card-credit.json"),
            ),
            (
                "capability",
                include_str!("../../coder/fixtures/nip-cj/router-card-capability.json"),
            ),
        ] {
            let mut app = super::tests::chat_fixture(0).0;
            let panel = app.chat.as_mut().unwrap();
            let Request::Chat {
                ticket,
                command: Command::Read { chat, .. },
            } = panel
                .tick(Instant::now() + std::time::Duration::from_secs(2))
                .unwrap()
            else {
                panic!("read")
            };
            let mut meta = Meta::default();
            meta.carded(&serde_json::from_str(fixture).unwrap());
            if name == "credit" {
                meta.route = Some("eval.credit".into());
            }
            panel.outcome(
                ticket,
                Ok(Snapshot {
                    chat: Some(chat),
                    total: 2,
                    turns: vec![
                        Turn::user("Show this card"),
                        Turn::assistant("Answer text", Some(meta)),
                    ],
                    ..Snapshot::default()
                }),
            );
            app.present();
            let (frame, _) = rust_native_desktop::capture(&mut app, 1200.0, 840.0, 2.0);
            let panel = app.chat.as_ref().unwrap();
            assert!(panel.transcript.rows() > 2, "{name}");
            if let Some(directory) = &captures {
                std::fs::create_dir_all(directory).unwrap();
                std::fs::write(directory.join(format!("{name}.png")), frame.png().unwrap())
                    .unwrap();
            }
        }
    }
}
