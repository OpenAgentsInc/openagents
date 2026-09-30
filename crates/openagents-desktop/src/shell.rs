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
            chats:(0..chats).map(|index| Summary{id:if index==0 {chat.clone()} else {format!("{:032x}",index)},title:format!("Saved conversation {index}"),started:1,updated:1,coder:None,archived:false,pinned:false,named:false}).collect(),
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
            let leading = if state.collapsed {
                0.0
            } else {
                state
                    .sidebar_width
                    .clamp(chrome::SIDEBAR_MIN, chrome::SIDEBAR_MAX)
                    .min((chat.viewport.0 - 360.0).max(0.0))
            };
            chat.column_width = (chat.viewport.0 - leading - 16.0).clamp(1.0, 768.0);
            chat.show_saved(
                state.page == Page::Saved,
                self.model.project().map(|project| project.label.clone()),
            );
            chat.sync_sidebar(state);
        }
        let mut root = self.navigation.as_ref().map_or_else(
            || root(&self.model, unix_now()),
            |state| chrome::root(state, &self.model, unix_now()),
        );
        if self.model.nearby().is_none()
            && (self
                .navigation
                .as_ref()
                .is_some_and(|state| matches!(state.page, Page::Chat(_) | Page::Saved))
                || self.chat.as_ref().is_some_and(|chat| chat.modal()))
            && let Some(chat) = &mut self.chat
            && let rust_native::Element::Stack { children, .. } = &mut root.element
            && let Some(panes) = children.get_mut(1)
            && let rust_native::Element::Stack { children, .. } = &mut panes.element
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

    fn chat_effects(&mut self, now: Instant) {
        if self.chat.as_ref().is_some_and(|chat| chat.search_focused())
            && let Some(state) = &mut self.navigation
        {
            state.collapsed = false;
        }
        let keys = self
            .chat
            .as_mut()
            .map_or_else(Vec::new, |chat| chat.take_activated());
        for key in keys {
            let action = if let Some(key) = key.strip_prefix("command:") {
                openagents_desktop::chat_action::Action::Command { key: key.into() }
            } else {
                openagents_desktop::chat_action::Action::Card { key }
            };
            self.activate(Intent::Chat { action }, now);
        }
        let requests = self
            .chat
            .as_mut()
            .map_or_else(Vec::new, |chat| chat.take_requests());
        if requests.iter().any(|r| {
            matches!(
                r,
                Request::Chat {
                    command: openagents_chat::service::Command::Create { .. }
                        | openagents_chat::service::Command::Read { .. },
                    ..
                }
            )
        }) && let Some(state) = &mut self.navigation
        {
            state.page = Page::Chat(0);
        }
        self.send(requests, now);
        if let Some(screen) = self.chat.as_mut().and_then(|chat| chat.take_navigation()) {
            use openagents_chat::router::Screen;
            let action = match screen {
                Screen::Keys => chrome::Action::Settings,
                Screen::Computers => chrome::Action::Computers,
                _ => chrome::Action::Grid,
            };
            self.activate(Intent::Navigate { action }, now);
        }
        self.present();
    }

    /// Sends requests; inline, runs them and applies what comes back.
    fn send(&mut self, requests: Vec<Request>, now: Instant) {
        let mut queue: std::collections::VecDeque<Request> = requests.into();
        while let Some(request) = queue.pop_front() {
            match &mut self.runner {
                Runner::Background(worker) => worker.send(request),
                Runner::Inline(context) => {
                    if let Some(outcome) = context.run(request) {
                        if let Outcome::Saved { ticket, result } = outcome {
                            if self
                                .chat
                                .as_mut()
                                .is_some_and(|panel| panel.saved_outcome(ticket, *result))
                                && let Some(state) = &mut self.navigation
                            {
                                state.page = Page::Chat(0);
                            }
                        } else if let Outcome::TaskChat {
                            chat,
                            ticket,
                            result,
                        } = outcome
                        {
                            if let Some(panel) = &mut self.chat {
                                panel.task_outcome(chat, ticket, *result);
                            }
                        } else if let Outcome::Chat { ticket, result } = outcome {
                            if let Some(chat) = &mut self.chat {
                                chat.outcome(ticket, *result);
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
            if let Outcome::Saved { ticket, result } = outcome {
                if self
                    .chat
                    .as_mut()
                    .is_some_and(|panel| panel.saved_outcome(ticket, *result))
                    && let Some(state) = &mut self.navigation
                {
                    state.page = Page::Chat(0);
                }
                continue;
            }
            if let Outcome::TaskChat {
                chat,
                ticket,
                result,
            } = outcome
            {
                if let Some(panel) = &mut self.chat {
                    panel.task_outcome(chat, ticket, *result);
                }
                continue;
            }
            if let Outcome::Chat { ticket, result } = outcome {
                if let Some(chat) = &mut self.chat {
                    chat.outcome(ticket, *result);
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
            font_family: rust_native::layout::display::FontFamily::Geist,
            background: openagents_chat_app::visual::SIDEBAR,
            text: openagents_chat_app::visual::TEXT,
            muted: openagents_chat_app::visual::MUTED,
            rule: openagents_chat_app::visual::BORDER,
            focus: Color::rgb(184, 207, 231),
            button_radius: 7.0,
            icon_size: 28.0,
            body: 14.0,
            heading: 26.0,
            status: 12.0,
            column: 768.0,
            ..Theme::openagents()
        }
    }

    fn window_layout(&self) -> WindowLayout {
        self.navigation
            .as_ref()
            .map_or(WindowLayout::Column, |state| WindowLayout::HeaderSplit {
                header_height: 38,
                split: SplitLayout {
                    leading_width: state.sidebar_width,
                    min_leading_width: chrome::SIDEBAR_MIN,
                    max_leading_width: chrome::SIDEBAR_MAX,
                    min_content_width: 360.0,
                    collapsed: state.collapsed,
                    center_content: true,
                },
            })
    }

    fn resize_leading_pane(&mut self, width: f32, _: Instant) {
        if let Some(state) = &mut self.navigation {
            state.resize(width);
            self.present();
        }
    }

    fn key_bindings(&self) -> &'static [KeyBinding] {
        if self.navigation.is_none() || self.chat.as_ref().is_some_and(|chat| chat.modal()) {
            return &[];
        }
        if self.chat.is_some() {
            return &[KeyBinding {
                key: "b",
                shift: false,
                node: "shell-toggle-sidebar",
            }];
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
            if requests.iter().any(|r| {
                matches!(
                    r,
                    Request::Chat {
                        command: openagents_chat::service::Command::Create { .. }
                            | openagents_chat::service::Command::Read { .. },
                        ..
                    }
                )
            }) && let Some(state) = &mut self.navigation
            {
                state.page = Page::Chat(0);
            }
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
            if action == chrome::Action::Saved {
                if let Some(state) = &mut self.navigation {
                    state.activate(action);
                }
                if let Some(chat) = &mut self.chat {
                    chat.show_saved(
                        true,
                        self.model.project().map(|project| project.label.clone()),
                    );
                    if let Some(request) = chat.read_saved() {
                        self.send(vec![request], now);
                    }
                }
                self.present();
                return;
            }

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
        if self
            .chat
            .as_mut()
            .is_some_and(|chat| chat.shortcut(&event, self.presenter.view(), now))
        {
            self.chat_effects(now);
            return true;
        }
        if !self
            .navigation
            .as_ref()
            .is_some_and(|state| matches!(state.page, Page::Chat(_)))
            && !self.chat.as_ref().is_some_and(|chat| chat.aux_focused())
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
            self.chat_effects(now);
            return true;
        }
        false
    }

    fn pointer_down(&mut self, target: Option<&str>, point: (f32, f32), _now: Instant) -> bool {
        let consumed = self
            .chat
            .as_mut()
            .is_some_and(|chat| chat.pointer_down(target, point));
        if consumed {
            self.present();
        }
        consumed
    }
    fn context_menu(&mut self, target: Option<&str>, now: Instant) -> bool {
        if let Some(number) = target
            .and_then(|key| key.strip_prefix("sidebar-chat-"))
            .and_then(|s| s.parse::<u64>().ok())
        {
            self.activate(
                Intent::Navigate {
                    action: chrome::Action::SelectChat { id: number },
                },
                now,
            );
        }
        if self.chat.is_none() {
            return false;
        }
        self.activate(
            Intent::Chat {
                action: openagents_desktop::chat_action::Action::Menu,
            },
            now,
        );
        true
    }
    fn modal_root(&self) -> Option<&str> {
        self.chat.as_ref().and_then(|chat| chat.modal_root())
    }
    fn allows_focus(&self, key: &str) -> bool {
        self.chat.as_ref().is_none_or(|chat| chat.allows_focus(key))
    }
    fn tooltip(&self, key: &str) -> Option<String> {
        self.chat.as_ref().and_then(|chat| chat.tooltip(key))
    }

    fn dropped_file(&mut self, path: std::path::PathBuf, _now: Instant) -> bool {
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
        chat.dropped_file(path);
        self.present();
        true
    }

    fn surface_version(&self, resource: &str) -> Option<u64> {
        if let Some(percent) = parse_ring(resource) {
            return Some(u64::from(percent));
        }
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

    fn fullscreen_changed(&mut self, fullscreen: bool) {
        if let Some(state) = &mut self.navigation
            && state.fullscreen != fullscreen
        {
            state.fullscreen = fullscreen;
            self.present();
        }
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
            && !self.chat.as_ref().is_some_and(|chat| chat.aux_focused())
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
        if parse_ring(resource).is_some() {
            return Some((22.0_f32.min(available), 22.0));
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
        if let Some(percent) = parse_ring(resource) {
            let track = Color::rgb(58, 64, 73);
            let fill = if percent >= 90 {
                Color::rgb(214, 168, 92)
            } else {
                Color::rgb(220, 225, 233)
            };
            frame.usage_ring(rect, f32::from(percent) / 100.0, track, fill);
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

/// `engine-ring:{provider}:{percent}`, with `percent` from 0 to 100.
fn parse_ring(resource: &str) -> Option<u8> {
    let rest = resource.strip_prefix("engine-ring:")?;
    let (_, percent) = rest.rsplit_once(':')?;
    let percent = percent.parse().ok()?;
    (percent <= 100).then_some(percent)
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

    #[test]
    fn the_usage_ring_paints_from_its_resource() {
        use rust_native::style::Color;
        use rust_native_desktop::{App, Frame, PxRect};
        let (mut app, _, _) = preview();
        assert_eq!(
            app.surface_size("engine-ring:codex:72", 100.0),
            Some((22.0, 22.0))
        );
        assert_eq!(app.surface_version("engine-ring:codex:72"), Some(72));
        assert_eq!(app.surface_version("engine-ring:codex:101"), None);
        let mut frame = Frame::new(24, 24, Color::rgb(0, 0, 0));
        app.paint_surface(
            "engine-ring:codex:100",
            &mut frame,
            PxRect {
                x: 0.0,
                y: 0.0,
                w: 24.0,
                h: 24.0,
            },
        );
        assert_eq!(frame.pixel(12, 1), [214, 168, 92]);
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
                    let other_id = other.chat.clone().unwrap();
                    app.chat.as_mut().unwrap().outcome(ticket, Ok(other));
                    app.present();
                    rust_native_desktop::capture(&mut app, 1200.0, 840.0, 2.0);
                    // The new chat holds no turns, and none of the streaming
                    // chat's words reach its screen (its welcome and starter
                    // questions are its only rows).
                    assert!(chats.turns(&other_id).is_empty(), "new chat has no turns");
                    // The sidebar rightly lists the first chat's title, so
                    // check for the streamed reply's own words.
                    let shown = format!("{:?}", app.presenter.view());
                    let reply: String = previous.chars().take(40).collect();
                    assert!(
                        !shown.contains(&reply),
                        "the other chat's reply reached the new chat"
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
                                "shell-new-chat".into()
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
    fn coder_offer_dispatch_and_binding_mount_at_default_and_minimum_sizes() {
        let mut app = super::tests::chat_fixture(0).0;
        let panel = app.chat.as_mut().unwrap();
        let Request::Chat {
            ticket,
            command: Command::Create { chat },
        } = panel.new_chat()
        else {
            panic!("create")
        };
        let snapshot = Snapshot {
            chat: Some(chat.clone()),
            computer: true,
            ready_computer: Some("Scratch Mac".into()),
            total: 2,
            turns: vec![
                Turn::user("fix the flaky test in openagents"),
                Turn::assistant("Ready for Coder.", None),
            ],
            ..Default::default()
        };
        panel.outcome(ticket, Ok(snapshot.clone()));
        let directory =
            std::env::var_os("OPENAGENTS_HANDOFF_CAPTURE_DIR").map(std::path::PathBuf::from);
        for (name, width, height) in [("offer", 1200.0, 840.0), ("offer-minimum", 760.0, 540.0)] {
            app.present();
            let (frame, _) = rust_native_desktop::capture(&mut app, width, height, 1.0);
            assert!(
                app.chat
                    .as_ref()
                    .unwrap()
                    .transcript
                    .control_bounds("coder-run")
                    .is_some()
            );
            if let Some(directory) = &directory {
                std::fs::create_dir_all(directory).unwrap();
                std::fs::write(directory.join(format!("{name}.png")), frame.png().unwrap())
                    .unwrap();
            }
        }
        let view = app.view().clone();
        let panel = app.chat.as_mut().unwrap();
        let Request::Chat { ticket, command } = panel
            .action(
                openagents_desktop::chat_action::Action::Card {
                    key: "coder-run".into(),
                },
                &view,
                Instant::now(),
            )
            .unwrap()
        else {
            panic!("dispatch")
        };
        assert_eq!(command, Command::RunCoder { chat });
        panel.outcome(
            ticket,
            Ok(Snapshot {
                coder: Some(openagents_chat::basic_chats::Spawned {
                    host: "a".repeat(64),
                    task: "b".repeat(64),
                    project: Some("openagents".into()),
                    at: Some(10),
                }),
                ..snapshot
            }),
        );
        app.present();
        let (frame, _) = rust_native_desktop::capture(&mut app, 760.0, 540.0, 1.0);
        assert!(
            app.chat
                .as_ref()
                .unwrap()
                .transcript
                .control_bounds("coder-run")
                .is_none()
        );
        if let Some(directory) = directory {
            std::fs::write(directory.join("dispatched.png"), frame.png().unwrap()).unwrap();
        }
    }

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

#[cfg(test)]
mod image_fixtures {
    use super::*;
    use openagents_chat_app::attachments::Image;
    use rust_native_desktop::{App, input::TextInput};
    use std::time::Duration;
    #[test]
    fn dropped_images_preview_remove_and_refuse_unsupported_send_without_losing_text() {
        let (mut app, now) = super::tests::chat_fixture(0);
        let create = app.chat.as_mut().unwrap().new_chat();
        app.send(vec![create], now);
        app.present();
        app.text_input(TextInput::Commit("Keep this caption"), now);
        let root = tempfile::tempdir().unwrap();
        let image = Image::pixels(
            96,
            60,
            (0..96 * 60)
                .flat_map(|i| {
                    if i % 96 < 48 {
                        [255, 0, 0, 255]
                    } else {
                        [0, 255, 0, 255]
                    }
                })
                .collect(),
        )
        .unwrap();
        let path = root.path().join("example.png");
        std::fs::write(&path, image.bytes.as_slice()).unwrap();
        assert!(app.dropped_file(path.clone(), now));
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.chat.as_ref().unwrap().images().is_empty() {
            assert!(Instant::now() < deadline);
            app.tick(Instant::now());
            std::thread::sleep(Duration::from_millis(5));
        }
        for count in 2..=4 {
            assert!(app.dropped_file(path.clone(), now));
            while app.chat.as_ref().unwrap().images().len() < count {
                assert!(Instant::now() < deadline);
                app.tick(Instant::now());
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        let id = app.chat.as_ref().unwrap().images()[0].id.clone();
        app.activate(
            Intent::Chat {
                action: openagents_desktop::chat_action::Action::Send,
            },
            now,
        );
        assert_eq!(app.chat.as_ref().unwrap().draft(), "Keep this caption");
        assert_eq!(app.chat.as_ref().unwrap().state().unwrap().total, 0);
        assert!(
            serde_json::to_string(app.view().view())
                .unwrap()
                .contains("text only")
        );
        for (width, height, scale, name) in [
            (1200.0, 840.0, 2.0, "default"),
            (760.0, 540.0, 1.0, "minimum"),
        ] {
            app.viewport(width, height, scale);
            app.present();
            let (frame, scene) = rust_native_desktop::capture(&mut app, width, height, scale);
            let remove = scene
                .hits
                .iter()
                .find(|hit| hit.key == format!("image-remove-{id}"))
                .unwrap();
            assert!(remove.rect.y >= 0.0 && remove.rect.y + remove.rect.h <= height);
            assert!(
                frame
                    .pixels
                    .chunks_exact(4)
                    .any(|pixel| pixel == [255, 0, 0, 255])
            );
            assert!(
                frame
                    .pixels
                    .chunks_exact(4)
                    .any(|pixel| pixel == [0, 255, 0, 255])
            );
            if let Ok(path) = std::env::var("OPENAGENTS_IMAGE_CAPTURE_DIR") {
                std::fs::create_dir_all(&path).unwrap();
                std::fs::write(
                    std::path::Path::new(&path).join(format!("{name}.png")),
                    frame.png().unwrap(),
                )
                .unwrap();
            }
        }
        app.activate(
            Intent::Chat {
                action: openagents_desktop::chat_action::Action::RemoveImage { id },
            },
            now,
        );
        assert_eq!(app.chat.as_ref().unwrap().images().len(), 3);
        assert_eq!(app.chat.as_ref().unwrap().draft(), "Keep this caption");
    }
}

#[cfg(test)]
mod chat_management {
    use super::*;
    use openagents_desktop::chat_action::Action as ChatAction;
    use rust_native_desktop::{
        App,
        input::{SurfaceInput, TextInput},
    };
    #[test]
    fn native_titlebar_preserves_controls_and_docked_composer_in_both_modes() {
        let now = Instant::now();
        let (mut app, _) = DesktopApp::performance_fixture(0, 1, now);
        for fullscreen in [false, true, false] {
            app.fullscreen_changed(fullscreen);
            for (width, height, scale) in [(1200.0, 840.0, 2.0), (760.0, 540.0, 1.0)] {
                let (_, scene) = rust_native_desktop::capture(&mut app, width, height, scale);
                assert!(scene.unsupported.is_empty());
                let header = scene.bounds["shell-titlebar"];
                assert_eq!(header.y, 0.0);
                assert_eq!(header.h, 38.0);
                let split = scene.split.as_ref().unwrap();
                assert!(split.leading.rect.y >= 38.0);
                let toggle = scene
                    .hits
                    .iter()
                    .find(|hit| hit.key == "shell-toggle-sidebar")
                    .unwrap();
                assert_eq!(toggle.rect.y + toggle.rect.h / 2.0, 21.0);
                assert!(
                    toggle.rect.x
                        >= if cfg!(target_os = "macos") && !fullscreen {
                            88.0
                        } else {
                            12.0
                        }
                );
                let composer = scene.bounds["chat-composer"];
                assert!(composer.y + composer.h <= height);
                assert_eq!(composer.h, 47.0);
            }
        }
    }

    #[test]
    fn a_512_project_sidebar_keeps_every_chat_within_the_node_budget() {
        let now = Instant::now();
        let (mut app, mut snapshot) = DesktopApp::performance_fixture(0, 512, now);
        for (index, row) in snapshot.chats.iter_mut().enumerate() {
            row.coder = Some(openagents_chat::basic_chats::Spawned {
                host: "a".repeat(64),
                task: format!("{index:064x}"),
                project: Some(format!("Project {index:03}")),
                at: Some(1),
            });
        }
        snapshot.list_total = 512;
        snapshot.list_version = 2;
        app.performance_stream(snapshot, now + std::time::Duration::from_secs(2));
        let (_, scene) = rust_native_desktop::capture(&mut app, 1200.0, 840.0, 1.0);
        assert_eq!(
            scene
                .hits
                .iter()
                .filter(|hit| hit.key.starts_with("sidebar-chat-"))
                .count(),
            512
        );
        assert!(scene.bounds.contains_key("project-more-group"));
        assert!(scene.bounds.len() < rust_native::view::MAX_NODES);
        assert!(scene.ops.len() < 300);
    }

    #[test]
    fn a_512_chat_sidebar_paints_only_its_viewport() {
        let (mut app, _) = DesktopApp::performance_fixture(0, 512, Instant::now());
        let (frame, scene) = rust_native_desktop::capture(&mut app, 1200.0, 840.0, 2.0);
        let visible = scene
            .hits
            .iter()
            .filter(|hit| {
                hit.key.starts_with("sidebar-chat-")
                    && hit.clip.is_none_or(|clip| {
                        hit.rect.y + hit.rect.h > clip.y && hit.rect.y < clip.y + clip.h
                    })
            })
            .count();
        assert!(visible > 0 && visible < 20, "{visible}");
        assert!(scene.ops.len() < 300, "{} operations", scene.ops.len());
        if let Some(path) = std::env::var_os("OPENAGENTS_LIST_CAPTURE_DIR") {
            let path = std::path::PathBuf::from(path);
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(path.join("512-chats.png"), frame.png().unwrap()).unwrap();
        }
    }
    #[test]
    fn pin_rename_search_archive_and_restore_use_real_host_state() {
        let (mut app, now) = super::tests::chat_fixture(0);
        let request = app.chat.as_mut().unwrap().new_chat();
        app.send(vec![request], now);
        app.present();
        app.activate(
            Intent::Chat {
                action: ChatAction::Pin,
            },
            now,
        );
        assert!(
            app.navigation
                .as_ref()
                .unwrap()
                .chats
                .iter()
                .any(|s| s.section == chrome::Section::Pinned)
        );
        app.activate(
            Intent::Chat {
                action: ChatAction::Rename,
            },
            now,
        );
        app.text_input(
            TextInput::Key {
                key: "a",
                text: None,
                command: true,
                alt: false,
                shift: false,
            },
            now,
        );
        app.text_input(TextInput::Commit("Rocket plan"), now);
        app.text_input(
            TextInput::Key {
                key: "Enter",
                text: None,
                command: false,
                alt: false,
                shift: false,
            },
            now,
        );
        assert!(
            app.navigation
                .as_ref()
                .unwrap()
                .chats
                .iter()
                .any(|s| s.title == "Rocket plan")
        );
        app.surface_input(
            openagents_desktop::chat::SEARCH,
            SurfaceInput::Down {
                x: 10.0,
                y: 10.0,
                shift: false,
            },
            now,
        );
        app.text_input(TextInput::Commit("roCKet"), now);
        assert_eq!(app.navigation.as_ref().unwrap().chats.len(), 1);
        app.text_input(
            TextInput::Key {
                key: "a",
                text: None,
                command: true,
                alt: false,
                shift: false,
            },
            now,
        );
        app.text_input(TextInput::Commit("missing"), now);
        assert!(app.navigation.as_ref().unwrap().chats.is_empty());
        app.text_input(
            TextInput::Key {
                key: "a",
                text: None,
                command: true,
                alt: false,
                shift: false,
            },
            now,
        );
        app.text_input(TextInput::Commit("Rocket"), now);
        app.activate(
            Intent::Chat {
                action: ChatAction::Archive,
            },
            now,
        );
        let row = app.navigation.as_ref().unwrap().chats[0].clone();
        assert_eq!(row.section, chrome::Section::Archived);
        app.activate(
            Intent::Navigate {
                action: chrome::Action::SelectChat { id: row.id },
            },
            now,
        );
        app.activate(
            Intent::Chat {
                action: ChatAction::Restore,
            },
            now,
        );
        assert_eq!(
            app.navigation.as_ref().unwrap().chats[0].section,
            chrome::Section::Pinned
        );
        app.activate(
            Intent::Chat {
                action: ChatAction::Pin,
            },
            now,
        );
        assert_eq!(
            app.navigation.as_ref().unwrap().chats[0].section,
            chrome::Section::Recent
        );
        for (width, height, scale) in [(1200.0, 840.0, 2.0), (760.0, 540.0, 1.0)] {
            let (frame, scene) = rust_native_desktop::capture(&mut app, width, height, scale);
            for key in ["chat-attach", "chat-send", "chat-menu"] {
                let hit = scene.hits.iter().find(|h| h.key == key).unwrap();
                assert!(hit.rect.y + hit.rect.h <= height, "{key}");
            }
            if let Some(path) = std::env::var_os("OPENAGENTS_LIST_CAPTURE_DIR") {
                let path = std::path::PathBuf::from(path);
                std::fs::create_dir_all(&path).unwrap();
                std::fs::write(path.join(format!("list-{width}.png")), frame.png().unwrap())
                    .unwrap();
            }
        }
    }
}

#[cfg(test)]
mod command_fixtures {
    use super::*;
    use openagents_desktop::chat_action::Action as ChatAction;
    use rust_native_desktop::{App, input::TextInput};
    #[test]
    fn composer_wraps_and_collapses_without_losing_text_or_clipping_controls() {
        let (mut app, now) = super::tests::chat_fixture(0);
        key(&mut app, now, "n", true, false);
        for (draft, expected) in [
            ("Prompt with immediate spaces  ", 49.0),
            ("First line\nSecond line  ", 120.0),
        ] {
            key(&mut app, now, "a", true, false);
            app.text_input(TextInput::Commit(draft), now);
            for (width, height) in [(1200.0, 840.0), (760.0, 540.0)] {
                let (_, scene) = rust_native_desktop::capture(&mut app, width, height, 1.0);
                let card = scene.bounds["chat-composer-card"];
                assert_eq!(card.h, expected);
                for node in ["chat-attach", "chat-send"] {
                    let rect = scene.hits.iter().find(|hit| hit.key == node).unwrap().rect;
                    assert!(rect.y >= card.y && rect.y + rect.h <= card.y + card.h);
                }
                assert!(!scene.hits.iter().any(|hit| hit.key == "chat-paste-image"));
                assert_eq!(app.chat.as_ref().unwrap().draft(), draft);
            }
        }
        key(&mut app, now, "a", true, false);
        app.text_input(TextInput::Commit("x "), now);
        app.navigation.as_mut().unwrap().collapsed = true;
        app.present();
        let (_, scene) = rust_native_desktop::capture(&mut app, 1200.0, 840.0, 2.0);
        assert_eq!(scene.bounds["chat-composer-card"].h, 49.0);
        assert_eq!(app.chat.as_ref().unwrap().draft(), "x ");
    }
    fn key(app: &mut DesktopApp, now: Instant, key: &str, command: bool, shift: bool) {
        assert!(app.text_input(
            TextInput::Key {
                key,
                text: None,
                command,
                alt: false,
                shift
            },
            now
        ));
    }
    fn capture(app: &mut DesktopApp, name: &str, width: f32, height: f32) {
        app.viewport(width, height, 1.0);
        let (frame, scene) = rust_native_desktop::capture(app, width, height, 1.0);
        assert!(scene.unsupported.is_empty(), "{:?}", scene.unsupported);
        if let Some(path) = std::env::var_os("OPENAGENTS_COMMAND_CAPTURE_DIR") {
            let path = std::path::PathBuf::from(path);
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(path.join(format!("{name}.png")), frame.png().unwrap()).unwrap();
        }
    }
    #[test]
    fn the_header_menu_keeps_management_actions_reachable_and_preserves_the_draft() {
        let (mut app, now) = super::tests::chat_fixture(0);
        key(&mut app, now, "n", true, false);
        app.text_input(TextInput::Commit("Unsent text  "), now);
        let (_, scene) = rust_native_desktop::capture(&mut app, 760.0, 540.0, 1.0);
        assert!(!scene.hits.iter().any(|hit| hit.key == "chat-archive"));
        let hit = scene
            .hits
            .iter()
            .find(|hit| hit.key == "chat-menu")
            .unwrap();
        assert_eq!(
            scene.hit(hit.rect.x + 16.0, hit.rect.y + 16.0).unwrap().key,
            "chat-menu"
        );
        let view = app.view().view();
        let intent = app
            .view()
            .activate(&rust_native::Activation {
                instance: view.instance.clone(),
                revision: view.revision,
                node: hit.key.clone(),
            })
            .unwrap()
            .clone();
        app.activate(intent, now);
        capture(&mut app, "header-menu-minimum", 760.0, 540.0);
        let (_, scene) = rust_native_desktop::capture(&mut app, 760.0, 540.0, 1.0);
        for key in ["command-rename", "command-pin", "command-archive"] {
            let hit = scene.hits.iter().find(|hit| hit.key == key).unwrap();
            assert!(hit.enabled && hit.rect.y + hit.rect.h <= 540.0);
        }
        app.activate(
            Intent::Chat {
                action: ChatAction::Command { key: "pin".into() },
            },
            now,
        );
        assert!(app.navigation.as_ref().unwrap().chats[0].section == chrome::Section::Pinned);
        assert_eq!(app.chat.as_ref().unwrap().draft(), "Unsent text  ");
    }
    #[test]
    fn keys_palette_context_menu_and_modal_admission_share_one_registry() {
        let (mut app, now) = super::tests::chat_fixture(0);
        key(&mut app, now, "n", true, false);
        app.text_input(TextInput::Commit("Keep this unsent draft"), now);
        let first = app.navigation.as_ref().unwrap().chats[0].id;
        key(&mut app, now, "k", true, false);
        assert!(!app.allows_focus("sidebar-new-chat"));
        assert!(app.allows_focus("command-new"));
        app.text_input(
            TextInput::Preedit {
                text: "日本",
                selection: Some((0, 6)),
            },
            now,
        );
        key(&mut app, now, "Enter", false, false);
        assert!(
            app.chat.as_ref().unwrap().modal(),
            "IME candidate confirmation cannot execute a command"
        );
        key(&mut app, now, "Escape", false, false);
        assert!(
            app.chat.as_ref().unwrap().modal(),
            "first Escape cancels composition"
        );
        assert!(
            serde_json::to_string(app.view().view())
                .unwrap()
                .contains("command-new")
        );
        capture(&mut app, "palette", 1200.0, 840.0);
        capture(&mut app, "palette-minimum", 760.0, 540.0);
        let count = app.navigation.as_ref().unwrap().chats.len();
        key(&mut app, now, "n", true, false);
        assert_eq!(
            app.navigation.as_ref().unwrap().chats.len(),
            count,
            "global chords cannot escape a modal"
        );
        key(&mut app, now, "Escape", false, false);
        assert_eq!(app.chat.as_ref().unwrap().draft(), "Keep this unsent draft");
        key(&mut app, now, "k", true, false);
        app.text_input(TextInput::Commit("settings"), now);
        key(&mut app, now, "Enter", false, false);
        assert_eq!(app.navigation.as_ref().unwrap().page, Page::Settings);
        key(&mut app, now, "n", true, false);
        assert!(matches!(
            app.navigation.as_ref().unwrap().page,
            Page::Chat(_)
        ));
        key(&mut app, now, "Tab", true, true);
        assert_eq!(app.navigation.as_ref().unwrap().page, Page::Chat(first));
        assert_eq!(app.chat.as_ref().unwrap().draft(), "Keep this unsent draft");
        key(&mut app, now, "f", true, false);
        app.text_input(TextInput::Commit("new"), now);
        assert_eq!(app.navigation.as_ref().unwrap().search, "new");
        assert!(app.context_menu(Some(&format!("sidebar-chat-{first}")), now));
        capture(&mut app, "context-menu", 1200.0, 840.0);
        app.activate(
            Intent::Chat {
                action: ChatAction::Command {
                    key: "archive".into(),
                },
            },
            now,
        );
        assert!(
            serde_json::to_string(app.view().view())
                .unwrap()
                .contains("Archive this conversation?"),
            "{}",
            serde_json::to_string(app.view().view()).unwrap()
        );
        capture(&mut app, "archive-dialog", 760.0, 540.0);
        assert!(!app.allows_focus("sidebar-settings"));
        assert!(app.pointer_down(Some("sidebar-settings"), (10.0, 10.0), now));
        assert!(!app.chat.as_ref().unwrap().modal());
        assert!(matches!(
            app.navigation.as_ref().unwrap().page,
            Page::Chat(_)
        ));
        assert_eq!(app.chat.as_ref().unwrap().draft(), "Keep this unsent draft");
        app.activate(
            Intent::Chat {
                action: ChatAction::Rename,
            },
            now,
        );
        key(&mut app, now, "Tab", false, false);
        key(&mut app, now, "Tab", false, false);
        key(&mut app, now, "Enter", false, false);
        assert!(
            !app.chat.as_ref().unwrap().modal(),
            "Tab traps focus and Cancel closes rename"
        );
        assert_eq!(app.chat.as_ref().unwrap().draft(), "Keep this unsent draft");
        key(&mut app, now, ",", true, false);
        assert_eq!(app.navigation.as_ref().unwrap().page, Page::Settings);
        assert!(
            app.tooltip("sidebar-new-chat")
                .unwrap()
                .contains("Cmd/Ctrl+N")
        );
    }
}

#[cfg(test)]
mod task_fixtures {
    use super::*;
    use nostr::activity_summary::{self, Attention, Phase, SubjectKind, SummaryDraft};
    use openagents_chat::{basic_chats::Spawned, service::Snapshot};
    use openagents_chat_app::task_chat;
    use std::time::Duration;

    #[test]
    fn task_modes_mount_and_paint_at_default_and_minimum_sizes() {
        for (name, phase, attention, label) in [
            (
                "running",
                Phase::Running,
                Attention::None,
                "Coder is working",
            ),
            (
                "question",
                Phase::Waiting,
                Attention::Input,
                "Coder asked a question",
            ),
            (
                "approval",
                Phase::Waiting,
                Attention::Approval,
                "Coder asked for approval",
            ),
        ] {
            let (mut app, now) = super::tests::chat_fixture(0);
            let mut panel = openagents_desktop::chat::Panel::new(now);
            let Request::Chat {
                ticket,
                command: openagents_chat::service::Command::Create { chat },
            } = panel.new_chat()
            else {
                panic!("create")
            };
            let snapshot = Snapshot {
                chat: Some(chat.clone()),
                coder: Some(Spawned {
                    host: "a".repeat(64),
                    task: "b".repeat(64),
                    project: Some("scratch".into()),
                    at: None,
                }),
                ..Snapshot::default()
            };
            panel.outcome(ticket, Ok(snapshot.clone()));
            for _ in 0..8 {
                match panel.tick(now + Duration::from_secs(1)) {
                    Some(Request::Chat { ticket, .. }) => {
                        panel.outcome(ticket, Ok(snapshot.clone()))
                    }
                    Some(Request::TaskChat {
                        chat,
                        ticket,
                        request: task_chat::Request::Activity { task },
                    }) => {
                        let summary = activity_summary::encode(&SummaryDraft {
                            host: &"a".repeat(64),
                            subject_kind: SubjectKind::Task,
                            subject: &task,
                            sequence: 4,
                            phase,
                            headline: label,
                            attention,
                            updated_at: unix_now(),
                        })
                        .unwrap();
                        panel.task_outcome(chat, ticket, Ok(task_chat::Answer::Activity(summary)));
                        break;
                    }
                    _ => {}
                }
            }
            app.chat = Some(panel);
            app.present();
            for (width, height, scale) in [(1200.0, 840.0, 2.0), (760.0, 540.0, 1.0)] {
                let (frame, scene) = rust_native_desktop::capture(&mut app, width, height, scale);
                assert!(scene.unsupported.is_empty(), "{:?}", scene.unsupported);
                let words = openagents_desktop::screens::words(&app.view().view().root);
                assert!(words.iter().any(|word| word
                    == if attention == Attention::None {
                        "Queue"
                    } else {
                        "Answer"
                    }));
                let send = scene
                    .hits
                    .iter()
                    .find(|hit| hit.key == "chat-send")
                    .unwrap();
                assert!(send.rect.y + send.rect.h <= height);
                assert!(scene.hits.iter().any(|hit| hit.key == "chat-stop"));
                if let Some(path) = std::env::var_os("OPENAGENTS_TASK_CAPTURE_DIR") {
                    let path = std::path::PathBuf::from(path);
                    std::fs::create_dir_all(&path).unwrap();
                    std::fs::write(
                        path.join(format!("{name}-{width}.png")),
                        frame.png().unwrap(),
                    )
                    .unwrap();
                }
            }
        }
    }
}

#[cfg(test)]
mod saved_fixtures {
    use super::*;
    use openagents_desktop::{
        chat_action::Action as ChatAction,
        control::HostControl,
        fake::FakeHost,
        model::{Agent, Screen},
    };
    use rust_native_desktop::input::TextInput;
    #[test]
    fn both_saved_harnesses_open_read_only_without_changing_an_unsent_draft() {
        let temp = tempfile::tempdir().unwrap();
        let codex = temp.path().join("codex");
        let claude = temp.path().join("claude");
        std::fs::create_dir_all(codex.join("sessions")).unwrap();
        std::fs::create_dir_all(claude.join("projects/scratch")).unwrap();
        std::fs::write(codex.join("session_index.jsonl"), "{\"id\":\"one\",\"thread_name\":\"Scratch Codex\",\"updated_at\":\"2026-09-30T12:00:00Z\"}\n").unwrap();
        std::fs::write(codex.join("sessions/rollout-one.jsonl"), "{\"type\":\"session_meta\",\"payload\":{\"id\":\"one\"}}\n{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"Codex fixture reply with **bold** and `code`.\"}]}}\n").unwrap();
        std::fs::write(claude.join("projects/scratch/two.jsonl"), "{\"type\":\"summary\",\"summary\":\"Scratch Claude\"}\n{\"type\":\"user\",\"sessionId\":\"two\",\"timestamp\":\"2026-09-30T12:01:00Z\",\"message\":{\"role\":\"user\",\"content\":\"Claude fixture request\"}}\n{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"Claude fixture reply\"}]}}\n").unwrap();
        let originals = [
            codex.join("sessions/rollout-one.jsonl"),
            claude.join("projects/scratch/two.jsonl"),
        ]
        .map(|path| (path.clone(), std::fs::read(path).unwrap()));
        let mut fake = FakeHost::new("Scratch computer", unix_now());
        fake.add_project("/synthetic/checkout").unwrap();
        let context = Context::new(
            Box::new(fake.clone()),
            Some(fake),
            None,
            None,
            temp.path().to_path_buf(),
        )
        .with_saved_history(coder_history::Config {
            codex: Some(codex),
            claude: Some(claude),
            ..Default::default()
        });
        let now = Instant::now();
        let mut app =
            DesktopApp::inline_chat(Model::new(now, Screen::Connect, Agent::Enabled), context);
        app.activate(
            Intent::Navigate {
                action: chrome::Action::NewChat,
            },
            now,
        );
        app.text_input(TextInput::Commit("Keep this draft  "), now);
        app.send(vec![Request::Refresh], now);
        app.activate(
            Intent::Navigate {
                action: chrome::Action::Saved,
            },
            now,
        );
        let words = openagents_desktop::screens::words(&app.view().view().root);
        assert!(words.iter().any(|word| word.contains("Scratch Codex")));
        assert!(words.iter().any(|word| word.contains("Claude Code")));
        assert!(words.iter().any(|word| word.contains("2026-09-30")));
        let hits = rust_native_desktop::capture(&mut app, 1200.0, 840.0, 2.0)
            .1
            .hits;
        let ids: Vec<_> = hits
            .iter()
            .filter_map(|hit| {
                hit.key
                    .strip_prefix("saved-")
                    .filter(|suffix| suffix.len() == 64)
            })
            .map(str::to_owned)
            .collect();
        assert_eq!(ids.len(), 2);
        for (index, id) in ids.into_iter().enumerate() {
            app.activate(
                Intent::Chat {
                    action: ChatAction::SavedSelect { id },
                },
                now,
            );
            for (width, height, scale) in [(1200.0, 840.0, 2.0), (760.0, 540.0, 1.0)] {
                app.viewport(width, height, scale);
                let (frame, scene) = rust_native_desktop::capture(&mut app, width, height, scale);
                assert!(scene.unsupported.is_empty());
                assert!(!scene.bounds.contains_key("chat-composer"));
                let continued = scene
                    .hits
                    .iter()
                    .find(|hit| hit.key == "saved-continue")
                    .unwrap();
                assert!(continued.enabled && continued.rect.y + continued.rect.h <= height);
                if let Some(path) = std::env::var_os("OPENAGENTS_SAVED_CAPTURE_DIR") {
                    let path = std::path::PathBuf::from(path);
                    std::fs::create_dir_all(&path).unwrap();
                    std::fs::write(
                        path.join(format!("session-{index}-{width}.png")),
                        frame.png().unwrap(),
                    )
                    .unwrap();
                }
            }
            assert!(!app.text_input(TextInput::Commit("Cannot edit saved history"), now));
            assert_eq!(app.chat.as_ref().unwrap().draft(), "Keep this draft  ");
            app.activate(
                Intent::Chat {
                    action: ChatAction::SavedList,
                },
                now,
            );
        }
        for (path, bytes) in originals {
            assert_eq!(std::fs::read(path).unwrap(), bytes);
        }
    }
}
