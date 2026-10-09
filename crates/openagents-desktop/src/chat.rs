//! Desktop chat presentation and local drafts over host-owned conversation state.
mod access;

use crate::chat_action::Action;
use crate::chrome::{Chat, Section, State};
use crate::control::ControlResult;
use crate::model::{Intent, Request};
use openagents_chat::basic_coder::Role;
use openagents_chat::service::{Command, Snapshot};
use openagents_chat_app::attention;
use openagents_chat_app::coder_run::{self, Action as RunAction, Run};
use openagents_chat_app::command_panel;
use openagents_chat_app::projection::{Appearance, Projection, Reply};
use openagents_chat_app::session::Session;
use openagents_chat_app::task_chat::{self, Action as TaskAction};
use rust_native::style::{Color, Space, Style, TextWeight};
use rust_native::{Axis, Element, Glyph, Icon, Node, TextRole, ValidatedView};
use rust_native_desktop::composer::{
    Submission,
    field::{Action as FieldAction, Field},
};
use rust_native_desktop::{
    Frame, PxRect,
    input::{SurfaceInput, TextInput},
    text::Fonts,
    transcript::Transcript,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

pub const TRANSCRIPT: &str = "chat-transcript";
pub const COMPOSER: &str = "composer:chat-composer";
/// The read-only change pane. The resource carries no file bytes.
pub const CHANGES: &str = "changes-lines";
const CHANGES_LINE: f32 = 18.0;
/// The most dropped files waiting to be read; a larger drop keeps these.
const MAX_PENDING_DROPS: usize = 16;

pub const COMMAND_QUERY: &str = "composer:command-query";
pub const SEARCH: &str = "composer:chat-search";
pub const RENAME: &str = "composer:chat-rename";

pub struct Panel {
    commands: openagents_chat_app::commands::Overlay,
    command_rows: BTreeMap<String, usize>,
    /// Points of palette results scrolled above the viewport; hover never moves it.
    command_offset: f32,
    command_reveal: bool,
    /// The painted header and footer rules around the palette's results.
    command_band: (Option<PxRect>, Option<PxRect>),
    saved: openagents_chat_app::retained::Session,
    saved_visible: bool,
    saved_project: Option<String>,
    command_query: Field,
    command_token: String,
    /// The account menu's heading (`chrome::account_name`).
    account_name: String,
    menu_point: Option<(f32, f32)>,
    menu_navigation: bool,
    search: Field,
    rename: Option<(String, Field)>,
    rename_pending: Option<(u64, String, String)>,
    rename_focus: usize,
    /// Whether the screens still in development show (`crate::preview`).
    pub preview: bool,
    /// The selection the context menu offers **Give feedback** on (#10127):
    /// its text and the transcript row it starts in.
    feedback_offer: Option<(String, Option<String>)>,
    /// The open **Give feedback** dialog. It reuses the rename dialog's
    /// field, focus, and keys (`rename`), with its own words and Send.
    feedback: Option<FeedbackDialog>,
    feedback_sender: crate::feedback::Sender,
    aux_rect: Option<PxRect>,
    session: Session,
    ids: BTreeMap<String, u64>,
    fields: BTreeMap<String, Field>,
    submissions: BTreeMap<String, (String, Submission)>,
    tasks: BTreeMap<String, task_chat::Session>,
    task_submissions: BTreeMap<(String, String), Submission>,
    /// Coder runs on this computer, by chat ([`coder_run`]).
    runs: BTreeMap<String, Run>,
    /// Each chat's Coder ending the person has seen, by its mark
    /// ([`Run::mark`] or the task summary's sequence): a seen ending no
    /// longer asks for attention ([`attention`]).
    seen: BTreeMap<String, u64>,
    run_submissions: BTreeMap<(String, u64), Submission>,
    /// Messages sent from this window, by chat and request: a coding
    /// reply to one starts Coder here at once, as `openagents chat` does.
    sent: std::collections::BTreeSet<(String, String)>,
    /// The folders a new run tries first: this computer's projects, the
    /// one shown on Phones and computers first.
    coder_projects: Vec<String>,
    /// Whether a coding reply waits for **Run Coder** instead of starting
    /// Coder at once: the settings' `coder.start` (#10036), read when a
    /// reply is judged coding work.
    coder_asks_first: Box<dyn Fn() -> bool>,
    task_editor: BTreeMap<String, u64>,
    born: Instant,
    pub viewport: (f32, f32, f32),
    pub column_width: f32,
    pub transcript: Transcript,
    fonts: Fonts,
    transcript_rows: Vec<Arc<Node<()>>>,
    /// The latest reply's follow-up suggestions, by card key and label:
    /// chips above the composer, as the phone shows them (#10075).
    followups: Vec<(String, String)>,
    /// An empty chat's starter suggestions (the phone's shared list,
    /// `first_run::SUGGESTIONS`), by card key and label: chips above the
    /// centered composer, in the follow-ups' row and style (#10097).
    starters: Vec<(String, String)>,
    projection: Projection,
    transcript_size: (f32, f32),
    composer_rect: Option<PxRect>,
    rows_dirty: bool,
    activated: Vec<String>,
    press_revision: Option<(u64, u64)>,
    /// The transcript button a press began on, for a release on the next
    /// revision ([`rust_native::Press`]).
    press: Option<rust_native::Press<Pressed>>,
    /// The transcript revision shown: bumped when its rows, the session,
    /// or the task change.
    shown: u64,
    shown_state: (u64, u64, u64),
    rows_generation: u64,
    queued: Vec<Request>,
    navigation: Option<openagents_chat::router::Screen>,
    desktop_navigation: Option<crate::chrome::Action>,
    /// The deck a reply's typed `open_presentation` offer names, for the
    /// shell's slide viewer.
    presentation: Option<String>,
    /// A reply's typed `open_screen` offer for `routes.map`, for the
    /// shell's Map page (#10102).
    map: bool,
    sidebar_width: f32,
    notice: Option<String>,
    waker: Option<rust_native_desktop::Waker>,
    image_input: Option<(
        String,
        rust_native_desktop::composer::Stamp,
        std::sync::mpsc::Receiver<crate::chat_images::Result>,
    )>,
    /// Dropped files waiting for the one being read ([`MAX_PENDING_DROPS`]).
    pending_drops: std::collections::VecDeque<std::path::PathBuf>,
    /// Whether the composer takes images: the phone's switch,
    /// [`openagents_chat_app::coder_tab::ATTACHMENTS_ENABLED`], so both
    /// apps turn attachments on and off together (#10095).
    attachments: bool,
    /// A change bound by hand, as a fixture does; otherwise the selected
    /// task's or run's reviewer shows.
    changes_bound: Option<openagents_chat_app::changes::Reviewer>,
    changes_open: bool,
    changes_scroll: f32,
    changes_highlighter: Option<rust_native::syntax::Highlighter>,
    /// The Gym's trainer key, read when a hosted run first needs it
    /// (`chat_gym`, #10096).
    #[cfg(not(windows))]
    gym_trainer: Option<crate::chat_gym::Trainer>,
}

/// **Give feedback**'s dialog: the selection it quotes and how its Send
/// went.
struct FeedbackDialog {
    selection: playtest::report::Selection,
    /// `Sent`, `Saved…`, or why it wasn't sent.
    status: Option<String>,
    sending: Option<std::sync::mpsc::Receiver<Result<String, String>>>,
    sent: bool,
}

/// A transcript button's action and target when a press began on it.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Pressed {
    Task(String, task_chat::Target),
    Run(String, coder_run::Target),
    Other(String),
}

impl Pressed {
    /// The controls that must work while Coder's events stream.
    fn late(&self) -> bool {
        match self {
            Self::Task(_, target) => target.action.late(),
            Self::Run(_, target) => target.action.late(),
            Self::Other(_) => false,
        }
    }
}

impl Panel {
    pub fn new(now: Instant) -> Self {
        let transcript = chat_transcript();
        Self {
            commands: openagents_chat_app::commands::Overlay::default(),
            command_rows: BTreeMap::new(),
            command_offset: 0.0,
            command_reveal: false,
            command_band: (None, None),
            saved: openagents_chat_app::retained::Session::default(),
            saved_visible: false,
            saved_project: None,
            command_query: chat_field("Search commands and chats…"),
            command_token: String::new(),
            account_name: "This computer".into(),
            menu_point: None,
            menu_navigation: false,
            search: search_field(),
            rename: None,
            rename_pending: None,
            rename_focus: 0,
            preview: crate::preview::ON,
            feedback_offer: None,
            feedback: None,
            feedback_sender: if cfg!(test) {
                std::sync::Arc::new(|_| Ok(playtest::feedback::SENT.into()))
            } else {
                crate::feedback::live()
            },
            aux_rect: None,
            session: Session::new(now),
            ids: BTreeMap::new(),
            fields: BTreeMap::new(),
            submissions: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_submissions: BTreeMap::new(),
            runs: BTreeMap::new(),
            seen: BTreeMap::new(),
            run_submissions: BTreeMap::new(),
            sent: Default::default(),
            coder_projects: vec![],
            coder_asks_first: if cfg!(test) {
                Box::new(|| false)
            } else {
                Box::new(coder_asks_first)
            },
            task_editor: BTreeMap::new(),
            born: now,
            viewport: (1200.0, 840.0, 1.0),
            column_width: 768.0,
            transcript,
            fonts: Fonts::new(),
            transcript_rows: vec![],
            followups: vec![],
            starters: vec![],
            projection: Projection::default(),
            transcript_size: (0.0, 0.0),
            composer_rect: None,
            rows_dirty: true,
            activated: vec![],
            press_revision: None,
            press: None,
            shown: 0,
            shown_state: (0, 0, 0),
            rows_generation: 0,
            queued: vec![],
            navigation: None,
            desktop_navigation: None,
            presentation: None,
            map: false,
            sidebar_width: crate::chrome::SIDEBAR_DEFAULT,
            notice: None,
            waker: None,
            image_input: None,
            pending_drops: std::collections::VecDeque::new(),
            attachments: openagents_chat_app::coder_tab::ATTACHMENTS_ENABLED,
            changes_bound: None,
            changes_open: false,
            changes_scroll: 0.0,
            changes_highlighter: None,
            #[cfg(not(windows))]
            gym_trainer: None,
        }
    }
    /// Where **Give feedback** sends its reports; tests record them.
    pub fn set_feedback_sender(&mut self, sender: crate::feedback::Sender) {
        self.feedback_sender = sender;
    }
    /// Whether the composer takes images: the shared
    /// [`openagents_chat_app::coder_tab::ATTACHMENTS_ENABLED`] unless
    /// [`Panel::set_attachments`] changed it. Off as of 2026-10-01 (#10095):
    /// the desktop is text only, as the phone is (#10093).
    pub fn attachments_enabled(&self) -> bool {
        self.attachments
    }

    /// Turn image attachments on or off for this window; the image
    /// pipeline's tests turn them on. Turning them off drops any images a
    /// draft holds.
    pub fn set_attachments(&mut self, on: bool) {
        self.attachments = on;
        self.text_only();
    }

    /// While attachments are off, drop any images a draft still holds, so
    /// a draft (a restored one included) is words only and its send or
    /// Coder start carries no images. Quiet: nothing to tell the person.
    fn text_only(&mut self) {
        if self.attachments {
            return;
        }
        if !self.session.images.is_empty() {
            self.session.images = openagents_chat_app::attachments::Drafts::default();
            self.rows_dirty = true;
        }
        self.pending_drops.clear();
    }

    fn import_image(&mut self, source: crate::chat_images::Source) {
        // Text only while attachments are off: no picker, no clipboard
        // image, no dropped image (#10095).
        if !self.attachments {
            return;
        }
        if self.image_input.is_some() {
            self.notice =
                Some("An image is already being imported. Try again when it finishes.".into());
            return;
        }
        let Some(chat) = self.session.selected.clone() else {
            return;
        };
        let Some(stamp) = self
            .fields
            .get(&chat)
            .and_then(|field| field.draft.stamp().ok())
        else {
            return;
        };
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        let wake = self.waker.clone();
        if std::thread::Builder::new()
            .name("chat-image-input".into())
            .spawn(move || {
                let result = crate::chat_images::read(source);
                let _ = send.send(result);
                if let Some(wake) = wake {
                    wake.wake();
                }
            })
            .is_ok()
        {
            self.image_input = Some((chat, stamp, receive));
        }
    }
    pub fn dropped_file(&mut self, path: std::path::PathBuf) {
        if self.saved_visible {
            return;
        }
        // Any other file puts its path in the message now.
        if let Some(result) = crate::chat_images::dropped_path(&path) {
            match result {
                crate::chat_images::Result::Text(text) => {
                    let at_ms = self.born.elapsed().as_millis() as u64;
                    if let Some(field) = self.field()
                        && let Ok(stamp) = field.draft.stamp()
                    {
                        let _ = field.draft.apply(
                            &stamp,
                            rust_native_desktop::composer::Input::Paste(&text),
                            at_ms,
                        );
                    }
                }
                crate::chat_images::Result::Failed(error) => self.notice = Some(error),
                _ => {}
            }
            return;
        }
        // Text only while attachments are off (#10095): a dropped image is
        // dropped, quietly, and the draft stays words only.
        if !self.attachments {
            return;
        }
        // Several images dropped at once arrive one by one; each waits for
        // the one before it.
        if self.image_input.is_some() {
            if self.pending_drops.len() < MAX_PENDING_DROPS {
                self.pending_drops.push_back(path);
            }
            return;
        }
        self.import_image(crate::chat_images::Source::File(path));
    }
    fn poll_images(&mut self, at_ms: u64) {
        let Some((_, _, receiver)) = &self.image_input else {
            return;
        };
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(_) => crate::chat_images::Result::Failed("Image import stopped. Try again.".into()),
        };
        let Some((chat, stamp, _)) = self.image_input.take() else {
            return;
        };
        match result {
            crate::chat_images::Result::Image(image) => {
                let result = self.session.images.add(&chat, image);
                if self.session.selected.as_deref() == Some(&chat) {
                    self.notice = Some(result.err().unwrap_or_else(|| {
                        "Image added. It goes to Coder when Coder starts.".into()
                    }));
                }
            }
            crate::chat_images::Result::Text(text) => {
                if let Some(field) = self.fields.get_mut(&chat) {
                    let _ = field.draft.apply(
                        &stamp,
                        rust_native_desktop::composer::Input::Paste(&text),
                        at_ms,
                    );
                }
            }
            crate::chat_images::Result::Failed(error) => self.notice = Some(error),
            crate::chat_images::Result::Cancelled => {}
        }
        if let Some(path) = self.pending_drops.pop_front() {
            self.import_image(crate::chat_images::Source::File(path));
        }
    }
    fn request(&mut self, command: Command) -> Request {
        request(self.session.request(command))
    }
    pub fn start(&mut self, waker: rust_native_desktop::Waker) {
        self.search.start(waker.clone());
        self.command_query.start(waker.clone());
        for field in self.fields.values_mut() {
            field.start(waker.clone());
        }
        let wake = waker.clone();
        self.transcript.start(Arc::new(move || wake.wake()));
        // The Gym's hosted runner, in the real window only. Its trainer key
        // is read when a hosted run first needs it, never here (#10096).
        // The Gym shows only in a preview build (#11120).
        #[cfg(not(windows))]
        if self.preview {
            let wake = waker.clone();
            if let Some((gym, trainer)) = crate::chat_gym::launch(Arc::new(move || wake.wake())) {
                self.use_gym_trainer(gym, trainer);
            }
        }
        self.waker = Some(waker);
    }
    /// Use `gym` for the chat's Gym: the one `chat_gym` loads with its
    /// trainer, runner, and saved runs, or a test's.
    pub fn use_gym(&mut self, gym: openagents_chat_app::gym::Gym) {
        self.session.cards.gym = gym;
        self.rows_dirty = true;
    }
    /// Use `gym` with `trainer` reading its key when a hosted run needs it.
    #[cfg(not(windows))]
    pub fn use_gym_trainer(
        &mut self,
        gym: openagents_chat_app::gym::Gym,
        trainer: crate::chat_gym::Trainer,
    ) {
        self.use_gym(gym);
        self.gym_trainer = Some(trainer);
    }
    /// The chat's Gym.
    pub fn gym(&self) -> &openagents_chat_app::gym::Gym {
        &self.session.cards.gym
    }
    /// Read the trainer key when a hosted run waits for it, and take what
    /// hosted runs saw since the last tick.
    fn poll_gym(&mut self) {
        #[cfg(not(windows))]
        if let Some(trainer) = self.gym_trainer.as_mut()
            && trainer.tick(&mut self.session.cards.gym)
        {
            self.rows_dirty = true;
        }
        // A computer run's phase is its Coder run's, in this chat's own
        // transcript; only hosted runs report here.
        if self.session.cards.gym.settle(&|_, _| None) {
            self.rows_dirty = true;
        }
    }
    pub fn show_saved(&mut self, visible: bool, project: Option<String>) {
        if self.saved_visible != visible {
            self.rows_dirty = true;
            self.transcript.jump_to_tail();
        }
        if visible && let Some(field) = self.field() {
            field.focused = false;
        }
        self.saved_visible = visible;
        self.saved_project = project;
    }
    pub fn read_saved(&mut self) -> Option<Request> {
        self.saved
            .list(false)
            .map(|(ticket, request)| Request::Saved { ticket, request })
    }
    pub fn saved_outcome(
        &mut self,
        ticket: u64,
        result: Result<
            openagents_chat_app::retained::Answer,
            openagents_chat_app::retained::Failure,
        >,
    ) -> bool {
        let snapshot = self.saved.outcome(ticket, result);
        self.rows_dirty = true;
        if let Some(snapshot) = snapshot {
            let Some(id) = snapshot.chat.clone() else {
                return false;
            };
            let selected = self.saved_visible;
            if selected {
                self.select(&id);
            }
            let (ticket, _) = self.session.request(Command::Read {
                chat: id,
                before: None,
            });
            self.outcome(ticket, Ok(snapshot));
            self.saved_visible = false;
            return selected;
        }
        false
    }
    fn saved_body(&mut self) -> Node<Intent> {
        let mut children = vec![];
        if let Some(chat) = &self.saved.selected {
            children.push(text(
                "saved-title",
                format!(
                    "{} · {}",
                    openagents_chat_app::retained::harness(chat.harness),
                    chat.title
                ),
                TextRole::Heading,
            ));
            children.push(text(
                "saved-time",
                chat.updated_at.as_deref().unwrap_or("Time unavailable"),
                TextRole::Status,
            ));
            if self.rows_dirty {
                self.transcript_rows = self
                    .saved
                    .reader
                    .project()
                    .into_iter()
                    .map(Arc::new)
                    .collect();
                self.rows_generation += 1;
                let (width, height) = self.transcript_size;
                if width > 0.0 && height > 0.0 {
                    let _ =
                        self.transcript
                            .update_shared(self.transcript_rows.clone(), width, height);
                }
                self.rows_dirty = false;
            }
            self.shown();
            children.push(Node {
                key: "chat-transcript".into(),
                style: Style::default(),
                element: Element::Surface {
                    label: "Saved session transcript · read-only".into(),
                    resource: TRANSCRIPT.into(),
                },
            });
            if self.saved.previous.is_some() {
                children.push(button(
                    "saved-earlier",
                    "Load earlier",
                    Action::SavedEarlier,
                    !self.saved.busy(),
                ));
            }
        } else {
            children.push(text(
                "saved-heading",
                "Codex and Claude Code",
                TextRole::Heading,
            ));
            children.push(text(
                "saved-description",
                "Saved on this computer · read-only",
                TextRole::Status,
            ));
            for chat in &self.saved.chats {
                children.push(button(
                    &format!("saved-{}", chat.id),
                    &format!(
                        "{} · {}\n{}{}",
                        openagents_chat_app::retained::harness(chat.harness),
                        chat.title,
                        chat.updated_at.as_deref().unwrap_or("Time unavailable"),
                        if chat.archived { " · Archived" } else { "" }
                    ),
                    Action::SavedSelect {
                        id: chat.id.clone(),
                    },
                    chat.source_id.is_some() && !self.saved.busy(),
                ));
            }
            if self.saved.chats.is_empty() && !self.saved.busy() {
                children.push(text(
                    "saved-empty",
                    "No saved Codex or Claude Code sessions were found.",
                    TextRole::Status,
                ));
            }
        }
        if self.saved.busy() {
            children.push(text(
                "saved-loading",
                "Reading saved session…",
                TextRole::Status,
            ));
        }
        if let Some(error) = &self.saved.error {
            children.push(text("saved-error", error, TextRole::Status));
            if self.saved.can_retry() {
                children.push(button(
                    "saved-retry",
                    "Retry",
                    Action::SavedRetry,
                    !self.saved.busy(),
                ));
            }
        }
        stack("saved-body", Axis::Vertical, children)
    }
    fn saved_footer(&self) -> Node<Intent> {
        let busy = self.saved.busy();
        if self.saved.selected.is_some() {
            return stack(
                "saved-footer",
                Axis::Vertical,
                vec![
                    text(
                        "saved-context-hint",
                        self.saved_project.as_ref().map_or(
                            "Choose a project in Settings to continue.",
                            |_| "Coder picks up where this session left off",
                        ),
                        TextRole::Status,
                    ),
                    stack(
                        "saved-actions",
                        Axis::Wrap,
                        vec![
                            button(
                                "saved-continue",
                                &self
                                    .saved_project
                                    .as_ref()
                                    .map_or("Continue with Coder".into(), |project| {
                                        format!("Continue with Coder · {project}")
                                    }),
                                Action::SavedContinue,
                                self.saved.can_continue() && self.saved_project.is_some(),
                            ),
                            button(
                                "saved-refresh",
                                "Refresh session",
                                Action::SavedRefresh,
                                !busy,
                            ),
                            button("saved-list", "Back to sessions", Action::SavedList, !busy),
                        ],
                    ),
                ],
            );
        }
        stack(
            "saved-list-footer",
            Axis::Wrap,
            vec![
                button("saved-refresh", "Refresh", Action::SavedRefresh, !busy),
                button(
                    "saved-previous",
                    "Previous",
                    Action::SavedPrevious,
                    !busy && self.saved.has_previous_list(),
                ),
                button(
                    "saved-more",
                    "More sessions",
                    Action::SavedMore,
                    !busy && self.saved.next.is_some(),
                ),
            ],
        )
    }
    pub fn tick(&mut self, now: Instant) -> Option<Request> {
        self.transcript.poll_highlights();
        self.poll_feedback();
        self.poll_gym();
        if self.saved_visible {
            return self
                .saved
                .open_more()
                .map(|(ticket, request)| Request::Saved { ticket, request });
        }
        let at_ms = now.saturating_duration_since(self.born).as_millis() as u64;
        self.poll_images(at_ms);
        self.text_only();
        self.search.poll_clipboard(at_ms);
        self.command_query.poll_clipboard(at_ms);
        if let Some((_, field)) = &mut self.rename {
            field.poll_clipboard(at_ms);
        }
        for field in self.fields.values_mut() {
            field.poll_clipboard(at_ms);
        }
        let previous = self.session.selected.clone();
        let outcome = self.session.tick(now).map(request);
        if previous != self.session.selected {
            self.selected_changed(previous);
        }
        if outcome.is_some() {
            return outcome;
        }
        // A task started here is recorded on its thread, so the host's
        // threads (and the phone) show it.
        let bind = self
            .runs
            .iter_mut()
            .find_map(|(chat, run)| run.take_bind().map(|bind| (chat.clone(), bind)));
        if let Some((chat, (task, project))) = bind {
            return Some(self.request(Command::BindCoder {
                chat,
                host: coder_run::LOCAL.into(),
                task,
                project: Some(project),
            }));
        }
        if let Some(chat) = self.session.selected.clone()
            && let Some((ticket, request)) = self.tasks.get_mut(&chat).and_then(|t| t.tick(now))
        {
            return Some(Request::TaskChat {
                chat,
                ticket,
                request,
            });
        }
        // The shown chat's run first, then the others.
        let selected = self.session.selected.clone();
        let mut chats: Vec<String> = self.runs.keys().cloned().collect();
        chats.sort_by_key(|chat| Some(chat) != selected.as_ref());
        for chat in chats {
            let run = self.runs.get_mut(&chat)?;
            let revision = run.revision;
            let ticked = run.tick(now);
            if revision != run.revision && selected.as_ref() == Some(&chat) {
                self.rows_dirty = true;
            }
            if let Some((ticket, request)) = ticked {
                return Some(Request::CoderRun {
                    chat,
                    ticket,
                    request,
                });
            }
        }
        None
    }
    fn busy(&self) -> bool {
        if let Some(run) = self.run() {
            return run.busy();
        }
        self.task()
            .map_or_else(|| self.session.busy(), task_chat::Session::busy)
    }
    fn run(&self) -> Option<&Run> {
        self.runs.get(self.session.selected.as_ref()?)
    }
    /// This computer's project folders, the shown one first: where a new
    /// Coder run starts unless the chat has its own.
    /// The name the account menu shows: the signed-in person's, or this
    /// computer's.
    pub fn set_account_name(&mut self, name: String) {
        self.account_name = name;
    }
    pub fn set_coder_projects(&mut self, folders: Vec<String>) {
        self.coder_projects = folders;
    }
    /// Decide whether a coding reply asks first with `asks` instead of
    /// the settings file.
    pub fn set_coder_asks_first(&mut self, asks: impl Fn() -> bool + 'static) {
        self.coder_asks_first = Box::new(asks);
    }
    /// Read `coder.start` from `file`, the settings file the window keeps
    /// (its Settings page changes it), each time a reply is judged coding
    /// work. A file Coder's loader refuses asks first, as
    /// `coder::task::local::Local::asks_first` does.
    pub fn read_coder_start_from(&mut self, file: std::path::PathBuf) {
        self.set_coder_asks_first(move || {
            coder::task::settings::Settings::load(&file).map_or(true, |settings| {
                settings.coder.start == coder::task::settings::Start::AskFirst
            })
        });
    }
    /// Start Coder on this computer for `chat`, as `openagents chat` does.
    fn start_run(&mut self, chat: &str) {
        if self.runs.contains_key(chat) {
            return;
        }
        // Text only while attachments are off: the run carries no images.
        self.text_only();
        let Some(snapshot) = self.session.states.get(chat) else {
            return;
        };
        if snapshot.coder.is_some() {
            return;
        }
        let chat_title = self
            .session
            .summaries
            .iter()
            .find(|summary| summary.id == chat)
            .map_or_else(|| "Coder task".to_owned(), |summary| summary.title.clone());
        // The task is titled by the message that asked for the work, not
        // the chat's first message (#10073).
        let title = openagents_chat::delegation::title(&chat_title, &snapshot.turns);
        let prompt = openagents_chat::delegation::prompt(&chat_title, &snapshot.turns);
        // The draft's images go to this computer's run, never to the
        // hosted conversation; a refusal keeps the draft.
        let images = match self.session.images.uploads(chat) {
            Ok(images) => images,
            Err(reason) => {
                self.notice = Some(reason);
                return;
            }
        };
        // The engine the person asked for, from the reply's typed offer
        // (#10076).
        let engine = openagents_chat::delegation::requested(&snapshot.turns);
        self.runs.insert(
            chat.to_owned(),
            Run::start_with_images(
                chat,
                &title,
                &prompt,
                self.coder_projects.clone(),
                images,
                Instant::now(),
            )
            .requesting(engine),
        );
        if self.session.selected.as_deref() == Some(chat) {
            self.rows_dirty = true;
        }
    }
    /// A Gym run that goes to Coder (`gym::Effect::Computer`): on the
    /// desktop the ready computer is this one, so the test set runs as
    /// the open chat's Coder run, and the Gym records where, as the phone
    /// records its computer's task.
    fn gym_coder(&mut self, run: &str, prompt: &str) {
        let started = match self.session.selected.clone() {
            Some(chat) if !self.runs.contains_key(&chat) => {
                let label = self
                    .session
                    .states
                    .get(&chat)
                    .and_then(|state| state.ready_computer.clone())
                    .unwrap_or_else(|| "This computer".into());
                self.runs.insert(
                    chat.clone(),
                    Run::start(
                        &chat,
                        "Gym test",
                        prompt,
                        self.coder_projects.clone(),
                        Instant::now(),
                    ),
                );
                Ok((coder_run::LOCAL.to_owned(), label, chat))
            }
            Some(_) => {
                Err("Coder is already working in this chat. Try again when it's done.".into())
            }
            None => Err("Open a chat to run this.".into()),
        };
        self.session.cards.gym.on_computer(run, started);
    }
    /// A Gym run's follow-up or stop (`gym::Effect::Command`), sent to its
    /// Coder run on this computer.
    fn gym_command(&mut self, host: &str, task: &str, text: &str, stop: bool) -> Option<Request> {
        let run = self
            .runs
            .get_mut(task)
            .filter(|_| host == coder_run::LOCAL)?;
        let action = if stop {
            RunAction::Stop
        } else {
            RunAction::Send
        };
        let (ticket, request) = run.action(action, if stop { "" } else { text })?;
        Some(Request::CoderRun {
            chat: task.to_owned(),
            ticket,
            request,
        })
    }
    /// The runner's answer for a chat's run.
    pub fn run_outcome(
        &mut self,
        chat: String,
        ticket: u64,
        result: Result<coder_run::Answer, String>,
    ) {
        let Some(run) = self.runs.get_mut(&chat) else {
            return;
        };
        let revision = run.revision;
        let accepted = run.outcome(ticket, result, Instant::now());
        if revision != run.revision && self.session.selected.as_ref() == Some(&chat) {
            self.rows_dirty = true;
        }
        if run.take_delivered() {
            self.session.images.clear(&chat);
            self.rows_dirty = true;
        }
        let submission = self.run_submissions.remove(&(chat.clone(), ticket));
        if accepted
            && let Some(submission) = submission
            && let Some(field) = self.fields.get_mut(&chat)
        {
            let _ = field.draft.accepted(&submission);
            field.focused = true;
        }
    }
    fn run_action(&mut self, action: RunAction, view: &ValidatedView<Intent>) -> Option<Request> {
        let chat = self.session.selected.clone()?;
        let composing = matches!(
            action,
            RunAction::Send | RunAction::Queue | RunAction::Steer
        );
        let submission = if composing {
            let field = self.field()?;
            let stamp = field.draft.stamp().ok()?;
            let submission = field.draft.submission(view, &stamp, None).ok()?;
            if submission.text.len() > coder_run::MAX_MESSAGE {
                self.notice = Some(
                    "Coder accepts messages up to 16 KiB. Shorten this draft to send it.".into(),
                );
                return None;
            }
            Some(submission)
        } else {
            None
        };
        let run = self.runs.get_mut(&chat)?;
        let revision = run.revision;
        let requested = run.action(action, submission.as_ref().map_or("", |s| s.text.as_str()));
        let changed = revision != run.revision;
        if changed {
            self.rows_dirty = true;
        }
        if let Some(submission) = submission {
            match &requested {
                Some((ticket, coder_run::Request::Continue { .. })) => {
                    self.run_submissions
                        .insert((chat.clone(), *ticket), submission);
                }
                // Queued, or held until the stop ends the turn.
                _ if changed => {
                    if let Some(field) = self.fields.get_mut(&chat) {
                        let _ = field.draft.accepted(&submission);
                        field.focused = true;
                    }
                }
                _ => {}
            }
        }
        let (ticket, request) = requested?;
        Some(Request::CoderRun {
            chat,
            ticket,
            request,
        })
    }
    /// The transcript revision shown now.
    fn shown(&mut self) -> u64 {
        let state = (
            self.session.revision,
            self.task().map_or(0, |task| task.revision),
            self.rows_generation,
        );
        if state != self.shown_state {
            self.shown_state = state;
            self.shown += 1;
        }
        self.shown
    }
    /// What the transcript button `key` does now, and to what.
    fn pressed(&self, key: &str) -> Pressed {
        let chat = self.session.selected.clone().unwrap_or_default();
        if let Some(target) = self.task().and_then(|task| task.target(key)) {
            return Pressed::Task(chat, target);
        }
        if let Some(target) = self.run().and_then(|run| run.target(key)) {
            return Pressed::Run(chat, target);
        }
        Pressed::Other(key.into())
    }
    /// Whether a release on `key` ends `press` with an action, though the
    /// press began on another revision of the transcript.
    fn late_release(&mut self, press: &rust_native::Press<Pressed>, key: &str) -> bool {
        let revision = self.shown();
        let offered = self.pressed(key);
        press
            .release(revision, key, Some(&offered), Pressed::late)
            .is_some()
    }
    /// How many times a Coder run in these chats was passed over or refused
    /// for capacity: a start that ran another engine than the first, or
    /// none, a provider switch mid-run, or an ending for no capacity. The
    /// shell reads the engine again when it grows (#10105). Typed events
    /// only; no text is read.
    #[must_use]
    pub fn capacity_signals(&self) -> usize {
        use openagents_chat::coder_events::{CoderEvent, Runner};
        self.runs
            .values()
            .flat_map(|run| run.lines())
            .filter(|line| match &line.event {
                CoderEvent::ProviderSwitched(_) => true,
                CoderEvent::Failure(failure) => failure.ending.as_deref() == Some("no_capacity"),
                CoderEvent::CoderStarted(started) => match &started.runner {
                    Some(Runner::Runs { passed, .. }) => !passed.is_empty(),
                    Some(Runner::NoCapacity { .. }) => true,
                    _ => false,
                },
                _ => false,
            })
            .count()
    }

    /// Each chat with Coder work, its title, and what Coder is doing, for
    /// desktop notifications ([`crate::notices`]).
    pub fn coder_statuses(&self) -> Vec<(String, String, crate::notices::Status)> {
        use crate::notices::Status;
        use nostr::activity_summary::{Attention, Phase};
        let title = |chat: &str| {
            self.session
                .summaries
                .iter()
                .find(|summary| summary.id == chat)
                .map(|summary| summary.title.clone())
                .unwrap_or_default()
        };
        let runs = self.runs.iter().map(|(chat, run)| {
            let status = match run.state() {
                coder_run::State::Waiting
                    if run.lines().last().is_some_and(|line| {
                        matches!(
                            line.event,
                            openagents_chat::coder_events::CoderEvent::Approval(_)
                        )
                    }) =>
                {
                    Status::Approval
                }
                coder_run::State::Waiting => Status::Question,
                _ if run.finished() => Status::Finished,
                _ => Status::Working,
            };
            (chat.clone(), title(chat), status)
        });
        let tasks = self
            .tasks
            .iter()
            .filter(|(chat, _)| !self.runs.contains_key(*chat))
            .map(|(chat, task)| {
                let status = match task.summary.as_ref().map(|s| (s.phase, s.attention)) {
                    Some((Phase::Waiting, Attention::Approval)) => Status::Approval,
                    Some((Phase::Waiting, Attention::Input)) => Status::Question,
                    Some((Phase::Completed, _)) => Status::Finished,
                    Some((Phase::Failed, _)) => Status::Failed,
                    _ => Status::Working,
                };
                (chat.clone(), title(chat), status)
            });
        runs.chain(tasks).collect()
    }
    fn task(&self) -> Option<&task_chat::Session> {
        self.tasks.get(self.session.selected.as_ref()?)
    }
    pub fn task_outcome(
        &mut self,
        chat: String,
        ticket: u64,
        result: ControlResult<task_chat::Answer>,
    ) {
        let Some(task) = self.tasks.get_mut(&chat) else {
            return;
        };
        let revision = task.revision;
        let operation = task
            .pending_request(ticket)
            .and_then(|request| match request {
                task_chat::Request::Operation { request, .. } => Some(request.clone()),
                _ => None,
            });
        let accepted = match result {
            Err(crate::control::ControlError::Refused { code, .. }) if code == "source_changed" => {
                task.source_changed(ticket);
                false
            }
            Err(crate::control::ControlError::Refused { code, message })
                if code != "unavailable" && code != "Unavailable" =>
            {
                task.refused(ticket, message);
                false
            }
            other => task.outcome(ticket, other.map_err(|error| error.to_string())),
        };
        if accepted
            && let Some(operation) = operation
            && let Some(submission) = self.task_submissions.remove(&(chat.clone(), operation))
            && let Some(field) = self.fields.get_mut(&chat)
        {
            let _ = field.draft.accepted(&submission);
            field.focused = true;
        }
        if revision != task.revision && self.session.selected.as_ref() == Some(&chat) {
            self.rows_dirty = true;
        }
    }
    fn task_action(
        &mut self,
        action: TaskAction,
        view: &ValidatedView<Intent>,
        now: Instant,
    ) -> Option<Request> {
        let chat = self.session.selected.clone()?;
        let submitted = matches!(
            action,
            TaskAction::Send | TaskAction::Queue | TaskAction::Steer
        );
        let submission = if submitted {
            // Text only while attachments are off: a restored draft that
            // still holds images sends its words alone.
            self.text_only();
            if let Some(reason) = self.session.images.text_only_refusal(&chat) {
                self.notice = Some(reason.into());
                return None;
            }
            let field = self.field()?;
            let stamp = field.draft.stamp().ok()?;
            let submission = field.draft.submission(view, &stamp, None).ok()?;
            if submission.text.len() > 16 * 1024 {
                self.notice = Some(
                    "Coder accepts messages up to 16 KiB. Shorten this draft to send it.".into(),
                );
                return None;
            }
            Some(submission)
        } else {
            None
        };
        let task = self.tasks.get_mut(&chat)?;
        let revision = task.revision;
        let editing = matches!(action, TaskAction::EditQueued(_));
        let request = task.action(
            action,
            submission.as_ref().map_or("", |s| s.text.as_str()),
            task_chat::unix_now(),
        );
        if editing && let Some(text) = task.editing_text().map(str::to_owned) {
            *self.task_editor.entry(chat.clone()).or_default() += 1;
            let mut field = chat_field(task.placeholder());
            // The replacement text mounts with the new editing token in footer().
            field.focused = true;
            if let Some(wake) = self.waker.clone() {
                field.start(wake);
            }
            self.fields.insert(chat.clone(), field);
            self.notice = Some(format!(
                "Editing queued message: {}",
                text.chars().take(80).collect::<String>()
            ));
        }
        let moved = revision != task.revision;
        if moved {
            self.rows_dirty = true;
        }
        // A question's typed answer that moved its decision panel to the
        // next page is taken, though nothing is sent yet.
        if request.is_none()
            && moved
            && let Some(submission) = &submission
            && let Some(field) = self.fields.get_mut(&chat)
        {
            let _ = field.draft.accepted(submission);
            field.focused = true;
        }
        let (ticket, request) = request?;
        if let Some(submission) = submission
            && let task_chat::Request::Operation {
                request: operation, ..
            } = &request
        {
            self.task_submissions
                .insert((chat.clone(), operation.clone()), submission);
        }
        let _ = now;
        Some(Request::TaskChat {
            chat,
            ticket,
            request,
        })
    }
    pub fn images(&self) -> &[openagents_chat_app::attachments::Image] {
        self.session
            .selected
            .as_ref()
            .map_or(&[], |id| self.session.images.get(id))
    }
    pub fn state(&self) -> Option<&Snapshot> {
        self.session.state()
    }
    /// The selected conversation's local draft, separate from saved messages.
    pub fn draft(&self) -> &str {
        self.session
            .selected
            .as_ref()
            .and_then(|id| self.fields.get(id))
            .map_or("", Field::text)
    }
    fn field(&mut self) -> Option<&mut Field> {
        let id = self.session.selected.as_ref()?;
        self.fields.get_mut(id)
    }
    /// Asks the host how many of this person's replies took each route
    /// (`Command::Routes`), for the Map page (#10085). The counts come
    /// back to [`Panel::route_counts`] and never leave this computer.
    pub fn request_routes(&mut self) -> Request {
        self.request(Command::Routes {})
    }

    /// The last route counts the host answered.
    pub fn route_counts(&self) -> Option<&std::collections::BTreeMap<String, u64>> {
        self.session.routes.as_ref()
    }

    /// Puts `text` in the selected chat's composer, unsent: a Map page
    /// step's message (#10085). The person reads it, edits it, and sends
    /// it, or doesn't.
    pub fn prefill(&mut self, text: &str) {
        if let Some(field) = self.field() {
            field.focused = true;
            field.input(TextInput::Commit(text), 0);
        }
    }

    pub fn new_chat(&mut self) -> Request {
        let previous = self.session.selected.clone();
        let result = request(self.session.new_chat());
        self.selected_changed(previous);
        result
    }
    fn select(&mut self, id: &str) {
        let previous = self.session.selected.clone();
        if self.session.select(id) {
            self.selected_changed(previous);
        }
    }
    fn selected_changed(&mut self, previous: Option<String>) {
        self.search.input(TextInput::FocusLost, 0);
        self.rename = None;
        self.rename_pending = None;
        self.feedback = None;
        if let Some(field) = previous.and_then(|id| self.fields.get_mut(&id)) {
            field.input(TextInput::FocusLost, 0);
        }
        if let Some(id) = &self.session.selected {
            let field = self
                .fields
                .entry(id.clone())
                .or_insert_with(|| chat_field(openagents_chat_app::coder_run::PLACEHOLDER));
            field.focused = true;
            if let Some(waker) = &self.waker {
                field.start(waker.clone());
            }
        }
        self.transcript = chat_transcript();
        if let Some(waker) = &self.waker {
            let wake = waker.clone();
            self.transcript.start(Arc::new(move || wake.wake()));
        }
        self.transcript_rows.clear();
        self.projection = Projection::default();
        self.transcript_size = (0.0, 0.0);
        self.rows_dirty = true;
        self.notice = None;
        self.changes_bound = None;
        self.changes_open = false;
        self.changes_scroll = 0.0;
    }
    pub fn select_numeric(&mut self, id: u64) -> Option<Request> {
        let id = self
            .ids
            .iter()
            .find(|(_, number)| **number == id)?
            .0
            .clone();
        self.select(&id);
        Some(self.request(Command::Read {
            chat: id,
            before: None,
        }))
    }
    /// Repaints the chat in the scheme the app now paints with
    /// ([`openagents_chat_app::visual::current`], #11028): the transcript's
    /// palette and syntax colors, the open fields' ink, the change pane's
    /// highlighter, and the metrics at `size` (they carry the inline-code
    /// ink). The views read the scheme when they are next built.
    pub fn apply_visual(&mut self, size: openagents_chat_app::preferences::TextSize) {
        let visual = openagents_chat_app::visual::current();
        self.transcript.set_palette(&visual.colors);
        self.transcript.set_syntax_palette(visual.syntax);
        let fields = self
            .fields
            .values_mut()
            .chain([&mut self.search, &mut self.command_query])
            .chain(self.rename.as_mut().map(|(_, field)| field));
        for field in fields {
            field.set_colors(visual.text, visual.faint, visual.accent);
            field.set_appearance(field_appearance());
        }
        self.changes_highlighter = None;
        if let Some(doc) = self
            .reviewer_mut()
            .and_then(openagents_chat_app::changes::Reviewer::document_mut)
        {
            doc.clear_spans();
        }
        self.set_text_size(size);
        self.rows_dirty = true;
    }

    /// Draws the transcript's text at `size` (Settings, #10021), laying
    /// the rows out again at once.
    pub fn set_text_size(&mut self, size: openagents_chat_app::preferences::TextSize) {
        if self.transcript.set_metrics(size.transcript()).is_ok() {
            let (width, height) = self.transcript_size;
            if width > 0.0 && height > 0.0 {
                let _ = self
                    .transcript
                    .update_shared(self.transcript_rows.clone(), width, height);
            }
            self.rows_dirty = true;
        }
    }
    /// Every conversation's list metadata, as the host last reported it.
    #[must_use]
    pub fn summaries(&self) -> &[openagents_chat::basic_chats::Summary] {
        &self.session.summaries
    }
    /// The archived chats Settings lists, newest first
    /// ([`openagents_chat_app::chat_list::archived`]).
    pub fn archived(&self) -> Vec<crate::settings::Archived> {
        openagents_chat_app::chat_list::archived(&self.session.summaries)
            .into_iter()
            .map(|summary| crate::settings::Archived {
                id: summary.id.clone(),
                title: summary.title.clone(),
            })
            .collect()
    }
    /// Restores an archived chat without opening it: the shared restore
    /// command the chat menu sends for the open chat.
    pub fn restore(&mut self, chat: &str) -> Option<Request> {
        self.session
            .summaries
            .iter()
            .any(|summary| summary.id == chat && summary.archived)
            .then(|| {
                self.request(Command::Restore {
                    chat: chat.to_owned(),
                })
            })
    }
    /// Each chat with Coder work and what its row shows
    /// ([`attention::Indicator`]) at `now` and Unix second `unix`. The
    /// chat open on the chat page (`viewing`) has its current ending
    /// marked seen.
    fn indicators(
        &mut self,
        now: Instant,
        unix: u64,
        viewing: bool,
    ) -> BTreeMap<String, attention::Indicator> {
        let mut out = BTreeMap::new();
        let open = self.session.selected.clone().filter(|_| viewing);
        for (chat, run) in &self.runs {
            let (activity, silent) = run.activity(now);
            if open.as_ref() == Some(chat) {
                self.seen.insert(chat.clone(), run.mark());
            }
            let seen = self.seen.get(chat) == Some(&run.mark());
            out.insert(chat.clone(), attention::indicator(activity, silent, seen));
        }
        for (chat, task) in &self.tasks {
            if self.runs.contains_key(chat) {
                continue;
            }
            let Some(summary) = &task.summary else {
                continue;
            };
            if open.as_ref() == Some(chat) {
                self.seen.insert(chat.clone(), summary.sequence);
            }
            let seen = self.seen.get(chat) == Some(&summary.sequence);
            out.insert(chat.clone(), attention::of_summary(summary, unix, seen));
        }
        out
    }

    pub fn sync_sidebar(&mut self, state: &mut State) {
        self.sync_sidebar_at(state, Instant::now(), task_chat::unix_now());
    }

    fn sync_sidebar_at(&mut self, state: &mut State, now: Instant, unix: u64) {
        state.profile_open =
            self.commands.kind == Some(openagents_chat_app::commands::Kind::Profile);
        self.sidebar_width = state.sidebar_width;
        let mut next = self.ids.values().max().copied().unwrap_or(0) + 1;
        for summary in &self.session.summaries {
            if !self.ids.contains_key(&summary.id) {
                self.ids.insert(summary.id.clone(), next);
                next += 1;
            }
        }
        if let Some(selected) = &self.session.selected
            && !self.ids.contains_key(selected)
        {
            let id = self.ids.values().max().copied().unwrap_or(0) + 1;
            self.ids.insert(selected.clone(), id);
        }
        state.search = self.search.text().into();
        state.total_chats = self.session.summaries.len();
        state.projects.clear();
        // The shared list order (#10100): Pinned, then Recent newest first
        // across projects (a project is a label on its row), then Archived,
        // so a new chat opens at the top, as on the phone.
        // Within each group, a chat that needs the person rises above
        // quieter ones (#10468); equal indicators stay newest first.
        let viewing = matches!(state.page, crate::chrome::Page::Chat(_));
        let indicators = self.indicators(now, unix, viewing);
        let indicator = |summary: &openagents_chat::basic_chats::Summary| {
            indicators.get(&summary.id).copied().unwrap_or_default()
        };
        let mut listed =
            openagents_chat_app::chat_list::search(&self.session.summaries, &state.search);
        openagents_chat_app::chat_list::by_attention(&mut listed, indicator);
        for summary in &listed {
            if let openagents_chat_app::chat_list::Group::Project(project) =
                openagents_chat_app::chat_list::group(summary)
            {
                state.projects.insert(self.ids[&summary.id], project);
            }
        }
        let chats = listed
            .into_iter()
            .map(|summary| Chat {
                id: self.ids[&summary.id],
                title: summary.title.clone(),
                detail: "OpenAgents · Saved",
                indicator: indicator(summary),
                section: if summary.archived {
                    Section::Archived
                } else if summary.pinned {
                    Section::Pinned
                } else {
                    Section::Recent
                },
            })
            .collect();
        let selecting = matches!(state.page, crate::chrome::Page::Chat(_));
        state.sync_chats(
            chats,
            self.session
                .selected
                .as_ref()
                .and_then(|id| self.ids.get(id))
                .copied()
                .filter(|_| selecting),
        );
    }
    pub fn outcome(&mut self, ticket: u64, result: ControlResult<Snapshot>) {
        if self
            .rename_pending
            .as_ref()
            .is_some_and(|(pending, _, _)| *pending == ticket)
        {
            let (_, chat, title) = self.rename_pending.take().expect("matched pending rename");
            if result
                .as_ref()
                .is_ok_and(|snapshot| snapshot.storage_error.is_none())
                && self.session.selected.as_deref() == Some(&chat)
                && self
                    .rename
                    .as_ref()
                    .is_some_and(|(_, field)| field.text().trim() == title.trim())
            {
                self.close_overlay();
            }
        }
        let previous = self.session.selected.clone();
        let revision = self.session.revision;
        let accepted = self
            .session
            .outcome(ticket, result.map_err(|error| error.to_string()));
        if let Some(snapshot) = self.session.state()
            && let (Some(chat), Some(binding)) = (&snapshot.chat, &snapshot.coder)
            && binding.host == coder_run::LOCAL
        {
            // Coder on this computer: its events, from the first.
            let replace = self
                .runs
                .get(chat)
                .is_none_or(|run| run.task.as_ref().is_some_and(|task| *task != binding.task));
            if replace {
                self.runs.insert(
                    chat.clone(),
                    Run::follow(chat, &binding.task, binding.project.clone(), Instant::now()),
                );
                self.rows_dirty = true;
            }
        } else if let Some(snapshot) = self.session.state()
            && let (Some(chat), Some(binding)) = (&snapshot.chat, &snapshot.coder)
        {
            let replace = self.tasks.get(chat).is_none_or(|task| {
                task.binding.task != binding.task || task.binding.host != binding.host
            });
            if replace {
                self.tasks.insert(
                    chat.clone(),
                    task_chat::Session::new(binding.clone(), Instant::now()),
                );
                self.rows_dirty = true;
            }
        }
        self.run_if_coding();
        for (_, request) in accepted {
            if let Some((id, submission)) = self.submissions.remove(&request)
                && let Some(field) = self.fields.get_mut(&id)
            {
                let _ = field.draft.accepted(&submission);
                field.focused = true;
            }
        }
        if previous != self.session.selected {
            self.selected_changed(previous);
        }
        if revision != self.session.revision {
            self.rows_dirty = true;
        }
    }
    /// A coding reply to a message sent from this window starts Coder
    /// here at once, as `openagents chat` does; the reply's offer is the
    /// router's judgment.
    fn run_if_coding(&mut self) {
        let Some(snapshot) = self.session.state() else {
            return;
        };
        let Some(chat) = snapshot.chat.clone() else {
            return;
        };
        if snapshot.busy {
            return;
        }
        // Images sent with a message wait for its reply: a coding reply
        // leaves them in the draft for the start below or **Run Coder**,
        // which carries them; any other keeps them and says so.
        if self.session.images.bound(&chat).is_some() {
            let computer = snapshot.computer;
            let turns = snapshot.turns.clone();
            if self.session.images.settle(&chat, &turns, false, computer)
                == Some(openagents_chat_app::attachments::Settled::Kept)
            {
                self.notice = Some(openagents_chat_app::attachments::ONLY_TO_CODER.into());
            }
        }
        let Some(snapshot) = self.session.state() else {
            return;
        };
        let sent: Vec<String> = self
            .sent
            .iter()
            .filter(|(id, _)| *id == chat)
            .map(|(_, request)| request.clone())
            .collect();
        let mut run = false;
        // The message whose reply hands it to Coder: after a run's turn
        // ended, it is Coder's next turn (#10094).
        let mut asked = None;
        for request in sent {
            let Some(at) = snapshot
                .turns
                .iter()
                .rposition(|turn| turn.request.as_deref() == Some(request.as_str()))
            else {
                continue;
            };
            let Some(reply) = snapshot.turns.get(at + 1) else {
                continue;
            };
            self.sent.remove(&(chat.clone(), request));
            // The router's typed `open_presentation` offer, never the
            // reply's words, opens a deck (#10058).
            if reply.role == Role::Assistant
                && !reply.stopped
                && let Some(deck) = presentation_offer(reply.meta.as_ref())
            {
                self.presentation = Some(deck);
            }
            // Likewise the typed `open_screen` offer for `routes.map`
            // opens the Map page when the reply arrives (#10102).
            if reply.role == Role::Assistant && !reply.stopped && map_offer(reply.meta.as_ref()) {
                self.map = true;
            }
            if reply.role == Role::Assistant
                && !reply.stopped
                && openagents_chat::delegation::offered(reply.meta.as_ref(), snapshot.computer)
            {
                run = true;
                asked = snapshot.turns.get(at).map(|turn| turn.text.clone());
            }
        }
        // A chat whose Coder run has ended: the router judged this message
        // more work for it, so it continues the same task, in the same
        // worktree, as its next turn. The person started Coder in this
        // chat already, so `coder.start: ask_first` does not ask again.
        if run
            && let Some(text) = asked
            && let Some(existing) = self.runs.get_mut(&chat)
            && existing.routes_followups()
        {
            // It goes at the run's next tick.
            if existing.continue_with(&text) {
                self.rows_dirty = true;
            }
            return;
        }
        // `coder.start: ask_first` leaves the offer's **Run Coder** to
        // the person, as `openagents chat` leaves `run-coder`.
        if run && !(self.coder_asks_first)() {
            self.start_run(&chat);
        }
    }
    /// The transcript's rows as last published, for tests and captures.
    pub fn transcript_rows(&self) -> &[Arc<Node<()>>] {
        &self.transcript_rows
    }
    /// The chat's Coder run on this computer, if it has one.
    pub fn coder_run(&self, chat: &str) -> Option<&Run> {
        self.runs.get(chat)
    }
    pub fn take_activated(&mut self) -> Vec<String> {
        std::mem::take(&mut self.activated)
    }
    pub fn take_requests(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.queued)
    }
    pub fn take_desktop_navigation(&mut self) -> Option<crate::chrome::Action> {
        self.desktop_navigation.take()
    }
    /// Takes a finished reply's typed offers: an `open_presentation` offer
    /// holds its deck for the shell's slide viewer, and an `open_screen`
    /// offer for `routes.map` holds the Map page, as a finished reply to a
    /// message sent from this window does ([`presentation_offer`],
    /// [`map_offer`]).
    pub fn receive_offers(&mut self, meta: Option<&openagents_chat::router::Meta>) {
        if let Some(deck) = presentation_offer(meta) {
            self.presentation = Some(deck);
        }
        if map_offer(meta) {
            self.map = true;
        }
    }
    /// Whether a reply's typed `routes.map` offer asked for the Map page;
    /// taken once.
    pub fn take_map(&mut self) -> bool {
        std::mem::take(&mut self.map)
    }
    /// The deck a reply's typed `open_presentation` offer asked for.
    pub fn take_presentation(&mut self) -> Option<String> {
        self.presentation.take()
    }
    pub fn take_navigation(&mut self) -> Option<openagents_chat::router::Screen> {
        self.navigation.take()
    }
    pub fn navigation_notice(&mut self, notice: String) {
        self.notice = Some(notice);
    }
    /// The line the panel shows under the transcript, if any.
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }
    pub fn mounted(&mut self, view: &ValidatedView<Intent>) {
        let _ = self.search.draft.mount(view, "chat-search");
        let _ = self.command_query.draft.mount(view, "command-query");
        if let Some((_, field)) = &mut self.rename {
            let _ = field.draft.mount(view, "chat-rename");
        }
        if let Some(field) = self.field()
            && let Ok(mount) = field.draft.mount(view, "chat-composer")
            && mount.focus
        {
            field.focused = true;
        }
    }
    pub fn action(
        &mut self,
        action: Action,
        view: &ValidatedView<Intent>,
        now: Instant,
    ) -> Option<Request> {
        let saved = match &action {
            Action::SavedSelect { id } => self.saved.select(id),
            Action::SavedRefresh => {
                let id = self.saved.selected.as_ref().map(|chat| chat.id.clone());
                if let Some(id) = id {
                    self.saved.select(&id)
                } else {
                    self.saved.list(false)
                }
            }
            Action::SavedMore => self.saved.list(true),
            Action::SavedPrevious => self.saved.previous_list(),
            Action::SavedEarlier => self.saved.earlier(),
            Action::SavedContinue => self
                .saved
                .continue_in(self.saved_project.as_deref().unwrap_or("")),
            Action::SavedRetry => self.saved.retry(),
            Action::SavedList => {
                self.saved.selected = None;
                self.rows_dirty = true;
                return None;
            }
            _ => None,
        };
        if matches!(
            action,
            Action::SavedSelect { .. }
                | Action::SavedRefresh
                | Action::SavedMore
                | Action::SavedPrevious
                | Action::SavedEarlier
                | Action::SavedContinue
                | Action::SavedRetry
        ) {
            self.rows_dirty = true;
            return saved.map(|(ticket, request)| Request::Saved { ticket, request });
        }
        match &action {
            Action::Profile => {
                if self.commands.kind == Some(openagents_chat_app::commands::Kind::Profile) {
                    self.close_overlay();
                } else {
                    self.open_commands(openagents_chat_app::commands::Kind::Profile);
                }
                return None;
            }
            Action::Palette => {
                self.open_commands(openagents_chat_app::commands::Kind::Palette);
                return None;
            }
            Action::Menu => {
                self.open_commands(openagents_chat_app::commands::Kind::Menu);
                return None;
            }
            Action::DismissOverlay => {
                self.close_overlay();
                return None;
            }
            Action::SaveName if self.feedback.is_some() => {
                self.send_feedback();
                return None;
            }
            Action::CancelRename if self.feedback.is_some() => {
                self.close_overlay();
                return None;
            }
            Action::Command { key } => {
                let entries = self.command_entries();
                if let Some(entry) = entries
                    .iter()
                    .find(|entry| &entry.key == key && entry.enabled)
                    .cloned()
                {
                    let confirm = self.commands.kind
                        == Some(openagents_chat_app::commands::Kind::ConfirmArchive);
                    if entry.action == openagents_chat_app::commands::Action::Archive && !confirm {
                        self.open_commands(openagents_chat_app::commands::Kind::ConfirmArchive);
                    } else {
                        self.close_overlay();
                        self.run_command(entry.action, view, now);
                    }
                }
                return None;
            }
            _ => {}
        }
        let id = self.session.selected.clone()?;
        match action {
            Action::SavedSelect { .. }
            | Action::SavedRefresh
            | Action::SavedMore
            | Action::SavedPrevious
            | Action::SavedEarlier
            | Action::SavedContinue
            | Action::SavedRetry
            | Action::SavedList => None,
            Action::Palette
            | Action::Profile
            | Action::Menu
            | Action::DismissOverlay
            | Action::Command { .. } => None,
            Action::Card { key } if key == "changes-open" && self.show_changes() => {
                self.changes_open = true;
                self.changes_scroll = 0.0;
                None
            }
            Action::Card { key } if key == "changes-close" => {
                self.changes_open = false;
                None
            }
            Action::Card { key } if key == "changes-refresh" => {
                if let Some(reviewer) = self.reviewer_mut() {
                    reviewer.refresh();
                    self.changes_scroll = 0.0;
                }
                None
            }
            Action::Card { key } if key == "changes-publish" => {
                if let Some(reviewer) = self.reviewer_mut() {
                    reviewer.publish();
                }
                None
            }
            Action::Card { key } if key == "changes-link" => {
                if let Some((_, url)) = self.reviewer().and_then(|r| r.card(true)?.link) {
                    open_link(&url);
                }
                None
            }
            Action::Card { key } => {
                if let Some(action) = self.task().and_then(|task| task.actions.get(&key)).cloned() {
                    return self.task_action(action, view, now);
                }
                if key == "coder-steer" {
                    return self.run_action(RunAction::Steer, view);
                }
                if let Some(action) = self.run().and_then(|run| run.actions.get(&key)).cloned() {
                    return self.run_action(action, view);
                }
                // Run Coder starts it on this computer (#10033).
                if self.session.cards.actions.get(&key)
                    == Some(&openagents_chat_app::cards::Action::RunCoder)
                {
                    self.start_run(&id);
                    return None;
                }
                let previous = self.session.selected.clone();
                let effect = self.session.card_action(&key);
                if previous != self.session.selected {
                    self.selected_changed(previous);
                }
                match effect {
                    openagents_chat_app::cards::Effect::Requests(requests) => {
                        self.rows_dirty = true;
                        let mut requests = requests.into_iter().map(request);
                        let first = requests.next();
                        self.queued.extend(requests);
                        first
                    }
                    openagents_chat_app::cards::Effect::Navigate(screen) => {
                        self.navigation = Some(screen);
                        None
                    }
                    openagents_chat_app::cards::Effect::Notice(notice) => {
                        self.notice = Some(notice);
                        None
                    }
                    openagents_chat_app::cards::Effect::Draft(text) => {
                        let at = now.duration_since(self.born).as_millis() as u64;
                        if let Some(field) = self.field()
                            && let Ok(stamp) = field.draft.stamp()
                        {
                            let _ = field.draft.apply(
                                &stamp,
                                rust_native_desktop::composer::Input::Paste(&text),
                                at,
                            );
                            field.focused = true;
                        }
                        None
                    }
                    openagents_chat_app::cards::Effect::GymCoder { run, prompt } => {
                        self.rows_dirty = true;
                        self.gym_coder(&run, &prompt);
                        None
                    }
                    openagents_chat_app::cards::Effect::GymCommand {
                        host,
                        task,
                        text,
                        stop,
                    } => {
                        self.rows_dirty = true;
                        self.gym_command(&host, &task, &text, stop)
                    }
                    openagents_chat_app::cards::Effect::OpenCoder { host, task } => {
                        self.rows_dirty = true;
                        // A Gym run on this computer is the Coder run in
                        // its own chat.
                        if host == coder_run::LOCAL && self.runs.contains_key(&task) {
                            let previous = self.session.selected.clone();
                            if self.session.select(&task) {
                                self.selected_changed(previous);
                            }
                        } else {
                            self.notice = Some("Open this Coder chat on your phone.".into());
                        }
                        None
                    }
                    // A Gym sheet opened or closed, or its state changed.
                    openagents_chat_app::cards::Effect::None => {
                        self.rows_dirty = true;
                        None
                    }
                }
            }
            Action::AttachImage => {
                self.import_image(crate::chat_images::Source::Picker);
                None
            }
            Action::PasteImage => {
                self.import_image(crate::chat_images::Source::Clipboard);
                None
            }
            Action::RemoveImage { id: image } => {
                self.session.images.remove(&id, &image);
                self.notice = None;
                None
            }
            Action::Send => {
                // Text only while attachments are off: a restored draft
                // that still holds images sends its words alone.
                self.text_only();
                if self.task().is_some() {
                    return self.task_action(TaskAction::Send, view, now);
                }
                // While Coder works, or asks, the message is Coder's; once
                // its turn has ended the router reads it, with the run's
                // result as context (#10094).
                if self.run().is_some_and(|run| !run.routes_followups()) {
                    return self.run_action(RunAction::Send, view);
                }
                let field = self.field()?;
                let stamp = field.draft.stamp().ok()?;
                let submission = field.draft.submission(view, &stamp, None).ok()?;
                let id = self.session.selected.clone()?;
                let send_id = uuid::Uuid::new_v4().simple().to_string();
                // Only the words go to the router; the draft's images stay
                // here, bound to this message, for the Coder start its
                // reply may lead to.
                let command = self
                    .session
                    .submit(send_id.clone(), submission.text.clone())?;
                if self.session.images.bound(&id) == Some(send_id.as_str()) {
                    self.notice = Some(openagents_chat_app::attachments::HELD_FOR_CODER.into());
                }
                self.sent.insert((id.clone(), send_id.clone()));
                self.submissions.insert(send_id, (id, submission));
                Some(request(command))
            }
            Action::Pin => {
                let pinned = !self
                    .session
                    .summaries
                    .iter()
                    .find(|row| row.id == id)
                    .is_some_and(|row| row.pinned);
                Some(self.request(Command::Pin { chat: id, pinned }))
            }
            Action::Rename => {
                let title = self
                    .session
                    .summaries
                    .iter()
                    .find(|row| row.id == id)
                    .map_or("", |row| row.title.as_str())
                    .to_owned();
                if let Some(field) = self.field() {
                    field.focused = false;
                }
                self.search.focused = false;
                let mut field = chat_field("Chat title");
                field.set_unframed(true);
                field
                    .set_metrics(rust_native_desktop::composer::field::Metrics {
                        font_size: 14.0,
                        line_height: 22.75,
                        padding: [0.0; 4],
                        min_height: 22.75,
                        max_height: 136.5,
                    })
                    .expect("valid rename field metrics");
                if let Some(wake) = self.waker.clone() {
                    field.start(wake);
                }
                field.focused = true;
                // The initial title mounts with the next semantic view.
                self.rename_focus = 0;
                self.feedback = None;
                self.rename = Some((title, field));
                None
            }
            Action::CancelRename => {
                self.close_overlay();
                None
            }
            Action::SaveName => {
                let title = self.rename.as_ref()?.1.text().to_owned();
                self.save_name(id, title)
            }
            Action::Stop => {
                if self.task().is_some() {
                    self.task_action(TaskAction::Stop, view, now)
                } else if self.run().is_some() {
                    self.run_action(RunAction::Stop, view)
                } else {
                    Some(self.request(Command::Stop { chat: id }))
                }
            }
            Action::Retry => {
                if self.task().is_some() {
                    self.task_action(TaskAction::Retry, view, now)
                } else if self.run().is_some() {
                    self.run_action(RunAction::Retry, view)
                } else {
                    self.session.retry().map(request)
                }
            }
            Action::Restore => Some(self.request(Command::Restore { chat: id })),
            Action::Archive => Some(self.request(Command::Archive { chat: id })),
            Action::Earlier => self.session.earlier().map(request),
            Action::Latest => {
                self.transcript.jump_to_tail();
                None
            }
            Action::Followup { text } => {
                let at = now.duration_since(self.born).as_millis() as u64;
                let field = self.field()?;
                let stamp = field.draft.stamp().ok()?;
                field
                    .draft
                    .apply(
                        &stamp,
                        rust_native_desktop::composer::Input::Paste(&text),
                        at,
                    )
                    .ok()?;
                field.focused = true;
                None
            }
        }
    }
    pub fn modal_root(&self) -> Option<&str> {
        if self.commands.kind.is_some() {
            Some("command-panel")
        } else if self.rename.is_some() {
            Some("chat-rename-controls")
        } else {
            None
        }
    }
    pub fn modal(&self) -> bool {
        self.commands.kind.is_some() || self.rename.is_some()
    }
    pub fn focus_composer(&mut self) {
        if let Some(field) = self.field() {
            field.focused = true;
        }
    }
    pub fn search_focused(&self) -> bool {
        self.search.focused
    }
    pub fn aux_focused(&self) -> bool {
        self.modal() || self.search.focused
    }
    /// The shared command registry as the window shows it now, for the
    /// native menu bar (`appmenu`), whose items run these same commands.
    pub fn command_registry(&self) -> Vec<openagents_chat_app::commands::Entry> {
        self.registry()
    }
    /// The selected chat's ID.
    pub fn selected_chat(&self) -> Option<&str> {
        self.session.selected.as_deref()
    }
    fn registry(&self) -> Vec<openagents_chat_app::commands::Entry> {
        let mut entries = self.full_registry();
        // The Verse, the Map, and Give feedback show only in a preview
        // build (#11120).
        entries.retain(|entry| crate::preview::shows_entry(&entry.key, self.preview));
        entries
    }
    fn full_registry(&self) -> Vec<openagents_chat_app::commands::Entry> {
        if self.commands.kind == Some(openagents_chat_app::commands::Kind::Profile) {
            return openagents_chat_app::commands::profile_registry();
        }
        let mut entries = vec![];
        if self.commands.kind == Some(openagents_chat_app::commands::Kind::Menu)
            && self.feedback_offer.is_some()
        {
            entries.push(openagents_chat_app::commands::feedback_entry());
        }
        entries.extend(openagents_chat_app::commands::registry(
            &self.session.summaries,
            if self.saved_visible {
                None
            } else {
                self.session.selected.as_deref()
            },
            !self.saved_visible && self.busy(),
        ));
        entries
    }
    /// The open overlay's entries.
    fn command_entries(&self) -> Vec<openagents_chat_app::commands::Entry> {
        command_panel::limit(
            self.commands.kind.as_ref(),
            self.commands.entries(&self.registry()),
        )
    }
    /// The palette results' greatest height at the current window height.
    fn palette_results_height(&self) -> f32 {
        command_panel::results_height(self.viewport.1)
    }
    /// Each palette row's scroll extent and the content height, in points.
    fn palette_extents(
        &self,
        entries: &[openagents_chat_app::commands::Entry],
    ) -> (Vec<(f32, f32)>, f32) {
        command_panel::extents(entries, |entry| self.palette_history(entry).is_some())
    }
    fn palette_history(
        &self,
        entry: &openagents_chat_app::commands::Entry,
    ) -> Option<&openagents_chat::basic_chats::Summary> {
        let openagents_chat_app::commands::Action::Switch(id) = &entry.action else {
            return None;
        };
        self.session
            .summaries
            .iter()
            .find(|summary| &summary.id == id)
    }
    /// Scrolls the palette results by a wheel over them. Rows move beneath a
    /// resting pointer without taking its selection, as in the reference.
    pub fn wheel_commands(&mut self, point: (f32, f32), dy: f32) -> bool {
        if self.commands.kind != Some(openagents_chat_app::commands::Kind::Palette) {
            return false;
        }
        let scale = self.viewport.2.max(0.01);
        let inside = match self.command_band {
            (Some(top), Some(bottom)) => {
                point.0 >= top.x / scale
                    && point.0 < (top.x + top.w) / scale
                    && point.1 >= (top.y + top.h) / scale
                    && point.1 < bottom.y / scale
            }
            _ => false,
        };
        if inside {
            let (_, full) = self.palette_extents(&self.command_entries());
            if let Some(offset) =
                command_panel::wheel(self.command_offset, dy, full, self.palette_results_height())
            {
                self.command_offset = offset;
                return true;
            }
        }
        // The scrim owns the wheel: the conversation beneath never scrolls.
        false
    }
    pub fn commands_open(&self) -> bool {
        self.commands.kind.is_some()
    }
    fn open_commands(&mut self, kind: openagents_chat_app::commands::Kind) {
        let searchable = kind == openagents_chat_app::commands::Kind::Palette;
        self.menu_point = None;
        self.menu_navigation = false;
        self.command_offset = 0.0;
        self.command_reveal = false;
        self.rename = None;
        self.search.focused = false;
        if let Some(field) = self.field() {
            field.focused = false;
        }
        self.feedback_offer = None;
        if kind == openagents_chat_app::commands::Kind::Menu && !self.saved_visible {
            let text = self.transcript.selected_text();
            if !text.trim().is_empty() {
                self.feedback_offer = Some((text, self.transcript.selected_row_key()));
            }
        }
        self.commands.open(kind);
        self.command_token = uuid::Uuid::new_v4().simple().to_string();
        self.command_query = chat_field(command_panel::QUERY_PLACEHOLDER);
        self.command_query.focused = searchable;
        self.command_query.set_unframed(true);
        self.command_query
            .set_metrics(rust_native_desktop::composer::field::Metrics {
                font_size: 14.0,
                line_height: 22.0,
                padding: [3.0, 0.0, 3.0, 0.0],
                min_height: 28.0,
                max_height: 28.0,
            })
            .expect("valid command search metrics");
        if let Some(wake) = self.waker.clone() {
            self.command_query.start(wake);
        }
    }
    fn close_overlay(&mut self) {
        self.commands.close();
        self.command_band = (None, None);
        self.command_rows.clear();
        self.command_query.focused = false;
        self.rename = None;
        self.rename_pending = None;
        self.feedback = None;
        if let Some(field) = self.field() {
            field.focused = true;
        }
    }
    pub fn hover_command(&mut self, target: Option<&str>) -> bool {
        use openagents_chat_app::commands::Kind;
        if !matches!(
            self.commands.kind,
            Some(Kind::Palette | Kind::Menu | Kind::Profile)
        ) {
            return false;
        }
        let Some(key) = target.and_then(|key| key.strip_prefix("command-")) else {
            return false;
        };
        let Some(&index) = self.command_rows.get(key) else {
            return false;
        };
        let changed = self.commands.selected != index
            || (self.commands.kind != Some(Kind::Palette) && !self.menu_navigation);
        self.commands.selected = index;
        self.menu_navigation = true;
        changed
    }
    pub fn anchor_context_menu(&mut self, point: (f32, f32)) {
        if point.0.is_finite() && point.1.is_finite() {
            self.menu_point = Some(point);
        }
    }
    pub fn allows_focus(&self, key: &str) -> bool {
        if self.commands.kind.is_some() {
            key.starts_with("command-")
        } else if self.rename.is_some() {
            matches!(key, "chat-save-name" | "chat-cancel-name")
        } else {
            true
        }
    }
    pub fn pointer_down(&mut self, target: Option<&str>, point: (f32, f32)) -> bool {
        if !self.modal() {
            return false;
        }
        if target.is_some_and(|key| self.allows_focus(key)) {
            return false;
        }
        let inside_field = (self.rename.is_some()
            || self.commands.kind == Some(openagents_chat_app::commands::Kind::Palette))
            && self.aux_rect.is_some_and(|rect| {
                let scale = self.viewport.2;
                point.0 >= rect.x / scale
                    && point.0 < (rect.x + rect.w) / scale
                    && point.1 >= rect.y / scale
                    && point.1 < (rect.y + rect.h) / scale
            });
        if inside_field {
            return false;
        }
        if self.rename.is_some() {
            // Zeron's dialog scrim consumes outside presses. Only Cancel,
            // Escape, or a successful rename dismisses this dialog.
            return true;
        }
        self.close_overlay();
        true
    }
    pub fn tooltip(&self, key: &str) -> Option<String> {
        if self.modal() {
            return None;
        }
        match key {
            "chat-latest" => return Some("Scroll to bottom".into()),
            "chat-attach" => return Some("Attach image".into()),
            "chat-paste-image" => return Some("Paste image or text".into()),
            "chat-menu" => return Some("Chat actions · Shift+F10".into()),
            "chat-send" => {
                return Some(format!(
                    "{} · Enter",
                    self.task().map_or("Send", |task| match task.mode() {
                        openagents_chat_app::coder_tab::Mode::Queue => "Queue",
                        openagents_chat_app::coder_tab::Mode::Answer => "Answer",
                        _ => "Send",
                    })
                ));
            }
            _ => {}
        }
        let id = match key {
            "sidebar-new-chat" | "shell-new-chat" => "new",
            "chat-stop" => "stop",
            "sidebar-settings" => "settings",
            "chat-rename-start" => "rename",
            "chat-pin" => "pin",
            "chat-archive" => "archive",
            _ => return None,
        };
        self.registry()
            .into_iter()
            .find(|e| e.key == id)
            .map(|e| format!("{} · {}", e.label, e.hint))
    }
    pub fn shortcut(
        &mut self,
        event: &TextInput<'_>,
        view: &ValidatedView<Intent>,
        now: Instant,
    ) -> bool {
        let TextInput::Key {
            key,
            command,
            control,
            shift,
            ..
        } = event
        else {
            return false;
        };
        use openagents_chat_app::commands::Scope;
        let scope = if self.modal() {
            Scope::Overlay
        } else if self
            .field()
            .is_some_and(|f| f.draft.editor().is_some_and(|e| e.is_composing()))
            || (self.search.focused && self.search.draft.editor().is_some_and(|e| e.is_composing()))
        {
            Scope::Composing
        } else if self.field().is_some_and(|f| f.focused) || self.search.focused {
            Scope::Editor
        } else {
            Scope::Window
        };
        if !*command
            && !*control
            && matches!(scope, Scope::Editor | Scope::Window)
            && self.decision_number(key, view, now)
        {
            return true;
        }
        let Some(action) =
            openagents_chat_app::commands::shortcut(key, *command, *control, *shift, scope)
        else {
            return false;
        };
        if self.saved_visible
            && matches!(
                action,
                openagents_chat_app::commands::Action::Stop
                    | openagents_chat_app::commands::Action::Rename
                    | openagents_chat_app::commands::Action::Pin
                    | openagents_chat_app::commands::Action::Archive
                    | openagents_chat_app::commands::Action::Restore
            )
        {
            return false;
        }
        self.run_command(action, view, now);
        true
    }
    /// Number key `key`, 1 to 9, picks that option of the selected chat's
    /// decision panel while Coder waits on a question or an approval and
    /// the draft is empty (#10469). Any other key, or a number the page
    /// does not list, types as usual.
    fn decision_number(&mut self, key: &str, view: &ValidatedView<Intent>, now: Instant) -> bool {
        use openagents_chat_app::decision::Control;
        let number = match key.as_bytes() {
            [digit @ b'1'..=b'9'] => usize::from(*digit - b'0'),
            _ => return false,
        };
        if self.modal() || !self.draft().is_empty() {
            return false;
        }
        let request = if self
            .task()
            .and_then(task_chat::Session::decision)
            .is_some_and(|flow| flow.takes_number(number))
        {
            self.task_action(TaskAction::Decide(Control::Pick(number - 1)), view, now)
        } else if self
            .run()
            .and_then(Run::decision)
            .is_some_and(|flow| flow.takes_number(number))
        {
            self.run_action(RunAction::Decide(Control::Pick(number - 1)), view)
        } else {
            return false;
        };
        self.rows_dirty = true;
        if let Some(request) = request {
            self.queued.push(request);
        }
        true
    }
    fn run_command(
        &mut self,
        action: openagents_chat_app::commands::Action,
        view: &ValidatedView<Intent>,
        now: Instant,
    ) {
        use openagents_chat_app::commands::Action as C;
        let request = match action {
            C::NewChat => Some(self.new_chat()),
            // With too few chats for the sidebar's filter (#10072), the
            // palette searches them instead.
            C::Search
                if self.session.summaries.len() < crate::chrome::SEARCH_MIN_CHATS
                    && self.search.text().is_empty() =>
            {
                self.open_commands(openagents_chat_app::commands::Kind::Palette);
                None
            }
            C::Search => {
                if let Some(field) = self.field() {
                    field.focused = false;
                }
                self.search.focused = true;
                None
            }
            C::Computers => {
                self.navigation = Some(openagents_chat::router::Screen::Computers);
                None
            }
            C::Grid => {
                self.navigation = Some(openagents_chat::router::Screen::VerseGym);
                None
            }
            C::Saved => {
                self.desktop_navigation = Some(crate::chrome::Action::Saved);
                None
            }
            C::Map => {
                self.desktop_navigation = Some(crate::chrome::Action::Map);
                None
            }
            C::Settings => {
                self.navigation = Some(openagents_chat::router::Screen::Keys);
                None
            }
            C::Palette => {
                self.open_commands(openagents_chat_app::commands::Kind::Palette);
                None
            }
            C::Menu => {
                self.open_commands(openagents_chat_app::commands::Kind::Menu);
                None
            }
            C::Switch(id) => {
                if self.session.summaries.iter().any(|s| s.id == id) {
                    self.select(&id);
                    Some(self.request(Command::Read {
                        chat: id,
                        before: None,
                    }))
                } else {
                    None
                }
            }
            C::Cycle(back) => {
                let rows = openagents_chat_app::chat_list::search(&self.session.summaries, "");
                if rows.is_empty() {
                    None
                } else {
                    let at = rows
                        .iter()
                        .position(|r| Some(&r.id) == self.session.selected.as_ref())
                        .unwrap_or(0);
                    let id = rows[if back {
                        (at + rows.len() - 1) % rows.len()
                    } else {
                        (at + 1) % rows.len()
                    }]
                    .id
                    .clone();
                    self.select(&id);
                    Some(self.request(Command::Read {
                        chat: id,
                        before: None,
                    }))
                }
            }
            C::Stop if self.busy() || self.task().is_some_and(|task| task.active()) => {
                self.action(Action::Stop, view, now)
            }
            C::Rename => self.action(Action::Rename, view, now),
            C::Pin => self.action(Action::Pin, view, now),
            C::Archive => self.action(Action::Archive, view, now),
            C::Restore => self.action(Action::Restore, view, now),
            C::Dismiss => {
                self.close_overlay();
                None
            }
            C::Feedback => {
                self.open_feedback();
                None
            }
            C::Stop => None,
        };
        if let Some(request) = request {
            self.queued.push(request);
        }
    }

    fn save_name(&mut self, chat: String, title: String) -> Option<Request> {
        if self.rename_pending.is_some() {
            return None;
        }
        let request = self.request(Command::Rename {
            chat: chat.clone(),
            title: title.clone(),
        });
        if let Request::Chat { ticket, .. } = &request {
            self.rename_pending = Some((*ticket, chat, title));
        }
        Some(request)
    }
    pub fn input(&mut self, event: TextInput<'_>, now: Instant) -> FieldAction {
        if self.saved_visible && !self.modal() && !self.search.focused {
            return FieldAction::Unhandled;
        }
        let at = now.duration_since(self.born).as_millis() as u64;
        if let TextInput::Key { key, .. } = &event {
            let composing =
                if self.commands.kind == Some(openagents_chat_app::commands::Kind::Palette) {
                    Some(&mut self.command_query)
                } else {
                    self.rename.as_mut().map(|(_, field)| field)
                };
            if let Some(field) = composing.filter(|field| {
                field.focused
                    && field
                        .draft
                        .editor()
                        .is_some_and(|editor| editor.is_composing())
            }) {
                if *key == "Escape" {
                    field.input(TextInput::CancelComposition, at);
                } else {
                    field.input(event, at);
                }
                if self.commands.kind == Some(openagents_chat_app::commands::Kind::Palette) {
                    self.commands.query = self.command_query.text().to_owned();
                    self.commands.selected = 0;
                    self.command_offset = 0.0;
                }
                return FieldAction::Edited;
            }
        }
        if let TextInput::Key { key: "Escape", .. } = &event
            && self.modal()
        {
            self.close_overlay();
            return FieldAction::Edited;
        }
        if self.commands.kind.is_some() {
            if let TextInput::Key {
                key,
                shift,
                command,
                ..
            } = &event
            {
                if *command && matches!(*key, "q" | "w") {
                    return FieldAction::Unhandled;
                }
                if matches!(*key, "Tab" | "ArrowDown" | "ArrowUp") {
                    self.menu_navigation = true;
                    let entries = self.command_entries();
                    self.commands
                        .navigate_entries(*key == "ArrowUp" || (*key == "Tab" && *shift), &entries);
                    self.command_reveal = true;
                    return FieldAction::Edited;
                }
                if *key == "Enter" {
                    let entries = self.command_entries();
                    if let Some(entry) = entries.get(self.commands.selected).filter(|e| e.enabled) {
                        self.activated.push(format!("command:{}", entry.key));
                    }
                    return FieldAction::Edited;
                }
            }
            if self.commands.kind != Some(openagents_chat_app::commands::Kind::Palette) {
                // Menus have no text field. Native text and IME events must not
                // filter their rows through the palette's hidden editor.
                return FieldAction::Edited;
            }
            let result = self.command_query.input(event, at);
            let query = self.command_query.text().to_owned();
            if self.commands.query != query {
                self.commands.query = query;
                self.commands.selected = 0;
                self.command_offset = 0.0;
            }
            return if result == FieldAction::Unhandled {
                FieldAction::Edited
            } else {
                result
            };
        }
        if self.search.focused {
            let result = self.search.input(event, at);
            return if result == FieldAction::Send {
                FieldAction::Edited
            } else {
                result
            };
        }
        if self.rename.is_some() {
            if let TextInput::Key {
                key: "Tab", shift, ..
            } = &event
            {
                self.rename_focus = (self.rename_focus + if *shift { 2 } else { 1 }) % 3;
                if let Some((_, field)) = &mut self.rename {
                    field.input(TextInput::FocusLost, at);
                    field.focused = self.rename_focus == 0;
                }
                return FieldAction::Edited;
            }
            if let TextInput::Key { key: "Enter", .. } = &event
                && self.rename_focus == 1
            {
                self.close_overlay();
                return FieldAction::Edited;
            }
        }
        if let Some((_, field)) = &mut self.rename {
            let result = if self.rename_focus == 2
                && matches!(&event, TextInput::Key { key: "Enter", .. })
            {
                FieldAction::Send
            } else {
                field.input(event, at)
            };
            if result == FieldAction::Send && self.feedback.is_some() {
                self.send_feedback();
                return FieldAction::Edited;
            }
            if result == FieldAction::Send {
                if let Some(chat) = self.session.selected.clone() {
                    let title = field.text().to_owned();
                    if let Some(request) = self.save_name(chat, title) {
                        self.queued.push(request);
                    }
                }
                return FieldAction::Edited;
            }
            if result != FieldAction::Unhandled {
                return result;
            }
            return FieldAction::Unhandled;
        }
        if let TextInput::Key {
            key, command: true, ..
        } = &event
            && matches!(*key, "v" | "V")
            // While attachments are off the field pastes text only.
            && self.attachments
            && self.field().is_some_and(|field| field.focused)
        {
            self.import_image(crate::chat_images::Source::Clipboard);
            return FieldAction::Edited;
        }
        if let TextInput::Key {
            key, command: true, ..
        } = &event
            && matches!(*key, "c" | "C")
            && self.field().is_none_or(|field| !field.focused)
        {
            let text = self.transcript.selected_text();
            if !text.is_empty() {
                let _ = std::thread::Builder::new()
                    .name("transcript-copy".into())
                    .spawn(move || {
                        rust_native_desktop::input::copy(&text);
                    });
                return FieldAction::Edited;
            }
        }
        let at = now.duration_since(self.born).as_millis() as u64;
        self.field()
            .map_or(FieldAction::Unhandled, |field| field.input(event, at))
    }
    pub fn surface(&mut self, resource: &str, event: SurfaceInput, now: Instant) -> bool {
        if matches!(event, SurfaceInput::Down { .. }) {
            let at = now.duration_since(self.born).as_millis() as u64;
            if resource != SEARCH {
                self.search.input(TextInput::FocusLost, at);
            }
            if resource != RENAME
                && let Some((_, field)) = &mut self.rename
            {
                field.input(TextInput::FocusLost, at);
            }
            if resource != COMPOSER
                && let Some(field) = self.field()
            {
                field.input(TextInput::FocusLost, at);
            }
        }
        if resource == COMMAND_QUERY || resource == SEARCH || resource == RENAME {
            let field = if resource == COMMAND_QUERY {
                Some(&mut self.command_query)
            } else if resource == SEARCH {
                Some(&mut self.search)
            } else {
                self.rename.as_mut().map(|(_, f)| f)
            };
            if let Some(field) = field {
                if matches!(event, SurfaceInput::Move { .. }) && !field.dragging() {
                    return false;
                }
                field.pointer(
                    event,
                    &mut self.fonts,
                    now.duration_since(self.born).as_millis() as u64,
                );
                return true;
            }
            return false;
        }
        if resource == TRANSCRIPT {
            let version = self.transcript.version();
            let moved = matches!(event, SurfaceInput::Move { .. });
            let at_ms = now.saturating_duration_since(self.born).as_millis() as u64;
            if matches!(event, SurfaceInput::Down { .. }) {
                self.press_revision = Some((
                    self.session.revision,
                    self.task().map_or(0, |task| task.revision),
                ));
            }
            let released = match event {
                SurfaceInput::Up { x, y } => Some((x, y)),
                _ => None,
            };
            if matches!(event, SurfaceInput::Down { .. })
                && let Some(field) = self.field()
            {
                field.input(TextInput::FocusLost, at_ms);
            }
            if let Some(action) = self.transcript.pointer(event, &mut self.fonts) {
                let destination = match action {
                    rust_native_desktop::transcript::Action::Activate(key) => {
                        if let Some((x, y)) = released {
                            self.transcript.release_pressed_button(x, y);
                        }
                        let press = self.press.take();
                        if self.press_revision.take()
                            == Some((
                                self.session.revision,
                                self.task().map_or(0, |task| task.revision),
                            ))
                            || press.is_some_and(|press| self.late_release(&press, &key))
                        {
                            self.activated.push(key);
                        }
                        return true;
                    }
                    rust_native_desktop::transcript::Action::Copy(text) => {
                        let _ =
                            std::thread::Builder::new()
                                .name("code-copy".into())
                                .spawn(move || {
                                    rust_native_desktop::input::copy(&text);
                                });
                        return true;
                    }
                    rust_native_desktop::transcript::Action::OpenLink(destination) => destination,
                    rust_native_desktop::transcript::Action::Earlier => return true,
                };
                // Only an explicit pointer release opens an HTTP(S) destination.
                open_link(&destination);
            }
            // A press on a button remembers what it did and to what.
            if matches!(event, SurfaceInput::Down { .. }) {
                let revision = self.shown();
                self.press = self
                    .transcript
                    .pressed_button()
                    .map(str::to_owned)
                    .map(|key| rust_native::Press::new(revision, &key, self.pressed(&key)));
            }
            // The press's row was replaced while the button was held (a
            // Coder event landed): the same control, for the same target,
            // still runs on the next revision when it must work while
            // events stream.
            if let Some((x, y)) = released
                && let Some(key) = self.transcript.release_pressed_button(x, y)
                && let Some(press) = self.press.take()
                && self.late_release(&press, &key)
            {
                self.press_revision = None;
                self.activated.push(key);
                return true;
            }
            return !moved || version != self.transcript.version();
        }
        if resource == COMPOSER {
            if matches!(event, SurfaceInput::Move { .. })
                && self
                    .session
                    .selected
                    .as_ref()
                    .and_then(|id| self.fields.get(id))
                    .is_none_or(|field| !field.dragging())
            {
                return false;
            }
            let at = now.duration_since(self.born).as_millis() as u64;
            if let Some(id) = self.session.selected.clone()
                && let Some(field) = self.fields.get_mut(&id)
            {
                field.pointer(event, &mut self.fonts, at);
            }
            return true;
        }
        if resource == CHANGES {
            if let SurfaceInput::Wheel { dy, .. } = event {
                let viewport = self.changes_viewport();
                let limit = self
                    .reviewer()
                    .and_then(openagents_chat_app::changes::Reviewer::document)
                    .map_or(0.0, |doc| doc.scroll_limit(viewport, CHANGES_LINE));
                self.changes_scroll = (self.changes_scroll - dy).clamp(0.0, limit);
            }
            return !matches!(event, SurfaceInput::Move { .. });
        }
        false
    }
    pub fn next_wake(&self, now: Instant) -> Instant {
        let wake = self.task().map_or_else(
            || self.session.next_wake(now),
            |task| self.session.next_wake(now).min(task.next_wake(now)),
        );
        // A hosted Gym run is read again each second, beside its wakes; a
        // run waiting for its trainer key starts the read on the next tick.
        let gym = &self.session.cards.gym;
        #[cfg(not(windows))]
        let read_key = self
            .gym_trainer
            .as_ref()
            .is_some_and(|trainer| trainer.to_start(gym));
        #[cfg(windows)]
        let read_key = false;
        let wake = if read_key {
            now
        } else if gym.active().is_some_and(|run| {
            matches!(
                run.place,
                Some(openagents_chat_app::gym::Place::Hosted { .. })
            )
        }) {
            wake.min(now + std::time::Duration::from_secs(1))
        } else {
            wake
        };
        self.runs
            .values()
            .map(|run| run.next_wake(now))
            .fold(wake, Instant::min)
    }
    pub fn version(&self, resource: &str) -> Option<u64> {
        match resource {
            COMMAND_QUERY => Some(self.command_query.version()),
            resource if command_panel::is_surface(resource) => Some(0),
            SEARCH => Some(self.search.version()),
            RENAME => self.rename.as_ref().map(|(_, field)| field.version()),
            TRANSCRIPT => Some(self.transcript.version()),
            CHANGES => Some(
                (self.changes_scroll.to_bits() as u64)
                    ^ u64::from(u8::from(self.changes_open))
                    ^ self.reviewer().map_or(0, |reviewer| {
                        reviewer.revision()
                            ^ reviewer.document().map_or(0, |doc| doc.len() as u64) << 32
                    }),
            ),
            COMPOSER => self
                .session
                .selected
                .as_ref()
                .and_then(|id| self.fields.get(id))
                .map(Field::version),
            resource if resource.starts_with("image:") => self
                .session
                .selected
                .as_ref()
                .and_then(|id| {
                    self.session
                        .images
                        .get(id)
                        .iter()
                        .find(|image| resource == format!("image:{}", image.id))
                })
                .map(|_| 1),
            _ => None,
        }
    }
    fn image_height(&self, available: f32) -> f32 {
        let count = self
            .session
            .selected
            .as_ref()
            .map_or(0, |id| self.session.images.get(id).len());
        if count == 0 {
            return 0.0;
        }
        let columns = ((available + 8.0) / 128.0).floor().max(1.0) as usize;
        count.div_ceil(columns) as f32 * 112.0
    }
    pub fn size(&self, resource: &str, available: f32) -> Option<(f32, f32)> {
        let composer_height = self
            .session
            .selected
            .as_ref()
            .and_then(|id| self.fields.get(id))
            .map_or(56.0, |field| field.height(available));
        match resource {
            SEARCH => Some((available, 28.0)),
            COMMAND_QUERY => Some((available, 28.0)),
            resource if command_panel::is_surface(resource) => {
                command_panel::surface_size(resource, available, cfg!(target_os = "macos"))
            }
            RENAME => self
                .rename
                .as_ref()
                .map(|(_, field)| (available, field.height(available))),
            TRANSCRIPT => Some((
                available,
                (self.viewport.1
                    - if available < 620.0 { 288.0 } else { 240.0 }
                    - composer_height
                    - self.image_height(available))
                .max(40.0),
            )),
            CHANGES => Some((available, self.changes_viewport())),
            COMPOSER => Some((available, composer_height)),
            resource if resource.starts_with("image:") => self
                .session
                .selected
                .as_ref()
                .and_then(|id| {
                    self.session
                        .images
                        .get(id)
                        .iter()
                        .find(|image| resource == format!("image:{}", image.id))
                })
                .map(|image| (image.preview_width as f32, image.preview_height as f32)),
            _ => None,
        }
    }
    pub fn paint(&mut self, resource: &str, frame: &mut Frame, rect: PxRect) -> bool {
        let scale = self.viewport.2;
        if resource == command_panel::RULE_HEADER {
            self.command_band.0 = Some(rect);
        } else if resource == command_panel::RULE_FOOTER {
            self.command_band.1 = Some(rect);
        }
        if matches!(
            resource,
            command_panel::RULE | command_panel::RULE_HEADER | command_panel::RULE_FOOTER
        ) {
            frame.fill(rect, 0.0, command_panel::rule_color());
            return true;
        }
        if resource == command_panel::SHORTCUT_GLYPH {
            rust_native_desktop::paint::keycap(
                frame,
                &mut self.fonts,
                rect,
                scale,
                &command_panel::shortcut_parts(cfg!(target_os = "macos")),
                (
                    command_panel::KEYCAP_RADIUS,
                    5.0,
                    command_panel::KEYCAP_LINE_HEIGHT,
                ),
                command_panel::keycap_fill(),
                openagents_chat_app::visual::current().muted,
            );
            return true;
        }
        if resource == command_panel::SEARCH_GLYPH {
            rust_native_desktop::paint_icon(
                frame,
                rect,
                Glyph::Search,
                rust_native_desktop::theme::IconSet::Solar,
                openagents_chat_app::visual::current().muted,
            );
            return true;
        }
        if resource == COMMAND_QUERY || resource == SEARCH || resource == RENAME {
            let field = if resource == COMMAND_QUERY {
                Some(&mut self.command_query)
            } else if resource == SEARCH {
                Some(&mut self.search)
            } else {
                self.rename.as_mut().map(|(_, f)| f)
            };
            if let Some(field) = field {
                field.paint(frame, rect, scale, &mut self.fonts);
                self.aux_rect = Some(rect);
                return true;
            }
        }
        if let Some(image) = self.session.selected.as_ref().and_then(|id| {
            self.session
                .images
                .get(id)
                .iter()
                .find(|image| resource == format!("image:{}", image.id))
        }) {
            if let Ok(preview) = rust_native_desktop::image::Image::from_rgba(
                image.preview_width,
                image.preview_height,
                image.preview.to_vec(),
            ) {
                let size = PxRect {
                    w: image.preview_width as f32 * scale,
                    h: image.preview_height as f32 * scale,
                    ..rect
                };
                rust_native_desktop::image::paint(frame, &preview, size);
            }
            return true;
        }
        if resource == TRANSCRIPT {
            let size = (rect.w / scale, rect.h / scale);
            if size != self.transcript_size {
                if let Err(error) =
                    self.transcript
                        .update_shared(self.transcript_rows.clone(), size.0, size.1)
                {
                    self.session.error = Some(format!("Couldn't lay out conversation: {error}"));
                    #[cfg(test)]
                    panic!("chat layout failed: {error}");
                }
                self.transcript_size = size;
            }
            self.transcript.paint(frame, rect, scale, &mut self.fonts);
            return true;
        }
        if resource == CHANGES {
            self.paint_changes(frame, rect, scale);
            return true;
        }
        if resource == COMPOSER {
            self.composer_rect = Some(rect);
            if let Some(id) = &self.session.selected
                && let Some(field) = self.fields.get_mut(id)
            {
                field.paint(frame, rect, scale, &mut self.fonts);
            }
            return true;
        }
        false
    }
    pub fn cursor(&self) -> Option<(f64, f64)> {
        if self.commands.kind.is_some()
            && self.commands.kind != Some(openagents_chat_app::commands::Kind::Palette)
        {
            return None;
        }
        let aux = if self.commands.kind.is_some() {
            Some(&self.command_query)
        } else if self.search.focused {
            Some(&self.search)
        } else {
            self.rename.as_ref().map(|(_, f)| f).filter(|f| f.focused)
        };
        if let Some(field) = aux {
            let rect = self.aux_rect?;
            let scale = self.viewport.2;
            return Some((
                (rect.x / scale + field.caret.0) as f64,
                (rect.y / scale + field.caret.1 + 20.0) as f64,
            ));
        }
        let field = self
            .session
            .selected
            .as_ref()
            .and_then(|id| self.fields.get(id))?;
        if !field.focused {
            return None;
        }
        let rect = self.composer_rect?;
        let scale = self.viewport.2;
        Some((
            (rect.x / scale + field.caret.0) as f64,
            (rect.y / scale + field.caret.1 + 20.0) as f64,
        ))
    }
    pub fn overlay_layout(&self) -> Option<rust_native_desktop::OverlayLayout> {
        use openagents_chat_app::commands::Kind;
        use rust_native_desktop::{OverlayLayout, OverlayPlacement};
        if let Some(kind) = &self.commands.kind {
            Some(OverlayLayout {
                width: command_panel::width(kind, self.sidebar_width),
                placement: if *kind == Kind::Menu {
                    self.menu_point.map_or(
                        OverlayPlacement::TopRight { top: 40, right: 10 },
                        |(x, y)| OverlayPlacement::At { x, y },
                    )
                } else if *kind == Kind::Profile {
                    OverlayPlacement::Above {
                        anchor: "sidebar-footer",
                        gap: 8,
                    }
                } else {
                    OverlayPlacement::Center
                },
                scrim: command_panel::scrim(kind),
            })
        } else if self.rename.is_some() {
            Some(OverlayLayout {
                width: if self.feedback.is_some() { 440 } else { 360 },
                placement: OverlayPlacement::Center,
                scrim: Some(Color {
                    alpha: 89,
                    ..Color::rgb(0, 0, 0)
                }),
            })
        } else if !self.saved_visible && !self.modal() && !self.transcript.at_tail() {
            Some(OverlayLayout {
                width: 0,
                placement: OverlayPlacement::Above {
                    anchor: "chat-composer-card",
                    gap: 6,
                },
                scrim: None,
            })
        } else {
            None
        }
    }
    pub fn floating(&mut self) -> Option<Node<Intent>> {
        if self.commands.kind.is_some() {
            return Some(self.command_panel());
        }
        if self.feedback.is_some() && self.rename.is_some() {
            return Some(self.feedback_panel());
        }
        if self.rename.is_some() {
            return Some(self.rename_panel());
        }
        self.overlay_layout()?;
        let mut button = button("chat-latest", "Scroll to bottom", Action::Latest, true);
        if let Element::Button { icon, .. } = &mut button.element {
            *icon = Some(Icon {
                glyph: Glyph::ArrowDown,
                circular: false,
                pill: false,
            });
        }
        button.style.background = Some(openagents_chat_app::visual::current().raised);
        button.style.foreground = Some(openagents_chat_app::visual::current().text);
        button.style.weight = Some(TextWeight::Normal);
        button.style.glyph_size = Some(13);
        button.style.glyph_gap = Some(6);
        button.style.glyph_color = Some(openagents_chat_app::visual::current().muted);
        button.style.text_size = Some(13);
        button.style.line_height = Some(18);
        button.style.button_padding = Some([10, 5]);
        button.style.radius = Some(15);
        let mut pill = stack("chat-latest-pill", Axis::Vertical, vec![button]);
        pill.style.gap = Some(Space::None);
        pill.style.radius = Some(15);
        pill.style.border = Some(openagents_chat_app::visual::current().border);
        pill.style.padding_points = Some([0, 2, 0, 0]);
        pill.style.background = Some(openagents_chat_app::visual::current().raised);
        Some(pill)
    }
    /// Opens **Give feedback** on the selection the context menu offered:
    /// the rename dialog's field, empty, under the quoted text.
    fn open_feedback(&mut self) {
        let Some((text, row)) = self.feedback_offer.take() else {
            return;
        };
        let index = row
            .as_deref()
            .and_then(|row| openagents_chat_app::feedback::turn_index(row, "turn-"));
        let thread = self.session.selected.clone();
        let state = self.session.state();
        let selection = openagents_chat_app::feedback::selection(
            &text,
            thread.as_deref(),
            state.map_or(&[][..], |state| state.turns.as_slice()),
            state.map_or(0, |state| state.start),
            index,
        );
        if let Some(field) = self.field() {
            field.focused = false;
        }
        self.search.focused = false;
        let mut field = chat_field(playtest::feedback::PLACEHOLDER);
        field.set_unframed(true);
        field
            .set_metrics(rust_native_desktop::composer::field::Metrics {
                font_size: 14.0,
                line_height: 22.75,
                padding: [0.0; 4],
                min_height: 45.5,
                max_height: 136.5,
            })
            .expect("valid feedback field metrics");
        if let Some(wake) = self.waker.clone() {
            field.start(wake);
        }
        field.focused = true;
        self.rename_focus = 0;
        self.rename_pending = None;
        self.rename = Some((String::new(), field));
        self.feedback = Some(FeedbackDialog {
            selection,
            status: None,
            sending: None,
            sent: false,
        });
    }
    /// Files the open dialog's comment on its selection, on a thread.
    fn send_feedback(&mut self) {
        let comment = self
            .rename
            .as_ref()
            .map(|(_, field)| field.text().to_owned())
            .unwrap_or_default();
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let Some(dialog) = &mut self.feedback else {
            return;
        };
        if dialog.sent || dialog.sending.is_some() {
            return;
        }
        let report = match playtest::feedback::report(
            crate::feedback::context(at),
            dialog.selection.clone(),
            &comment,
        ) {
            Ok(report) => report,
            Err(why) => {
                dialog.status = Some(why);
                return;
            }
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let sender = self.feedback_sender.clone();
        let wake = self.waker.clone();
        dialog.status = None;
        dialog.sending = Some(rx);
        let _ = std::thread::Builder::new()
            .name("feedback".into())
            .spawn(move || {
                let _ = tx.send(sender(report));
                if let Some(wake) = wake {
                    wake.wake();
                }
            });
    }
    /// Shows how a Send went once its thread answers.
    fn poll_feedback(&mut self) {
        let Some(dialog) = &mut self.feedback else {
            return;
        };
        let Some(result) = dialog.sending.as_ref().and_then(|rx| rx.try_recv().ok()) else {
            return;
        };
        dialog.sending = None;
        match result {
            Ok(said) => {
                dialog.sent = true;
                dialog.status = Some(said);
            }
            Err(why) => dialog.status = Some(why),
        }
        self.rows_dirty = true;
    }
    /// Whether the open feedback dialog finished sending; tests wait on it.
    pub fn feedback_status(&self) -> Option<(bool, Option<&str>)> {
        self.feedback
            .as_ref()
            .map(|dialog| (dialog.sent, dialog.status.as_deref()))
    }
    fn feedback_panel(&self) -> Node<Intent> {
        let visual = openagents_chat_app::visual::current();
        let (_, field) = self.rename.as_ref().expect("an open feedback dialog");
        let dialog = self.feedback.as_ref().expect("an open feedback dialog");
        let input = Node {
            key: "chat-rename".into(),
            style: Style::default(),
            element: Element::Composer {
                token: "feedback".into(),
                placeholder: playtest::feedback::PLACEHOLDER.into(),
                max_bytes: playtest::report::MAX_TEXT_CHARS * 4,
                enabled: !dialog.sent,
                busy: false,
                stop: None,
                choices: vec![],
                draft: Some(field.text().into()),
                focus: field.focused,
            },
        };
        let mut title = text(
            "chat-rename-title",
            playtest::feedback::BUTTON,
            TextRole::Body,
        );
        title.style.text_size = Some(15);
        title.style.line_height = Some(24);
        title.style.weight = Some(TextWeight::Semibold);
        let shown: String = dialog.selection.text.chars().take(280).collect();
        let shown = if shown.len() < dialog.selection.text.len() {
            format!("“{}…”", shown.trim_end())
        } else {
            format!("“{shown}”")
        };
        let mut quote = text("chat-feedback-quote", shown, TextRole::Body);
        quote.style.text_size = Some(13);
        quote.style.line_height = Some(20);
        quote.style.foreground = Some(visual.muted);
        let mut quote_frame = stack("chat-feedback-quote-frame", Axis::Vertical, vec![quote]);
        quote_frame.style.padding_points = Some([4, 0, 4, 12]);
        quote_frame.style.border = Some(Color {
            alpha: 20,
            ..visual.ink
        });
        let mut quote_margin = stack(
            "chat-feedback-quote-margin",
            Axis::Vertical,
            vec![quote_frame],
        );
        quote_margin.style.padding_points = Some([10, 0, 0, 0]);
        let mut field_frame = stack("chat-rename-field", Axis::Vertical, vec![input]);
        field_frame.style.padding_points = Some([8, 12, 8, 12]);
        field_frame.style.radius = Some(8);
        field_frame.style.background = Some(Color {
            alpha: 10,
            ..visual.ink
        });
        field_frame.style.border = Some(Color {
            alpha: 20,
            ..visual.ink
        });
        let mut field_margin = stack(
            "chat-rename-field-margin",
            Axis::Vertical,
            vec![field_frame],
        );
        field_margin.style.padding_points = Some([12, 0, 0, 0]);
        let mut cancel = button(
            "chat-cancel-name",
            if dialog.sent { "Close" } else { "Cancel" },
            Action::CancelRename,
            true,
        );
        let mut send = button(
            "chat-save-name",
            "Send",
            Action::SaveName,
            !dialog.sent && dialog.sending.is_none(),
        );
        for button in [&mut cancel, &mut send] {
            button.style.text_size = Some(13);
            button.style.line_height = Some(21);
            button.style.min_height = Some(33);
            button.style.button_padding = Some([12, 6]);
            button.style.radius = Some(8);
            button.style.intrinsic_width = Some(true);
        }
        cancel.style.background = Some(Color {
            alpha: 0,
            ..visual.text
        });
        cancel.style.foreground = Some(visual.muted);
        cancel.style.hover_background = Some(Color {
            alpha: 15,
            ..visual.ink
        });
        cancel.style.hover_foreground = Some(visual.text);
        send.style.background = Some(visual.text);
        send.style.foreground = Some(visual.on_text);
        send.style.weight = Some(TextWeight::Medium);
        send.style.hover_background = Some(visual.text_hover);
        let mut status = text(
            "chat-rename-space",
            dialog.status.clone().unwrap_or_default(),
            TextRole::Status,
        );
        status.style.foreground = Some(visual.muted);
        let mut buttons = stack(
            "chat-rename-buttons",
            Axis::Horizontal,
            vec![status, cancel, send],
        );
        buttons.style.gap_points = Some(8);
        buttons.style.padding_points = Some([16, 0, 0, 0]);
        let mut panel = stack(
            "chat-rename-controls",
            Axis::Vertical,
            vec![title, quote_margin, field_margin, buttons],
        );
        panel.style.background = Some(visual.panel);
        panel.style.border = Some(Color {
            alpha: 26,
            ..visual.ink
        });
        panel.style.radius = Some(16);
        panel.style.padding_points = Some([20; 4]);
        panel.style.gap = Some(Space::None);
        panel
    }
    fn rename_panel(&self) -> Node<Intent> {
        let visual = openagents_chat_app::visual::current();
        let (title, field) = self.rename.as_ref().expect("an open rename dialog");
        let input = Node {
            key: "chat-rename".into(),
            style: Style::default(),
            element: Element::Composer {
                token: format!("rename-{}", self.session.selected.as_deref().unwrap_or("")),
                placeholder: "Chat title".into(),
                max_bytes: 160,
                enabled: true,
                busy: false,
                stop: None,
                choices: vec![],
                draft: Some(if field.draft.editor().is_some() {
                    field.text().into()
                } else {
                    title.clone()
                }),
                focus: field.focused,
            },
        };
        let mut title = text("chat-rename-title", "Rename chat", TextRole::Body);
        title.style.text_size = Some(15);
        title.style.line_height = Some(24);
        title.style.weight = Some(TextWeight::Semibold);
        let mut field_frame = stack("chat-rename-field", Axis::Vertical, vec![input]);
        field_frame.style.padding_points = Some([8, 12, 8, 12]);
        field_frame.style.radius = Some(8);
        field_frame.style.background = Some(Color {
            alpha: 10,
            ..visual.ink
        });
        field_frame.style.border = Some(Color {
            alpha: 20,
            ..visual.ink
        });
        let mut field_margin = stack(
            "chat-rename-field-margin",
            Axis::Vertical,
            vec![field_frame],
        );
        field_margin.style.padding_points = Some([12, 0, 0, 0]);
        let mut cancel = button("chat-cancel-name", "Cancel", Action::CancelRename, true);
        let mut save = button(
            "chat-save-name",
            "Rename",
            Action::SaveName,
            self.rename_pending.is_none(),
        );
        for button in [&mut cancel, &mut save] {
            button.style.text_size = Some(13);
            button.style.line_height = Some(21);
            button.style.min_height = Some(33);
            button.style.button_padding = Some([12, 6]);
            button.style.radius = Some(8);
            button.style.intrinsic_width = Some(true);
        }
        cancel.style.background = Some(Color {
            alpha: 0,
            ..visual.text
        });
        cancel.style.foreground = Some(visual.muted);
        cancel.style.hover_background = Some(Color {
            alpha: 15,
            ..visual.ink
        });
        cancel.style.hover_foreground = Some(visual.text);
        save.style.background = Some(visual.text);
        save.style.foreground = Some(visual.on_text);
        save.style.weight = Some(TextWeight::Medium);
        save.style.hover_background = Some(visual.text_hover);
        let mut buttons = stack(
            "chat-rename-buttons",
            Axis::Horizontal,
            vec![
                text("chat-rename-space", "", TextRole::Status),
                cancel,
                save,
            ],
        );
        buttons.style.gap_points = Some(8);
        buttons.style.padding_points = Some([16, 0, 0, 0]);
        // Reimplemented from Zeron's public dialog primitives. The original
        // conversation stays mounted beneath the centered card.
        let mut panel = stack(
            "chat-rename-controls",
            Axis::Vertical,
            vec![title, field_margin, buttons],
        );
        panel.style.background = Some(visual.panel);
        panel.style.border = Some(Color {
            alpha: 26,
            ..visual.ink
        });
        panel.style.radius = Some(16);
        panel.style.padding_points = Some([20; 4]);
        panel.style.gap = Some(Space::None);
        panel
    }
    fn command_panel(&mut self) -> Node<Intent> {
        self.command_rows.clear();
        let Some(kind) = self.commands.kind.clone() else {
            return command_panel::closed();
        };
        let entries = self.command_entries();
        if kind == openagents_chat_app::commands::Kind::Palette {
            let (extents, full) = self.palette_extents(&entries);
            self.command_offset = command_panel::scroll(
                &extents,
                full,
                self.palette_results_height(),
                self.commands.selected,
                self.command_offset,
                std::mem::take(&mut self.command_reveal),
            );
        }
        let history = |entry: &openagents_chat_app::commands::Entry| {
            self.palette_history(entry)
                .map(|summary| command_panel::History {
                    title: summary.title.clone(),
                    detail: summary
                        .coder
                        .as_ref()
                        .and_then(|coder| coder.project.as_deref())
                        .unwrap_or("OpenAgents · Saved")
                        .to_owned(),
                })
        };
        let (node, rows) = command_panel::view(
            command_panel::Panel {
                kind,
                entries,
                selected: self.commands.selected,
                navigating: self.menu_navigation,
                token: &self.command_token,
                query: &self.commands.query,
                offset: self.command_offset,
                results_height: self.palette_results_height(),
                history: &history,
                identity: &self.account_name,
            },
            |key| Intent::Chat {
                action: Action::Command { key: key.into() },
            },
        );
        self.command_rows = rows;
        node
    }
    pub fn body(&mut self) -> Node<Intent> {
        if self.saved_visible {
            return self.saved_body();
        }
        self.ensure_changes();
        let start = self.state().map_or(0, |state| state.start);
        if self.rows_dirty {
            let task_rows = self
                .session
                .selected
                .as_ref()
                .and_then(|id| self.tasks.get_mut(id))
                .map(task_chat::Session::rows);
            let mut followups = vec![];
            let mut starters = vec![];
            let rows = if let Some(rows) = task_rows {
                rows.into_iter().map(Arc::new).collect()
            } else {
                let state = self.session.state();
                let turns = state.map_or(&[][..], |state| state.turns.as_slice());
                let mut rows = self.projection.shared_rows(
                    turns,
                    start,
                    Reply {
                        partial: state.map_or("", |state| state.partial.as_str()),
                        busy: state.is_some_and(|state| state.busy),
                        failure: self
                            .session
                            .error
                            .as_deref()
                            .or_else(|| state.and_then(|state| state.failure.as_deref())),
                    },
                    &appearance(),
                );
                // Each Coder turn follows the chat reply that handed it to
                // Coder, by the message that asked (#10094).
                let shown_turns = turns.len();
                let anchor = |text: &str| {
                    let at = turns.iter().rposition(|turn| {
                        turn.role == Role::User && turn.text.trim() == text.trim()
                    })?;
                    Some(match turns.get(at + 1) {
                        Some(reply) if reply.role == Role::Assistant => at + 1,
                        _ => at,
                    })
                };
                let run_rows = self
                    .session
                    .selected
                    .as_ref()
                    .and_then(|id| self.runs.get_mut(id))
                    .map(|run| run.rows_anchored(&anchor));
                // Turns anchored before the newest message go between the
                // chat's messages; the rest go last, as before.
                let (between, run_rows) = match run_rows {
                    Some(anchored) => {
                        let (between, last): (Vec<_>, Vec<_>) = anchored
                            .into_iter()
                            .partition(|(at, _)| at.is_some_and(|at| at + 1 < shown_turns));
                        (
                            between,
                            Some(last.into_iter().map(|(_, row)| row).collect::<Vec<_>>()),
                        )
                    }
                    None => (Vec::new(), None),
                };
                if !between.is_empty() {
                    let mut merged = Vec::with_capacity(rows.len() + between.len());
                    let mut pending = between.into_iter().peekable();
                    for (index, row) in rows.into_iter().enumerate() {
                        merged.push(row);
                        while pending
                            .peek()
                            .is_some_and(|(at, _)| at.is_some_and(|at| at <= index))
                        {
                            let (_, node) = pending.next().expect("peeked");
                            merged.push(Arc::new(node));
                        }
                    }
                    merged.extend(pending.map(|(_, node)| Arc::new(node)));
                    rows = merged;
                }
                let busy = self.busy();
                let fallback = Snapshot {
                    chat: self.session.selected.clone(),
                    ..Snapshot::default()
                };
                let snapshot = self
                    .session
                    .selected
                    .as_ref()
                    .and_then(|id| self.session.states.get(id))
                    .unwrap_or(&fallback);
                let cards =
                    self.session
                        .cards
                        .rows_with(snapshot, busy, self.session.error.as_deref());
                rows.extend(
                    cards
                        .into_iter()
                        // The latest reply's follow-ups sit above the
                        // composer, not in the transcript (#10075).
                        .filter(|row| match &row.element {
                            Element::Button { label, .. }
                                if row.key.starts_with("coder-followup-") =>
                            {
                                followups.push((row.key.clone(), label.clone()));
                                false
                            }
                            _ => true,
                        })
                        // An empty chat's starters sit above the centered
                        // composer, in the same row (#10097).
                        .filter(|row| match &row.element {
                            Element::Button { label, .. }
                                if row.key.starts_with("coder-suggest-") =>
                            {
                                starters.push((row.key.clone(), label.clone()));
                                false
                            }
                            _ => true,
                        })
                        // A run here replaces the offer to start one.
                        .filter(|row| run_rows.is_none() || row.key != "coder-run")
                        .map(Arc::new),
                );
                // Coder on this computer: its events after the reply that
                // started it, as `openagents chat` prints them.
                rows.extend(run_rows.into_iter().flatten().map(Arc::new));
                rows
            };
            if self.transcript_rows.len() != rows.len()
                || self
                    .transcript_rows
                    .iter()
                    .zip(&rows)
                    .any(|(old, new)| !Arc::ptr_eq(old, new) && old != new)
            {
                self.transcript_rows = rows;
                self.rows_generation += 1;
                let size = self.transcript_size;
                if size.0 > 0.0 && size.1 > 0.0 {
                    let _ =
                        self.transcript
                            .update_shared(self.transcript_rows.clone(), size.0, size.1);
                }
            }
            self.followups = followups;
            self.starters = starters;
            self.rows_dirty = false;
        }
        self.shown();
        let mut controls = vec![];
        if start > 0 && self.task().is_none() {
            controls.push(button(
                "chat-earlier",
                "Load earlier",
                Action::Earlier,
                true,
            ));
        }
        let mut children = vec![Node {
            key: "chat-transcript".into(),
            style: Style {
                fill_height: Some(true),
                ..Style::default()
            },
            element: Element::Surface {
                label: "Conversation".into(),
                resource: TRANSCRIPT.into(),
            },
        }];
        if !controls.is_empty() {
            children.push(stack("chat-reading-controls", Axis::Horizontal, controls));
        }
        if let Some(notice) = &self.notice {
            children.push(text("chat-notice", notice, TextRole::Status));
        }
        if self.show_changes() {
            children.push(self.changes_card());
        }
        let mut body = stack("chat-body", Axis::Vertical, children);
        body.style.fill_height = Some(true);
        if self.changes_open && self.show_changes() {
            let mut split = stack(
                "chat-split",
                Axis::Horizontal,
                vec![body, self.changes_pane()],
            );
            split.style.fill_height = Some(true);
            return split;
        }
        body
    }
    /// Empty conversations keep the composer in the reading pane's center.
    /// The open chat's composer placeholder: "Ask OpenAgents anything" unless
    /// Coder works, or asks, in it (#10094).
    #[must_use]
    pub fn composer_placeholder(&self) -> &'static str {
        self.session
            .selected
            .as_deref()
            .map_or(openagents_chat_app::coder_run::PLACEHOLDER, |id| {
                self.placeholder_of(id)
            })
    }

    fn placeholder_of(&self, id: &str) -> &'static str {
        self.tasks.get(id).map_or_else(
            || {
                self.runs.get(id).map_or(
                    openagents_chat_app::coder_run::PLACEHOLDER,
                    Run::placeholder,
                )
            },
            task_chat::Session::placeholder,
        )
    }

    pub fn composer_centered(&self) -> bool {
        !self.saved_visible
            && self.transcript_rows.is_empty()
            && !self.busy()
            && self.task().is_none()
            && self.run().is_none()
            && self.session.selected.as_ref().is_some_and(|id| {
                !self
                    .session
                    .summaries
                    .iter()
                    .any(|summary| &summary.id == id && summary.archived)
            })
    }
    pub fn footer(&mut self) -> Node<Intent> {
        if self.saved_visible {
            return self.saved_footer();
        }
        let Some(id) = self.session.selected.clone() else {
            return text(
                "chat-no-selection",
                "Choose New chat to start.",
                TextRole::Body,
            );
        };
        let archived = self
            .session
            .summaries
            .iter()
            .find(|summary| summary.id == id)
            .is_some_and(|summary| summary.archived);
        if archived {
            return button("chat-restore", "Restore chat", Action::Restore, true);
        }
        let busy = self.busy();
        let task = self.tasks.get(&id);
        let run = self.runs.get(&id);
        let placeholder = self.placeholder_of(&id);
        // The composer always has its field, so the card never shows empty:
        // without one the surface paints nothing, not even the placeholder
        // (#10072). Every path that selects a chat should have made it.
        let waker = &self.waker;
        let field = self.fields.entry(id.clone()).or_insert_with(|| {
            let mut field = chat_field(openagents_chat_app::coder_run::PLACEHOLDER);
            if let Some(waker) = waker {
                field.start(waker.clone());
            }
            field
        });
        field.set_placeholder(placeholder);
        field.set_unframed(true);
        let draft = self.fields.get(&id).map(|field| {
            if field.draft.editor().is_none() {
                task.and_then(task_chat::Session::editing_text)
                    .unwrap_or(field.text())
                    .to_owned()
            } else {
                field.text().to_owned()
            }
        });
        let task_mode = task
            .map(task_chat::Session::mode)
            .or_else(|| run.map(Run::mode));
        let task_ready = task.is_none_or(|task| task.summary.is_some());
        let enabled = draft.as_ref().is_some_and(|text| !text.trim().is_empty())
            || (self.attachments && !self.session.images.get(&id).is_empty());
        let composer = Node {
            key: "chat-composer".into(),
            style: Style::default(),
            element: Element::Composer {
                token: if let Some(task) = task {
                    format!(
                        "task-{}-{}",
                        task.binding.task,
                        self.task_editor.get(&id).copied().unwrap_or(0)
                    )
                } else {
                    format!("chat-{id}")
                },
                placeholder: placeholder.into(),
                max_bytes: 32 * 1024,
                enabled: true,
                busy,
                stop: Some(Intent::Chat {
                    action: Action::Stop,
                }),
                choices: vec![],
                draft,
                focus: self.fields.get(&id).is_some_and(|field| field.focused),
            },
        };
        // Reimplemented from Zeron's composer: quiet utilities on the left,
        // one circular submission control on the right, and management in the header.
        // The attach control shows only while attachments are on; the
        // desktop is text only as of 2026-10-01 (#10095).
        let attach = self.attachments.then(|| {
            icon_button(
                "chat-attach",
                "Attach image",
                Action::AttachImage,
                !busy,
                Glyph::Paperclip,
                false,
            )
        });
        let mut buttons = vec![text("chat-toolbar-space", "", TextRole::Status)];
        if let Some(run) = run
            && run.active()
        {
            if let Some(choice) = run.steer_choice() {
                buttons.push(button(
                    "coder-steer",
                    choice.label(),
                    Action::Card {
                        key: "coder-steer".into(),
                    },
                    enabled && !busy,
                ));
            }
            buttons.push(icon_button(
                "chat-stop",
                "Stop Coder",
                Action::Stop,
                !busy && run.task.is_some(),
                Glyph::Stop,
                false,
            ));
        }
        if let Some(task) = task
            && task.active()
        {
            if let Some(choice) = task.steer_choice() {
                buttons.push(button(
                    "task-steer",
                    choice.label(),
                    Action::Card {
                        key: "task-steer".into(),
                    },
                    enabled && !busy,
                ));
            }
            buttons.push(icon_button(
                "chat-stop",
                "Stop Coder",
                Action::Stop,
                !busy,
                Glyph::Stop,
                false,
            ));
        }
        buttons.push(if busy && task.is_none() && run.is_none() {
            icon_button(
                "chat-stop",
                "Stop",
                Action::Stop,
                self.state().is_some_and(|state| state.busy),
                Glyph::Stop,
                true,
            )
        } else {
            icon_button(
                "chat-send",
                match task_mode {
                    Some(openagents_chat_app::coder_tab::Mode::Queue) => "Queue",
                    Some(openagents_chat_app::coder_tab::Mode::Answer) => "Answer",
                    _ => "Send",
                },
                Action::Send,
                enabled && !busy && task_ready,
                Glyph::ArrowUp,
                true,
            )
        });
        let mut previews = vec![];
        let images: &[openagents_chat_app::attachments::Image] = if self.attachments {
            self.session.images.get(&id)
        } else {
            &[]
        };
        for image in images {
            let short: String = image.name.chars().take(10).collect();
            previews.push(stack(
                &format!("image-row-{}", image.id),
                Axis::Vertical,
                vec![
                    Node {
                        key: format!("preview-{}", image.id),
                        style: Style::default(),
                        element: Element::Surface {
                            label: format!("{} · {} × {}", image.name, image.width, image.height),
                            resource: format!("image:{}", image.id),
                        },
                    },
                    button(
                        &format!("image-remove-{}", image.id),
                        &format!("Remove {short}"),
                        Action::RemoveImage {
                            id: image.id.clone(),
                        },
                        true,
                    ),
                ],
            ));
        }
        let mut content = vec![];
        // The latest reply's follow-ups: small chips, sized to their
        // words, wrapping above the composer, as on the phone (#10075).
        // An empty chat's starters take the same row, above the centered
        // composer, and leave once the chat has a message (#10097).
        let (row_key, suggestions) = if self.followups.is_empty() {
            ("chat-starters", &self.starters)
        } else {
            ("chat-followups", &self.followups)
        };
        if !suggestions.is_empty() {
            let chips = suggestions
                .iter()
                .map(|(key, label)| followup_chip(key, label, !busy))
                .collect();
            let mut row = stack(row_key, Axis::Wrap, chips);
            row.style.gap_points = Some(8);
            row.style.padding_points = Some([0, 4, 0, 4]);
            content.push(row);
        }
        let has_previews = !previews.is_empty();
        if has_previews {
            content.push(stack("image-previews", Axis::Wrap, previews));
        }
        let field_width = (self.column_width - 114.0).max(1.0);
        let compact = !has_previews
            && task.is_none_or(|task| !task.active())
            && run.is_none_or(|run| !run.active())
            && self.fields.get(&id).is_some_and(|field| {
                !field.text().contains('\n') && field.content_line_count(field_width - 16.0) == 1
            });
        if let Some(field) = self.fields.get_mut(&id) {
            field
                .set_metrics(composer_metrics(compact))
                .expect("valid composer metrics");
        }
        let mut card = if compact {
            buttons.remove(0); // Expanded-only flexible spacer.
            let send = buttons.pop().expect("composer send control");
            let mut row: Vec<_> = attach.into_iter().collect();
            row.extend([composer, send]);
            let mut card = stack("chat-composer-card", Axis::Horizontal, row);
            card.style.padding_start = Some(Space::Sm);
            card.style.padding_end = Some(Space::Sm);
            card.style.gap = Some(Space::Xs);
            card
        } else {
            if let Some(attach) = attach {
                buttons.insert(0, attach);
            }
            let mut toolbar = stack("chat-send-controls", Axis::Horizontal, buttons);
            toolbar.style.padding_points = Some([2, 8, 8, 8]);
            toolbar.style.min_height = Some(42);
            stack(
                "chat-composer-card",
                Axis::Vertical,
                vec![composer, toolbar],
            )
        };
        card.style.background = Some(openagents_chat_app::visual::current().composer);
        card.style.radius = Some(26);
        card.style.border = Some(openagents_chat_app::visual::current().composer_border);
        card.style.gap = Some(if compact { Space::Xs } else { Space::None });
        content.push(card);
        let mut footer = stack("chat-footer", Axis::Vertical, content);
        footer.style.padding_start = Some(Space::Md);
        footer.style.padding_end = Some(Space::Md);
        footer
    }
    /// Whether the change pane is open beside the conversation.
    #[must_use]
    pub fn changes_open(&self) -> bool {
        self.changes_open && self.show_changes()
    }

    /// Bind a change by hand, as a fixture does. The card stays hidden
    /// until the task's summary says the work is finished.
    pub fn bind_changes(&mut self, review: coder_access::review::TaskReview) {
        self.changes_bound = Some(openagents_chat_app::changes::Reviewer::showing(
            review,
            Instant::now(),
        ));
        self.changes_open = false;
        self.changes_scroll = 0.0;
    }

    /// The selected chat's change: one bound by hand, the host task's, or
    /// the run's on this computer, once its turn finished.
    fn reviewer(&self) -> Option<&openagents_chat_app::changes::Reviewer> {
        let finished = self.task().is_some_and(task_chat::Session::finished)
            || self.run().is_some_and(Run::finished);
        if let Some(bound) = &self.changes_bound {
            return finished.then_some(bound);
        }
        match self.task() {
            Some(task) => task.reviewer(),
            None => self.run().and_then(Run::reviewer),
        }
    }

    fn reviewer_mut(&mut self) -> Option<&mut openagents_chat_app::changes::Reviewer> {
        if self.changes_bound.is_some() {
            return self.changes_bound.as_mut();
        }
        let id = self.session.selected.clone()?;
        if self.tasks.contains_key(&id) {
            return self.tasks.get_mut(&id)?.reviewer_mut();
        }
        self.runs.get_mut(&id)?.reviewer_mut()
    }

    fn show_changes(&self) -> bool {
        self.reviewer()
            .is_some_and(openagents_chat_app::changes::Reviewer::has_change)
    }

    fn ensure_changes(&mut self) {
        if !self.show_changes() {
            self.changes_open = false;
        }
    }

    fn changes_viewport(&self) -> f32 {
        (self.viewport.1 - 180.0).max(40.0)
    }

    /// The card: what changed, at which revisions, how complete the diff
    /// is, whether the view is stale, the publication, and the controls,
    /// all from the shared reviewer.
    fn changes_lines(&self, prefix: &str) -> Vec<Node<Intent>> {
        let Some(card) = self.reviewer().and_then(|reviewer| reviewer.card(true)) else {
            return Vec::new();
        };
        let mut lines = vec![
            text(
                &format!("{prefix}-title"),
                "What changed",
                TextRole::Heading,
            ),
            text(&format!("{prefix}-summary"), card.summary, TextRole::Status),
        ];
        if let Some(revisions) = card.revisions {
            lines.push(text(
                &format!("{prefix}-revisions"),
                revisions,
                TextRole::Status,
            ));
        }
        for note in card.notes {
            let mut line = text(
                &format!("{prefix}-note-{}", note.key),
                note.text,
                TextRole::Status,
            );
            if note.tone == openagents_chat_app::changes::Tone::Warning {
                line.style.foreground = Some(openagents_chat_app::visual::current().warning);
            }
            lines.push(line);
        }
        if let Some((label, url)) = card.link {
            lines.push(text(&format!("{prefix}-link-url"), url, TextRole::Status));
            lines.push(button(
                &format!("{prefix}-link"),
                &format!("Open {}", label.to_lowercase()),
                Action::Card {
                    key: "changes-link".into(),
                },
                true,
            ));
        }
        let mut buttons = Vec::new();
        for (action, label) in card.actions {
            // The card and the pane show the same controls under their own
            // node keys; each runs the one card action.
            let name = match action {
                openagents_chat_app::changes::CardAction::Open if prefix == "changes" => "open",
                openagents_chat_app::changes::CardAction::Open => continue,
                openagents_chat_app::changes::CardAction::Refresh => "refresh",
                openagents_chat_app::changes::CardAction::Publish => "publish",
            };
            buttons.push(button(
                &format!("{prefix}-{name}"),
                label,
                Action::Card {
                    key: format!("changes-{name}"),
                },
                true,
            ));
        }
        if !buttons.is_empty() {
            lines.push(stack(
                &format!("{prefix}-buttons"),
                Axis::Horizontal,
                buttons,
            ));
        }
        lines
    }

    fn changes_card(&self) -> Node<Intent> {
        let mut card = stack(
            "changes-card",
            Axis::Vertical,
            self.changes_lines("changes"),
        );
        card.style.background = Some(openagents_chat_app::visual::current().selected);
        card.style.padding_top = Some(Space::Sm);
        card.style.padding_bottom = Some(Space::Sm);
        card.style.padding_start = Some(Space::Sm);
        card.style.padding_end = Some(Space::Sm);
        card.style.radius = Some(10);
        card
    }

    fn changes_pane(&self) -> Node<Intent> {
        let mut children = self.changes_lines("changes-pane");
        children.push(button(
            "changes-close",
            "Close",
            Action::Card {
                key: "changes-close".into(),
            },
            true,
        ));
        children.push(Node {
            key: "changes-lines".into(),
            style: Style {
                fill_height: Some(true),
                ..Style::default()
            },
            element: Element::Surface {
                label: "What changed".into(),
                resource: CHANGES.into(),
            },
        });
        let mut pane = stack("changes-pane", Axis::Vertical, children);
        pane.style.background = Some(openagents_chat_app::visual::current().canvas);
        pane.style.fill_height = Some(true);
        pane.style.min_height = Some(200);
        pane
    }

    fn paint_changes(&mut self, frame: &mut Frame, rect: PxRect, scale: f32) {
        let viewport = rect.h / scale.max(0.01);
        let (first, count) = self
            .reviewer()
            .and_then(openagents_chat_app::changes::Reviewer::document)
            .map_or((0, 0), |doc| {
                doc.window(self.changes_scroll, viewport, CHANGES_LINE)
            });
        if count == 0 {
            return;
        }
        if self.changes_highlighter.is_none() {
            self.changes_highlighter = Some(rust_native::syntax::Highlighter::with_palette(
                openagents_chat_app::visual::current().syntax,
            ));
        }
        let highlighter = self.changes_highlighter.take().expect("highlighter");
        if let Some(doc) = self
            .reviewer_mut()
            .and_then(openagents_chat_app::changes::Reviewer::document_mut)
        {
            doc.ensure_spans(first, count, &highlighter);
        }
        self.changes_highlighter = Some(highlighter);
        let visible: Vec<_> = self
            .reviewer()
            .and_then(openagents_chat_app::changes::Reviewer::document)
            .map(|doc| {
                doc.lines()
                    .iter()
                    .skip(first)
                    .take(count)
                    .map(|line| {
                        (
                            line.kind,
                            line.text.clone(),
                            line.spans.clone().unwrap_or_default(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let font = rust_native::layout::display::Font {
            size: 13.0,
            weight: rust_native::layout::display::Weight::Regular,
            family: rust_native::layout::display::FontFamily::PaperMono,
            italic: false,
            mono: true,
        };
        let line_px = CHANGES_LINE * scale;
        let visual = openagents_chat_app::visual::current();
        for (index, (kind, text, spans)) in visible.iter().enumerate() {
            let top = rect.y + index as f32 * line_px;
            let gutter = match kind {
                openagents_chat_app::changes::Kind::Add => Some(visual.diff_add_bg),
                openagents_chat_app::changes::Kind::Remove => Some(visual.diff_remove_bg),
                _ => None,
            };
            if let Some(color) = gutter {
                frame.fill(
                    PxRect {
                        x: rect.x,
                        y: top,
                        w: rect.w,
                        h: line_px,
                    },
                    0.0,
                    color,
                );
            }
            let color = match kind {
                openagents_chat_app::changes::Kind::Add => visual.diff_add,
                openagents_chat_app::changes::Kind::Remove => visual.diff_remove,
                openagents_chat_app::changes::Kind::File => visual.text,
                openagents_chat_app::changes::Kind::Hunk
                | openagents_chat_app::changes::Kind::Meta => visual.muted,
                openagents_chat_app::changes::Kind::Context => visual.text,
            };
            self.fonts.draw_highlighted_run(
                frame,
                text,
                font,
                rect.x + 8.0 * scale,
                top + line_px * 0.78,
                scale,
                color,
                spans,
                0,
            );
        }
    }
}
/// Open an `http(s)` link in the person's browser. Anything else is
/// ignored.
pub fn open_link(destination: &str) {
    if !destination.starts_with("https://") && !destination.starts_with("http://") {
        return;
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("/usr/bin/open")
            .arg(destination)
            .spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open")
            .arg(destination)
            .spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("rundll32.exe")
            .args(["url.dll,FileProtocolHandler", destination])
            .spawn();
    }
}

fn chat_transcript() -> Transcript {
    let mut transcript = Transcript::default();
    transcript.set_font_family(rust_native::layout::display::FontFamily::PaperMono);
    transcript
        .set_metrics(openagents_chat_app::visual::current().transcript)
        .expect("valid chat metrics");
    transcript.set_palette(&openagents_chat_app::visual::current().colors);
    transcript.set_syntax_palette(openagents_chat_app::visual::current().syntax);
    transcript
}

fn composer_metrics(compact: bool) -> rust_native_desktop::composer::field::Metrics {
    rust_native_desktop::composer::field::Metrics {
        font_size: 14.0,
        line_height: 22.75,
        padding: if compact {
            [12.0, 8.0, 12.0, 8.0]
        } else {
            [16.0, 16.0, 4.0, 16.0]
        },
        min_height: if compact { 47.0 } else { 76.0 },
        max_height: 260.0,
    }
}

fn search_field() -> Field {
    let mut field = chat_field("Search chats");
    field.set_unframed(true);
    field
        .set_metrics(rust_native_desktop::composer::field::Metrics {
            font_size: 12.0,
            line_height: 16.0,
            padding: [6.0, 8.0, 6.0, 8.0],
            min_height: 28.0,
            max_height: 28.0,
        })
        .expect("valid filter metrics");
    field
}

fn chat_field(placeholder: &str) -> Field {
    let mut field = Field::with_placeholder(placeholder);
    field.set_font_family(rust_native::layout::display::FontFamily::PaperMono);
    field
        .set_metrics(composer_metrics(true))
        .expect("valid composer metrics");
    field.set_colors(
        openagents_chat_app::visual::current().text,
        openagents_chat_app::visual::current().faint,
        openagents_chat_app::visual::current().accent,
    );
    field.set_appearance(field_appearance());
    field
}

/// The appearance a field paints its selection and frame in: the scheme
/// the app paints with (#11028).
fn field_appearance() -> rust_native_desktop::theme::Appearance {
    match openagents_chat_app::visual::scheme() {
        openagents_chat_app::visual::Scheme::Dark => rust_native_desktop::theme::Appearance::Dark,
        openagents_chat_app::visual::Scheme::Light => rust_native_desktop::theme::Appearance::Light,
    }
}

fn appearance() -> Appearance<'static> {
    Appearance {
        prefix: "turn-",
        body_suffix: "-body",
        streaming_key: "stream".into(),
        working_key: "working",
        working_label: "OpenAgents is replying…",
        failed_key: "talk-failed",
        markdown_style: Style::default(),
        status_style: Style::default(),
    }
}

fn stack(key: &str, axis: Axis, children: Vec<Node<Intent>>) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            gap: Some(Space::Sm),
            ..Style::default()
        },
        element: Element::Stack { axis, children },
    }
}
fn text(key: &str, value: impl Into<String>, role: TextRole) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style::default(),
        element: Element::Text {
            value: value.into(),
            role,
        },
    }
}
fn button(key: &str, label: &str, action: Action, enabled: bool) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            background: Some(openagents_chat_app::visual::current().selected),
            foreground: Some(openagents_chat_app::visual::current().text),
            weight: Some(TextWeight::Normal),
            ..Style::default()
        },
        element: Element::Button {
            shortcut: None,
            label: label.into(),
            enabled,
            icon: None,
            intent: Intent::Chat { action },
        },
    }
}
/// A follow-up suggestion: a pill as wide as its words, which sends them.
fn followup_chip(key: &str, label: &str, enabled: bool) -> Node<Intent> {
    let mut node = button(key, label, Action::Card { key: key.into() }, enabled);
    node.style.background = Some(openagents_chat_app::visual::current().selected);
    node.style.foreground = Some(openagents_chat_app::visual::current().text);
    node.style.glyph_color = Some(openagents_chat_app::visual::current().muted);
    node.style.text_size = Some(13);
    node.style.button_padding = Some([12, 6]);
    node.style.glyph_size = Some(13);
    node.style.glyph_gap = Some(6);
    if let Element::Button { icon, .. } = &mut node.element {
        *icon = Some(Icon {
            glyph: Glyph::Ask,
            circular: false,
            pill: true,
        });
    }
    node
}
fn icon_button(
    key: &str,
    label: &str,
    action: Action,
    enabled: bool,
    glyph: Glyph,
    primary: bool,
) -> Node<Intent> {
    let mut node = button(key, label, action, enabled);
    node.style.glyph_size = Some(match glyph {
        Glyph::Paperclip => 18,
        Glyph::Stop => 11,
        _ => 14,
    });
    node.style.background = Some(if primary {
        openagents_chat_app::visual::current().text
    } else {
        Color {
            alpha: 0,
            ..Color::rgb(0, 0, 0)
        }
    });
    node.style.foreground = Some(if primary {
        openagents_chat_app::visual::current().sidebar
    } else {
        openagents_chat_app::visual::current().muted
    });
    if let Element::Button { icon, .. } = &mut node.element {
        *icon = Some(Icon {
            glyph,
            circular: true,
            pill: false,
        });
    }
    node
}

fn request((ticket, command): (u64, Command)) -> Request {
    Request::Chat { ticket, command }
}

#[cfg(test)]
mod start_setting_tests {
    use super::*;
    use openagents_chat::basic_coder::Turn;
    use openagents_chat::service::Snapshot;

    /// A coding reply to a message sent from this window, on a computer
    /// that can run Coder.
    fn replied(asks_first: fn() -> bool) -> Panel {
        replied_on(asks_first, openagents_chat::delegation::DISPATCH_ROUTE)
    }

    /// A reply on the computer lane whose judgment named `route`.
    fn replied_on(asks_first: fn() -> bool, route: &str) -> Panel {
        let mut panel = Panel::new(Instant::now());
        panel.set_coder_asks_first(asks_first);
        let chat = "c".repeat(32);
        let mut user = Turn::user("add a unit test for slugify");
        user.request = Some("r1".into());
        panel.session.states.insert(
            chat.clone(),
            Snapshot {
                chat: Some(chat.clone()),
                computer: true,
                turns: vec![
                    user,
                    Turn::assistant(
                        "Working on this.",
                        Some(openagents_chat::router::Meta {
                            route: Some(route.into()),
                            ..Default::default()
                        }),
                    ),
                ],
                ..Default::default()
            },
        );
        panel.session.select(&chat);
        panel.sent.insert((chat, "r1".into()));
        panel.run_if_coding();
        panel
    }

    /// `coder.start` (#10036): `at_once`, the default, starts Coder for a
    /// coding reply; `ask_first` leaves the offer's **Run Coder**, which
    /// still starts it.
    #[test]
    fn the_start_setting_decides_whether_a_coding_reply_runs_at_once() {
        let chat = "c".repeat(32);
        let at_once = replied(|| false);
        assert!(at_once.coder_run(&chat).is_some());
        let mut asks = replied(|| true);
        assert!(asks.coder_run(&chat).is_none());
        assert!(asks.sent.is_empty(), "the reply was judged once");
        asks.start_run(&chat);
        assert!(asks.coder_run(&chat).is_some());
    }

    /// Once a chat's Coder run has finished, Send goes to the router
    /// (#10094): the composer says so, and a reply that hands the next
    /// message to Coder continues the same task with it, as its next turn;
    /// a reply that answered continues nothing. The chat's messages and the
    /// run's turns interleave, each turn after the reply that handed it.
    #[test]
    fn after_a_finished_run_the_router_decides_and_a_dispatch_continues_the_task() {
        let fixture =
            include_str!("../../openagents-chat/fixtures/coder-events/question-then-result.ndjson");
        let lines: Vec<openagents_chat::coder_events::Line> = fixture
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let task = lines[0].task.clone();
        let lines: Vec<_> = lines.into_iter().filter(|line| line.task == task).collect();
        let finished = |route: &str| {
            let mut panel = replied_on(|| true, openagents_chat::delegation::DISPATCH_ROUTE);
            let chat = "c".repeat(32);
            let now = Instant::now();
            let mut run = Run::follow(&chat, &task, None, now);
            let (ticket, _) = run.tick(now).unwrap();
            run.outcome(
                ticket,
                Ok(coder_run::Answer::Lines {
                    lines: lines.clone(),
                    state: coder_run::State::Ended,
                }),
                now,
            );
            panel.runs.insert(chat.clone(), run);
            assert_eq!(panel.composer_placeholder(), "Ask OpenAgents anything");
            // The follow-up and the router's reply to it.
            let snapshot = panel.session.states.get_mut(&chat).unwrap();
            let mut asked = Turn::user("now add a test");
            asked.request = Some("r2".into());
            snapshot.turns.push(asked);
            snapshot.turns.push(Turn::assistant(
                "Working on adding a test.",
                Some(openagents_chat::router::Meta {
                    route: Some(route.into()),
                    ..Default::default()
                }),
            ));
            panel.sent.insert((chat.clone(), "r2".into()));
            panel.run_if_coding();
            panel
        };
        let mut handed = finished(openagents_chat::delegation::DISPATCH_ROUTE);
        let (_, _, request) = next_run(&mut handed);
        assert_eq!(
            request,
            coder_run::Request::Continue {
                task: task.clone(),
                text: "now add a test".into()
            }
        );
        let mut answered = finished("general");
        let chat = "c".repeat(32);
        let run = answered.runs.get_mut(&chat).unwrap();
        let (_, request) = run
            .tick(Instant::now() + std::time::Duration::from_secs(5))
            .unwrap();
        assert!(
            !matches!(request, coder_run::Request::Continue { .. }),
            "{request:?}"
        );
        assert!(
            answered
                .coder_run(&"c".repeat(32))
                .is_some_and(Run::routes_followups)
        );
    }

    /// A reply that answered the question on the computer lane, such as
    /// the working directory from the desktop's context (route `meta`),
    /// starts nothing, even at once (#10079).
    #[test]
    fn a_reply_that_answered_starts_no_coder() {
        let chat = "c".repeat(32);
        let answered = replied_on(|| false, "meta");
        assert!(answered.coder_run(&chat).is_none());
    }

    /// The window's settings file decides `coder.start` for the next coding
    /// reply (#10070): Settings writes it, and the chat reads it each time.
    #[test]
    fn the_start_setting_in_the_windows_file_applies_to_the_next_reply() {
        use coder::task::settings::{Settings, Start};
        let home = tempfile::tempdir().unwrap();
        let file = home.path().join("settings.json");
        let chat = "c".repeat(32);
        let reply = |file: &std::path::Path| {
            let mut panel = replied(|| true);
            panel.runs.clear();
            panel.read_coder_start_from(file.to_path_buf());
            panel.sent.insert((chat.clone(), "r1".into()));
            panel.run_if_coding();
            panel.coder_run(&chat).is_some()
        };
        // No file: the default, at once.
        assert!(reply(&file));
        let mut settings = Settings::default();
        settings.coder.start = Start::AskFirst;
        settings.save(&file).unwrap();
        assert!(!reply(&file));
        settings.coder.start = Start::AtOnce;
        settings.save(&file).unwrap();
        assert!(reply(&file));
        // A file the loader refuses asks first.
        std::fs::write(&file, "not the settings").unwrap();
        assert!(!reply(&file));
    }

    /// The next Coder run request the panel sends, answering reads.
    fn next_run(panel: &mut Panel) -> (String, u64, coder_run::Request) {
        for step in 0..40 {
            match panel.tick(Instant::now() + std::time::Duration::from_millis(step * 10)) {
                Some(Request::CoderRun {
                    chat,
                    ticket,
                    request,
                }) => return (chat, ticket, request),
                Some(Request::Chat { ticket, command }) => {
                    let snapshot = panel.session.states.values().next().cloned().unwrap();
                    let _ = command;
                    panel.outcome(ticket, Ok(snapshot));
                }
                _ => {}
            }
        }
        panic!("no Coder run request")
    }

    /// Run Coder on this computer carries the chat's draft images as their
    /// exact bytes; a start that fails keeps them in the draft, and only an
    /// accepted start lets them go. The hosted send still refuses them.
    #[test]
    fn a_run_carries_the_drafts_images_and_keeps_them_until_accepted() {
        let chat = "c".repeat(32);
        let mut panel = replied(|| true);
        // The image pipeline, with attachments turned on (#10095).
        panel.set_attachments(true);
        let image = openagents_chat_app::attachments::Image::pixels(5, 4, vec![77; 80]).unwrap();
        let bytes = image.bytes.as_ref().clone();
        panel.session.images.add(&chat, image).unwrap();
        assert!(panel.session.images.text_only_refusal(&chat).is_some());
        panel.start_run(&chat);
        let (started, ticket, request) = next_run(&mut panel);
        assert_eq!(started, chat);
        let coder_run::Request::Start { images, .. } = &request else {
            panic!("{request:?}")
        };
        assert_eq!(images.len(), 1);
        assert_eq!(*images[0].bytes, bytes);
        assert_eq!(images[0].reference.size, bytes.len() as u64);
        assert_eq!(images[0].reference.media_type, "image/png");
        // A failed start, or one whose answer is lost: the draft keeps them.
        panel.run_outcome(chat.clone(), ticket, Err("Coder could not start".into()));
        assert_eq!(panel.session.images.get(&chat).len(), 1);
        // Retried and accepted: the task holds them; the draft lets go.
        let retried = panel
            .runs
            .get_mut(&chat)
            .unwrap()
            .action(coder_run::Action::Retry, "")
            .unwrap();
        let coder_run::Request::Start { images, .. } = &retried.1 else {
            panic!("{:?}", retried.1)
        };
        assert_eq!(*images[0].bytes, bytes);
        panel.run_outcome(
            chat.clone(),
            retried.0,
            Ok(coder_run::Answer::Started {
                task: "b".repeat(64),
                project: "openagents".into(),
                checkout: "/w/openagents".into(),
            }),
        );
        assert!(panel.session.images.get(&chat).is_empty());
    }

    /// A chat with no turns yet, a draft holding a PNG, and the words typed;
    /// Send pressed. Returns the panel, the image's bytes, and the send's
    /// request ID and ticket.
    fn sent_with_a_screenshot(asks_first: fn() -> bool) -> (Panel, Vec<u8>, String, u64) {
        let now = Instant::now();
        let chat = "c".repeat(32);
        let mut panel = Panel::new(now);
        // The image pipeline, with attachments turned on (#10095).
        panel.set_attachments(true);
        panel.set_coder_asks_first(asks_first);
        panel.session.states.insert(
            chat.clone(),
            Snapshot {
                chat: Some(chat.clone()),
                ..Default::default()
            },
        );
        panel.session.select(&chat);
        panel.selected_changed(None);
        let pixels: Vec<u8> = (0..32u32 * 24 * 4).map(|i| (i % 251) as u8).collect();
        let image = openagents_chat_app::attachments::Image::pixels(32, 24, pixels).unwrap();
        let bytes = image.bytes.as_ref().clone();
        panel.session.images.add(&chat, image).unwrap();
        let view = rust_native::View::new("send-images", 1, panel.footer())
            .validate()
            .unwrap();
        panel.mounted(&view);
        panel.input(TextInput::Commit("fix this layout bug"), now);
        let Some(Request::Chat { ticket, command }) = panel.action(Action::Send, &view, now) else {
            panic!("a send")
        };
        // Only the words go to the hosted router.
        let Command::Send {
            chat: to,
            request,
            text,
        } = command
        else {
            panic!("{command:?}")
        };
        assert_eq!(
            (to.as_str(), text.as_str()),
            (chat.as_str(), "fix this layout bug")
        );
        assert_eq!(
            panel.notice(),
            Some(openagents_chat_app::attachments::HELD_FOR_CODER)
        );
        assert_eq!(panel.session.images.get(&chat).len(), 1);
        assert_eq!(panel.session.images.bound(&chat), Some(request.as_str()));
        (panel, bytes, request, ticket)
    }

    /// The router's reply to the message `request`, offering Coder or not.
    fn answered(panel: &mut Panel, request: &str, ticket: u64, coding: bool) {
        use openagents_chat::router::{Meta, Offer};
        let chat = "c".repeat(32);
        let mut user = Turn::user("fix this layout bug");
        user.request = Some(request.into());
        let meta = coding.then(|| Meta {
            offers: vec![Offer::RunCoder],
            ..Meta::default()
        });
        let snapshot = Snapshot {
            chat: Some(chat.clone()),
            turns: vec![user, Turn::assistant("On it.", meta)],
            ..Default::default()
        };
        panel.session.states.insert(chat, snapshot.clone());
        panel.outcome(ticket, Ok(snapshot));
    }

    /// One send with words and a screenshot (#10070): the words go to the
    /// router, and its coding reply starts Coder at once on this computer
    /// with the screenshot's exact bytes; an accepted start lets the
    /// draft's image go.
    #[test]
    fn one_send_with_words_and_a_screenshot_starts_coder_with_its_bytes() {
        let chat = "c".repeat(32);
        let (mut panel, bytes, request, ticket) = sent_with_a_screenshot(|| false);
        answered(&mut panel, &request, ticket, true);
        let (started, ticket, run) = next_run(&mut panel);
        assert_eq!(started, chat);
        let coder_run::Request::Start { images, prompt, .. } = &run else {
            panic!("{run:?}")
        };
        assert!(prompt.contains("fix this layout bug"), "{prompt}");
        assert_eq!(images.len(), 1);
        assert_eq!(*images[0].bytes, bytes);
        assert_eq!(
            images[0].reference.digest,
            coder_access::media::digest(&bytes)
        );
        panel.run_outcome(
            chat.clone(),
            ticket,
            Ok(coder_run::Answer::Started {
                task: "b".repeat(64),
                project: "openagents".into(),
                checkout: "/w/openagents".into(),
            }),
        );
        assert!(panel.session.images.get(&chat).is_empty());
    }

    /// A reply that does not lead to Coder keeps the images in the draft
    /// and says images go only to Coder; nothing starts.
    #[test]
    fn a_reply_that_is_not_coding_keeps_the_images() {
        let chat = "c".repeat(32);
        let (mut panel, _, request, ticket) = sent_with_a_screenshot(|| false);
        answered(&mut panel, &request, ticket, false);
        assert!(panel.coder_run(&chat).is_none());
        assert_eq!(panel.session.images.get(&chat).len(), 1);
        assert_eq!(panel.session.images.bound(&chat), None);
        assert_eq!(
            panel.notice(),
            Some(openagents_chat_app::attachments::ONLY_TO_CODER)
        );
    }

    /// `ask_first`: the coding reply waits for **Run Coder**, which carries
    /// the images sent with the message.
    #[test]
    fn ask_first_and_run_coder_carry_the_images_sent_with_the_message() {
        let chat = "c".repeat(32);
        let (mut panel, bytes, request, ticket) = sent_with_a_screenshot(|| true);
        answered(&mut panel, &request, ticket, true);
        assert!(panel.coder_run(&chat).is_none());
        assert_eq!(panel.session.images.get(&chat).len(), 1);
        assert_ne!(
            panel.notice(),
            Some(openagents_chat_app::attachments::ONLY_TO_CODER)
        );
        panel.start_run(&chat);
        let (_, _, run) = next_run(&mut panel);
        let coder_run::Request::Start { images, .. } = &run else {
            panic!("{run:?}")
        };
        assert_eq!(*images[0].bytes, bytes);
    }

    /// Attachments off (#10095, the switch the phone shares): a draft
    /// restored from before still holding an image shows no image or
    /// attach control, sends its words only, and Command-V is the field's
    /// text paste, never an image import.
    #[test]
    fn a_restored_draft_with_images_sends_its_words_only() {
        let now = Instant::now();
        let chat = "c".repeat(32);
        let mut panel = Panel::new(now);
        assert!(!panel.attachments_enabled());
        panel.session.states.insert(
            chat.clone(),
            Snapshot {
                chat: Some(chat.clone()),
                ..Default::default()
            },
        );
        panel.session.select(&chat);
        panel.selected_changed(None);
        let image = openagents_chat_app::attachments::Image::pixels(2, 2, vec![9; 16]).unwrap();
        panel.session.images.add(&chat, image).unwrap();
        let view = rust_native::View::new("text-only", 1, panel.footer())
            .validate()
            .unwrap();
        let tree = format!("{:?}", view.view());
        assert!(!tree.contains("chat-attach") && !tree.contains("image-previews"));
        panel.mounted(&view);
        panel.input(TextInput::Commit("just the words"), now);
        let _ = panel.input(
            TextInput::Key {
                key: "v",
                text: None,
                control: false,
                command: true,
                alt: false,
                shift: false,
            },
            now,
        );
        assert!(panel.image_input.is_none());
        let Some(Request::Chat { command, .. }) = panel.action(Action::Send, &view, now) else {
            panic!("a send")
        };
        let Command::Send { text, .. } = command else {
            panic!("{command:?}")
        };
        assert_eq!(text, "just the words");
        assert!(panel.session.images.is_empty());
        assert_ne!(
            panel.notice(),
            Some(openagents_chat_app::attachments::HELD_FOR_CODER)
        );
    }

    /// Attachments off: Run Coder on this computer starts with the words
    /// and no images, though the draft held one.
    #[test]
    fn run_coder_carries_no_images_while_attachments_are_off() {
        let chat = "c".repeat(32);
        let mut panel = replied(|| true);
        let image = openagents_chat_app::attachments::Image::pixels(2, 2, vec![9; 16]).unwrap();
        panel.session.images.add(&chat, image).unwrap();
        panel.start_run(&chat);
        let (_, _, request) = next_run(&mut panel);
        let coder_run::Request::Start { images, .. } = &request else {
            panic!("{request:?}")
        };
        assert!(images.is_empty());
        assert!(panel.session.images.is_empty());
    }
}

#[cfg(test)]
mod image_tests {
    use super::*;
    #[test]
    fn a_delayed_image_keeps_its_original_conversation_and_text_paste_is_stamped() {
        let now = Instant::now();
        let mut panel = Panel::new(now);
        panel.session.select("original");
        panel.fields.insert("original".into(), Field::default());
        let view = rust_native::View::new("image-test", 1, panel.footer())
            .validate()
            .unwrap();
        let field = panel.fields.get_mut("original").unwrap();
        field.draft.mount(&view, "chat-composer").unwrap();
        let stamp = field.draft.stamp().unwrap();
        let (send, receiver) = std::sync::mpsc::sync_channel(1);
        panel.image_input = Some(("original".into(), stamp, receiver));
        panel.session.select("other");
        send.send(crate::chat_images::Result::Image(
            openagents_chat_app::attachments::Image::pixels(1, 1, vec![0; 4]).unwrap(),
        ))
        .unwrap();
        panel.poll_images(0);
        assert!(panel.images().is_empty());
        panel.session.select("original");
        assert_eq!(panel.images().len(), 1);
        let field = panel.fields.get_mut("original").unwrap();
        let old = field.draft.stamp().unwrap();
        field
            .draft
            .apply(&old, rust_native_desktop::composer::Input::Text("newer"), 1)
            .unwrap();
        let (send, receiver) = std::sync::mpsc::sync_channel(1);
        panel.image_input = Some(("original".into(), old, receiver));
        send.send(crate::chat_images::Result::Text("stale clipboard".into()))
            .unwrap();
        panel.poll_images(2);
        assert_eq!(panel.draft(), "newer");
    }
}

/// The deck a reply's typed `open_presentation` offer names. Only the
/// router's typed offer counts; the reply's words never open a deck.
pub fn presentation_offer(meta: Option<&openagents_chat::router::Meta>) -> Option<String> {
    meta?.offers.iter().find_map(|offer| match offer {
        openagents_chat::router::Offer::OpenPresentation { deck } => Some(deck.clone()),
        _ => None,
    })
}

/// Whether a reply's typed offers include `open_screen` for `routes.map`
/// (bank entry `meta.map.desktop`). Only the router's typed offer counts;
/// the reply's words never open the Map page.
pub fn map_offer(meta: Option<&openagents_chat::router::Meta>) -> bool {
    meta.is_some_and(|meta| {
        meta.offers.iter().any(|offer| {
            matches!(
                offer,
                openagents_chat::router::Offer::OpenScreen {
                    screen: openagents_chat::router::Screen::RoutesMap
                }
            )
        })
    })
}

#[cfg(test)]
mod presentation_tests {
    use super::*;
    use openagents_chat::basic_coder::Turn;
    use openagents_chat::router::{Meta, Offer};
    use openagents_chat::service::Snapshot;

    /// A reply to "open the deck", sent from this window, with `meta`.
    fn replied(text: &str, meta: Option<Meta>, sent: bool) -> Panel {
        let mut panel = Panel::new(Instant::now());
        let chat = "d".repeat(32);
        let mut user = Turn::user("open the three devdays later deck");
        user.request = Some("r1".into());
        panel.session.states.insert(
            chat.clone(),
            Snapshot {
                chat: Some(chat.clone()),
                turns: vec![user, Turn::assistant(text, meta)],
                ..Default::default()
            },
        );
        panel.session.select(&chat);
        if sent {
            panel.sent.insert((chat, "r1".into()));
        }
        panel.run_if_coding();
        panel
    }

    fn offered(deck: &str) -> Option<Meta> {
        Some(Meta {
            tier: Some("canned".into()),
            answer: Some("presentation.open@1".into()),
            route: Some("presentation.open".into()),
            offers: vec![Offer::OpenPresentation { deck: deck.into() }],
            ..Meta::default()
        })
    }

    /// A finished reply's typed `open_presentation` offer holds its deck
    /// for the shell, once (#10058).
    #[test]
    fn a_replys_typed_offer_holds_its_deck_for_the_viewer() {
        let mut panel = replied(
            "Opening Three DevDays Later.",
            offered("three-devdays-later"),
            true,
        );
        assert_eq!(
            panel.take_presentation().as_deref(),
            Some("three-devdays-later")
        );
        assert_eq!(panel.take_presentation(), None, "taken once");
        // A reply to a message another device sent opens nothing here.
        let mut elsewhere = replied(
            "Opening Three DevDays Later.",
            offered("three-devdays-later"),
            false,
        );
        assert_eq!(elsewhere.take_presentation(), None);
    }

    /// A finished reply's typed `open_screen` offer for `routes.map`
    /// holds the Map page for the shell, once, only for a message sent
    /// from this window; another screen's offer, or none, holds nothing
    /// (#10102).
    #[test]
    fn a_replys_typed_routes_map_offer_holds_the_map_once() {
        use openagents_chat::router::Screen;
        let map = |screen| {
            Some(Meta {
                route: Some("meta.map".into()),
                offers: vec![Offer::OpenScreen { screen }],
                ..Meta::default()
            })
        };
        let mut panel = replied("Here is the route map.", map(Screen::RoutesMap), true);
        assert!(panel.take_map());
        assert!(!panel.take_map(), "taken once");
        let mut elsewhere = replied("Here is the route map.", map(Screen::RoutesMap), false);
        assert!(!elsewhere.take_map(), "a reply to another device's message");
        for meta in [None, Some(Meta::default()), map(Screen::Wallet)] {
            let mut panel = replied("Open the map to see how we route things.", meta, true);
            assert!(!panel.take_map());
        }
    }

    /// Words never open the viewer: a reply that names a deck, with no
    /// offer or with another offer, holds nothing (#10058).
    #[test]
    fn a_reply_that_only_names_a_deck_opens_nothing() {
        for meta in [
            None,
            Some(Meta::default()),
            Some(Meta {
                offers: vec![Offer::RunCoder],
                ..Meta::default()
            }),
        ] {
            let mut panel = replied(
                "Opening three-devdays-later, the Three DevDays Later presentation.",
                meta,
                true,
            );
            assert_eq!(panel.take_presentation(), None);
        }
    }
}

#[cfg(test)]
mod task_tests {
    use super::*;
    use nostr::activity_summary::{self, Attention, Phase, SubjectKind, SummaryDraft};
    fn panel() -> (Panel, Instant) {
        let now = Instant::now();
        let mut panel = Panel::new(now);
        panel.session.select("chat");
        panel.selected_changed(None);
        let mut task = task_chat::Session::new(
            openagents_chat::basic_chats::Spawned {
                host: "a".repeat(64),
                task: "b".repeat(64),
                project: Some("scratch".into()),
                at: None,
            },
            now,
        );
        task.summary = Some(
            activity_summary::encode(&SummaryDraft {
                host: &task.binding.host,
                subject_kind: SubjectKind::Task,
                subject: &task.binding.task,
                sequence: 4,
                phase: Phase::Running,
                headline: "Coder is working",
                attention: Attention::None,
                updated_at: task_chat::unix_now(),
            })
            .unwrap(),
        );
        panel.tasks.insert("chat".into(), task);
        (panel, now)
    }
    fn mount(panel: &mut Panel, revision: u64) -> ValidatedView<Intent> {
        let view = rust_native::View::new("task-editor", revision, panel.footer())
            .validate()
            .unwrap();
        panel.mounted(&view);
        view
    }
    fn receipt() -> task_chat::Answer {
        task_chat::Answer::Operation(coder_access::protocol::Outcome::Dispatched {
            receipt: coder_access::protocol::Receipt {
                operation: "task.command".into(),
                reference: "b".repeat(64),
            },
        })
    }
    #[test]
    fn a_running_task_offers_queue_stop_and_steer_and_retries_clear_only_the_acknowledged_draft() {
        let (mut panel, now) = panel();
        panel.body();
        let view = mount(&mut panel, 1);
        let labels = crate::screens::words(&view.view().root);
        assert!(labels.iter().any(|label| label == "Queue"));
        assert!(labels.iter().any(|label| label == "Stop Coder"));
        assert!(labels.iter().any(|label| label == "Stop and send"));
        panel.input(TextInput::Commit("next turn  "), now);
        let Request::TaskChat {
            chat,
            ticket,
            request,
        } = panel.action(Action::Send, &view, now).unwrap()
        else {
            panic!("task request")
        };
        panel.task_outcome(
            chat.clone(),
            ticket,
            Err(crate::control::ControlError::Unreachable),
        );
        assert_eq!(panel.draft(), "next turn  ");
        let Request::TaskChat {
            ticket: retry,
            request: same,
            ..
        } = panel.action(Action::Retry, &view, now).unwrap()
        else {
            panic!("retry")
        };
        assert_eq!(request, same);
        panel.task_outcome(chat, retry, Ok(receipt()));
        assert_eq!(panel.draft(), "");
    }
    #[test]
    fn a_delayed_task_acknowledgment_preserves_edits_and_a_refusal_keeps_the_draft() {
        let (mut panel, now) = panel();
        let view = mount(&mut panel, 1);
        panel.input(TextInput::Commit("original"), now);
        let Request::TaskChat { chat, ticket, .. } =
            panel.action(Action::Send, &view, now).unwrap()
        else {
            panic!("send")
        };
        panel.input(TextInput::Commit(" newer"), now);
        panel.task_outcome(chat.clone(), ticket, Ok(receipt()));
        assert_eq!(panel.draft(), "original newer");
        let view = mount(&mut panel, 2);
        let Request::TaskChat { ticket, .. } = panel.action(Action::Send, &view, now).unwrap()
        else {
            panic!("send")
        };
        panel.task_outcome(
            chat,
            ticket,
            Err(crate::control::ControlError::Refused {
                code: "conflict".into(),
                message: "The turn changed.".into(),
            }),
        );
        assert_eq!(panel.draft(), "original newer");
        assert!(!panel.busy());
    }

    fn finish(panel: &mut Panel) {
        let task = panel.tasks.get_mut("chat").expect("task");
        let host = task.binding.host.clone();
        let subject = task.binding.task.clone();
        task.summary = Some(
            activity_summary::encode(&SummaryDraft {
                host: &host,
                subject_kind: SubjectKind::Task,
                subject: &subject,
                sequence: 8,
                phase: Phase::Completed,
                headline: "Coder finished",
                attention: Attention::Completed,
                updated_at: task_chat::unix_now(),
            })
            .unwrap(),
        );
    }

    fn five_thousand_lines() -> String {
        let mut diff = String::from(
            "diff --git a/src/answer.rs b/src/answer.rs\n--- a/src/answer.rs\n+++ b/src/answer.rs\n@@ -1 +1,4996 @@\n fn keep() {}\n",
        );
        for index in 0..4995 {
            diff.push_str(&format!("+fn line_{index}() {{ return {index}; }}\n"));
        }
        diff
    }

    /// A finished change of `diff` at exact revisions, as a computer reads
    /// it.
    fn review_of(diff: &str) -> coder_access::review::TaskReview {
        let doc = openagents_chat_app::changes::parse(diff);
        let added = doc
            .lines()
            .iter()
            .filter(|line| line.kind == openagents_chat_app::changes::Kind::Add)
            .count() as u64;
        coder_access::review::TaskReview {
            task: "b".repeat(64),
            base: "1".repeat(40),
            head_commit: "1".repeat(40),
            head: "2".repeat(40),
            files: vec![coder_access::review::FileCount {
                path: "src/answer.rs".into(),
                status: coder_access::review::FileStatus::Modified,
                added: Some(added),
                removed: Some(0),
            }],
            files_total: 1,
            added,
            removed: 0,
            uncounted: 0,
            diff: diff.into(),
            completeness: coder_access::review::Completeness::Complete,
            publication: None,
        }
    }

    fn document(panel: &Panel) -> &openagents_chat_app::changes::Document {
        panel
            .reviewer()
            .and_then(openagents_chat_app::changes::Reviewer::document)
            .expect("diff")
    }

    fn walk(node: &Node<Intent>, visit: &mut impl FnMut(&Node<Intent>)) {
        visit(node);
        if let Element::Stack { children, .. } = &node.element {
            for child in children {
                walk(child, visit);
            }
        }
    }

    /// A finished host task's change is read with `task.review` and
    /// published with `task.publish` naming the reviewed head; an older host
    /// that knows no review shows the transcript's diff with no revisions
    /// and no Publish (#10067, #10068).
    #[test]
    fn a_host_task_change_is_reviewed_and_published_through_the_host() {
        let (mut panel, now) = panel();
        finish(&mut panel);
        let next = |panel: &mut Panel, at: Instant| {
            for step in 0..20 {
                if let Some(Request::TaskChat {
                    chat,
                    ticket,
                    request: task_chat::Request::Operation { operation, .. },
                }) = panel.tick(at + std::time::Duration::from_millis(step * 10))
                    && matches!(
                        operation,
                        coder_access::protocol::Operation::ReviewTask { .. }
                            | coder_access::protocol::Operation::PublishTask { .. }
                    )
                {
                    return (chat, ticket, operation);
                }
            }
            panic!("no review or publish asked");
        };
        let (chat, ticket, operation) = next(&mut panel, now);
        assert_eq!(
            operation,
            coder_access::protocol::Operation::ReviewTask {
                task: "b".repeat(64)
            }
        );
        panel.task_outcome(
            chat,
            ticket,
            Ok(task_chat::Answer::Operation(
                coder_access::protocol::Outcome::Review {
                    review: Box::new(review_of(&five_thousand_lines())),
                },
            )),
        );
        let mut keys = Vec::new();
        walk(&panel.body(), &mut |node| keys.push(node.key.clone()));
        assert!(
            keys.iter().any(|key| key == "changes-revisions"),
            "{keys:?}"
        );
        let view = mount(&mut panel, 9);
        panel.action(
            Action::Card {
                key: "changes-publish".into(),
            },
            &view,
            now,
        );
        let (_, _, operation) = next(&mut panel, now);
        assert_eq!(
            operation,
            coder_access::protocol::Operation::PublishTask {
                task: "b".repeat(64),
                base: "1".repeat(40),
                head_commit: "1".repeat(40),
                head: "2".repeat(40),
            }
        );
        // An older host: no revisions, no Publish.
        let (mut older, now) = panel_finished();
        let (chat, ticket, _) = next(&mut older, now);
        older.task_outcome(chat, ticket, Ok(task_chat::Answer::Unsupported));
        let reviewer = older.reviewer().expect("a reviewer");
        assert!(reviewer.unsupported());
        assert!(reviewer.card(true).is_none_or(|card| {
            card.revisions.is_none()
                && !card
                    .actions
                    .iter()
                    .any(|(action, _)| *action == openagents_chat_app::changes::CardAction::Publish)
        }));
    }

    fn panel_finished() -> (Panel, Instant) {
        let (mut panel, now) = panel();
        finish(&mut panel);
        (panel, now)
    }

    #[test]
    fn a_finished_task_diff_opens_in_a_read_only_pane_and_scrolls_by_line() {
        let (mut panel, now) = panel();
        panel.bind_changes(review_of(&five_thousand_lines()));
        let hidden = panel.body();
        let mut keys = Vec::new();
        walk(&hidden, &mut |node| keys.push(node.key.clone()));
        assert!(!keys.iter().any(|key| key == "changes-card"));
        finish(&mut panel);
        let card = panel.body();
        keys.clear();
        walk(&card, &mut |node| {
            keys.push(node.key.clone());
            if let Element::Text { value, .. } | Element::Button { label: value, .. } =
                &node.element
            {
                assert!(crate::words::banned_in(value).is_empty(), "{value}");
            }
        });
        assert!(keys.iter().any(|key| key == "changes-card"));
        assert!(keys.iter().any(|key| key == "changes-open"));
        assert!(!keys.iter().any(|key| key == "changes-pane"));
        let view = mount(&mut panel, 3);
        assert!(
            panel
                .action(
                    Action::Card {
                        key: "changes-open".into(),
                    },
                    &view,
                    now,
                )
                .is_none()
        );
        assert!(panel.changes_open());
        let open = panel.body();
        keys.clear();
        let mut nodes = 0usize;
        let mut composer = false;
        walk(&open, &mut |node| {
            nodes += 1;
            keys.push(node.key.clone());
            if matches!(node.element, Element::Composer { .. }) {
                composer = true;
            }
        });
        assert!(keys.iter().any(|key| key == "changes-pane"));
        assert!(keys.iter().any(|key| key == "changes-close"));
        assert!(!composer);
        assert!(nodes < 40, "{nodes}");
        let doc = document(&panel);
        assert_eq!(doc.len(), 5_000);
        let (first, count) =
            doc.window(panel.changes_scroll, panel.changes_viewport(), CHANGES_LINE);
        assert_eq!(first, 0);
        assert!(count < 80);
        assert!(count < doc.len());
        panel.surface(
            CHANGES,
            SurfaceInput::Wheel {
                x: 10.0,
                y: 10.0,
                dx: 0.0,
                dy: -18.0 * 120.0,
            },
            now,
        );
        let (next, shown) =
            document(&panel).window(panel.changes_scroll, panel.changes_viewport(), CHANGES_LINE);
        assert!(next > first);
        assert!(shown < 80);
        let limit = document(&panel).scroll_limit(panel.changes_viewport(), CHANGES_LINE);
        panel.surface(
            CHANGES,
            SurfaceInput::Wheel {
                x: 10.0,
                y: 10.0,
                dx: 0.0,
                dy: -(limit + 5_000.0),
            },
            now,
        );
        let (last, shown) =
            document(&panel).window(panel.changes_scroll, panel.changes_viewport(), CHANGES_LINE);
        assert_eq!(last + shown, 5_000);
        let mut frame = Frame::new(320, 96, Color::rgb(6, 6, 6));
        assert!(panel.paint(
            CHANGES,
            &mut frame,
            PxRect {
                x: 0.0,
                y: 0.0,
                w: 320.0,
                h: 96.0,
            },
        ));
        assert!(
            frame
                .pixels
                .chunks(4)
                .any(|pixel| pixel[0] != 6 || pixel[1] != 6 || pixel[2] != 6),
            "the visible lines paint"
        );
        let colored = document(&panel)
            .lines()
            .iter()
            .any(|line| line.spans.as_ref().is_some_and(|spans| !spans.is_empty()));
        assert!(colored, "visible code lines carry syntax spans");
        let view = mount(&mut panel, 4);
        panel.action(
            Action::Card {
                key: "changes-close".into(),
            },
            &view,
            now,
        );
        assert!(!panel.changes_open());
        let closed = panel.body();
        keys.clear();
        walk(&closed, &mut |node| keys.push(node.key.clone()));
        assert!(!keys.iter().any(|key| key == "changes-pane"));
        assert!(keys.iter().any(|key| key == "changes-card"));
    }
}

/// The settings' `coder.start` on this computer (`coder::task::settings`):
/// whether a coding reply waits for **Run Coder**.
fn coder_asks_first() -> bool {
    coder::task::local::Local::here(std::path::PathBuf::new()).asks_first()
}

/// The sidebar section a summary sorts into: pinned, then recent, then archived.
#[cfg(test)]
mod sidebar_order_tests {
    use super::*;
    use openagents_chat::basic_chats::{Spawned, Summary};

    fn summary(id: &str, updated: u64, project: Option<&str>) -> Summary {
        Summary {
            id: id.into(),
            title: id.into(),
            started: updated,
            updated,
            coder: project.map(|project| Spawned {
                host: "local".into(),
                task: "t".into(),
                project: Some(project.into()),
                at: None,
            }),
            archived: false,
            pinned: false,
            named: false,
        }
    }

    /// A new chat (no project yet) is the newest, so it opens at the top
    /// of Recent, above older Coder chats that name a project.
    #[test]
    fn a_new_chat_is_listed_first_above_older_project_chats() {
        let mut panel = Panel::new(Instant::now());
        panel.session.summaries = vec![
            summary("older-project", 10, Some("openagents")),
            summary("old-plain", 5, None),
            summary("new", 20, None),
        ];
        let mut state = State::default();
        panel.sync_sidebar(&mut state);
        let titles: Vec<&str> = state
            .chats
            .iter()
            .filter(|chat| chat.section == Section::Recent)
            .map(|chat| chat.title.as_str())
            .collect();
        assert_eq!(titles, ["new", "older-project", "old-plain"]);
    }

    /// #10468: a chat whose Coder run is working rises above a newer quiet
    /// chat and says so; silent past the stale bound, it says stale.
    #[test]
    fn a_working_chat_rises_and_goes_stale_when_silent() {
        let start = Instant::now();
        let mut panel = Panel::new(start);
        panel.session.summaries = vec![summary("busy", 10, None), summary("quiet", 20, None)];
        panel
            .runs
            .insert("busy".into(), Run::follow("busy", "task", None, start));
        let rows = |panel: &mut Panel, now: Instant| {
            let mut state = State::default();
            panel.sync_sidebar_at(&mut state, now, 0);
            state
                .chats
                .iter()
                .filter(|chat| chat.section == Section::Recent)
                .map(|chat| (chat.title.clone(), chat.indicator))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            rows(&mut panel, start),
            [
                ("busy".to_owned(), attention::Indicator::Working),
                ("quiet".to_owned(), attention::Indicator::Idle),
            ]
        );
        let later = start + attention::STALE_AFTER + std::time::Duration::from_secs(1);
        assert_eq!(
            rows(&mut panel, later),
            [
                ("busy".to_owned(), attention::Indicator::Stale),
                ("quiet".to_owned(), attention::Indicator::Idle),
            ]
        );
    }
}
