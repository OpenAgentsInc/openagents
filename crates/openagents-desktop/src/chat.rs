//! Desktop chat presentation and local drafts over host-owned conversation state.
use crate::chat_action::Action;
use crate::chrome::{Chat, Section, State};
use crate::control::ControlResult;
use crate::model::{Intent, Request};
use openagents_chat::basic_coder::{Role, Turn};
use openagents_chat::service::{Command, Snapshot};
use openagents_chat_app::projection::{self, Appearance, Projection, Reply};
use openagents_chat_app::session::Session;
use rust_native::style::{Space, Style};
use rust_native::{Axis, Element, MessageRole, Node, TextRole, ValidatedView};
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

pub struct Panel {
    session: Session,
    ids: BTreeMap<String, u64>,
    fields: BTreeMap<String, Field>,
    submissions: BTreeMap<String, (String, Submission)>,
    born: Instant,
    pub viewport: (f32, f32, f32),
    pub transcript: Transcript,
    fonts: Fonts,
    transcript_rows: Vec<Node<()>>,
    projection: Projection,
    transcript_size: (f32, f32),
    composer_rect: Option<PxRect>,
    rows_dirty: bool,
    activated: Vec<String>,
    press_revision: Option<u64>,
    queued: Vec<Request>,
    navigation: Option<openagents_chat::router::Screen>,
    notice: Option<String>,
    waker: Option<rust_native_desktop::Waker>,
}

impl Panel {
    pub fn new(now: Instant) -> Self {
        Self {
            session: Session::new(now),
            ids: BTreeMap::new(),
            fields: BTreeMap::new(),
            submissions: BTreeMap::new(),
            born: now,
            viewport: (1200.0, 840.0, 1.0),
            transcript: Transcript::default(),
            fonts: Fonts::new(),
            transcript_rows: vec![],
            projection: Projection::default(),
            transcript_size: (0.0, 0.0),
            composer_rect: None,
            rows_dirty: true,
            activated: vec![],
            press_revision: None,
            queued: vec![],
            navigation: None,
            notice: None,
            waker: None,
        }
    }
    fn request(&mut self, command: Command) -> Request {
        request(self.session.request(command))
    }
    pub fn start(&mut self, waker: rust_native_desktop::Waker) {
        for field in self.fields.values_mut() {
            field.start(waker.clone());
        }
        let wake = waker.clone();
        self.transcript.start(Arc::new(move || wake.wake()));
        self.waker = Some(waker);
    }
    pub fn tick(&mut self, now: Instant) -> Option<Request> {
        self.transcript.poll_highlights();
        let at_ms = now.saturating_duration_since(self.born).as_millis() as u64;
        for field in self.fields.values_mut() {
            field.poll_clipboard(at_ms);
        }
        let previous = self.session.selected.clone();
        let outcome = self.session.tick(now).map(request);
        if previous != self.session.selected {
            self.selected_changed(previous);
        }
        outcome
    }
    fn busy(&self) -> bool {
        self.session.busy()
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
        if let Some(field) = previous.and_then(|id| self.fields.get_mut(&id)) {
            field.input(TextInput::FocusLost, 0);
        }
        if let Some(id) = &self.session.selected {
            let field = self.fields.entry(id.clone()).or_default();
            field.focused = true;
            if let Some(waker) = &self.waker {
                field.start(waker.clone());
            }
        }
        self.transcript = Transcript::default();
        if let Some(waker) = &self.waker {
            let wake = waker.clone();
            self.transcript.start(Arc::new(move || wake.wake()));
        }
        self.transcript_rows.clear();
        self.projection = Projection::default();
        self.transcript_size = (0.0, 0.0);
        self.rows_dirty = true;
        self.notice = None;
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
    pub fn sync_sidebar(&mut self, state: &mut State) {
        for summary in &self.session.summaries {
            if !self.ids.contains_key(&summary.id) {
                let id = self.ids.values().max().copied().unwrap_or(0) + 1;
                self.ids.insert(summary.id.clone(), id);
            }
        }
        if let Some(selected) = &self.session.selected
            && !self.ids.contains_key(selected)
        {
            let id = self.ids.values().max().copied().unwrap_or(0) + 1;
            self.ids.insert(selected.clone(), id);
        }
        let chats = self
            .session
            .summaries
            .iter()
            .map(|summary| Chat {
                id: self.ids[&summary.id],
                title: summary.title.clone(),
                detail: "OpenAgents · Saved",
                section: if summary.archived {
                    Section::Archived
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
        let previous = self.session.selected.clone();
        let revision = self.session.revision;
        let accepted = self
            .session
            .outcome(ticket, result.map_err(|error| error.to_string()));
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
    pub fn take_activated(&mut self) -> Vec<String> {
        std::mem::take(&mut self.activated)
    }
    pub fn take_requests(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.queued)
    }
    pub fn take_navigation(&mut self) -> Option<openagents_chat::router::Screen> {
        self.navigation.take()
    }
    pub fn navigation_notice(&mut self, notice: String) {
        self.notice = Some(notice);
    }
    pub fn mounted(&mut self, view: &ValidatedView<Intent>) {
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
        let id = self.session.selected.clone()?;
        match action {
            Action::Card { key } => {
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
                    openagents_chat_app::cards::Effect::None => None,
                }
            }
            Action::Send => {
                let field = self.field()?;
                let stamp = field.draft.stamp().ok()?;
                let submission = field.draft.submission(view, &stamp, None).ok()?;
                let id = self.session.selected.clone()?;
                let send_id = uuid::Uuid::new_v4().simple().to_string();
                let command = self
                    .session
                    .submit(send_id.clone(), submission.text.clone())?;
                self.submissions.insert(send_id, (id, submission));
                Some(request(command))
            }
            Action::Stop => Some(self.request(Command::Stop { chat: id })),
            Action::Retry => self.session.retry().map(request),
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
    pub fn input(&mut self, event: TextInput<'_>, now: Instant) -> FieldAction {
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
        if resource == TRANSCRIPT {
            let version = self.transcript.version();
            let moved = matches!(event, SurfaceInput::Move { .. });
            let at_ms = now.saturating_duration_since(self.born).as_millis() as u64;
            if matches!(event, SurfaceInput::Down { .. }) {
                self.press_revision = Some(self.session.revision);
            }
            if matches!(event, SurfaceInput::Down { .. })
                && let Some(field) = self.field()
            {
                field.input(TextInput::FocusLost, at_ms);
            }
            if let Some(action) = self.transcript.pointer(event, &mut self.fonts) {
                let destination = match action {
                    rust_native_desktop::transcript::Action::Activate(key) => {
                        if self.press_revision.take() == Some(self.session.revision) {
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
                if destination.starts_with("https://") || destination.starts_with("http://") {
                    #[cfg(target_os = "macos")]
                    {
                        let _ = std::process::Command::new("/usr/bin/open")
                            .arg(&destination)
                            .spawn();
                    }
                    #[cfg(target_os = "linux")]
                    {
                        let _ = std::process::Command::new("xdg-open")
                            .arg(&destination)
                            .spawn();
                    }
                    #[cfg(target_os = "windows")]
                    {
                        let _ = std::process::Command::new("rundll32.exe")
                            .args(["url.dll,FileProtocolHandler", &destination])
                            .spawn();
                    }
                }
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
        false
    }
    pub fn next_wake(&self, now: Instant) -> Instant {
        self.session.next_wake(now)
    }
    pub fn version(&self, resource: &str) -> Option<u64> {
        match resource {
            TRANSCRIPT => Some(self.transcript.version()),
            COMPOSER => self
                .session
                .selected
                .as_ref()
                .and_then(|id| self.fields.get(id))
                .map(Field::version),
            _ => None,
        }
    }
    pub fn size(&self, resource: &str, available: f32) -> Option<(f32, f32)> {
        let composer_height = self
            .session
            .selected
            .as_ref()
            .and_then(|id| self.fields.get(id))
            .map_or(56.0, |field| field.height(available));
        match resource {
            TRANSCRIPT => Some((
                available,
                (self.viewport.1 - 240.0 - composer_height).max(40.0),
            )),
            COMPOSER => Some((available, composer_height)),
            _ => None,
        }
    }
    pub fn paint(&mut self, resource: &str, frame: &mut Frame, rect: PxRect) -> bool {
        let scale = self.viewport.2;
        if resource == TRANSCRIPT {
            let size = (rect.w / scale, rect.h / scale);
            if size != self.transcript_size {
                if let Err(error) =
                    self.transcript
                        .update(self.transcript_rows.clone(), size.0, size.1)
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
    pub fn body(&mut self) -> Node<Intent> {
        let start = self.state().map_or(0, |state| state.start);
        if self.rows_dirty {
            let state = self.session.state();
            let turns = state.map_or(&[][..], |state| state.turns.as_slice());
            let mut rows = self.projection.rows(
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
            if rows.is_empty() {
                rows.push(message("welcome".into(),&Turn::assistant("How can we help?\n\nAsk a question, explore an idea, or work through a problem.",None)));
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
            rows.extend(self.session.cards.rows_with(
                snapshot,
                busy,
                self.session.error.as_deref(),
            ));
            if self.transcript_rows != rows {
                self.transcript_rows = rows;
                let size = self.transcript_size;
                if size.0 > 0.0 && size.1 > 0.0 {
                    let _ = self
                        .transcript
                        .update(self.transcript_rows.clone(), size.0, size.1);
                }
            }
            self.rows_dirty = false;
        }
        let mut controls = vec![];
        if start > 0 {
            controls.push(button(
                "chat-earlier",
                "Load earlier",
                Action::Earlier,
                true,
            ));
        }
        if !self.transcript.at_tail() {
            controls.push(button("chat-latest", "Latest", Action::Latest, true));
        }
        let mut children = vec![Node {
            key: "chat-transcript".into(),
            style: Style::default(),
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
        stack("chat-body", Axis::Vertical, children)
    }
    pub fn footer(&mut self) -> Node<Intent> {
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
        let draft = self.fields.get(&id).map(|field| field.text().to_owned());
        let enabled = draft.as_ref().is_some_and(|text| !text.trim().is_empty());
        let composer = Node {
            key: "chat-composer".into(),
            style: Style::default(),
            element: Element::Composer {
                token: format!("chat-{id}"),
                placeholder: "Message OpenAgents…".into(),
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
        let mut buttons = vec![text(
            "chat-key-hint",
            "Enter to send · Shift+Enter for a new line",
            TextRole::Status,
        )];
        buttons.push(if busy {
            button(
                "chat-stop",
                "Stop",
                Action::Stop,
                self.state().is_some_and(|state| state.busy),
            )
        } else {
            button("chat-send", "Send", Action::Send, enabled)
        });
        buttons.push(button("chat-archive", "Archive", Action::Archive, true));
        stack(
            "chat-footer",
            Axis::Vertical,
            vec![
                composer,
                stack("chat-send-controls", Axis::Horizontal, buttons),
            ],
        )
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
fn message(key: String, turn: &Turn) -> Node<()> {
    projection::message(
        &key,
        if turn.role == Role::User {
            MessageRole::User
        } else {
            MessageRole::Assistant
        },
        rust_native::markdown::parse(&turn.text),
        &appearance(),
    )
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
        style: Style::default(),
        element: Element::Button {
            label: label.into(),
            enabled,
            icon: None,
            intent: Intent::Chat { action },
        },
    }
}

fn request((ticket, command): (u64, Command)) -> Request {
    Request::Chat { ticket, command }
}
