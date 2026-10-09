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

#[path = "settings_shell.rs"]
mod settings_shell;

/// The release acceptance gate's driver (#10080).
#[cfg(not(windows))]
#[path = "acceptance.rs"]
pub mod acceptance;

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
    /// Whether the window is in front, and what Coder's notifications last
    /// saw ([`openagents_desktop::notices`]).
    focused: bool,
    notices: openagents_desktop::notices::Notices,
    /// The last Coder activity heard for each chat, for the sound a
    /// change plays ([`openagents_chat_app::cues`]).
    cues: openagents_chat_app::cues::Cues,
    /// How many capacity events Coder's runs had at the last look; more
    /// reads the engine again ([`openagents_desktop::chat::Panel::capacity_signals`]).
    capacity_seen: usize,
    /// A downloaded update's version, for the update strip ([`crate::strip`]).
    update_ready: Option<String>,
    #[cfg(not(windows))]
    grid: Option<openagents_desktop::grid::Shared>,
    #[cfg(not(windows))]
    normal_wake: Option<Instant>,
    /// The slide viewer over the page, while it shows (#10057).
    slides: Option<openagents_desktop::slides::Slides>,
    /// The Map page, only while it shows (#10085).
    map: Option<openagents_desktop::route_map::MapPage>,
    /// The window's size in points and its scale.
    viewport: (f32, f32, f32),
    /// A full-screen change for the window to make (`fullscreen_request`).
    fullscreen_want: Option<bool>,
    /// Whether the window was full screen at the last look.
    window_fullscreen: bool,
    /// The Verse's full screen put the window in full screen, so leaving
    /// it takes the window out again (#10116).
    #[cfg(not(windows))]
    verse_entered: bool,
    /// A provider key test or OpenRouter sign-in running off the window's
    /// thread (BYOK, #10176); its answer is read on the next tick.
    providers_job: Option<settings_shell::ProviderJob>,
    /// Wakes the window when such an answer comes back.
    waker: Option<Waker>,
    /// The system's light or dark appearance, as the window last said
    /// (`None` before it opens, or when the platform does not say).
    system_scheme: Option<openagents_chat_app::visual::Scheme>,
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// The plain refusal for a deck a chat reply named that this app doesn't
/// ship: the worker's words for it, with the decks there are (#10058).
pub(crate) fn unknown_deck() -> String {
    let titles: Vec<String> = openagents_deck::decks()
        .into_iter()
        .map(|deck| deck.title)
        .collect();
    let list = match titles.as_slice() {
        [] => return "We can't find that deck.".to_string(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    };
    format!("We can't find that deck. The decks we can open are {list}.")
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

    /// For a capture: the host named `name` has answered, as a running
    /// app's first answer would, without sending the window's requests.
    pub fn answer_as(&mut self, name: &str) {
        use openagents_desktop::control::HostControl;
        let mut host = openagents_desktop::fake::FakeHost::new(name, unix_now());
        self.model.host = Some(openagents_desktop::model::Refreshed {
            status: host.status().expect("a status"),
            devices: host.devices().expect("devices"),
            projects: host.projects().expect("projects"),
            autostart: host.autostart().expect("autostart"),
            nearby: None,
            watchers: Vec::new(),
            background: None,
        });
        if let Some(chat) = &mut self.chat {
            chat.set_account_name(chrome::account_name(&self.model));
        }
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
            focused: true,
            capacity_seen: 0,
            notices: openagents_desktop::notices::Notices::default(),
            cues: openagents_chat_app::cues::Cues::default(),
            update_ready: None,
            #[cfg(not(windows))]
            grid: None,
            #[cfg(not(windows))]
            normal_wake: None,
            slides: None,
            map: None,
            viewport: (1200.0, 840.0, 1.0),
            fullscreen_want: None,
            window_fullscreen: false,
            #[cfg(not(windows))]
            verse_entered: false,
            providers_job: None,
            waker: None,
            system_scheme: None,
        };
        app.present();
        app
    }

    pub fn model(&self) -> &Model {
        &self.model
    }

    /// Shows the deck filed under `deck_id` in the slide viewer, over the
    /// page, animating open unless "Reduce motion" is on.
    pub fn open_presentation(
        &mut self,
        deck_id: &str,
        now: Instant,
    ) -> Result<(), openagents_deck::UnknownDeck> {
        use std::sync::atomic::Ordering;
        #[cfg(not(windows))]
        let system = self.live && crate::platform::reduce_motion();
        #[cfg(windows)]
        let system = false;
        let reduce = system || self.reduce_motion().load(Ordering::Relaxed);
        let mut slides = openagents_desktop::slides::Slides::open(deck_id, now, reduce)?;
        if let Some(chat) = &self.chat {
            slides.set_unit(chat.viewport.2);
        }
        self.slides = Some(slides);
        self.present();
        Ok(())
    }

    /// Opens the deck a chat reply's typed `open_presentation` offer names
    /// (#10058); a deck `openagents_deck::decks()` doesn't list gets a plain
    /// refusal and no viewer.
    fn chat_presentation(&mut self, now: Instant) {
        let Some(deck) = self.chat.as_mut().and_then(|chat| chat.take_presentation()) else {
            return;
        };
        if self.open_presentation(&deck, now).is_err()
            && let Some(chat) = &mut self.chat
        {
            chat.navigation_notice(unknown_deck());
        }
    }

    /// Opens the Map page when a chat reply's typed `open_screen` offer for
    /// `routes.map` holds it (#10102). Desktop only; the panel holds it only
    /// for a reply to a message this window sent.
    fn chat_map(&mut self, now: Instant) {
        if self.chat.as_mut().is_some_and(|chat| chat.take_map()) {
            self.activate(
                Intent::Navigate {
                    action: chrome::Action::Map,
                },
                now,
            );
        }
    }

    /// The slide viewer, while it shows.
    #[cfg(test)]
    pub fn presentation(&self) -> Option<&openagents_desktop::slides::Slides> {
        self.slides.as_ref()
    }

    /// The slide viewer, to set it up in a test.
    #[cfg(test)]
    pub fn presentation_mut(&mut self) -> Option<&mut openagents_desktop::slides::Slides> {
        self.slides.as_mut()
    }

    /// Advances the slide viewer's animation to `now`, drops it once it
    /// has closed, and says when it next wants a frame.
    fn tick_slides(&mut self, now: Instant) -> Option<Instant> {
        let wake = self.tick_viewer(now);
        self.deck_scene();
        wake
    }

    fn tick_viewer(&mut self, now: Instant) -> Option<Instant> {
        let slides = self.slides.as_mut()?;
        slides.tick(now);
        if slides.closed() {
            self.slides = None;
            self.present();
            return None;
        }
        slides.next_wake(now)
    }

    /// Tells the window's Grid layer where the showing slide wants the
    /// Grid drawn behind it (the Episode 289 title slide), or that none
    /// does. Windows has no Grid layer, and its slides keep the plain
    /// background.
    fn deck_scene(&mut self) {
        self.deck_map();
        #[cfg(not(windows))]
        if let Some(grid) = &self.grid {
            let scene = self.slides.as_mut().and_then(|slides| {
                slides.set_scene_host(true);
                slides.scene()
            });
            if grid.borrow().deck != scene {
                grid.borrow_mut().deck = scene;
            }
        }
    }

    /// Hands the slide viewer this computer's data for a slide that shows
    /// the live route map (`scene: routes`), as the Map page gets it.
    fn deck_map(&mut self) {
        if !self
            .slides
            .as_ref()
            .is_some_and(openagents_desktop::slides::Slides::wants_routes)
        {
            return;
        }
        let first = self
            .slides
            .as_ref()
            .is_some_and(|slides| slides.routes().is_none());
        let local = self.map_local();
        if let Some(slides) = &mut self.slides {
            slides.routes_local(local);
        }
        // This person's route counts, as the Map page asks for them.
        if first && let Some(request) = self.chat.as_mut().map(|chat| chat.request_routes()) {
            self.send(vec![request], Instant::now());
        }
    }

    fn strip_shows(&self) -> bool {
        self.model.nearby().is_none()
            && self.navigation.as_ref().is_some_and(|state| {
                crate::strip::shows(self.update_ready.as_deref(), Some(state.page))
            })
    }

    /// Shows a desktop notification for each chat whose Coder now asks
    /// for the person, finished, or failed while the window is away, and
    /// plays that change's sound. Only a real window notifies or plays;
    /// captures and tests never do.
    fn notify(&mut self) {
        let Some(chat) = &self.chat else {
            return;
        };
        let statuses = chat.coder_statuses();
        let cues = self.cues.observe(
            statuses
                .iter()
                .map(|(id, _, status)| (id.clone(), status.activity())),
        );
        let mut notices = self.notices.observe(statuses, self.focused);
        let background = self
            .model
            .host
            .as_ref()
            .and_then(|host| host.background.clone());
        notices.extend(self.notices.observe_background(background, self.focused));
        if self.live && !self.fixture && self.notifications_on() {
            for notice in notices {
                crate::platform::notify(notice);
            }
        }
        // One sound a pass, the most urgent, whether or not the window is
        // in front.
        let cue = cues.into_iter().map(|(_, cue)| cue);
        let cue = openagents_chat_app::cues::Cue::most_urgent(cue);
        if let Some(cue) = cue
            && self.live
            && !self.fixture
            && self.sounds_on()
        {
            crate::sound::play(cue);
        }
    }

    #[cfg(not(windows))]
    pub fn set_grid(&mut self, grid: openagents_desktop::grid::Shared) {
        self.grid = Some(grid);
        self.present();
    }

    #[cfg(not(windows))]
    fn grid_active(&self) -> bool {
        self.navigation
            .as_ref()
            .is_some_and(|state| state.page == Page::Grid)
            && self.model.nearby().is_none()
            && !self
                .chat
                .as_ref()
                .is_some_and(|chat| chat.modal() || chat.aux_focused())
    }

    /// Whether the Verse page shows: it is the page and no phone prompt
    /// covers it.
    #[cfg(not(windows))]
    fn verse_page(&self) -> bool {
        self.navigation
            .as_ref()
            .is_some_and(|state| state.page == Page::Grid)
            && self.model.nearby().is_none()
    }

    /// Whether the Verse covers the window (#10116).
    fn verse_full(&self) -> bool {
        #[cfg(not(windows))]
        return self.verse_page() && self.grid.as_ref().is_some_and(|grid| grid.borrow().full);
        #[cfg(windows)]
        false
    }

    /// Puts the Verse in full screen or takes it out (#10116): the sidebar
    /// and title bar step aside and the world covers the window, which goes
    /// full screen with it; leaving takes the window out of full screen
    /// only when the Verse put it there.
    #[cfg(not(windows))]
    fn set_verse_full(&mut self, on: bool) {
        let Some(grid) = &self.grid else {
            return;
        };
        if grid.borrow().full == on {
            return;
        }
        grid.borrow_mut().full = on;
        if on {
            if !self.window_fullscreen {
                self.fullscreen_want = Some(true);
                self.verse_entered = true;
            }
        } else {
            if std::mem::take(&mut self.verse_entered) && self.window_fullscreen {
                self.fullscreen_want = Some(false);
            }
            if self.fullscreen_want == Some(true) {
                self.fullscreen_want = None;
            }
        }
    }

    /// Whether "Reduce motion" is on: the system's or the person's.
    fn motion_reduced(&self) -> bool {
        use std::sync::atomic::Ordering;
        #[cfg(not(windows))]
        let system = self.live && crate::platform::reduce_motion();
        #[cfg(windows)]
        let system = false;
        system || self.reduce_motion().load(Ordering::Relaxed)
    }

    /// What only this computer knows for the map: this person's route
    /// counts (from the host's answer to `Command::Routes`) and the
    /// engines' readiness. Neither is sent anywhere.
    fn map_local(&self) -> openagents_chat_app::route_map::Local {
        openagents_chat_app::route_map::Local {
            routes: self
                .chat
                .as_ref()
                .and_then(|chat| chat.route_counts().cloned())
                .unwrap_or_default(),
            engines: self.model.engine.clone(),
        }
    }

    /// Builds the Map page when it opens and drops it when it's left
    /// (#10085), so nothing of it runs on any other page.
    fn sync_map(&mut self) {
        let open = self
            .navigation
            .as_ref()
            .is_some_and(|state| state.page == Page::Map)
            && self.model.nearby().is_none();
        if !open {
            self.map = None;
            return;
        }
        let reduce = self.motion_reduced();
        let local = self.map_local();
        let key = format!("{:?}|{:?}", local.routes, local.engines);
        if self.map.is_none() {
            let mut page = openagents_desktop::route_map::MapPage::new(
                openagents_desktop::route_map::build(local),
                reduce,
            );
            page.set_unit(self.viewport.2);
            page.set_window_height(self.viewport.1);
            page.set_local_key(key);
            self.map = Some(page);
            if let Some(request) = self.chat.as_mut().map(|chat| chat.request_routes()) {
                self.send(vec![request], Instant::now());
            }
        } else if let Some(page) = &mut self.map {
            page.set_reduce_motion(reduce);
            if page.local_key() != key {
                page.refresh(openagents_desktop::route_map::build(local));
                page.set_local_key(key);
            }
        }
    }

    /// Carries out the steps the Map page asked for, after the tap.
    fn map_effects(&mut self, now: Instant) {
        let effects = self
            .map
            .as_mut()
            .map_or_else(Vec::new, |page| page.take_effects());
        for effect in effects {
            use openagents_desktop::route_map::Effect;
            match effect {
                Effect::Chat(message) => {
                    self.activate(
                        Intent::Navigate {
                            action: chrome::Action::NewChat,
                        },
                        now,
                    );
                    if let Some(chat) = &mut self.chat {
                        chat.prefill(&message);
                    }
                }
                Effect::Copy(command) => {
                    if self.live {
                        rust_native_desktop::input::copy(&command);
                    }
                }
                Effect::Open(url) => {
                    if self.live {
                        openagents_desktop::chat::open_link(&url);
                    }
                }
                Effect::Settings => {
                    self.activate(
                        Intent::Navigate {
                            action: chrome::Action::Settings,
                        },
                        now,
                    );
                    self.activate(
                        Intent::Settings {
                            action: openagents_desktop::settings::Action::Pane {
                                pane: openagents_desktop::settings::Pane::Coder,
                            },
                        },
                        now,
                    );
                }
            }
        }
    }

    /// The Map page's graph, while it shows.
    pub fn map_view(&self) -> Option<&openagents_chat_app::route_map::Map> {
        self.map.as_ref().map(|page| page.map())
    }

    /// Ends the Map page's camera move at once, as a capture needs.
    pub fn settle_map(&mut self) {
        if let Some(page) = &mut self.map {
            page.tick(Instant::now() + openagents_desktop::route_map::EASE * 4);
        }
        self.present();
    }

    fn present(&mut self) {
        self.sync_map();
        #[cfg(not(windows))]
        if let Some(grid) = &self.grid {
            grid.borrow_mut().suspend(!self.grid_active());
            let open = self
                .navigation
                .as_ref()
                .is_some_and(|state| state.page == Page::Grid);
            // The window's layer loads the world only while this is set
            // and releases it when it clears (#10071).
            grid.borrow_mut().set_open(open);
            if !open {
                grid.borrow_mut().stop();
            } else if !self.grid_active() {
                grid.borrow_mut().input(
                    rust_native_desktop::input::NativeInput::Cancel,
                    Instant::now(),
                );
            }
        }
        // Leaving the Verse page leaves its full screen (#10116).
        #[cfg(not(windows))]
        if !self.verse_page() {
            self.set_verse_full(false);
        }
        if let (Some(chat), Some(state)) = (&mut self.chat, &mut self.navigation) {
            let leading = if state.collapsed {
                0.0
            } else {
                state
                    .sidebar_width
                    .clamp(chrome::SIDEBAR_MIN, chrome::SIDEBAR_MAX)
                    .min((chat.viewport.0 - 360.0).max(0.0))
            };
            let pane = if chat.changes_open() { 420.0 } else { 0.0 };
            chat.column_width = (chat.viewport.0 - leading - pane - 16.0).clamp(1.0, 768.0);
            chat.show_saved(
                state.page == Page::Saved,
                self.model.project().map(|project| project.label.clone()),
            );
            chat.sync_sidebar(state);
            state.settings.set_archived(chat.archived());
        }
        if let Some(state) = &mut self.navigation {
            state.record();
        }
        let mut root = self.navigation.as_ref().map_or_else(
            || root(&self.model, unix_now()),
            |state| chrome::root(state, &self.model, unix_now()),
        );
        // The Verse page shows the world and its controls whenever it is
        // the page, so the world keeps its place under a palette (#10116).
        #[cfg(not(windows))]
        if self.verse_page()
            && let Some(grid) = &self.grid
            && let rust_native::Element::Stack { children, .. } = &mut root.element
        {
            if grid.borrow().full
                && let Some(header) = children.get_mut(0)
            {
                // Full screen: no title bar over the world.
                *header = rust_native::Node {
                    key: "shell-titlebar".into(),
                    style: rust_native::style::Style::default(),
                    element: rust_native::Element::Stack {
                        axis: rust_native::Axis::Horizontal,
                        children: vec![],
                    },
                };
            }
            if let Some(panes) = children.get_mut(1)
                && let rust_native::Element::Stack { children, .. } = &mut panes.element
                && let Some(content) = children.get_mut(1)
                && let rust_native::Element::Stack { children, .. } = &mut content.element
            {
                children[1] = grid.borrow_mut().view();
            }
        }
        if self.model.nearby().is_none()
            && (self
                .navigation
                .as_ref()
                .is_some_and(|state| matches!(state.page, Page::Chat(_) | Page::Saved)))
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
        if self.model.nearby().is_none()
            && let Some(page) = &self.map
            && let rust_native::Element::Stack { children, .. } = &mut root.element
            && let Some(panes) = children.get_mut(1)
            && let rust_native::Element::Stack { children, .. } = &mut panes.element
            && let Some(content) = children.get_mut(1)
            && let rust_native::Element::Stack { children, .. } = &mut content.element
        {
            children[1] = page.view();
        }
        let strip = self.strip_shows();
        if self.model.nearby().is_none()
            && let Some(floating) = self
                .chat
                .as_mut()
                .and_then(|chat| chat.floating())
                .or_else(|| strip.then(crate::strip::node))
            && let rust_native::Element::Stack { children, .. } = &mut root.element
        {
            children.push(floating);
        }
        if let Some(slides) = &self.slides
            && let rust_native::Element::Stack { children, .. } = &mut root.element
            && children.len() >= 2
        {
            children.truncate(2);
            children.push(slides.node());
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
        self.chat_presentation(now);
        self.chat_map(now);
        if let Some(screen) = self.chat.as_mut().and_then(|chat| chat.take_navigation()) {
            use openagents_chat::router::Screen;
            let action = match screen {
                Screen::Keys => chrome::Action::Settings,
                Screen::Computers => chrome::Action::Computers,
                Screen::RoutesMap => chrome::Action::Map,
                _ => chrome::Action::Grid,
            };
            self.activate(Intent::Navigate { action }, now);
        }
        if let Some(action) = self
            .chat
            .as_mut()
            .and_then(|chat| chat.take_desktop_navigation())
        {
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
                        } else if let Outcome::CoderRun {
                            chat,
                            ticket,
                            result,
                        } = outcome
                        {
                            if let Some(panel) = &mut self.chat {
                                panel.run_outcome(chat, ticket, *result);
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
            if let Outcome::CoderRun {
                chat,
                ticket,
                result,
            } = outcome
            {
                if let Some(panel) = &mut self.chat {
                    panel.run_outcome(chat, ticket, *result);
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
        let Some(state) = &self.navigation else {
            return Theme::default();
        };
        let size = state.settings.preferences.text_size;
        // The theme seam (#11028): every color here and in the chat's views
        // comes from the scheme `apply_theme` set.
        let visual = openagents_chat_app::visual::current();
        Theme {
            appearance: match visual.scheme {
                openagents_chat_app::visual::Scheme::Light => {
                    rust_native_desktop::theme::Appearance::Light
                }
                openagents_chat_app::visual::Scheme::Dark => {
                    rust_native_desktop::theme::Appearance::Dark
                }
            },
            icons: rust_native_desktop::theme::IconSet::Solar,
            font_family: rust_native::layout::display::FontFamily::PaperMono,
            background: visual.sidebar,
            text: visual.text,
            muted: visual.muted,
            rule: visual.border,
            focus: visual.accent,
            button: visual.selected,
            button_text: visual.text,
            // The web's type scale and control radius (#11120).
            button_radius: 8.0,
            icon_size: 28.0,
            body: size.scale(oa_tokens::typography::text::SM.size),
            heading: size.scale(oa_tokens::typography::heading::LG.size),
            status: size.scale(oa_tokens::typography::text::XS.size),
            column: 768.0,
            ..Theme::openagents()
        }
    }

    fn system_appearance(&mut self, appearance: Option<rust_native_desktop::theme::Appearance>) {
        self.system_scheme = appearance.map(|appearance| match appearance {
            rust_native_desktop::theme::Appearance::Light => {
                openagents_chat_app::visual::Scheme::Light
            }
            rust_native_desktop::theme::Appearance::Dark => {
                openagents_chat_app::visual::Scheme::Dark
            }
        });
        self.apply_theme();
    }

    fn window_layout(&self) -> WindowLayout {
        self.navigation
            .as_ref()
            .map_or(WindowLayout::Column, |state| WindowLayout::HeaderSplit {
                // The Verse in full screen covers the window: no title bar
                // and no sidebar (#10116).
                header_height: if self.verse_full() { 1 } else { 38 },
                split: SplitLayout {
                    leading_width: state.sidebar_width,
                    min_leading_width: chrome::SIDEBAR_MIN,
                    max_leading_width: chrome::SIDEBAR_MAX,
                    min_content_width: 360.0,
                    collapsed: state.collapsed || self.verse_full(),
                    // The Map and Verse pages take the whole pane (#10085,
                    // #10116).
                    // Settings starts at the top, as on the web (#11120).
                    center_content: !matches!(state.page, Page::Map | Page::Grid | Page::Settings),
                    center_footer: matches!(state.page, Page::Chat(_))
                        && self.model.nearby().is_none()
                        && self
                            .chat
                            .as_ref()
                            .is_some_and(|chat| chat.composer_centered()),
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
        if self.slides.is_some()
            || self.navigation.is_none()
            || self.chat.as_ref().is_some_and(|chat| chat.modal())
        {
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
        self.waker = Some(waker.clone());
        if let Some(chat) = &mut self.chat {
            chat.start(waker.clone());
        }
        crate::menubar::start(waker.clone());
        if self.live {
            crate::updates::start(waker.clone());
            crate::native::start(waker.clone());
        }
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
        self.poll_providers();
        let slides = self.tick_slides(now);
        let slides = match self.map.as_mut() {
            Some(page) => {
                page.tick(now);
                let frame = page.next_wake(now);
                match (slides, frame) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (a, b) => a.or(b),
                }
            }
            None => slides,
        };
        #[cfg(not(windows))]
        if let Some(grid) = &self.grid {
            let changed = grid.borrow_mut().poll();
            if grid.borrow().playing
                && let Some(wake) = self.normal_wake
                && wake > now
                && self.model.next_wake() > now
            {
                let wake = if grid.borrow().needs_tick() {
                    wake.min(now + openagents_desktop::grid::FRAME)
                } else {
                    wake
                };
                if changed {
                    self.present();
                }
                return Some(slides.map_or(wake, |frame| wake.min(frame)));
            }
        }
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
        if self.live
            && let Some(state) = &mut self.navigation
        {
            state.update = crate::updates::offer();
        }
        if self.live {
            self.update_ready = crate::updates::ready();
            let registry = self
                .chat
                .as_ref()
                .map(|chat| chat.command_registry())
                .unwrap_or_default();
            crate::native::tick(&registry)
                .into_iter()
                .for_each(|intent| self.activate(intent, now));
        }
        let requests = self.model.tick(now);
        self.send(requests, now);
        if let Some(chat) = &mut self.chat {
            chat.set_account_name(openagents_desktop::chrome::account_name(&self.model));
        }
        if let Some(chat) = &mut self.chat {
            // A coding request runs in this computer's projects, the one
            // Phones and computers shows first; none is needed.
            chat.set_coder_projects(
                self.model
                    .host
                    .as_ref()
                    .map(|host| {
                        host.projects
                            .iter()
                            .map(|project| project.folder.clone().unwrap_or(project.path.clone()))
                            .collect()
                    })
                    .unwrap_or_default(),
            );
        }
        if let Some(request) = self.chat.as_mut().and_then(|chat| chat.tick(now)) {
            self.send(vec![request], now);
        }
        // A reply arrives here (the background worker's outcomes above, or
        // an inline `send`), not on input: open the deck its typed
        // `open_presentation` offer holds now, not on the next key (#10082),
        // and the Map page its typed `routes.map` offer holds (#10102).
        self.chat_presentation(now);
        self.chat_map(now);
        let slides = slides
            .or_else(|| {
                self.slides
                    .as_ref()
                    .and_then(|viewer| viewer.next_wake(now))
            })
            .or_else(|| self.map.as_ref().and_then(|page| page.next_wake(now)));
        self.notify();
        // A run passed over or refused for capacity: show what the engine
        // reads now (#10105).
        if let Some(chat) = &self.chat {
            let signals = chat.capacity_signals();
            if signals > self.capacity_seen {
                self.model.read_engine(now);
            }
            self.capacity_seen = signals;
        }
        self.present();
        let wake = self.model.next_wake().min(
            self.chat
                .as_ref()
                .map_or(self.model.next_wake(), |chat| chat.next_wake(now)),
        );
        #[cfg(not(windows))]
        {
            self.normal_wake = Some(wake);
        }
        #[cfg(not(windows))]
        let wake = if self
            .grid
            .as_ref()
            .is_some_and(|grid| grid.borrow().needs_tick())
        {
            wake.min(now + openagents_desktop::grid::FRAME)
        } else {
            wake
        };
        // A key test in flight is looked at again soon, even without a
        // waker (a capture).
        let wake = if self.providers_job.is_some() {
            wake.min(now + std::time::Duration::from_millis(250))
        } else {
            wake
        };
        Some(slides.map_or(wake, |frame| wake.min(frame)))
    }

    fn view(&self) -> &ValidatedView<Intent> {
        self.presenter.view()
    }

    fn activate(&mut self, intent: Intent, now: Instant) {
        if let Intent::Grid { key } = intent {
            #[cfg(not(windows))]
            if self.grid_active()
                && let Some(grid) = &self.grid
            {
                if key == "full" {
                    let on = !grid.borrow().full;
                    self.set_verse_full(on);
                } else {
                    grid.borrow_mut().activate(&key);
                }
            }
            #[cfg(windows)]
            let _ = key;
            self.present();
            return;
        }
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
            self.chat_presentation(now);
            self.chat_map(now);
            if let Some(action) = self
                .chat
                .as_mut()
                .and_then(|chat| chat.take_desktop_navigation())
            {
                self.activate(Intent::Navigate { action }, now);
            }
            if let Some(screen) = self.chat.as_mut().and_then(|chat| chat.take_navigation()) {
                use openagents_chat::router::Screen;
                let action = match screen {
                    Screen::Computers => Some(chrome::Action::Computers),
                    Screen::Keys => Some(chrome::Action::Settings),
                    Screen::VerseGym | Screen::Verse => Some(chrome::Action::Grid),
                    // The route map's typed `open_screen` offer (#10085).
                    Screen::RoutesMap => Some(chrome::Action::Map),
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
        if let Intent::Settings { action } = intent {
            self.settings_action(action, now);
            return;
        }
        if let Intent::Map { action } = intent {
            if let Some(page) = &mut self.map {
                page.act(action, now);
            }
            self.map_effects(now);
            self.present();
            return;
        }
        if let Intent::Navigate { action } = intent {
            if matches!(action, chrome::Action::Back | chrome::Action::Forward) {
                let page = self
                    .navigation
                    .as_mut()
                    .and_then(|state| state.step(action == chrome::Action::Forward));
                let action = match page {
                    Some(Page::Chat(id)) => Some(chrome::Action::SelectChat { id }),
                    Some(Page::Saved) => Some(chrome::Action::Saved),
                    Some(Page::Grid) => Some(chrome::Action::Grid),
                    Some(Page::Map) => Some(chrome::Action::Map),
                    Some(Page::Computers) => Some(chrome::Action::Computers),
                    Some(Page::Settings) => Some(chrome::Action::Settings),
                    None => None,
                };
                if let Some(action) = action {
                    self.activate(Intent::Navigate { action }, now);
                } else {
                    self.present();
                }
                return;
            }
            if action == chrome::Action::Update {
                crate::updates::act();
                self.present();
                return;
            }
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
                chrome::Action::Grid
                    | chrome::Action::Map
                    | chrome::Action::Computers
                    | chrome::Action::Settings
            ) && let Some(chat) = &mut self.chat
            {
                chat.input(rust_native_desktop::input::TextInput::FocusLost, now);
            }
            let mut chat_request = None;
            if let Some(chat) = &mut self.chat {
                chat_request = match &action {
                    chrome::Action::NewChat => {
                        // A new chat shows the engines as they read now
                        // (#10105).
                        self.model.read_engine(now);
                        Some(chat.new_chat())
                    }
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
                if !state.shows_computers()
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
            if self
                .navigation
                .as_ref()
                .is_some_and(|state| matches!(state.page, Page::Chat(_)))
                && let Some(chat) = &mut self.chat
            {
                chat.focus_composer();
            }
            self.present();
            return;
        }
        if let Some(state) = &mut self.navigation
            && !state.shows_computers()
        {
            state.page = Page::Computers;
        }
        let requests = self.model.activate(intent, now);
        self.send(requests, now);
        self.present();
    }

    fn shown(&mut self, visible: bool, now: Instant) {
        #[cfg(not(windows))]
        if let Some(grid) = &self.grid {
            grid.borrow_mut().visible(visible);
        }
        self.model.shown(visible, now);
        let requests = self.model.tick(now);
        self.send(requests, now);
        self.present();
    }

    fn input(&mut self, now: Instant) {
        self.model.input(now);
    }

    fn native_input(
        &mut self,
        event: rust_native_desktop::input::NativeInput<'_>,
        now: Instant,
    ) -> bool {
        if let rust_native_desktop::input::NativeInput::Focus(focused) = event {
            if focused && !self.focused {
                // Back in front: the engine may have changed meanwhile, such
                // as a sign-in to another account (#10105).
                self.model.read_engine(now);
            }
            self.focused = focused;
        }
        // A wheel over the Map page's side panel scrolls it (#10085).
        if let rust_native_desktop::input::NativeInput::Wheel { x, lines, .. } = event
            && let Some(page) = &mut self.map
            && page.wheel_side(x, lines)
        {
            self.present();
            return true;
        }
        // An open command overlay owns the wheel, as Zeron's scrim does: the
        // palette's results scroll, and the conversation beneath never does.
        if let rust_native_desktop::input::NativeInput::Wheel { x, y, lines } = event
            && let Some(chat) = self.chat.as_mut()
            && chat.commands_open()
        {
            if chat.wheel_commands((x, y), lines * 40.0) {
                self.present();
            }
            return true;
        }
        // Full screen on the Verse page (#10116): Ctrl+Cmd+F, as macOS
        // names it, or F11; Esc leaves once the world has no use for it
        // (an open board closes and a held mouse is released first).
        #[cfg(not(windows))]
        if self.grid_active()
            && let rust_native_desktop::input::NativeInput::Key {
                code,
                pressed: true,
                repeat: false,
                control,
                logo,
                ..
            } = event
            && ((code == "KeyF" && control && logo) || code == "F11")
            && let Some(grid) = &self.grid
        {
            let on = !grid.borrow().full;
            self.set_verse_full(on);
            self.present();
            return true;
        }
        #[cfg(not(windows))]
        if let Some(grid) = &self.grid {
            let active = self.grid_active();
            let consumed = grid.borrow_mut().input(
                if active || matches!(event, rust_native_desktop::input::NativeInput::Focus(_)) {
                    event
                } else {
                    rust_native_desktop::input::NativeInput::Cancel
                },
                now,
            );
            if !consumed
                && active
                && grid.borrow().full
                && let rust_native_desktop::input::NativeInput::Key {
                    code: "Escape",
                    pressed: true,
                    ..
                } = event
            {
                self.set_verse_full(false);
                self.present();
                return true;
            }
            let changed = grid.borrow_mut().poll();
            if changed {
                self.present();
            }
            return consumed;
        }
        let _ = (event, now);
        false
    }

    fn cursor_capture(&self) -> bool {
        #[cfg(not(windows))]
        return self.grid_active()
            && self
                .grid
                .as_ref()
                .is_some_and(|grid| grid.borrow().capture());
        #[cfg(windows)]
        false
    }

    fn capture_failed(&mut self, _: Instant) {
        #[cfg(not(windows))]
        if let Some(grid) = &self.grid {
            grid.borrow_mut().capture_failed();
        }
        self.present();
    }

    fn graphics_failed(&mut self, error: &str, _: Instant) {
        #[cfg(not(windows))]
        if let Some(grid) = &self.grid {
            grid.borrow_mut().graphics_failed(error);
        }
        let _ = error;
        self.present();
    }

    fn text_input(
        &mut self,
        event: rust_native_desktop::input::TextInput<'_>,
        now: Instant,
    ) -> bool {
        if let Some(slides) = &mut self.slides {
            // The viewer takes every key but the window's command keys.
            let rust_native_desktop::input::TextInput::Key { key, command, .. } = event else {
                return true;
            };
            let taken = slides.key(key, command, now);
            self.deck_scene();
            self.present();
            return taken;
        }
        if self
            .chat
            .as_mut()
            .is_some_and(|chat| chat.shortcut(&event, self.presenter.view(), now))
        {
            self.chat_effects(now);
            return true;
        }
        // The Map page's keys, unless a chat overlay or field has them.
        if let rust_native_desktop::input::TextInput::Key {
            key,
            command,
            shift,
            ..
        } = event
            && !self
                .chat
                .as_ref()
                .is_some_and(|chat| chat.modal() || chat.aux_focused())
            && let Some(page) = &mut self.map
            && page.key(key, command, shift, now)
        {
            self.present();
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

    fn pointer_hover(&mut self, target: Option<&str>, _now: Instant) -> bool {
        let changed = self
            .chat
            .as_mut()
            .is_some_and(|chat| chat.hover_command(target));
        if changed {
            self.present();
        }
        changed
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
        if self.slides.is_some() {
            return Some(openagents_desktop::slides::NODE);
        }
        self.chat.as_ref().and_then(|chat| chat.modal_root())
    }

    fn context_menu_at(&mut self, target: Option<&str>, point: (f32, f32), now: Instant) -> bool {
        if !self.context_menu(target, now) {
            return false;
        }
        if let Some(chat) = &mut self.chat {
            chat.anchor_context_menu(point);
        }
        self.present();
        true
    }

    fn overlay_layout(&self) -> Option<rust_native_desktop::OverlayLayout> {
        if self.slides.is_some() {
            return Some(rust_native_desktop::OverlayLayout {
                width: 0,
                placement: rust_native_desktop::OverlayPlacement::Cover,
                scrim: None,
            });
        }
        self.chat
            .as_ref()
            .and_then(|chat| chat.overlay_layout())
            .or_else(|| self.strip_shows().then(crate::strip::layout))
    }
    fn focus_request(&mut self) -> bool {
        self.live && crate::native::focus_request()
    }
    fn allows_focus(&self, key: &str) -> bool {
        self.chat.as_ref().is_none_or(|chat| chat.allows_focus(key))
    }
    fn tooltip(&self, key: &str) -> Option<String> {
        self.chat
            .as_ref()
            .and_then(|chat| chat.tooltip(key))
            .or_else(|| chrome::engine_tooltip(&self.model, key))
    }
    fn access_value(&self, key: &str) -> Option<String> {
        self.chat.as_ref()?.access_value(key)
    }
    fn access_focus(&self) -> Option<String> {
        self.chat.as_ref()?.access_focus()
    }
    fn access_content(&self, resource: &str) -> Option<rust_native_desktop::access::Content> {
        if resource == openagents_desktop::route_map::RESOURCE {
            return self.map.as_ref().map(|page| page.access_content());
        }
        self.chat.as_ref()?.access_content(resource)
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
        if resource == openagents_desktop::slides::RESOURCE {
            return self.slides.as_ref().map(|slides| slides.version());
        }
        if resource == openagents_desktop::route_map::RESOURCE {
            return self.map.as_ref().map(|page| page.version());
        }
        if let Some(percent) = parse_ring(resource).or_else(|| parse_meter(resource)) {
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
        if resource == openagents_desktop::slides::RESOURCE {
            let handled = self
                .slides
                .as_mut()
                .is_some_and(|slides| slides.input(event, now));
            // A link card's click opens its link in the browser.
            if let Some(url) = self.slides.as_mut().and_then(|slides| slides.take_link())
                && self.live
            {
                openagents_desktop::chat::open_link(&url);
            }
            self.deck_scene();
            self.present();
            return handled;
        }
        if resource == openagents_desktop::route_map::RESOURCE {
            let handled = self.map.as_mut().is_some_and(|page| page.input(event, now));
            self.present();
            return handled;
        }
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
        let was = std::mem::replace(&mut self.window_fullscreen, fullscreen);
        // The window went full screen on the Verse page, by its green
        // button or the system's shortcut: the Verse covers it. It left
        // full screen: so does the Verse (#10116).
        #[cfg(not(windows))]
        if was != fullscreen && self.verse_page() && self.grid_active() {
            if let Some(grid) = &self.grid
                && grid.borrow().full != fullscreen
            {
                grid.borrow_mut().full = fullscreen;
                self.verse_entered = fullscreen;
                self.fullscreen_want = None;
                self.present();
            } else if !fullscreen {
                self.verse_entered = false;
            }
        }
        let _ = was;
        if let Some(state) = &mut self.navigation
            && state.fullscreen != fullscreen
        {
            state.fullscreen = fullscreen;
            self.present();
        }
    }

    fn fullscreen_request(&mut self, fullscreen: bool) -> Option<bool> {
        let want = self.fullscreen_want.take()?;
        (want != fullscreen).then_some(want)
    }

    fn viewport(&mut self, width: f32, height: f32, scale: f32) {
        if let Some(slides) = &mut self.slides {
            slides.set_unit(scale);
        }
        if let Some(page) = &mut self.map {
            page.set_unit(scale);
            page.set_window_height(height);
        }
        if self.viewport != (width, height, scale) {
            self.viewport = (width, height, scale);
            if self.map.is_some() {
                self.present();
            }
        }
        #[cfg(not(windows))]
        if let Some(grid) = &self.grid {
            grid.borrow_mut().viewport = (width, height, scale);
        }
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
        if resource == openagents_desktop::slides::RESOURCE {
            return Some((available, 1.0));
        }
        if resource == openagents_desktop::route_map::RESOURCE {
            return self.map.as_ref().map(|page| page.surface_size(available));
        }
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
        if parse_meter(resource).is_some() {
            return Some((METER_WIDTH.min(available), 28.0));
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
        if resource == openagents_desktop::slides::RESOURCE {
            self.deck_map();
            if let Some(slides) = &mut self.slides {
                slides.paint(frame, rect);
            }
            self.deck_scene();
            return;
        }
        if resource == openagents_desktop::route_map::RESOURCE {
            if let Some(page) = &mut self.map {
                page.paint(frame, rect);
            }
            return;
        }
        if self
            .chat
            .as_mut()
            .is_some_and(|chat| chat.paint(resource, frame, rect))
        {
            return;
        }
        if let Some(percent) = parse_ring(resource) {
            let (track, fill) = meter_colors(percent);
            frame.usage_ring(rect, f32::from(percent) / 100.0, track, fill);
            return;
        }
        if let Some(percent) = parse_meter(resource) {
            // A 4-point bar on the row's middle line (#10072).
            let scale = (rect.h / 28.0).max(0.5);
            let bar = PxRect {
                x: rect.x,
                y: (rect.y + rect.h / 2.0 - 2.0 * scale).round(),
                w: rect.w,
                h: (4.0 * scale).round().max(1.0),
            };
            let (track, fill) = meter_colors(percent);
            frame.fill(bar, bar.h / 2.0, track);
            if percent > 0 {
                let used = PxRect {
                    w: (bar.w * f32::from(percent) / 100.0).max(bar.h),
                    ..bar
                };
                frame.fill(used, bar.h / 2.0, fill);
            }
            return;
        }
        if resource == chrome::MARK {
            let (tile, edge, color) = mark_colors();
            frame.fill(rect, rect.w * 0.25, tile);
            frame.stroke(rect, rect.w * 0.25, rect.w / 64.0, edge);
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

/// The sidebar engine meter's width in points.
const METER_WIDTH: f32 = 32.0;

/// A usage ring's or meter's track and fill at `percent`, in the scheme the
/// app paints with (#11028): gold from 90%.
fn meter_colors(percent: u8) -> (Color, Color) {
    use openagents_chat_app::visual::{self, Scheme};
    let look = visual::current();
    match look.scheme {
        Scheme::Dark => (
            Color::rgb(58, 64, 73),
            if percent >= 90 {
                Color::rgb(214, 168, 92)
            } else {
                Color::rgb(220, 225, 233)
            },
        ),
        Scheme::Light => (
            look.border,
            if percent >= 90 {
                look.warning
            } else {
                look.text
            },
        ),
    }
}

/// The app mark's tile, its edge, and its letters, in the scheme the app
/// paints with.
fn mark_colors() -> (Color, Color, Color) {
    use openagents_chat_app::visual::{self, Scheme};
    let look = visual::current();
    match look.scheme {
        Scheme::Dark => (
            Color::rgb(29, 32, 38),
            Color::rgb(58, 64, 73),
            Color::rgb(220, 225, 233),
        ),
        Scheme::Light => (look.selected, look.composer_border, look.text),
    }
}

/// `engine-meter:{provider}:{percent}`, the sidebar's usage bar, with
/// `percent` from 0 to 100.
fn parse_meter(resource: &str) -> Option<u8> {
    let rest = resource.strip_prefix("engine-meter:")?;
    let (_, percent) = rest.rsplit_once(':')?;
    let percent = percent.parse().ok()?;
    (percent <= 100).then_some(percent)
}

/// `engine-ring:{provider}:{percent}`, with `percent` from 0 to 100.
fn parse_ring(resource: &str) -> Option<u8> {
    let rest = resource.strip_prefix("engine-ring:")?;
    let (_, percent) = rest.rsplit_once(':')?;
    let percent = percent.parse().ok()?;
    (percent <= 100).then_some(percent)
}

/// Paints `modules` black on a white rounded square filling `rect`, with
/// the quiet zone, on whole pixels so every module is sharp. Both schemes
/// keep black on white for a scanner's contrast (#11028); on the light
/// canvas a hairline marks the card's edge.
pub fn paint_code(frame: &mut Frame, rect: PxRect, modules: &Modules) {
    let white = Color::rgb(255, 255, 255);
    let black = Color::rgb(0, 0, 0);
    frame.fill(rect, rect.w * 0.04, white);
    let look = openagents_chat_app::visual::current();
    if look.scheme == openagents_chat_app::visual::Scheme::Light {
        frame.stroke(rect, rect.w * 0.04, (rect.w / 256.0).max(1.0), look.border);
    }
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
    #[cfg(not(windows))]
    #[test]
    fn grid_play_owns_input_and_releases_it_on_escape_focus_and_departure() {
        use openagents_desktop::grid::{Grid, WORLD};
        use rust_native_desktop::input::NativeInput;
        let now = Instant::now();
        let (mut app, _) = DesktopApp::performance_fixture(0, 1, now);
        assert!(app.text_input(
            rust_native_desktop::input::TextInput::Commit("Keep this Grid draft"),
            now
        ));
        let home = tempfile::tempdir().unwrap();
        let grid = Grid::new("ws://127.0.0.1:1".into(), home.path().into(), true);
        app.set_grid(grid.clone());
        app.activate(
            Intent::Navigate {
                action: Action::Grid,
            },
            now,
        );
        assert!(!grid.borrow().playing);
        app.activate(Intent::Grid { key: "play".into() }, now);
        let deadline = Instant::now() + Duration::from_secs(5);
        while grid.borrow().surface.is_none() && Instant::now() < deadline {
            app.tick(Instant::now());
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(grid.borrow().surface.is_some());
        for (width, height, scale) in [
            (1200.0, 840.0, 1.0),
            (760.0, 540.0, 1.0),
            (1200.0, 840.0, 2.0),
            (760.0, 540.0, 2.0),
        ] {
            let (frame, scene) = rust_native_desktop::capture_views(&mut app, width, height, scale);
            assert!(scene.unsupported.is_empty());
            let rect = scene
                .backdrop_rect(WORLD)
                .expect("the playable viewport is mounted");
            assert!(
                rect.x >= 0.0
                    && rect.y >= 0.0
                    && rect.x + rect.w <= width
                    && rect.y + rect.h <= height,
                "{rect:?}"
            );
            assert_eq!(
                frame.pixels[(((rect.y + rect.h / 2.0) * scale) as usize * frame.width
                    + ((rect.x + rect.w / 2.0) * scale) as usize)
                    * 4
                    + 3],
                0,
                "the CPU foreground leaves a transparent GPU viewport"
            );
            grid.borrow_mut().rect = rect;
        }
        let rect = grid.borrow().rect;
        assert!(app.native_input(
            NativeInput::Button {
                button: 1,
                pressed: true,
                x: rect.x + 10.0,
                y: rect.y + 10.0
            },
            now
        ));
        assert!(app.cursor_capture());
        assert!(app.native_input(
            NativeInput::Key {
                code: "KeyW",
                pressed: true,
                repeat: false,
                command: false,
                alt: false,
                control: false,
                logo: false
            },
            now
        ));
        assert!(app.native_input(NativeInput::Motion { dx: 25.0, dy: 5.0 }, now));
        assert!(app.native_input(
            NativeInput::Key {
                code: "Escape",
                pressed: true,
                repeat: false,
                command: false,
                alt: false,
                control: false,
                logo: false
            },
            now
        ));
        assert!(!app.cursor_capture());
        assert!(grid.borrow().playing);
        app.activate(
            Intent::Chat {
                action: openagents_desktop::chat_action::Action::Palette,
            },
            now,
        );
        assert!(!app.cursor_capture());
        assert!(!grid.borrow().needs_tick());
        assert!(!app.native_input(
            NativeInput::Key {
                code: "KeyW",
                pressed: true,
                repeat: false,
                command: false,
                alt: false,
                control: false,
                logo: false
            },
            now
        ));
        app.activate(
            Intent::Chat {
                action: openagents_desktop::chat_action::Action::DismissOverlay,
            },
            now,
        );
        assert!(grid.borrow().playing);
        app.capture_failed(now);
        assert!(!app.cursor_capture());
        app.native_input(NativeInput::Focus(false), now);
        assert!(!grid.borrow().needs_tick());
        app.native_input(NativeInput::Focus(true), now);
        app.shown(false, now);
        assert!(!grid.borrow().needs_tick());
        app.shown(true, now);
        app.activate(
            Intent::Navigate {
                action: Action::Computers,
            },
            now,
        );
        assert!(!grid.borrow().playing);
        assert!(grid.borrow().surface.is_none());
        assert!(!app.cursor_capture());
        assert!(!app.native_input(
            NativeInput::Key {
                code: "KeyW",
                pressed: true,
                repeat: false,
                command: false,
                alt: false,
                control: false,
                logo: false
            },
            now
        ));
        assert!(!home.path().join(".openagents").exists());
        assert_eq!(app.chat.as_ref().unwrap().draft(), "Keep this Grid draft");
    }

    /// The Verse page is the world, edge to edge across the content pane,
    /// with its controls over it, at every window size; full screen (its
    /// button, Ctrl+Cmd+F, F11, or the window's own) covers the window and
    /// Esc leaves it once the world has no use for Esc (#10116).
    #[cfg(not(windows))]
    #[test]
    fn the_verse_fills_its_pane_and_goes_full_screen() {
        use openagents_desktop::grid::{Grid, WORLD};
        use rust_native_desktop::input::NativeInput;
        let key = |code: &'static str, control: bool, logo: bool| NativeInput::Key {
            code,
            pressed: true,
            repeat: false,
            command: control || logo,
            alt: false,
            control,
            logo,
        };
        let now = Instant::now();
        let (mut app, _) = DesktopApp::performance_fixture(0, 1, now);
        let home = tempfile::tempdir().unwrap();
        let grid = Grid::new("ws://127.0.0.1:1".into(), home.path().into(), true);
        app.set_grid(grid.clone());
        app.activate(
            Intent::Navigate {
                action: Action::Grid,
            },
            now,
        );
        for (width, height) in [(1280.0, 800.0), (1920.0, 1080.0), (760.0, 540.0)] {
            let (_, scene) = rust_native_desktop::capture_views(&mut app, width, height, 1.0);
            assert!(scene.unsupported.is_empty());
            let page = scene.backdrop_rect(WORLD).expect("the Verse page");
            let divider = scene.split.as_ref().unwrap().divider.expect("a sidebar");
            // Right of the sidebar to the pane's edge, title bar to bottom.
            assert_eq!(page.x, divider.x + 4.0 + 8.0, "{page:?}");
            assert_eq!(page.y, 38.0);
            assert_eq!(page.x + page.w, width - 8.0, "{page:?}");
            assert_eq!(page.y + page.h, height - 8.0, "{page:?}");
            // The controls lie over the world: top left, and the hint at
            // the bottom; no label under it.
            for control in ["grid-watch", "grid-play", "grid-full"] {
                let rect = scene.bounds[control];
                assert!(
                    rect.y < page.y + 40.0 && rect.x < page.x + page.w,
                    "{control}"
                );
            }
            assert!(scene.bounds["grid-watch"].x < page.x + 20.0);
            let hint = scene.bounds["grid-controls"];
            assert!(hint.y + hint.h > page.y + page.h - 30.0, "{hint:?}");
            assert!(!scene.bounds.contains_key("shell-content-note"));
        }
        assert_eq!(app.fullscreen_request(false), None);
        // The toggle: the window goes full screen and the Verse covers it.
        app.activate(Intent::Grid { key: "full".into() }, now);
        assert!(grid.borrow().full);
        assert_eq!(app.fullscreen_request(false), Some(true));
        assert_eq!(app.fullscreen_request(false), None, "asked once");
        app.fullscreen_changed(true);
        assert!(grid.borrow().full);
        for (width, height) in [(1280.0, 800.0), (760.0, 540.0)] {
            let (_, scene) = rust_native_desktop::capture_views(&mut app, width, height, 1.0);
            assert!(!scene.bounds.contains_key("sidebar-verse"), "no sidebar");
            assert!(
                !scene.bounds.contains_key("shell-page-title"),
                "no title bar"
            );
            let page = scene.bounds[WORLD];
            assert!(page.y <= 1.0 && page.x <= 8.0 && page.w >= width - 16.0);
            assert_eq!(
                rust_native_desktop::App::window_layout(&app).header_height(),
                Some(1.0)
            );
        }
        // Esc leaves, and takes the window out of full screen.
        assert!(app.native_input(key("Escape", false, false), now));
        assert!(!grid.borrow().full);
        assert_eq!(app.fullscreen_request(true), Some(false));
        app.fullscreen_changed(false);
        // Ctrl+Cmd+F and F11 toggle; Cmd+F alone does not.
        assert!(!app.native_input(key("KeyF", false, true), now));
        assert!(!grid.borrow().full);
        assert!(app.native_input(key("KeyF", true, true), now));
        assert!(grid.borrow().full);
        assert!(app.native_input(key("KeyF", true, true), now));
        assert!(!grid.borrow().full);
        assert!(app.native_input(key("F11", false, false), now));
        assert!(grid.borrow().full);
        app.fullscreen_changed(true);
        // Leaving the page leaves full screen.
        app.activate(
            Intent::Navigate {
                action: Action::Computers,
            },
            now,
        );
        assert!(!grid.borrow().full);
        assert_eq!(app.fullscreen_request(true), Some(false));
        app.fullscreen_changed(false);
        // The window's own full screen (its green button) on the Verse
        // page covers the window too, and ends with it.
        app.activate(
            Intent::Navigate {
                action: Action::Grid,
            },
            now,
        );
        app.fullscreen_changed(true);
        assert!(grid.borrow().full);
        assert_eq!(app.fullscreen_request(true), None);
        app.fullscreen_changed(false);
        assert!(!grid.borrow().full);
        // Play in full screen: the mouse is held, Esc releases it and
        // keeps full screen, and the next Esc leaves.
        app.activate(Intent::Grid { key: "full".into() }, now);
        app.fullscreen_changed(true);
        app.activate(Intent::Grid { key: "play".into() }, now);
        let deadline = Instant::now() + Duration::from_secs(5);
        while grid.borrow().surface.is_none() && Instant::now() < deadline {
            app.tick(Instant::now());
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(grid.borrow().surface.is_some());
        grid.borrow_mut().rect = rust_native_desktop::Rect {
            x: 0.0,
            y: 0.0,
            w: 1280.0,
            h: 800.0,
        };
        assert!(app.native_input(
            NativeInput::Button {
                button: 1,
                pressed: true,
                x: 640.0,
                y: 400.0
            },
            now
        ));
        assert!(app.cursor_capture());
        assert!(app.native_input(key("Escape", false, false), now));
        assert!(!app.cursor_capture());
        assert!(grid.borrow().full && grid.borrow().playing);
        assert!(app.native_input(key("Escape", false, false), now));
        assert!(!grid.borrow().full && grid.borrow().playing);
        assert_eq!(app.fullscreen_request(true), Some(false));
        assert!(!home.path().join(".openagents").exists());
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

    /// The sidebar's engine rows and Settings → Coder read the engine
    /// again at once on a new chat and when the window comes back to the
    /// front, not only every minute (#10105).
    #[test]
    fn a_new_chat_and_focus_read_the_engine_again() {
        use openagents_desktop::model::Request;
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
            DesktopApp::inline_chat(Model::new(now, Screen::Home, Agent::Enabled), context);
        assert!(app.model.tick(now).contains(&Request::Engine));
        let later = now + Duration::from_secs(5);
        assert!(!app.model.tick(later).contains(&Request::Engine));
        app.activate(
            Intent::Navigate {
                action: Action::NewChat,
            },
            later,
        );
        assert!(app.model.next_wake() <= later);
        assert!(app.model.tick(later).contains(&Request::Engine));
        let after = later + Duration::from_secs(5);
        assert!(!app.model.tick(after).contains(&Request::Engine));
        app.native_input(rust_native_desktop::input::NativeInput::Focus(false), after);
        app.native_input(rust_native_desktop::input::NativeInput::Focus(true), after);
        assert!(app.model.tick(after).contains(&Request::Engine));
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
                control: false,
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
                control: false,
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
                    // chat's words reach its blank screen.
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
                control: false,
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
                                control: false,
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
pub(super) mod card_fixtures {
    use super::*;
    use openagents_chat::{
        basic_coder::Turn,
        router::Meta,
        service::{Command, Snapshot},
    };

    /// A chat whose earlier reply offered `old` follow-ups and whose
    /// latest reply offers `latest`, as the router signs them.
    pub(super) fn followups_fixture(old: &[&str], latest: &[&str]) -> (DesktopApp, Snapshot) {
        let meta = |labels: &[&str]| {
            let mut meta = Meta::default();
            for label in labels {
                meta.followups.push(openagents_chat::router::Followup {
                    label: (*label).into(),
                    answer: None,
                });
            }
            Some(meta)
        };
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
            chat: Some(chat),
            total: 4,
            turns: vec![
                Turn::user("Hello"),
                Turn::assistant("Hi. I can answer questions and run Coder.", meta(old)),
                Turn::user("Tell me more"),
                Turn::assistant("I am OpenAgents. Ask me anything.", meta(latest)),
            ],
            ..Default::default()
        };
        panel.outcome(ticket, Ok(snapshot.clone()));
        app.present();
        (app, snapshot)
    }

    /// The latest reply's follow-ups are small chips above the composer,
    /// sized to their words and wrapping, docked or centered; an earlier
    /// reply's never show, and none are in the transcript (#10075).
    #[test]
    fn followups_are_chips_above_the_composer_for_the_latest_reply_only() {
        const LATEST: [&str; 3] = [
            "What can you do?",
            "What model is this?",
            "What does it cost?",
        ];
        let directory =
            std::env::var_os("OPENAGENTS_CHIPS_CAPTURE_DIR").map(std::path::PathBuf::from);
        let (mut app, _) = followups_fixture(&["An older question"], &LATEST);
        for (width, height, scale, name) in [
            (1200.0, 840.0, 2.0, "default"),
            (760.0, 540.0, 2.0, "minimum"),
        ] {
            app.present();
            let (frame, scene) = rust_native_desktop::capture(&mut app, width, height, scale);
            let panel = app.chat.as_ref().unwrap();
            assert!(!panel.composer_centered());
            let row = scene.bounds["chat-followups"];
            let card = scene.bounds["chat-composer-card"];
            assert!(row.y + row.h <= card.y, "{row:?} above {card:?}");
            assert!(
                card.y - (row.y + row.h) <= 12.0,
                "directly above: {row:?} {card:?}"
            );
            let mut right = row.x;
            for (index, label) in LATEST.iter().enumerate() {
                let key = format!("coder-followup-{index}");
                let hit = scene.hits.iter().find(|hit| hit.key == key).unwrap();
                assert!(hit.enabled, "{key}");
                // Sized to its words: a short question is a small chip,
                // on one line, far narrower than the column.
                assert!(
                    hit.rect.w < 12.0 * label.len() as f32,
                    "{key}: {:?}",
                    hit.rect
                );
                assert!(hit.rect.w < card.w / 2.0, "{key}: {:?}", hit.rect);
                assert!(hit.rect.h <= 32.0, "{key}: {:?}", hit.rect);
                assert!(hit.rect.y >= row.y && hit.rect.y + hit.rect.h <= row.y + row.h);
                // One row, left to right, in the default window; the
                // minimum window may wrap them.
                if name == "default" {
                    assert!(hit.rect.x >= right, "{key}: {:?}", hit.rect);
                }
                right = right.max(hit.rect.x + hit.rect.w);
                // Not in the transcript.
                assert!(panel.transcript.control_bounds(&key).is_none(), "{key}");
            }
            assert!(right <= row.x + row.w);
            assert!(!scene.hits.iter().any(|hit| hit.key == "coder-followup-3"));
            if let Some(directory) = &directory {
                std::fs::create_dir_all(directory).unwrap();
                std::fs::write(
                    directory.join(format!("chips-{name}-{width}x{height}.png")),
                    frame.png().unwrap(),
                )
                .unwrap();
            }
        }
        // The earlier reply's follow-up is nowhere: not a chip, not a row.
        fn labels(node: &rust_native::Node<Intent>, out: &mut Vec<String>) {
            match &node.element {
                rust_native::Element::Button { label, .. } => out.push(label.clone()),
                rust_native::Element::Stack { children, .. } => {
                    children.iter().for_each(|child| labels(child, out))
                }
                _ => {}
            }
        }
        let view = app.view().clone();
        let mut shown = vec![];
        labels(&view.view().root, &mut shown);
        for label in LATEST {
            assert!(shown.iter().any(|shown| shown == label), "{shown:?}");
        }
        assert!(!shown.iter().any(|label| label == "An older question"));
        assert!(
            app.chat
                .as_ref()
                .unwrap()
                .transcript
                .control_bounds("coder-followup-0")
                .is_none()
        );
        // A tap sends the suggestion's words.
        let panel = app.chat.as_mut().unwrap();
        assert!(
            panel
                .action(
                    openagents_desktop::chat_action::Action::Card {
                        key: "coder-followup-1".into(),
                    },
                    &view,
                    Instant::now(),
                )
                .is_some_and(|request| matches!(
                    request,
                    Request::Chat {
                        command: Command::Send { ref text, .. },
                        ..
                    } if text == "What model is this?"
                )),
            "a tap sends the suggestion's words"
        );
        // While the reply to it comes, no chips.
        app.present();
        let (_, scene) = rust_native_desktop::capture(&mut app, 1200.0, 840.0, 1.0);
        assert!(!scene.bounds.contains_key("chat-followups"));
    }

    /// A new chat's starters, the phone's shared list, are chips above
    /// the centered composer, in the follow-ups' row and style, wrapping
    /// inside the column; a tap sends one; they leave once the chat has a
    /// message, and the latest reply's follow-ups take the row (#10097).
    #[test]
    fn starters_are_chips_above_the_centered_composer_until_a_message() {
        let starters: Vec<_> = openagents_chat_app::first_run::SUGGESTIONS
            .iter()
            .take(openagents_chat_app::first_run::SUGGESTIONS_SHOWN)
            .collect();
        assert_eq!(starters[0].label, "What is OpenAgents?");
        // Exactly the four starters, so every one shows (#11095).
        assert_eq!(
            openagents_chat_app::first_run::SUGGESTIONS.len(),
            openagents_chat_app::first_run::SUGGESTIONS_SHOWN
        );
        let directory =
            std::env::var_os("OPENAGENTS_STARTERS_CAPTURE_DIR").map(std::path::PathBuf::from);
        let (mut app, now) = super::tests::chat_fixture(0);
        for (width, height, name) in [(1200.0, 840.0, "default"), (760.0, 540.0, "minimum")] {
            app.present();
            let (frame, scene) = rust_native_desktop::capture(&mut app, width, height, 2.0);
            let panel = app.chat.as_ref().unwrap();
            assert!(panel.composer_centered(), "{name}");
            assert!(!scene.bounds.contains_key("chat-followups"));
            let row = scene.bounds["chat-starters"];
            let card = scene.bounds["chat-composer-card"];
            assert!(row.y + row.h <= card.y, "{row:?} above {card:?}");
            assert!(
                card.y - (row.y + row.h) <= 12.0,
                "directly above: {row:?} {card:?}"
            );
            // The composer stays in the pane's middle, not pushed to the
            // bottom.
            assert!(card.y + card.h < height - 60.0, "{card:?} in {height}");
            let mut previous: Option<rust_native_desktop::Rect> = None;
            for starter in &starters {
                let key = format!("coder-suggest-{}", starter.id);
                let hit = scene
                    .hits
                    .iter()
                    .find(|hit| hit.key == key)
                    .unwrap_or_else(|| panic!("{key} at {name}"));
                assert!(hit.enabled, "{key}");
                // Sized to its words, never stretched across the column;
                // "How do I connect my codebase?" is about half the card
                // at the minimum window (#11095).
                assert!(hit.rect.w < card.w * 0.6, "{key}: {:?}", hit.rect);
                assert!(hit.rect.h <= 32.0, "{key}: {:?}", hit.rect);
                assert!(
                    hit.rect.x >= row.x
                        && hit.rect.x + hit.rect.w <= row.x + row.w + 0.5
                        && hit.rect.y >= row.y
                        && hit.rect.y + hit.rect.h <= row.y + row.h,
                    "{key} inside the row: {:?} {row:?}",
                    hit.rect
                );
                // In order: right of the one before, or on a later line.
                if let Some(before) = previous {
                    assert!(
                        hit.rect.x >= before.x + before.w || hit.rect.y > before.y,
                        "{key}: {:?} after {before:?}",
                        hit.rect
                    );
                }
                previous = Some(hit.rect);
                assert!(panel.transcript.control_bounds(&key).is_none(), "{key}");
                // A click at the chip's middle reaches the chip, not the
                // empty transcript the centered group sits over (#10098).
                let (x, y) = (hit.rect.x + hit.rect.w / 2.0, hit.rect.y + hit.rect.h / 2.0);
                assert_eq!(
                    scene.surface_at(x, y),
                    None,
                    "{key} at {name}: the pointer goes to a surface"
                );
                assert_eq!(
                    scene.hit(x, y).map(|hit| hit.key.as_str()),
                    Some(key.as_str())
                );
            }
            // Send, in the centered composer, is no surface's either, and
            // the composer's text takes the pointer as before.
            let send = scene
                .hits
                .iter()
                .find(|hit| hit.key == "chat-send")
                .unwrap_or_else(|| panic!("chat-send at {name}"));
            let (x, y) = (
                send.rect.x + send.rect.w / 2.0,
                send.rect.y + send.rect.h / 2.0,
            );
            assert_eq!(scene.surface_at(x, y), None, "chat-send at {name}");
            let composer = scene
                .ops
                .iter()
                .find_map(|op| match op {
                    rust_native_desktop::layout::Op::Surface { resource, rect, .. }
                        if resource.starts_with("composer:") =>
                    {
                        Some(*rect)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("a composer surface at {name}"));
            assert!(
                scene
                    .surface_at(composer.x + 8.0, composer.y + composer.h / 2.0)
                    .is_some_and(|(resource, _)| resource.starts_with("composer:")),
                "the composer's text at {name}"
            );
            if let Some(directory) = &directory {
                std::fs::create_dir_all(directory).unwrap();
                std::fs::write(
                    directory.join(format!("starters-{name}-{width}x{height}.png")),
                    frame.png().unwrap(),
                )
                .unwrap();
            }
        }
        // A tap sends the starter's words.
        let view = app.view().clone();
        let panel = app.chat.as_mut().unwrap();
        let first = panel.action(
            openagents_desktop::chat_action::Action::Card {
                key: "coder-suggest-meta.who".into(),
            },
            &view,
            now,
        );
        assert!(
            matches!(
                first,
                Some(Request::Chat {
                    command: Command::UseSuggestion { ref id, .. },
                    ..
                }) if id == "meta.who"
            ),
            "{first:?}"
        );
        assert!(
            panel.take_requests().iter().any(|request| matches!(
                request,
                Request::Chat {
                    command: Command::Send { text, .. },
                    ..
                } if text == "What is OpenAgents?"
            )),
            "a tap sends the starter's words"
        );
        // Once the chat has a message, no starters.
        app.present();
        let (_, scene) = rust_native_desktop::capture(&mut app, 1200.0, 840.0, 1.0);
        assert!(!scene.bounds.contains_key("chat-starters"));
        assert!(
            !scene
                .hits
                .iter()
                .any(|hit| hit.key.starts_with("coder-suggest-"))
        );
        // A chat with replies shows the latest reply's follow-ups in the
        // row instead, above the docked composer.
        let (mut app, _) = followups_fixture(&[], &["What does it cost?"]);
        let (_, scene) = rust_native_desktop::capture(&mut app, 1200.0, 840.0, 1.0);
        assert!(!app.chat.as_ref().unwrap().composer_centered());
        assert!(!scene.bounds.contains_key("chat-starters"));
        assert!(scene.bounds.contains_key("chat-followups"));
    }

    /// Many or long follow-ups wrap onto more rows at the minimum window,
    /// each chip still inside the column and no wider than its words.
    #[test]
    fn followup_chips_wrap_at_the_minimum_window() {
        let latest = [
            "What can you do?",
            "What model is this?",
            "What does it cost?",
            "How do I connect my own computer?",
            "Can Coder work on my repository while I sleep?",
        ];
        let (mut app, _) = followups_fixture(&[], &latest);
        let directory =
            std::env::var_os("OPENAGENTS_CHIPS_CAPTURE_DIR").map(std::path::PathBuf::from);
        for (width, height, name) in [(1200.0, 840.0, "default"), (760.0, 540.0, "minimum")] {
            let (frame, scene) = rust_native_desktop::capture(&mut app, width, height, 2.0);
            let row = scene.bounds["chat-followups"];
            let card = scene.bounds["chat-composer-card"];
            assert!(row.y + row.h <= card.y);
            let hits: Vec<_> = (0..latest.len())
                .map(|index| {
                    scene
                        .hits
                        .iter()
                        .find(|hit| hit.key == format!("coder-followup-{index}"))
                        .unwrap()
                        .rect
                })
                .collect();
            let mut lines: Vec<_> = hits.iter().map(|rect| rect.y as i32).collect();
            lines.dedup();
            if name == "minimum" {
                assert!(lines.len() >= 2, "wraps at the minimum: {hits:?}");
            }
            for rect in &hits {
                assert!(rect.x >= row.x && rect.x + rect.w <= row.x + row.w + 0.5);
                assert!(rect.w < row.w, "{rect:?}");
            }
            if let Some(directory) = &directory {
                std::fs::create_dir_all(directory).unwrap();
                std::fs::write(
                    directory.join(format!("chips-wrap-{name}-{width}x{height}.png")),
                    frame.png().unwrap(),
                )
                .unwrap();
            }
        }
    }

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
                Turn::assistant(
                    "Ready for Coder.",
                    Some(openagents_chat::router::Meta {
                        route: Some(openagents_chat::delegation::DISPATCH_ROUTE.into()),
                        ..Default::default()
                    }),
                ),
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
        // Run Coder starts it on this computer (#10033): no host dispatch.
        assert!(
            panel
                .action(
                    openagents_desktop::chat_action::Action::Card {
                        key: "coder-run".into(),
                    },
                    &view,
                    Instant::now(),
                )
                .is_none()
        );
        let next = |panel: &mut openagents_desktop::chat::Panel| {
            for step in 0..20 {
                match panel.tick(Instant::now() + std::time::Duration::from_millis(step * 10)) {
                    Some(Request::Chat {
                        ticket,
                        command: Command::Read { .. } | Command::List {},
                    }) => panel.outcome(ticket, Ok(snapshot.clone())),
                    Some(request) => return request,
                    None => {}
                }
            }
            panic!("no request")
        };
        let Request::CoderRun {
            chat: started,
            ticket,
            request: openagents_chat_app::coder_run::Request::Start { title, prompt, .. },
        } = next(panel)
        else {
            panic!("start")
        };
        assert_eq!(started, chat);
        assert!(
            prompt.contains("fix the flaky test in openagents"),
            "{prompt}"
        );
        // The engine is told the routing is done and its job is the
        // person's task, never another engine's command line (#10084).
        assert_eq!(
            prompt,
            openagents_chat::delegation::prompt("New chat", &snapshot.turns),
        );
        assert!(prompt.contains("How this run started:"), "{prompt}");
        assert!(
            prompt.contains("never start another coding engine's command line"),
            "{prompt}"
        );
        assert!(!title.is_empty());
        panel.run_outcome(
            chat.clone(),
            ticket,
            Ok(openagents_chat_app::coder_run::Answer::Started {
                task: "b".repeat(64),
                project: "openagents".into(),
                checkout: "/w/openagents".into(),
            }),
        );
        // The thread records the task, so the host's threads show it.
        let Request::Chat { ticket, command } = next(panel) else {
            panic!("bind")
        };
        assert_eq!(
            command,
            Command::BindCoder {
                chat: chat.clone(),
                host: "local".into(),
                task: "b".repeat(64),
                project: Some("openagents".into()),
            }
        );
        panel.outcome(
            ticket,
            Ok(Snapshot {
                coder: Some(openagents_chat::basic_chats::Spawned {
                    host: "local".into(),
                    task: "b".repeat(64),
                    project: Some("openagents".into()),
                    at: Some(10),
                }),
                ..snapshot
            }),
        );
        assert_eq!(
            panel.coder_run(&chat).and_then(|run| run.task.as_deref()),
            Some("b".repeat(64).as_str())
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
        let now = Instant::now();
        let (mut app, mut snapshot) = DesktopApp::performance_fixture(2, 1, now);
        let mut meta = Meta::default();
        meta.followups.push(openagents_chat::router::Followup {
            label: "Who are you?".into(),
            answer: None,
        });
        snapshot.turns = vec![Turn::user("Hello"), Turn::assistant("Hello", Some(meta))];
        app.performance_stream(snapshot, now + std::time::Duration::from_secs(2));
        rust_native_desktop::capture(&mut app, 1200.0, 840.0, 2.0);
        // The follow-up is a chip above the composer, not a transcript row
        // (#10075); a click on it leaves the draft alone.
        let panel = app.chat.as_mut().unwrap();
        assert!(
            panel
                .transcript
                .control_bounds("coder-followup-0")
                .is_none()
        );
        let (_, scene) = rust_native_desktop::capture(&mut app, 1200.0, 840.0, 2.0);
        assert!(scene.hits.iter().any(|hit| hit.key == "coder-followup-0"));
        app.activate(
            Intent::Chat {
                action: openagents_desktop::chat_action::Action::Card {
                    key: "coder-followup-0".into(),
                },
            },
            now,
        );
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
    /// Files dropped together arrive one by one while the first is read:
    /// every image is attached and a dropped document's path joins the
    /// message, with no "already being imported" notice.
    #[test]
    fn files_dropped_together_are_all_taken() {
        let (mut app, now) = super::tests::chat_fixture(0);
        let create = app.chat.as_mut().unwrap().new_chat();
        app.send(vec![create], now);
        // The image pipeline, with attachments turned on (#10095).
        app.chat.as_mut().unwrap().set_attachments(true);
        app.present();
        let root = tempfile::tempdir().unwrap();
        let image = Image::pixels(4, 4, vec![200; 4 * 4 * 4]).unwrap();
        let mut paths = vec![];
        for name in ["one.png", "two.PNG", "three.png"] {
            let path = root.path().join(name);
            std::fs::write(&path, image.bytes.as_slice()).unwrap();
            paths.push(path);
        }
        let notes = root.path().join("notes.md");
        std::fs::write(&notes, b"# notes").unwrap();
        paths.push(notes.clone());
        for path in paths {
            assert!(app.dropped_file(path, now));
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.chat.as_ref().unwrap().images().len() < 3
            || !app.chat.as_ref().unwrap().draft().contains("notes.md")
        {
            assert!(Instant::now() < deadline, "the drop was not all taken");
            app.tick(Instant::now());
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            app.chat.as_ref().unwrap().draft().trim(),
            notes.display().to_string()
        );
        assert!(
            !serde_json::to_string(app.view().view())
                .unwrap()
                .contains("already being imported")
        );
    }

    #[test]
    fn dropped_images_preview_remove_and_stay_in_the_draft_when_the_words_send() {
        let (mut app, now) = super::tests::chat_fixture(0);
        let create = app.chat.as_mut().unwrap().new_chat();
        app.send(vec![create], now);
        // The image pipeline, with attachments turned on (#10095).
        app.chat.as_mut().unwrap().set_attachments(true);
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
        // Only the words went; the images stay in the draft, bound to the
        // message, for Coder (#10070).
        assert_eq!(app.chat.as_ref().unwrap().images().len(), 4);
        assert!(
            serde_json::to_string(app.view().view())
                .unwrap()
                .contains(openagents_chat_app::attachments::HELD_FOR_CODER)
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
    }

    /// Attachments off (#10095, the shared switch the phone uses): the
    /// composer has no attach control, a dropped image is dropped without
    /// a notice, and a dropped document still puts its path in the words.
    #[test]
    fn the_desktop_is_text_only_while_attachments_are_off() {
        const { assert!(!openagents_chat_app::coder_tab::ATTACHMENTS_ENABLED) };
        let (mut app, now) = super::tests::chat_fixture(0);
        let create = app.chat.as_mut().unwrap().new_chat();
        app.send(vec![create], now);
        app.present();
        assert!(!app.chat.as_ref().unwrap().attachments_enabled());
        for (width, height, scale) in [(1200.0, 840.0, 2.0), (760.0, 540.0, 1.0)] {
            let (_, scene) = rust_native_desktop::capture(&mut app, width, height, scale);
            assert!(scene.hits.iter().any(|hit| hit.key == "chat-send"));
            for key in ["chat-attach", "chat-paste-image"] {
                assert!(!scene.hits.iter().any(|hit| hit.key == key), "{key}");
            }
        }
        let root = tempfile::tempdir().unwrap();
        let image = Image::pixels(4, 4, vec![200; 4 * 4 * 4]).unwrap();
        let png = root.path().join("shot.png");
        std::fs::write(&png, image.bytes.as_slice()).unwrap();
        assert!(app.dropped_file(png, now));
        let until = Instant::now() + Duration::from_millis(300);
        while Instant::now() < until {
            app.tick(Instant::now());
            std::thread::sleep(Duration::from_millis(5));
        }
        let chat = app.chat.as_ref().unwrap();
        assert!(chat.images().is_empty());
        assert_eq!(chat.notice(), None);
        assert_eq!(chat.draft(), "");
        let view = serde_json::to_string(app.view().view()).unwrap();
        assert!(!view.contains("chat-attach") && !view.contains("image-previews"));
        let notes = root.path().join("notes.md");
        std::fs::write(&notes, b"# notes").unwrap();
        assert!(app.dropped_file(notes.clone(), now));
        assert_eq!(
            app.chat.as_ref().unwrap().draft().trim(),
            notes.display().to_string()
        );
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
    fn empty_chat_centers_the_draft_and_a_real_reply_docks_it() {
        for (width, height) in [(1200.0, 840.0), (760.0, 540.0)] {
            let now = Instant::now();
            let (mut app, mut snapshot) = DesktopApp::performance_fixture(0, 1, now);
            app.text_input(TextInput::Commit("Keep this draft  "), now);
            let (frame, empty) = rust_native_desktop::capture(&mut app, width, height, 1.0);
            assert_eq!(app.performance_counts().0, 0);
            let composer = empty.bounds["chat-composer-card"];
            assert_eq!(composer.h, 49.0);
            let pane = empty.split.unwrap().content.rect;
            // The centered group is the starter chips (#10097) above the
            // composer: the footer's middle sits where the composer's did.
            let footer = empty.bounds["chat-footer"];
            let footer_height = footer.h;
            assert!(empty.bounds.contains_key("chat-starters"));
            assert!((composer.y + composer.h - (footer.y + footer.h)).abs() < 0.01);
            assert!(
                (footer.y + footer.h / 2.0 - pane.y - (pane.h + footer_height) / 2.0 - 8.0).abs()
                    < 0.01,
                "composer {composer:?}, footer {footer:?}, reading clip {pane:?}"
            );
            assert!(
                empty
                    .surface_rect(openagents_desktop::chat::COMPOSER)
                    .unwrap()
                    .contains(composer.x + 50.0, composer.y + 20.0)
            );
            if let Some(path) = std::env::var_os("OPENAGENTS_LIST_CAPTURE_DIR") {
                let path = std::path::PathBuf::from(path);
                std::fs::create_dir_all(&path).unwrap();
                std::fs::write(
                    path.join(format!("empty-chat-{width}.png")),
                    frame.png().unwrap(),
                )
                .unwrap();
            }
            snapshot.total = 2;
            snapshot.turns = vec![
                openagents_chat::basic_coder::Turn::user("Hello"),
                openagents_chat::basic_coder::Turn::assistant("A real reply", None),
            ];
            app.performance_stream(snapshot, now + std::time::Duration::from_secs(2));
            let (_, conversation) = rust_native_desktop::capture(&mut app, width, height, 1.0);
            let docked = conversation.bounds["chat-composer-card"];
            assert_eq!(docked.y + docked.h, height - 8.0);
            assert_eq!(app.chat.as_ref().unwrap().draft(), "Keep this draft  ");
        }
    }
    /// Pixels in the composer's field that are the placeholder's ink.
    fn placeholder_ink(
        frame: &rust_native_desktop::Frame,
        rect: rust_native_desktop::Rect,
        scale: f32,
    ) -> usize {
        let faint = openagents_chat_app::visual::FAINT;
        let mut count = 0;
        let x0 = (rect.x * scale) as usize;
        let y0 = (rect.y * scale) as usize;
        for y in y0..((rect.y + rect.h) * scale) as usize {
            for x in x0..((rect.x + rect.w) * scale) as usize {
                let [r, g, b] = frame.pixel(x, y);
                if r.abs_diff(faint.red) < 24
                    && g.abs_diff(faint.green) < 24
                    && b.abs_diff(faint.blue) < 24
                {
                    count += 1;
                }
            }
        }
        count
    }
    #[test]
    fn the_placeholder_paints_in_the_centered_and_the_docked_composer() {
        for (width, height) in [(1200.0, 840.0), (760.0, 540.0)] {
            for scale in [1.0, 2.0] {
                for rows in [0, 4] {
                    let (mut app, now) = super::tests::chat_fixture(rows);
                    assert_eq!(
                        app.chat.as_ref().unwrap().composer_centered(),
                        rows == 0,
                        "an empty chat centers its composer"
                    );
                    let (frame, scene) =
                        rust_native_desktop::capture(&mut app, width, height, scale);
                    let field = scene
                        .surface_rect(openagents_desktop::chat::COMPOSER)
                        .unwrap();
                    let empty = placeholder_ink(&frame, field, scale);
                    assert!(
                        empty > (40.0 * scale * scale) as usize,
                        "Ask OpenAgents anything shows with {rows} rows at {width}x{height} {scale}x: {empty}"
                    );
                    if let Some(path) = std::env::var_os("OPENAGENTS_POLISH_EVIDENCE") {
                        let path = std::path::PathBuf::from(path);
                        std::fs::create_dir_all(&path).unwrap();
                        std::fs::write(
                            path.join(format!(
                                "composer-{}-{width}x{height}-{scale}x.png",
                                if rows == 0 { "centered" } else { "docked" }
                            )),
                            frame.png().unwrap(),
                        )
                        .unwrap();
                    }
                    app.text_input(TextInput::Commit("Hi"), now);
                    let (frame, scene) =
                        rust_native_desktop::capture(&mut app, width, height, scale);
                    let field = scene
                        .surface_rect(openagents_desktop::chat::COMPOSER)
                        .unwrap();
                    assert!(
                        placeholder_ink(&frame, field, scale) < empty / 4,
                        "typing hides the placeholder"
                    );
                }
            }
        }
    }
    #[test]
    fn engines_sit_above_the_footer_and_the_filter_waits_for_five_chats() {
        use openagents_desktop::control::{EngineReport, EngineRoute, RouteUsage, UsageWindow};
        let window = |name: &str, label: &str, used: u8, resets: &str| UsageWindow {
            name: name.into(),
            label: label.into(),
            used_percent: used,
            resets_at: Some(1_791_050_824),
            resets: Some(resets.into()),
        };
        let now = Instant::now();
        let (mut app, _) = DesktopApp::performance_fixture(0, 3, now);
        let report = EngineReport {
            enabled: true,
            adapter: "microcoder-repository".into(),
            model: "gpt-6-luna".into(),
            routes: vec![
                EngineRoute {
                    provider: "codex".into(),
                    name: "Codex".into(),
                    model: "gpt-6-luna".into(),
                    signed_in: true,
                    usage: RouteUsage::Windows {
                        windows: vec![
                            window("primary", "5 hours", 72, "2026-10-01 18:07 UTC"),
                            window("secondary", "Week", 31, "2026-10-06 09:00 UTC"),
                        ],
                        limit_reached: false,
                        used_percent: 72,
                    },
                },
                EngineRoute {
                    provider: "claude".into(),
                    name: "Claude Code".into(),
                    model: "claude-opus-5-5".into(),
                    signed_in: true,
                    usage: RouteUsage::Windows {
                        windows: vec![window("five_hour", "5 hours", 94, "2026-10-01 16:00 UTC")],
                        limit_reached: false,
                        used_percent: 94,
                    },
                },
            ],
            accounts: vec![],
            usage_probe: Some(90),
            refresh_due: false,
        };
        app.model.engine = Some(report.clone());
        app.present();
        let evidence = std::env::var_os("OPENAGENTS_POLISH_EVIDENCE").map(std::path::PathBuf::from);
        let save = |app: &mut DesktopApp, name: &str, width: f32, height: f32| {
            let (frame, scene) = rust_native_desktop::capture(app, width, height, 2.0);
            if let Some(path) = &evidence {
                std::fs::create_dir_all(path).unwrap();
                std::fs::write(
                    path.join(format!("{name}-{width}x{height}.png")),
                    frame.png().unwrap(),
                )
                .unwrap();
            }
            scene
        };
        for (width, height) in [(1200.0, 840.0), (760.0, 540.0)] {
            let scene = save(&mut app, "sidebar-few-chats", width, height);
            assert!(!scene.bounds.contains_key("chat-search"));
            let footer = scene.bounds["sidebar-footer"];
            // The theme toggle sits in the account row's corner (#11120).
            let theme = scene.bounds["sidebar-theme"];
            assert!(theme.y >= footer.y && theme.y + theme.h <= footer.y + footer.h);
            for index in 0..2 {
                let row = scene.bounds[&format!("sidebar-engine-row-{index}")];
                assert!(row.h <= 32.0, "one condensed line: {row:?}");
                assert!(row.y + row.h <= footer.y, "above the footer");
                assert!(
                    scene
                        .bounds
                        .contains_key(&format!("sidebar-engine-{index}-meter"))
                );
            }
            assert!(!scene.bounds.contains_key("engine-strip"));
        }
        let (mut app, _) = DesktopApp::performance_fixture(0, 5, now);
        app.model.engine = Some(report);
        app.present();
        for (width, height) in [(1200.0, 840.0), (760.0, 540.0)] {
            let scene = save(&mut app, "sidebar-five-chats", width, height);
            assert!(scene.bounds.contains_key("chat-search"));
        }
        app.activate(
            Intent::Navigate {
                action: chrome::Action::Grid,
            },
            now,
        );
        for (width, height) in [(1200.0, 840.0), (760.0, 540.0)] {
            save(&mut app, "verse-page", width, height);
        }
        app.activate(
            Intent::Settings {
                action: openagents_desktop::settings::Action::Pane {
                    pane: openagents_desktop::settings::Pane::Coder,
                },
            },
            now,
        );
        for (width, height) in [(1200.0, 840.0), (760.0, 540.0)] {
            let scene = save(&mut app, "settings-coder-engines", width, height);
            assert!(scene.bounds.contains_key("engine-strip"));
        }
    }
    #[test]
    fn a_sidebar_chat_row_is_one_unwrapped_line() {
        let (mut app, _) = DesktopApp::performance_fixture(0, 1, Instant::now());
        let row = app.navigation.as_ref().unwrap().chats[0].clone();
        for (width, height) in [(1200.0, 840.0), (760.0, 540.0)] {
            let (frame, scene) = rust_native_desktop::capture(&mut app, width, height, 1.0);
            let bounds = scene.bounds[&format!("sidebar-chat-{}", row.id)];
            // One line on the web's 36-point row (#11120).
            assert_eq!(bounds.h, 36.0);
            let runs: Vec<_> = scene
                .ops
                .iter()
                .filter_map(|op| {
                    if let rust_native_desktop::layout::Op::Text {
                        paragraph, x, y, ..
                    } = op
                        && *x >= bounds.x
                        && *x < bounds.x + bounds.w
                        && *y >= bounds.y
                        && *y < bounds.y + bounds.h
                    {
                        Some((paragraph, *y))
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(runs.len(), 1);
            assert_eq!(runs[0].0.text, row.title);
            assert_eq!(runs[0].0.font.size, 14.0);
            assert_eq!(runs[0].0.line_height, 20.0);
            assert!(runs.iter().all(|(paragraph, _)| paragraph.lines.len() == 1));
            if let Some(path) = std::env::var_os("OPENAGENTS_LIST_CAPTURE_DIR") {
                let path = std::path::PathBuf::from(path);
                std::fs::create_dir_all(&path).unwrap();
                std::fs::write(
                    path.join(format!("sidebar-type-{width}.png")),
                    frame.png().unwrap(),
                )
                .unwrap();
            }
        }
    }
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
                // Centered on the traffic lights, within a point.
                assert!((toggle.rect.y + toggle.rect.h / 2.0 - 21.0).abs() <= 1.0);
                assert!(
                    toggle.rect.x
                        >= if cfg!(target_os = "macos") {
                            if fullscreen { 12.0 } else { 88.0 }
                        } else {
                            10.0
                        }
                );
                let composer = scene.bounds["chat-composer"];
                assert!(composer.y + composer.h <= height);
                assert_eq!(composer.h, 47.0);
            }
        }
    }

    #[test]
    fn titlebar_labels_its_controls_and_steps_through_visited_chats() {
        let now = Instant::now();
        let (mut app, _) = DesktopApp::performance_fixture(0, 3, now);
        let start = if cfg!(target_os = "macos") {
            88.0
        } else {
            10.0
        };
        let hit = |scene: &rust_native_desktop::layout::Scene, key: &str| {
            scene
                .hits
                .iter()
                .find(|hit| hit.key == key)
                .unwrap_or_else(|| panic!("{key}"))
                .clone()
        };
        let ids: Vec<u64> = app
            .navigation
            .as_ref()
            .unwrap()
            .chats
            .iter()
            .map(|chat| chat.id)
            .collect();
        assert!(ids.len() >= 3);
        for (width, height, scale) in [(1200.0, 840.0, 1.0), (760.0, 540.0, 2.0)] {
            let (frame, scene) = rust_native_desktop::capture(&mut app, width, height, scale);
            if let Some(path) = std::env::var_os("OPENAGENTS_COMMAND_CAPTURE_DIR") {
                let path = std::path::PathBuf::from(path);
                std::fs::create_dir_all(&path).unwrap();
                std::fs::write(
                    path.join(format!("titlebar-{width}x{height}-{scale}x.png")),
                    frame.png().unwrap(),
                )
                .unwrap();
            }
            // The sidebar toggle carries its label (#11120), 24 points tall
            // and centered 21 points down the 38-point titlebar; Back,
            // Forward, and the plus are not drawn (New chat is the sidebar's
            // first row).
            let control = hit(&scene, "shell-toggle-sidebar");
            assert_eq!(control.rect.x, start);
            assert!(control.rect.h >= 22.0 && control.rect.h <= 24.0);
            assert!(control.rect.w > 24.0, "a visible label");
            assert!((control.rect.y + control.rect.h / 2.0 - 21.0).abs() <= 1.0);
            for key in ["shell-back", "shell-forward", "shell-new-chat"] {
                assert!(scene.hits.iter().all(|hit| hit.key != key), "{key}");
            }
            // The title begins 16 points past the 256-point sidebar.
            let title = scene.bounds["shell-page-title"];
            // The labelled toggle's width follows the system face, so the
            // title lands within a few points of that line.
            assert!((title.x - (256.0_f32 + 16.0).max(start + 124.0)).abs() < 4.0);
            let menu = hit(&scene, "chat-menu");
            assert!(menu.rect.w > 28.0, "a visible label");
            assert!((menu.rect.x + menu.rect.w + 6.0 - width).abs() < 0.01);
            let heading = scene
                .ops
                .iter()
                .find_map(|op| match op {
                    rust_native_desktop::layout::Op::Text {
                        paragraph, color, ..
                    } if paragraph.text
                        == app.navigation.as_ref().unwrap().selected().unwrap().title =>
                    {
                        Some((paragraph.font.size, paragraph.font.weight, color.alpha))
                    }
                    _ => None,
                })
                .expect("title text");
            assert_eq!(
                heading,
                (14.0, rust_native::layout::display::Weight::Medium, 217)
            );
        }
        let visit = |app: &mut DesktopApp, id: u64| {
            app.activate(
                Intent::Navigate {
                    action: chrome::Action::SelectChat { id },
                },
                now,
            );
        };
        let page = |app: &DesktopApp| app.navigation.as_ref().unwrap().page;
        visit(&mut app, ids[0]);
        visit(&mut app, ids[1]);
        visit(&mut app, ids[2]);
        assert_eq!(page(&app), Page::Chat(ids[2]));
        let step = |app: &mut DesktopApp, action| {
            app.activate(Intent::Navigate { action }, now);
        };
        step(&mut app, chrome::Action::Back);
        assert_eq!(page(&app), Page::Chat(ids[1]));
        step(&mut app, chrome::Action::Back);
        assert_eq!(page(&app), Page::Chat(ids[0]));
        step(&mut app, chrome::Action::Forward);
        assert_eq!(page(&app), Page::Chat(ids[1]));
        // Pages join the same history; a new visit drops the forward pages.
        step(&mut app, chrome::Action::Settings);
        assert_eq!(page(&app), Page::Settings);
        step(&mut app, chrome::Action::Back);
        assert_eq!(page(&app), Page::Chat(ids[1]));
        step(&mut app, chrome::Action::Forward);
        assert_eq!(page(&app), Page::Settings);
    }

    /// #10100: old Coder issue-flow chats in projects ("work on #10058")
    /// and a new chat with no project: the new chat is the sidebar's top
    /// row and selected; the chats come newest first, then the Coder chats
    /// under their project's name, as the web lists them (#11120).
    #[test]
    fn a_new_chat_is_the_top_sidebar_row_above_older_project_chats() {
        let now = Instant::now();
        let (mut app, mut snapshot) = DesktopApp::performance_fixture(0, 6, now);
        let projects = [
            None,
            Some(("work on #10058", "openagents", 1_000)),
            Some(("work on #10057", "openagents", 990)),
            Some(("work on #10060", "openagents-host-tasks", 1_010)),
            Some(("work on #10061", "openagents-host-tasks", 1_020)),
            None,
        ];
        for (index, (row, project)) in snapshot.chats.iter_mut().zip(projects).enumerate() {
            if let Some((title, project, updated)) = project {
                row.title = title.into();
                row.updated = updated;
                row.coder = Some(openagents_chat::basic_chats::Spawned {
                    host: "local".into(),
                    task: format!("{index:064x}"),
                    project: Some(project.into()),
                    at: Some(updated),
                });
            } else if index == 0 {
                row.title = "New chat".into();
                row.updated = 2_000;
            } else {
                row.title = "plain old".into();
                row.updated = 900;
            }
        }
        snapshot.list_total = 6;
        snapshot.list_version = 2;
        app.performance_stream(snapshot, now + std::time::Duration::from_secs(2));
        let (_, scene) = rust_native_desktop::capture(&mut app, 1200.0, 840.0, 1.0);
        let state = app.navigation.as_ref().unwrap();
        let mut rows: Vec<_> = state
            .chats
            .iter()
            .map(|chat| (scene.bounds[&format!("sidebar-chat-{}", chat.id)].y, chat))
            .collect();
        rows.sort_by(|a, b| a.0.total_cmp(&b.0));
        let titles: Vec<&str> = rows.iter().map(|(_, chat)| chat.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "New chat",
                "plain old",
                "work on #10061",
                "work on #10060",
                "work on #10058",
                "work on #10057",
            ]
        );
        assert_eq!(state.selected().map(|chat| chat.id), Some(rows[0].1.id));
        assert!(
            !scene
                .bounds
                .keys()
                .any(|key| key.starts_with("project-group-"))
        );
        // The project names its group, above its chats.
        let project = scene.ops.iter().any(|op| {
            matches!(
                op,
                rust_native_desktop::layout::Op::Text { paragraph, y, .. }
                    if paragraph.text == "openagents-host-tasks"
                        && *y < scene.bounds[&format!("sidebar-chat-{}", rows[2].1.id)].y
            )
        });
        assert!(project, "the project does not name its group");
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
        // One list, no project headers (#10100): each row names its project.
        assert!(
            !scene
                .bounds
                .keys()
                .any(|key| key.starts_with("project-group-") || key == "project-more-group")
        );
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
        // Enough chats for the sidebar's filter (#10072).
        for _ in 0..4 {
            let request = app.chat.as_mut().unwrap().new_chat();
            app.send(vec![request], now);
        }
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
                control: false,
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
                control: false,
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
                control: false,
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
                control: false,
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
            for key in ["chat-send", "chat-menu"] {
                let hit = scene.hits.iter().find(|h| h.key == key).unwrap();
                assert!(hit.rect.y + hit.rect.h <= height, "{key}");
            }
            // Text only (#10095): no attach control.
            assert!(!scene.hits.iter().any(|h| h.key == "chat-attach"));
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
    fn rename_dialog_keeps_the_chat_mounted_and_preserves_the_draft() {
        for (width, height) in [(1200.0, 840.0), (760.0, 540.0)] {
            for scale in [1.0, 2.0] {
                let (mut app, now) = super::tests::chat_fixture(0);
                key(&mut app, now, "n", true, false);
                app.text_input(TextInput::Commit("Keep this draft  "), now);
                let (_, before) = rust_native_desktop::capture(&mut app, width, height, scale);
                app.activate(
                    Intent::Chat {
                        action: ChatAction::Rename,
                    },
                    now,
                );
                let (frame, opened) = rust_native_desktop::capture(&mut app, width, height, scale);
                let card = opened.bounds["chat-rename-controls"];
                assert_eq!(card.w, 360.0);
                assert!((card.x + card.w / 2.0 - width / 2.0).abs() < 0.01);
                assert!((card.y + card.h / 2.0 - height / 2.0).abs() < 0.01);
                for key in ["chat-transcript", "chat-composer-card"] {
                    assert_eq!(opened.bounds[key], before.bounds[key]);
                }
                let field = opened.bounds["chat-rename-field"];
                let input = opened.bounds["chat-rename"];
                assert!(field.x > card.x && field.x + field.w < card.x + card.w);
                assert!(input.h >= 22.75 && input.h < 24.0);
                let cancel = opened.bounds["chat-cancel-name"];
                let save = opened.bounds["chat-save-name"];
                assert_eq!(cancel.h, 33.0);
                assert_eq!(save.h, 33.0);
                assert!((save.x - cancel.x - cancel.w - 8.0).abs() < 0.01);
                assert!(opened.ops.iter().any(|op| matches!(op,
                    rust_native_desktop::layout::Op::Text { paragraph, .. }
                    if paragraph.text == "Rename chat" && paragraph.font.size == 15.0
                        && paragraph.font.weight == rust_native::layout::display::Weight::Semibold)));
                if let Some(path) = std::env::var_os("OPENAGENTS_RENAME_EVIDENCE") {
                    let path = std::path::PathBuf::from(path);
                    std::fs::create_dir_all(&path).unwrap();
                    std::fs::write(
                        path.join(format!("rename-{width}x{height}-{scale}x.png")),
                        frame.png().unwrap(),
                    )
                    .unwrap();
                }
                assert!(!app.allows_focus("sidebar-profile"));
                assert!(app.pointer_down(Some("sidebar-profile"), (8.0, 8.0), now));
                assert!(app.chat.as_ref().unwrap().modal());
                key(&mut app, now, "a", true, false);
                app.text_input(
                    TextInput::Preedit {
                        text: "名前",
                        selection: Some((0, 6)),
                    },
                    now,
                );
                key(&mut app, now, "Escape", false, false);
                assert!(
                    app.chat.as_ref().unwrap().modal(),
                    "Escape first cancels composition"
                );
                key(&mut app, now, "Escape", false, false);
                assert!(!app.chat.as_ref().unwrap().modal());
                assert_eq!(app.chat.as_ref().unwrap().draft(), "Keep this draft  ");
                app.activate(
                    Intent::Chat {
                        action: ChatAction::Rename,
                    },
                    now,
                );
                key(&mut app, now, "Tab", false, false);
                key(&mut app, now, "Enter", false, false);
                assert!(
                    !app.chat.as_ref().unwrap().modal(),
                    "Cancel is first in button order"
                );
                assert_eq!(app.chat.as_ref().unwrap().draft(), "Keep this draft  ");
            }
        }
    }
    #[test]
    fn profile_footer_matches_source_geometry_and_preserves_navigation() {
        use rust_native::layout::display::Weight;
        use rust_native_desktop::layout::Op;
        for (width, height) in [(1200.0, 840.0), (760.0, 540.0)] {
            for scale in [1.0, 2.0] {
                let (mut app, now) = super::tests::chat_fixture(0);
                // The Verse entry shows only in a preview build (#11120).
                app.chat.as_mut().unwrap().preview = true;
                key(&mut app, now, "n", true, false);
                app.text_input(TextInput::Commit("Keep this draft  "), now);
                let (_, closed) = rust_native_desktop::capture(&mut app, width, height, scale);
                let footer = closed.bounds["sidebar-footer"];
                let profile = closed.bounds["sidebar-profile"];
                let theme = closed.bounds["sidebar-theme"];
                // The web's 36-point account row, with the theme toggle in
                // its corner (#11120).
                assert_eq!(profile.h, 36.0);
                assert!(profile.x + profile.w < theme.x);
                assert!(
                    (theme.x + theme.w - footer.x - footer.w).abs() < 0.5,
                    "{footer:?} {profile:?} {theme:?}"
                );
                assert!(
                    closed
                        .ops
                        .iter()
                        .any(|op| matches!(op, Op::Text { paragraph, .. }
                    if paragraph.font.weight == Weight::Medium && paragraph.font.size == 14.0))
                );
                assert!(closed.ops.iter().any(|op| matches!(op, Op::Text { paragraph, .. }
                    if paragraph.font.weight == Weight::Semibold && paragraph.font.size == 12.0 && paragraph.font.mono)));
                for key in ["computers", "grid", "saved", "commands"] {
                    app.activate(
                        Intent::Chat {
                            action: ChatAction::Profile,
                        },
                        now,
                    );
                    let (frame, opened) =
                        rust_native_desktop::capture(&mut app, width, height, scale);
                    let menu = opened.bounds["command-panel"];
                    assert_eq!(
                        menu.w,
                        app.navigation.as_ref().unwrap().sidebar_width - 16.0
                    );
                    assert!((menu.x - footer.x).abs() < 0.5);
                    assert!(menu.y + menu.h <= footer.y - 7.5);
                    assert!(!opened.bounds.contains_key("command-search-header"));
                    for action in ["computers", "grid", "saved", "commands"] {
                        let hit = opened
                            .hits
                            .iter()
                            .find(|hit| hit.key == format!("command-{action}"))
                            .unwrap();
                        assert!(hit.enabled);
                        assert_eq!(hit.rect.h, 32.0);
                    }
                    if key == "computers"
                        && let Some(path) = std::env::var_os("OPENAGENTS_PROFILE_EVIDENCE")
                    {
                        let path = std::path::PathBuf::from(path);
                        std::fs::create_dir_all(&path).unwrap();
                        std::fs::write(
                            path.join(format!("profile-{width}x{height}-{scale}x.png")),
                            frame.png().unwrap(),
                        )
                        .unwrap();
                    }
                    app.activate(
                        Intent::Chat {
                            action: ChatAction::Command { key: key.into() },
                        },
                        now,
                    );
                    app.chat_effects(now);
                    match key {
                        "computers" => {
                            assert_eq!(app.navigation.as_ref().unwrap().page, Page::Computers)
                        }
                        "grid" => assert_eq!(app.navigation.as_ref().unwrap().page, Page::Grid),
                        "saved" => assert_eq!(app.navigation.as_ref().unwrap().page, Page::Saved),
                        "commands" => {
                            assert!(app.chat.as_ref().unwrap().modal());
                            key_escape(&mut app, now);
                        }
                        _ => unreachable!(),
                    }
                    assert_eq!(app.chat.as_ref().unwrap().draft(), "Keep this draft  ");
                }
                app.activate(
                    Intent::Chat {
                        action: ChatAction::Profile,
                    },
                    now,
                );
                assert!(app.chat.as_ref().unwrap().modal());
                app.activate(
                    Intent::Chat {
                        action: ChatAction::Profile,
                    },
                    now,
                );
                assert!(!app.chat.as_ref().unwrap().modal());
                app.activate(
                    Intent::Navigate {
                        action: chrome::Action::Settings,
                    },
                    now,
                );
                assert_eq!(app.navigation.as_ref().unwrap().page, Page::Settings);
                app.activate(
                    Intent::Navigate {
                        action: chrome::Action::Settings,
                    },
                    now,
                );
                assert_eq!(app.navigation.as_ref().unwrap().page, Page::Saved);
                assert_eq!(app.chat.as_ref().unwrap().draft(), "Keep this draft  ");
            }
        }
    }
    fn key_escape(app: &mut DesktopApp, now: Instant) {
        key(app, now, "Escape", false, false);
    }

    #[test]
    fn palette_history_matches_sidebar_typography_and_keeps_its_action() {
        let (mut app, now) = super::tests::chat_fixture(0);
        key(&mut app, now, "n", true, false);
        let id = app
            .chat
            .as_ref()
            .unwrap()
            .state()
            .unwrap()
            .chat
            .clone()
            .unwrap();
        let title = app.navigation.as_ref().unwrap().chats[0].title.clone();
        key(&mut app, now, "k", true, false);
        app.text_input(TextInput::Commit("new"), now);
        for (width, height) in [(1200.0, 840.0), (760.0, 540.0)] {
            let (_, scene) = rust_native_desktop::capture(&mut app, width, height, 1.0);
            let key = format!("command-switch-{id}");
            let row = scene.bounds[&key];
            assert_eq!(row.h, 50.0);
            assert!(scene.bounds.contains_key("command-history-separator"));
            let runs: Vec<_> = scene
                .ops
                .iter()
                .filter_map(|op| {
                    if let rust_native_desktop::layout::Op::Text {
                        paragraph, x, y, ..
                    } = op
                        && *x >= row.x
                        && *x < row.x + row.w
                        && *y >= row.y
                        && *y < row.y + row.h
                    {
                        Some(paragraph)
                    } else {
                        None
                    }
                })
                .collect();
            // A starter chip under the palette can share the row's band;
            // the row itself is its detail line and its title.
            let detail = runs.iter().position(|run| run.font.size == 12.0);
            let named = runs.iter().position(|run| run.text == title);
            assert!(detail.is_some() && named.is_some() && detail < named);
            assert_eq!(runs[named.unwrap()].font.size, 14.0);
            let view = app.view().view();
            let intent = app
                .view()
                .activate(&rust_native::Activation {
                    instance: view.instance.clone(),
                    revision: view.revision,
                    node: key,
                })
                .unwrap();
            assert!(
                matches!(intent, Intent::Chat { action: ChatAction::Command { key } }
                if key == &format!("switch-{id}"))
            );
            capture(&mut app, &format!("palette-history-{width}"), width, height);
        }
    }
    #[test]
    fn palette_scrolls_its_reference_viewport_without_moving_rows_on_hover() {
        use rust_native_desktop::input::NativeInput;
        use rust_native_desktop::layout::{Rect, Scene};
        for (width, height) in [(1200.0, 840.0), (760.0, 540.0)] {
            for scale in [1.0, 2.0] {
                let now = Instant::now();
                let (mut app, _) = DesktopApp::performance_fixture(0, 500, now);
                key(&mut app, now, "k", true, false);
                let (_, initial) = rust_native_desktop::capture(&mut app, width, height, scale);
                let rows = |scene: &Scene| {
                    scene
                        .hits
                        .iter()
                        .filter(|hit| hit.key.starts_with("command-"))
                        .map(|hit| (hit.key.clone(), scene.bounds[&hit.key]))
                        .collect::<Vec<_>>()
                };
                let visible = |scene: &Scene| {
                    let window = scene.viewports[0].rect;
                    rows(scene)
                        .into_iter()
                        .filter(|(_, rect)| {
                            rect.y + rect.h > window.y && rect.y < window.y + window.h
                        })
                        .collect::<Vec<_>>()
                };
                // Zeron: results at most (height - 180) clamped to 100..360,
                // every action, and the first 30 matching conversations.
                let viewport = initial.viewports[0].clone();
                assert_eq!(viewport.key, "command-results");
                assert_eq!(viewport.rect.h, (height - 180.0_f32).clamp(100.0, 360.0));
                assert_eq!(viewport.offset, 0.0);
                let initial_rows = rows(&initial);
                let history: Vec<_> = initial_rows
                    .iter()
                    .filter(|(key, _)| key.starts_with("command-switch-"))
                    .collect();
                assert_eq!(history.len(), 30);
                assert!(history.iter().all(|(_, rect)| rect.h == 50.0));
                // Content height: 8-point insets, 2-point gaps, 32-point
                // actions, 50-point conversations, and the 15-point section rule.
                let actions = initial_rows.len() - history.len();
                let full = 8.0
                    + actions as f32 * 32.0
                    + 30.0 * 50.0
                    + (initial_rows.len() - 1) as f32 * 2.0
                    + 15.0
                    + 2.0
                    + 8.0;
                assert_eq!(viewport.limit + viewport.rect.h, full);
                let first_history = history[0].1;
                let last_action = initial_rows[actions - 1].1;
                assert_eq!(last_action.h, 32.0);
                assert_eq!(
                    first_history.y - (last_action.y + last_action.h),
                    2.0 + 15.0 + 2.0
                );
                // The bottom edge fades while rows remain below it.
                let fade = |scene: &Scene, y: f32| {
                    scene.ops.iter().any(|op| {
                        matches!(op, rust_native_desktop::layout::Op::Fill { rect, .. }
                            if rect.h == 1.0 && rect.y == y && rect.w == viewport.rect.w)
                    })
                };
                let bottom = viewport.rect.y + viewport.rect.h - 1.0;
                assert!(fade(&initial, bottom));
                assert!(!fade(&initial, viewport.rect.y));
                // Hovering the partly hidden edge row changes selection only.
                let edge = visible(&initial).last().unwrap().0.clone();
                assert!(app.pointer_hover(Some(&edge), now));
                for _ in 0..4 {
                    let (_, scene) = rust_native_desktop::capture(&mut app, width, height, scale);
                    assert_eq!(rows(&scene), initial_rows);
                    assert_eq!(scene.viewports[0].offset, 0.0);
                    assert_eq!(
                        scene.bounds["command-panel"],
                        initial.bounds["command-panel"]
                    );
                    assert!(!app.pointer_hover(Some(&edge), now));
                }
                // The wheel over the results scrolls them by points; the
                // selection stays on its row.
                let inside = (
                    viewport.rect.x + viewport.rect.w / 2.0,
                    viewport.rect.y + viewport.rect.h / 2.0,
                );
                assert!(app.native_input(
                    NativeInput::Wheel {
                        x: inside.0,
                        y: inside.1,
                        lines: -1.0,
                    },
                    now
                ));
                let (frame, wheeled) = rust_native_desktop::capture(&mut app, width, height, scale);
                if let Some(path) = std::env::var_os("OPENAGENTS_COMMAND_CAPTURE_DIR") {
                    let path = std::path::PathBuf::from(path);
                    std::fs::create_dir_all(&path).unwrap();
                    std::fs::write(
                        path.join(format!("palette-scroll-{width}x{height}-{scale}x.png")),
                        frame.png().unwrap(),
                    )
                    .unwrap();
                }
                assert_eq!(wheeled.viewports[0].offset, 40.0);
                assert_eq!(
                    wheeled.bounds["command-panel"],
                    initial.bounds["command-panel"]
                );
                for ((key, before), (after_key, after)) in initial_rows.iter().zip(rows(&wheeled)) {
                    assert_eq!(key, &after_key);
                    assert_eq!(after.y, before.y - 40.0);
                }
                assert!(fade(&wheeled, viewport.rect.y));
                // Outside the results, the scrim takes the wheel: nothing
                // beneath it scrolls and the results stay put.
                let transcript = wheeled.bounds.get("chat-transcript").copied();
                assert!(app.native_input(
                    NativeInput::Wheel {
                        x: 2.0,
                        y: height - 2.0,
                        lines: -3.0,
                    },
                    now
                ));
                let (_, scrim) = rust_native_desktop::capture(&mut app, width, height, scale);
                assert_eq!(scrim.viewports[0].offset, 40.0);
                assert_eq!(scrim.bounds.get("chat-transcript").copied(), transcript);
                if scale != 1.0 {
                    // Keyboard reveal is scale-independent layout; keep the
                    // repeated full captures to one scale.
                    continue;
                }
                // Keys reveal the selected row with the least movement.
                for _ in 0..40 {
                    key(&mut app, now, "ArrowDown", false, false);
                    let (_, scene) = rust_native_desktop::capture(&mut app, width, height, scale);
                    let window: Rect = scene.viewports[0].rect;
                    let selected = scene
                        .ops
                        .iter()
                        .find_map(|op| match op {
                            // The palette's row, not the sidebar's selected
                            // chat, which shares the color (#10100 put the
                            // open chat at the top of the sidebar).
                            rust_native_desktop::layout::Op::Fill { rect, color, .. }
                                if *color == openagents_chat_app::visual::SELECTED
                                    && rect.w < window.w
                                    && rect.x >= window.x - 0.01
                                    && rect.x + rect.w <= window.x + window.w + 0.01 =>
                            {
                                Some(*rect)
                            }
                            _ => None,
                        })
                        .expect("a selected row");
                    assert!(selected.y >= window.y - 0.01);
                    assert!(selected.y + selected.h <= window.y + window.h + 0.01);
                }
                // Wrapping to the first row returns to the top inset.
                for _ in 0..initial_rows.len() {
                    key(&mut app, now, "ArrowDown", false, false);
                    let (_, scene) = rust_native_desktop::capture(&mut app, width, height, scale);
                    if scene.viewports[0].offset == 0.0 {
                        assert_eq!(rows(&scene), initial_rows);
                        break;
                    }
                }
                let (_, top) = rust_native_desktop::capture(&mut app, width, height, scale);
                assert_eq!(top.viewports[0].offset, 0.0);
            }
        }
    }
    #[test]
    fn a_conversation_fits_both_window_sizes_under_the_zeron_titlebar() {
        // Retained closeout captures of a real transcript (Markdown, inline
        // code, lists, and a code block) at the default and minimum windows.
        for (width, height) in [(1200.0, 840.0), (760.0, 540.0)] {
            for scale in [1.0, 2.0] {
                let (mut app, _) = super::tests::chat_fixture(8);
                let (frame, scene) = rust_native_desktop::capture(&mut app, width, height, scale);
                assert!(scene.unsupported.is_empty(), "{:?}", scene.unsupported);
                assert_eq!(scene.bounds["shell-titlebar"].h, 38.0);
                let transcript = scene
                    .surface_rect(openagents_desktop::chat::TRANSCRIPT)
                    .expect("a transcript");
                let composer = scene.bounds["chat-composer-card"];
                assert!(transcript.y >= 38.0 && transcript.x + transcript.w <= width);
                assert!(composer.y + composer.h <= height);
                if let Some(path) = std::env::var_os("OPENAGENTS_COMMAND_CAPTURE_DIR") {
                    let path = std::path::PathBuf::from(path);
                    std::fs::create_dir_all(&path).unwrap();
                    std::fs::write(
                        path.join(format!("transcript-{width}x{height}-{scale}x.png")),
                        frame.png().unwrap(),
                    )
                    .unwrap();
                }
            }
        }
    }
    #[test]
    fn pointer_motion_and_keys_share_one_palette_selection() {
        let (mut app, now) = super::tests::chat_fixture(0);
        key(&mut app, now, "n", true, false);
        key(&mut app, now, "k", true, false);
        assert!(app.pointer_hover(Some("command-pin"), now));
        fn selected(app: &mut DesktopApp) -> String {
            let (_, scene) = rust_native_desktop::capture(app, 760.0, 540.0, 1.0);
            let selected: Vec<_> = scene
                .hits
                .iter()
                .filter(|hit| {
                    hit.key.starts_with("command-")
                        && scene.ops.iter().any(|op| {
                            matches!(op,
                    rust_native_desktop::layout::Op::Fill { rect, color, .. }
                    if *rect == hit.rect && *color == openagents_chat_app::visual::SELECTED)
                        })
                })
                .collect();
            assert_eq!(selected.len(), 1);
            selected[0].key.clone()
        }
        assert_eq!(selected(&mut app), "command-pin");
        key(&mut app, now, "ArrowDown", false, false);
        assert_ne!(selected(&mut app), "command-pin");
        // Moving inside the same row again takes selection back from the keys.
        assert!(app.pointer_hover(Some("command-pin"), now));
        assert_eq!(selected(&mut app), "command-pin");
        assert!(!app.pointer_hover(Some("command-pin"), now));
        assert!(!app.pointer_hover(Some("sidebar-settings"), now));
        let (_, scene) = rust_native_desktop::capture(&mut app, 760.0, 540.0, 1.0);
        let pin = scene.bounds["command-pin"];
        let selected_fills = scene
            .ops
            .iter()
            .filter(|op| {
                matches!(op,
            rust_native_desktop::layout::Op::Fill { rect, color, .. }
            if *rect == pin && *color == openagents_chat_app::visual::SELECTED)
            })
            .count();
        assert_eq!(selected_fills, 1);
        key(&mut app, now, "Escape", false, false);
        assert!(!app.pointer_hover(Some("command-pin"), now));
    }
    #[test]
    fn menu_rows_stay_visible_through_text_ime_and_idle_ticks() {
        for scale in [1.0, 2.0] {
            let (mut app, now) = super::tests::chat_fixture(0);
            key(&mut app, now, "n", true, false);
            app.text_input(TextInput::Commit("Keep this draft  "), now);
            // The host's first answer names the account row; let it land
            // before the idle ticks this test watches.
            let now = now + std::time::Duration::from_secs(1);
            app.tick(now);
            app.activate(
                Intent::Chat {
                    action: ChatAction::Menu,
                },
                now,
            );
            let (before, scene) = rust_native_desktop::capture(&mut app, 760.0, 540.0, scale);
            let menu = scene.bounds["command-panel"];
            let revision = app.view().view().revision;
            let surfaces: Vec<_> = scene
                .ops
                .iter()
                .filter_map(|op| {
                    if let rust_native_desktop::layout::Op::Surface { resource, rect, .. } = op {
                        Some((
                            resource.clone(),
                            rect.w,
                            app.surface_version(resource),
                            app.surface_size(resource, rect.w),
                        ))
                    } else {
                        None
                    }
                })
                .collect();
            for event in [
                TextInput::Commit("missing menu item"),
                TextInput::Preedit {
                    text: "pin",
                    selection: Some((0, 3)),
                },
                TextInput::CancelComposition,
            ] {
                assert!(app.text_input(event, now));
            }
            for step in 1..=4 {
                app.tick(now + std::time::Duration::from_millis(step * 500));
                assert_eq!(app.view().view().revision, revision);
                for (resource, available, version, size) in &surfaces {
                    assert!(version.is_some(), "untracked surface {resource}");
                    assert_eq!(app.surface_version(resource), *version, "{resource}");
                    assert_eq!(app.surface_size(resource, *available), *size, "{resource}");
                }
                let (after, scene) = rust_native_desktop::capture(&mut app, 760.0, 540.0, scale);
                assert_eq!(scene.bounds["command-panel"], menu);
                for key in ["command-rename", "command-pin", "command-archive"] {
                    assert!(scene.hits.iter().any(|hit| hit.key == key && hit.enabled));
                }
                for y in (menu.y * scale) as usize..((menu.y + menu.h) * scale) as usize {
                    let start = (y * before.width + (menu.x * scale) as usize) * 4;
                    let end = (y * before.width + ((menu.x + menu.w) * scale) as usize) * 4;
                    assert_eq!(&after.pixels[start..end], &before.pixels[start..end]);
                }
            }
            key(&mut app, now, "ArrowDown", false, false);
            key(&mut app, now, "Escape", false, false);
            assert!(!app.chat.as_ref().unwrap().modal());
            assert_eq!(app.chat.as_ref().unwrap().draft(), "Keep this draft  ");
        }
    }
    #[test]
    fn floating_menu_repaints_match_complete_frames() {
        use rust_native_desktop::{layout, paint, text::Fonts};
        for scale in [1.0, 2.0] {
            let (mut app, now) = super::tests::chat_fixture(0);
            let mut fonts = Fonts::new();
            let mut retained = paint::Retained::default();
            app.viewport(760.0, 540.0, scale);
            for step in 0..28 {
                match step {
                    1 | 14 => key(&mut app, now, "k", true, false),
                    3 | 5 | 15 => {
                        key(&mut app, now, "a", true, false);
                        app.text_input(TextInput::Commit(if step == 3 { "chat" } else { "" }), now);
                    }
                    6 | 12 | 17 => key(&mut app, now, "Escape", false, false),
                    7 | 18 => app.activate(
                        Intent::Chat {
                            action: ChatAction::Menu,
                        },
                        now,
                    ),
                    9 | 10 | 19 => key(&mut app, now, "ArrowDown", false, false),
                    21 => app.activate(
                        Intent::Chat {
                            action: ChatAction::Profile,
                        },
                        now,
                    ),
                    22 => {
                        app.text_input(TextInput::Commit("ignored profile text"), now);
                    }
                    23 => key(&mut app, now, "Escape", false, false),
                    24 => app.activate(
                        Intent::Chat {
                            action: ChatAction::Rename,
                        },
                        now,
                    ),
                    25 => {
                        key(&mut app, now, "a", true, false);
                        app.text_input(TextInput::Commit("A renamed conversation"), now);
                    }
                    27 => key(&mut app, now, "Escape", false, false),
                    _ => {}
                }
                let interaction = layout::Interaction {
                    hover: (step % 2 == 0).then(|| "command-pin".into()),
                    ..Default::default()
                };
                let mut scene = layout::lay_out_with_overlay(
                    app.view().view(),
                    &app.theme(),
                    &mut fonts,
                    &|resource, available| app.surface_size(resource, available),
                    &interaction,
                    760.0,
                    540.0,
                    app.window_layout(),
                    app.overlay_layout(),
                );
                for op in &mut scene.ops {
                    if let layout::Op::Surface {
                        resource, version, ..
                    } = op
                    {
                        *version = app.surface_version(resource);
                    }
                }
                let size = ((760.0 * scale) as usize, (540.0 * scale) as usize);
                let damage = retained.update(
                    &scene,
                    size,
                    scale,
                    0.0,
                    None,
                    &mut fonts,
                    &mut |resource, frame, rect| app.paint_surface(resource, frame, rect),
                );
                if matches!(step, 3 | 5) {
                    let pixels = damage.iter().map(|r| r.w * r.h).sum::<f32>();
                    assert!(
                        pixels < (size.0 * size.1) as f32 * 0.7,
                        "filtering repainted the panes at step {step}, scale {scale}: {pixels} pixels"
                    );
                }
                let mut complete = Frame::transparent(size.0, size.1);
                paint::paint(
                    &scene,
                    &mut complete,
                    scale,
                    0.0,
                    &mut fonts,
                    &mut |resource, frame, rect| app.paint_surface(resource, frame, rect),
                );
                assert_eq!(
                    retained.frame().unwrap().pixels,
                    complete.pixels,
                    "menu step {step} at scale {scale}"
                );
            }
        }
    }

    #[test]
    fn floating_controls_preserve_the_reader_and_composer_geometry() {
        let now = Instant::now();
        let (mut app, _) = DesktopApp::performance_fixture(100, 1, now);
        for (width, height) in [(1200.0, 840.0), (760.0, 540.0)] {
            let (_, before) = rust_native_desktop::capture(&mut app, width, height, 1.0);
            assert!(!before.bounds.contains_key("chat-key-hint"));
            app.performance_scroll(200.0, now);
            let (_, reading) = rust_native_desktop::capture(&mut app, width, height, 1.0);
            let pill = reading.bounds["chat-latest-pill"];
            let composer = reading.bounds["chat-composer-card"];
            assert_eq!(pill.h, 30.0);
            assert!(reading.ops.iter().any(|op| matches!(op, rust_native_desktop::layout::Op::Text {paragraph,color,..} if paragraph.text == "↓" && *color == openagents_chat_app::visual::MUTED && paragraph.font.size == 13.0)));
            assert!(reading.ops.iter().any(|op| matches!(op, rust_native_desktop::layout::Op::Text {paragraph,..} if paragraph.text == "Scroll to bottom" && paragraph.font.weight == rust_native::layout::display::Weight::Regular)));
            assert!((pill.y + pill.h + 6.0 - composer.y).abs() < 0.01);
            assert!((pill.x + pill.w / 2.0 - composer.x - composer.w / 2.0).abs() < 0.01);
            assert_eq!(
                before.bounds["chat-transcript"],
                reading.bounds["chat-transcript"]
            );
            capture(&mut app, &format!("latest-{width}"), width, height);
            key(&mut app, now, "k", true, false);
            let (_, palette) = rust_native_desktop::capture(&mut app, width, height, 1.0);
            assert!(palette.ops.iter().any(|op| matches!(op, rust_native_desktop::layout::Op::Surface {resource,..} if resource == "glyph:command-search")));
            assert!(palette.ops.iter().any(|op| matches!(op, rust_native_desktop::layout::Op::Text {paragraph,..} if paragraph.text == "New chat" && paragraph.font.size == 14.0 && paragraph.font.weight == rust_native::layout::display::Weight::Regular)));
            let shortcut = if cfg!(target_os = "macos") {
                "⌘N"
            } else {
                "Ctrl+N"
            };
            assert!(palette.ops.iter().any(|op| matches!(op, rust_native_desktop::layout::Op::Text {paragraph,..} if paragraph.text == shortcut && paragraph.font.mono && paragraph.font.size == 10.0)));
            let card = palette.bounds["command-panel"];
            assert_eq!(card.w, 560.0);
            let navigation = palette.bounds["command-navigation-cap"];
            assert_eq!(navigation.h, 16.0);
            assert_eq!(navigation.y, palette.bounds["command-selection-cap"].y);
            assert_eq!(navigation.y, palette.bounds["command-close-cap"].y);
            assert!(palette.ops.iter().any(|op| matches!(op, rust_native_desktop::layout::Op::Text {paragraph,..} if paragraph.text == "Navigate" && paragraph.lines.len() == 1)));
            assert!((card.x + card.w / 2.0 - width / 2.0).abs() < 0.01);
            assert_eq!(palette.bounds["chat-composer-card"], composer);
            assert_eq!(
                palette.bounds["chat-transcript"],
                reading.bounds["chat-transcript"]
            );
            key(&mut app, now, "Escape", false, false);
            app.activate(
                Intent::Chat {
                    action: ChatAction::Menu,
                },
                now,
            );
            let (_, menu) = rust_native_desktop::capture(&mut app, width, height, 1.0);
            assert_eq!(menu.bounds["command-panel"].w, 216.0);
            assert_eq!(menu.bounds["command-panel"].y, 40.0);
            assert_eq!(menu.bounds["chat-composer-card"], composer);
            assert!(menu.hits.iter().any(|hit| hit.key == "command-archive"));
            assert!(app.context_menu_at(None, (width - 10.0, height - 10.0), now));
            let (_, edge_menu) = rust_native_desktop::capture(&mut app, width, height, 1.0);
            let edge = edge_menu.bounds["command-panel"];
            assert_eq!(edge.x + edge.w, width - 8.0);
            assert_eq!(edge.y + edge.h, height - 8.0);
            key(&mut app, now, "Escape", false, false);
            app.activate(
                Intent::Chat {
                    action: ChatAction::Latest,
                },
                now,
            );
            let (_, latest) = rust_native_desktop::capture(&mut app, width, height, 1.0);
            assert!(!latest.bounds.contains_key("chat-latest-pill"));
        }
    }

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
                let rect = scene
                    .hits
                    .iter()
                    .find(|hit| hit.key == "chat-send")
                    .unwrap()
                    .rect;
                assert!(rect.y >= card.y && rect.y + rect.h <= card.y + card.h);
                for key in ["chat-attach", "chat-paste-image"] {
                    assert!(!scene.hits.iter().any(|hit| hit.key == key), "{key}");
                }
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
                control: command && key == "Tab",
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
        // With fewer than five chats Cmd/Ctrl+F opens the palette (#10072).
        key(&mut app, now, "f", true, false);
        assert!(app.chat.as_ref().unwrap().modal());
        key_escape(&mut app, now);
        // Enough chats for the sidebar's filter (#10072).
        for _ in 0..4 {
            let request = app.chat.as_mut().unwrap().new_chat();
            app.send(vec![request], now);
        }
        app.present();
        app.activate(
            Intent::Navigate {
                action: chrome::Action::SelectChat { id: first },
            },
            now,
        );
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
    #[cfg_attr(
        windows,
        ignore = "coder-history reads saved sessions on Linux and macOS only"
    )]
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
        // Rows show the session's timestamp, which follows the fixture
        // files' write time; the calendar date depends on when this runs.
        assert!(
            words
                .iter()
                .any(|word| word.starts_with("Codex · Scratch Codex\n20") && word.ends_with('Z')),
            "{words:?}"
        );
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

/// The Coder events of a run on this computer, as the desktop chat draws
/// them (#10033): every event type the scripted provider emits
/// (`microcoder`'s `local_run` tests write the fixtures), outlined under
/// `snapshots/dsk-10-coder-*.txt`.
#[cfg(test)]
#[path = "late_click_tests.rs"]
mod late_click_tests;
#[cfg(test)]
#[path = "native_tests.rs"]
mod native_tests;

/// The slide viewer over the page (#10057).
#[cfg(test)]
#[path = "slides_shell_tests.rs"]
mod slides_shell_tests;

/// Screen readers' view of the chat window (#10024).
#[cfg(test)]
#[path = "access_tests.rs"]
mod access_tests;

/// The Map page in the window (#10085).
#[cfg(test)]
#[path = "route_map_shell_tests.rs"]
mod route_map_shell_tests;

#[cfg(test)]
mod coder_events {
    use super::*;
    use openagents_chat::{
        basic_chats::Spawned,
        basic_coder::Turn,
        coder_events::Line,
        service::{Command, Snapshot},
    };
    use openagents_chat_app::coder_run::{Answer, Request as RunRequest, State as RunState};
    use rust_native::{Element, Node};

    const QUESTION_THEN_RESULT: &str =
        include_str!("../../openagents-chat/fixtures/coder-events/question-then-result.ndjson");
    const OTHER_ENDINGS: &str =
        include_str!("../../openagents-chat/fixtures/coder-events/other-endings.ndjson");

    pub(super) fn tasks(text: &str) -> Vec<Vec<Line>> {
        let mut out: Vec<Vec<Line>> = vec![];
        for line in text.lines() {
            let line: Line = serde_json::from_str(line).unwrap();
            match out.last_mut() {
                Some(task) if task[0].task == line.task => task.push(line),
                _ => out.push(vec![line]),
            }
        }
        out
    }

    /// A thread bound to `lines`' task, after one poll that read them.
    pub(super) fn window(lines: &[Line], state: RunState) -> DesktopApp {
        let mut app = super::tests::chat_fixture(0).0;
        let now = Instant::now();
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
            total: 2,
            turns: vec![
                Turn::user("add a unit test for slugify"),
                Turn::assistant("Working on adding a unit test for slugify.", None),
            ],
            coder: Some(Spawned {
                host: "local".into(),
                task: lines[0].task.clone(),
                project: Some("slugs".into()),
                at: Some(1),
            }),
            ..Snapshot::default()
        };
        panel.outcome(ticket, Ok(snapshot.clone()));
        let mut polled = false;
        for step in 0..20 {
            match panel.tick(now + std::time::Duration::from_millis(step * 300)) {
                Some(Request::Chat { ticket, .. }) => panel.outcome(ticket, Ok(snapshot.clone())),
                Some(Request::CoderRun {
                    chat: at,
                    ticket,
                    request: RunRequest::Poll { task },
                }) => {
                    assert_eq!(
                        (at.as_str(), task.as_str()),
                        (chat.as_str(), lines[0].task.as_str())
                    );
                    panel.run_outcome(
                        chat.clone(),
                        ticket,
                        Ok(Answer::Lines {
                            lines: lines.to_vec(),
                            state,
                        }),
                    );
                    polled = true;
                    break;
                }
                _ => {}
            }
        }
        assert!(polled, "the bound thread follows its task");
        app.present();
        app
    }

    /// Each transcript row, one node a line, with what it says.
    fn outline(rows: &[std::sync::Arc<Node<()>>]) -> String {
        fn write(node: &Node<()>, depth: usize, out: &mut String) {
            let pad = "  ".repeat(depth);
            let line = match &node.element {
                Element::Text { value, role } => {
                    format!("{} {value:?}", format!("{role:?}").to_lowercase())
                }
                Element::Button { label, enabled, .. } => {
                    format!(
                        "button {label:?}{}",
                        if *enabled { "" } else { " (disabled)" }
                    )
                }
                Element::Message { role, .. } => format!("message {role:?}").to_lowercase(),
                Element::Markdown { blocks } => {
                    format!("markdown {:?}", rust_native::markdown::plain(blocks))
                }
                Element::Tool {
                    name,
                    detail,
                    state,
                    ..
                } => format!("tool {name:?} {detail:?} [{state:?}]")
                    .replace("[D", "[d")
                    .replace("[R", "[r")
                    .replace("[F", "[f"),
                Element::Working { label } => format!("working {label:?}"),
                Element::Stack { axis, .. } => {
                    let card = node.style.background.is_some_and(|c| c.alpha > 0);
                    format!("{axis:?} stack{}", if card { " [card]" } else { "" }).to_lowercase()
                }
                other => format!("{other:?}").chars().take(40).collect(),
            };
            out.push_str(&format!("{pad}{line}\n"));
            if let Element::Stack { children, .. }
            | Element::Message { children, .. }
            | Element::Tool { children, .. } = &node.element
            {
                for child in children {
                    write(child, depth + 1, out);
                }
            }
        }
        let mut out = String::new();
        for row in rows {
            write(row, 0, &mut out);
        }
        out
    }

    fn check_snapshot(name: &str, actual: &str) {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("snapshots")
            .join(format!("{name}.txt"));
        if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
            std::fs::write(&path, actual).unwrap();
            return;
        }
        let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
            panic!("no snapshot {name}; run with UPDATE_SNAPSHOTS=1. The view is:\n{actual}")
        });
        assert_eq!(
            expected, actual,
            "the {name} snapshot differs; run with UPDATE_SNAPSHOTS=1 to record it"
        );
    }

    /// Each case: its snapshot name, the lines read, and where the task is.
    fn cases() -> Vec<(&'static str, Vec<Line>, RunState)> {
        let whole = tasks(QUESTION_THEN_RESULT).remove(0);
        let others = tasks(OTHER_ENDINGS);
        let asked = whole
            .iter()
            .position(|line| line.event.name() == "question")
            .unwrap();
        let command = whole
            .iter()
            .position(|line| line.event.name() == "output")
            .unwrap();
        vec![
            (
                "dsk-10-coder-running",
                whole[..command].to_vec(),
                RunState::Running,
            ),
            (
                "dsk-10-coder-question",
                whole[..=asked].to_vec(),
                RunState::Waiting,
            ),
            ("dsk-10-coder-result", whole.clone(), RunState::Ended),
            (
                "dsk-10-coder-approval",
                others[0].clone(),
                RunState::Waiting,
            ),
            (
                "dsk-10-coder-no-capacity",
                others[1].clone(),
                RunState::Ended,
            ),
            ("dsk-10-coder-stopped", others[2].clone(), RunState::Ended),
        ]
    }

    #[test]
    fn every_event_type_renders_in_the_desktop_transcript() {
        let directory =
            std::env::var_os("OPENAGENTS_CODER_CAPTURE_DIR").map(std::path::PathBuf::from);
        let mut drawn = std::collections::BTreeSet::new();
        for (name, lines, state) in cases() {
            let mut app = window(&lines, state);
            let (frame, _) = rust_native_desktop::capture(&mut app, 1200.0, 840.0, 1.0);
            let text = outline(app.chat.as_ref().unwrap().transcript_rows());
            check_snapshot(name, &text);
            // Every event the case read shows: its row names it.
            for line in &lines {
                let key = format!("coder-{}", line.seq);
                let rows = app.chat.as_ref().unwrap().transcript_rows();
                // A waiting question or approval is the decision panel,
                // drawn in place of its card (#10469).
                let asking = matches!(line.event.name(), "question" | "approval")
                    && rows.iter().any(|row| row.key == "coder-decision");
                let shown = text.contains(&key) || rows.iter().any(|row| row.key == key) || asking;
                if shown {
                    drawn.insert(line.event.name());
                }
            }
            if let Some(directory) = &directory {
                std::fs::create_dir_all(directory).unwrap();
                std::fs::write(directory.join(format!("{name}.png")), frame.png().unwrap())
                    .unwrap();
            }
        }
        // Progress shows as the working line; a reply as the message; an
        // output inside its command's row; a waiting question or approval
        // as the decision panel. The rest are rows of their own.
        for name in [
            "coder_started",
            "step",
            "provider_switched",
            "question",
            "approval",
            "result",
            "failure",
            "stopped",
        ] {
            assert!(drawn.contains(name), "{name} draws no row: {drawn:?}");
        }
    }

    /// A real Grok Build run's tool calls (#10117): condensed, the looking
    /// calls are one row labelled by verb and the command one row; a click
    /// on the group opens it to each call and what it returned. Captures
    /// `dsk-11-coder-tools-condensed.png` and
    /// `dsk-11-coder-tools-expanded.png` under
    /// `OPENAGENTS_CODER_CAPTURE_DIR`.
    #[test]
    fn tool_calls_show_grouped_and_a_click_opens_a_group() {
        use openagents_desktop::chat::TRANSCRIPT;
        use rust_native_desktop::input::SurfaceInput;
        let lines = tasks(include_str!(
            "../../openagents-chat/fixtures/coder-events/tools-grok.ndjson"
        ))
        .remove(0);
        let directory =
            std::env::var_os("OPENAGENTS_CODER_CAPTURE_DIR").map(std::path::PathBuf::from);
        let mut app = window(&lines, RunState::Ended);
        let (frame, _) = rust_native_desktop::capture(&mut app, 1200.0, 840.0, 1.0);
        let text = outline(app.chat.as_ref().unwrap().transcript_rows());
        check_snapshot("dsk-11-coder-tools", &text);
        assert!(
            text.contains("tool \"Read 3 files, Listed 1 dir, Searched 2 patterns\" \"\" [done]")
        );
        assert!(text.contains("tool \"Run\" \"Build crate and show git history\" [done]"));
        let write = |name: &str, frame: &rust_native_desktop::Frame| {
            if let Some(directory) = &directory {
                std::fs::create_dir_all(directory).unwrap();
                std::fs::write(directory.join(format!("{name}.png")), frame.png().unwrap())
                    .unwrap();
            }
        };
        write("dsk-11-coder-tools-condensed", &frame);
        let group = format!("coder-{}", lines[4].seq);
        let now = Instant::now();
        let panel = app.chat.as_mut().unwrap();
        let before = panel.transcript.height();
        let bounds = panel
            .transcript
            .toggle_bounds(&group)
            .expect("the group opens with a click");
        let (x, y) = (bounds.x + 40.0, bounds.y + bounds.h / 2.0);
        assert!(panel.surface(TRANSCRIPT, SurfaceInput::Down { x, y, shift: false }, now));
        panel.surface(TRANSCRIPT, SurfaceInput::Up { x, y }, now);
        assert!(
            panel.transcript.toggle_bounds(&group).is_some(),
            "the group stays on screen"
        );
        assert!(
            panel.transcript.height() > before,
            "the open group shows its calls"
        );
        app.present();
        let (frame, _) = rust_native_desktop::capture(&mut app, 1200.0, 840.0, 1.0);
        write("dsk-11-coder-tools-expanded", &frame);
    }

    /// A finished run's worktree diff opens in the "What changed" pane
    /// (#10019).
    #[test]
    fn a_finished_run_shows_what_changed() {
        let whole = tasks(QUESTION_THEN_RESULT).remove(0);
        let mut app = window(&whole, RunState::Ended);
        let now = Instant::now();
        let panel = app.chat.as_mut().unwrap();
        let mut asked = false;
        for step in 0..20 {
            if let Some(Request::CoderRun {
                chat,
                ticket,
                request: RunRequest::Review { task },
            }) = panel.tick(now + std::time::Duration::from_millis(step * 10))
            {
                assert_eq!(task, whole[0].task);
                let diff = "diff --git a/test_slugs.py b/test_slugs.py\n--- /dev/null\n+++ b/test_slugs.py\n@@ -0,0 +1,2 @@\n+import unittest\n+x = 1\n";
                panel.run_outcome(
                    chat,
                    ticket,
                    Ok(Answer::Review(Box::new(coder_access::review::TaskReview {
                        task: task.clone(),
                        base: "1".repeat(40),
                        head_commit: "1".repeat(40),
                        head: "2".repeat(40),
                        files: vec![coder_access::review::FileCount {
                            path: "test_slugs.py".into(),
                            status: coder_access::review::FileStatus::Added,
                            added: Some(2),
                            removed: Some(0),
                        }],
                        files_total: 1,
                        added: 2,
                        removed: 0,
                        uncounted: 0,
                        diff: diff.into(),
                        completeness: coder_access::review::Completeness::Complete,
                        publication: None,
                    }))),
                );
                asked = true;
                break;
            }
        }
        assert!(asked, "a finished run reads its diff");
        fn has(node: &Node<openagents_desktop::model::Intent>, key: &str) -> bool {
            node.key == key
                || match &node.element {
                    Element::Stack { children, .. } => children.iter().any(|c| has(c, key)),
                    _ => false,
                }
        }
        let body = panel.body();
        assert!(has(&body, "changes-card"), "the What changed card shows");
    }

    fn change(task: &str, head: char, diff: &str) -> coder_access::review::TaskReview {
        coder_access::review::TaskReview {
            task: task.into(),
            base: "1a2b3c4d5e".repeat(4),
            head_commit: "1a2b3c4d5e".repeat(4),
            head: head.to_string().repeat(40),
            files: vec![
                coder_access::review::FileCount {
                    path: "src/slug.rs".into(),
                    status: coder_access::review::FileStatus::Modified,
                    added: Some(3),
                    removed: Some(1),
                },
                coder_access::review::FileCount {
                    path: "test_slugs.py".into(),
                    status: coder_access::review::FileStatus::Added,
                    added: Some(2),
                    removed: Some(0),
                },
            ],
            files_total: 2,
            added: 5,
            removed: 1,
            uncounted: 0,
            diff: diff.into(),
            completeness: coder_access::review::Completeness::Complete,
            publication: None,
        }
    }

    const TWO_FILES: &str = "diff --git a/src/slug.rs b/src/slug.rs\n--- a/src/slug.rs\n+++ b/src/slug.rs\n@@ -1,2 +1,4 @@\n-pub fn slug(s: &str) -> String { s.into() }\n+pub fn slug(s: &str) -> String {\n+    s.trim().to_lowercase().replace(' ', \"-\")\n+}\n fn keep() {}\ndiff --git a/test_slugs.py b/test_slugs.py\n--- /dev/null\n+++ b/test_slugs.py\n@@ -0,0 +1,2 @@\n+import unittest\n+x = 1\n";

    /// A finished run's change shows its exact revisions; a moved worktree
    /// makes the view stale and holds back Publish until a refresh; the
    /// refreshed head publishes once and the card links the pull request
    /// (#10067, #10068). With `OPENAGENTS_CHANGES_CAPTURE_DIR` set, each
    /// state is captured.
    #[test]
    fn a_reviewed_run_goes_stale_refreshes_and_publishes_once() {
        let whole = tasks(QUESTION_THEN_RESULT).remove(0);
        let task = whole[0].task.clone();
        let mut app = window(&whole, RunState::Ended);
        let captures =
            std::env::var_os("OPENAGENTS_CHANGES_CAPTURE_DIR").map(std::path::PathBuf::from);
        let capture = |app: &mut DesktopApp, name: &str| {
            app.present();
            let (frame, _) = rust_native_desktop::capture(app, 1400.0, 900.0, 2.0);
            if let Some(directory) = &captures {
                std::fs::create_dir_all(directory).unwrap();
                std::fs::write(directory.join(format!("{name}.png")), frame.png().unwrap())
                    .unwrap();
            }
        };
        let start = Instant::now();
        let next = |app: &mut DesktopApp, at: Instant| -> Option<(String, u64, RunRequest)> {
            let panel = app.chat.as_mut().unwrap();
            for step in 0..40 {
                match panel.tick(at + std::time::Duration::from_millis(step * 10)) {
                    Some(Request::CoderRun {
                        chat,
                        ticket,
                        request: request @ (RunRequest::Review { .. } | RunRequest::Publish { .. }),
                    }) => return Some((chat, ticket, request)),
                    Some(Request::CoderRun { chat, ticket, .. }) => {
                        panel.run_outcome(
                            chat,
                            ticket,
                            Ok(Answer::Lines {
                                lines: vec![],
                                state: RunState::Ended,
                            }),
                        );
                    }
                    _ => {}
                }
            }
            None
        };
        fn keys(node: &Node<openagents_desktop::model::Intent>, out: &mut Vec<String>) {
            out.push(node.key.clone());
            if let Element::Stack { children, .. } = &node.element {
                for child in children {
                    keys(child, out);
                }
            }
        }
        let shown = |app: &mut DesktopApp| {
            let mut out = vec![];
            keys(&app.chat.as_mut().unwrap().body(), &mut out);
            out
        };
        let click = |app: &mut DesktopApp, key: &str| {
            app.present();
            let view = app.view().clone();
            app.chat.as_mut().unwrap().action(
                openagents_desktop::chat_action::Action::Card { key: key.into() },
                &view,
                Instant::now(),
            )
        };
        // The first read names the base and the head.
        let (chat, ticket, request) = next(&mut app, start).expect("a review");
        assert_eq!(request, RunRequest::Review { task: task.clone() });
        app.chat.as_mut().unwrap().run_outcome(
            chat,
            ticket,
            Ok(Answer::Review(Box::new(change(&task, '2', TWO_FILES)))),
        );
        let card = shown(&mut app);
        assert!(
            card.iter().any(|key| key == "changes-revisions"),
            "{card:?}"
        );
        assert!(card.iter().any(|key| key == "changes-publish"));
        capture(&mut app, "card");
        assert!(click(&mut app, "changes-open").is_none());
        app.present();
        let mut all = vec![];
        keys(&app.view().view().root, &mut all);
        assert!(all.iter().any(|key| key == "changes-pane"), "{all:?}");
        capture(&mut app, "pane");
        assert!(click(&mut app, "changes-close").is_none());
        // The worktree moves: the next read names another head.
        let later = start + openagents_chat_app::changes::CHECK_EVERY * 2;
        let (chat, ticket, request) = next(&mut app, later).expect("a check");
        assert_eq!(request, RunRequest::Review { task: task.clone() });
        app.chat.as_mut().unwrap().run_outcome(
            chat,
            ticket,
            Ok(Answer::Review(Box::new(change(&task, '3', TWO_FILES)))),
        );
        let stale = shown(&mut app);
        assert!(
            stale.iter().any(|key| key == "changes-note-stale"),
            "{stale:?}"
        );
        assert!(stale.iter().any(|key| key == "changes-refresh"));
        assert!(!stale.iter().any(|key| key == "changes-publish"));
        capture(&mut app, "stale");
        // A publish of the stale view is refused before anything is asked.
        assert!(click(&mut app, "changes-publish").is_none());
        if let Some((chat, ticket, request)) = next(&mut app, later) {
            assert!(matches!(request, RunRequest::Review { .. }), "{request:?}");
            app.chat.as_mut().unwrap().run_outcome(
                chat,
                ticket,
                Ok(Answer::Review(Box::new(change(&task, '3', TWO_FILES)))),
            );
        }
        assert!(click(&mut app, "changes-refresh").is_none());
        assert!(click(&mut app, "changes-publish").is_none());
        let (chat, ticket, request) = next(&mut app, later).expect("a publish");
        let RunRequest::Publish {
            task: published,
            base,
            head_commit,
            head,
        } = request
        else {
            panic!("publish");
        };
        assert_eq!(
            (published.as_str(), head.as_str()),
            (task.as_str(), "3".repeat(40).as_str())
        );
        capture(&mut app, "publishing");
        app.chat.as_mut().unwrap().run_outcome(
            chat,
            ticket,
            Ok(Answer::Published(Box::new(
                coder_access::review::Publication {
                    operation: "9".repeat(64),
                    task: task.clone(),
                    base,
                    head_commit,
                    head,
                    landing: coder_access::review::Landing::DraftPullRequest,
                    state: coder_access::review::PublishState::Published,
                    branch: Some(format!("coder/review-{}-99999999", &task[..8])),
                    commit: Some("4".repeat(40)),
                    url: Some("https://github.com/example/scratch/pull/7".into()),
                    note: "Pushed 4444444444 and opened a draft pull request.".into(),
                },
            ))),
        );
        let published = shown(&mut app);
        assert!(
            published.iter().any(|key| key == "changes-link"),
            "{published:?}"
        );
        assert!(!published.iter().any(|key| key == "changes-publish"));
        capture(&mut app, "published");
        // A cut diff says so on the card.
        let mut cut = change(&task, '5', TWO_FILES);
        cut.diff
            .truncate(TWO_FILES.find("diff --git a/test").unwrap());
        cut.completeness = coder_access::review::Completeness::Truncated {
            shown: cut.diff.len() as u64,
            total: Some(TWO_FILES.len() as u64 + 400_000),
        };
        app.chat.as_mut().unwrap().bind_changes(cut);
        assert!(click(&mut app, "changes-open").is_none());
        let truncated = shown(&mut app);
        assert!(
            truncated
                .iter()
                .any(|key| key == "changes-pane-note-truncated"),
            "{truncated:?}"
        );
        capture(&mut app, "truncated");
    }

    #[test]
    fn a_run_s_buttons_can_be_pressed() {
        let others = tasks(OTHER_ENDINGS);
        let mut app = window(&others[0], RunState::Waiting);
        rust_native_desktop::capture(&mut app, 1200.0, 840.0, 1.0);
        let panel = app.chat.as_ref().unwrap();
        for key in ["coder-approve", "coder-deny"] {
            assert!(panel.transcript.control_bounds(key).is_some(), "{key}");
        }
    }

    #[test]
    fn output_and_progress_render_inside_their_rows() {
        let whole = tasks(QUESTION_THEN_RESULT).remove(0);
        let first_output = whole
            .iter()
            .position(|line| line.event.name() == "output")
            .unwrap();
        let app = window(&whole[..=first_output], RunState::Running);
        let text = outline(app.chat.as_ref().unwrap().transcript_rows());
        assert!(
            text.contains("tool \"Run\" \"printf 'import unittest\\\\n' > test_slugs.py\" [done]"),
            "{text}"
        );
        assert!(text.contains("working \"Working · step 1 · 0s\""), "{text}");
    }
}

/// Gym and eval cards through the desktop chat (#10020).
#[cfg(test)]
#[path = "gym_card_tests.rs"]
mod gym_card_tests;

/// Give feedback on selected text (#10127).
#[cfg(test)]
#[path = "feedback_shell_tests.rs"]
mod feedback_shell_tests;
