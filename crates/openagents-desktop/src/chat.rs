//! Desktop chat presentation and local drafts over host-owned conversation state.
use crate::chat_action::Action;
use crate::chrome::{Chat, Section, State};
use crate::control::ControlResult;
use crate::model::{Intent, Request};
use openagents_chat::basic_coder::{Role, Turn};
use openagents_chat::service::{Command, Snapshot};
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
use std::time::{Duration, Instant};

pub const TRANSCRIPT: &str = "chat-transcript";
pub const COMPOSER: &str = "composer:chat-composer";

struct PendingSend {
    chat: String,
    request: String,
    submission: Submission,
}

pub struct Panel {
    selected: Option<String>,
    ids: BTreeMap<String, u64>,
    states: BTreeMap<String, Snapshot>,
    fields: BTreeMap<String, Field>,
    summaries: Vec<openagents_chat::basic_chats::Summary>,
    pending: BTreeMap<u64, Command>,
    send: BTreeMap<String, PendingSend>,
    error: Option<String>,
    next_ticket: u64,
    poll: Instant,
    born: Instant,
    pub viewport: (f32, f32, f32),
    pub transcript: Transcript,
    fonts: Fonts,
    transcript_rows: Vec<Node<()>>,
    parsed: BTreeMap<String, (Turn, Node<()>)>,
    transcript_size: (f32, f32),
    composer_rect: Option<PxRect>,
    reading: bool,
    rows_dirty: bool,
    waker: Option<rust_native_desktop::Waker>,
    listed: bool,
}

impl Panel {
    pub fn new(now: Instant) -> Self {
        Self {
            selected: None,
            ids: BTreeMap::new(),
            states: BTreeMap::new(),
            fields: BTreeMap::new(),
            summaries: vec![],
            pending: BTreeMap::new(),
            send: BTreeMap::new(),
            error: None,
            next_ticket: 1,
            poll: now,
            born: now,
            listed: false,
            viewport: (1200.0, 840.0, 1.0),
            transcript: Transcript::default(),
            fonts: Fonts::new(),
            transcript_rows: vec![],
            parsed: BTreeMap::new(),
            transcript_size: (0.0, 0.0),
            composer_rect: None,
            reading: false,
            rows_dirty: true,
            waker: None,
        }
    }
    fn request(&mut self, command: Command) -> Request {
        let ticket = self.next_ticket;
        self.next_ticket += 1;
        self.pending.insert(ticket, command.clone());
        Request::Chat { ticket, command }
    }
    pub fn start(&mut self, waker: rust_native_desktop::Waker) {
        for field in self.fields.values_mut() {
            field.start(waker.clone());
        }
        self.waker = Some(waker);
    }
    pub fn tick(&mut self, now: Instant) -> Option<Request> {
        let at_ms = now.saturating_duration_since(self.born).as_millis() as u64;
        for field in self.fields.values_mut() {
            field.poll_clipboard(at_ms);
        }
        if now < self.poll
            || self
                .pending
                .values()
                .any(|command| matches!(command, Command::List { .. } | Command::Read { .. }))
        {
            return None;
        }
        self.poll = now + Duration::from_millis(if self.busy() { 100 } else { 1000 });
        if self.listed && self.selected.is_none() && self.error.is_none() {
            return Some(self.new_chat());
        }
        Some(
            self.request(
                self.selected
                    .as_ref()
                    .map_or(Command::List {}, |chat| Command::Read {
                        chat: chat.clone(),
                        before: None,
                    }),
            ),
        )
    }
    fn busy(&self) -> bool {
        self.state().is_some_and(|state| state.busy)
            || self
                .selected
                .as_ref()
                .is_some_and(|id| self.send.contains_key(id))
    }
    fn state(&self) -> Option<&Snapshot> {
        self.selected.as_ref().and_then(|id| self.states.get(id))
    }
    /// The selected conversation's local draft, separate from saved messages.
    pub fn draft(&self) -> &str {
        self.selected
            .as_ref()
            .and_then(|id| self.fields.get(id))
            .map_or("", Field::text)
    }
    fn field(&mut self) -> Option<&mut Field> {
        let id = self.selected.as_ref()?;
        self.fields.get_mut(id)
    }
    pub fn new_chat(&mut self) -> Request {
        let id = uuid::Uuid::new_v4().simple().to_string();
        self.select(&id);
        self.request(Command::Create { chat: id })
    }
    fn select(&mut self, id: &str) {
        if self.selected.as_deref() != Some(id) {
            if let Some(field) = self.field() {
                field.input(TextInput::FocusLost, 0);
            }
            self.selected = Some(id.into());
            let field = self.fields.entry(id.into()).or_default();
            field.focused = true;
            if let Some(waker) = &self.waker {
                field.start(waker.clone());
            }
            self.error = None;
            self.transcript = Transcript::default();
            self.transcript_rows.clear();
            self.parsed.clear();
            self.transcript_size = (0.0, 0.0);
            self.reading = false;
            self.rows_dirty = true;
        }
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
        for summary in &self.summaries {
            if !self.ids.contains_key(&summary.id) {
                let id = self.ids.values().max().copied().unwrap_or(0) + 1;
                self.ids.insert(summary.id.clone(), id);
            }
        }
        if let Some(selected) = &self.selected
            && !self.ids.contains_key(selected)
        {
            let id = self.ids.values().max().copied().unwrap_or(0) + 1;
            self.ids.insert(selected.clone(), id);
        }
        let chats = self
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
            self.selected
                .as_ref()
                .and_then(|id| self.ids.get(id))
                .copied()
                .filter(|_| selecting),
        );
    }
    pub fn outcome(&mut self, ticket: u64, result: ControlResult<Snapshot>) {
        let Some(command) = self.pending.remove(&ticket) else {
            return;
        };
        match result {
            Err(error) => {
                let target = match &command {
                    Command::List {} => None,
                    Command::Create { chat }
                    | Command::Read { chat, .. }
                    | Command::Send { chat, .. }
                    | Command::Retry { chat }
                    | Command::Stop { chat }
                    | Command::Archive { chat }
                    | Command::Restore { chat } => Some(chat),
                };
                if target.is_none() || target == self.selected.as_ref() {
                    self.error = Some(error.to_string());
                }
            }
            Ok(mut snapshot) => {
                self.listed = true;
                self.error = snapshot
                    .storage_error
                    .as_ref()
                    .map(|_| "Couldn't save chat. Check available disk space.".into());
                self.summaries = snapshot.chats.clone();
                if let Some(id) = snapshot.chat.clone() {
                    if matches!(
                        command,
                        Command::Read {
                            before: Some(_),
                            ..
                        }
                    ) {
                        if let Some(previous) = self.states.get(&id)
                            && snapshot.start + snapshot.turns.len() == previous.start
                        {
                            snapshot.turns.extend(previous.turns.clone());
                        }
                    } else if self.reading
                        && let Some(previous) = self.states.get(&id)
                        && previous.start <= snapshot.start
                        && previous.total <= snapshot.total
                    {
                        let count = snapshot.start - previous.start;
                        let mut earlier =
                            previous.turns[..count.min(previous.turns.len())].to_vec();
                        earlier.extend(snapshot.turns);
                        snapshot.turns = earlier;
                        snapshot.start = previous.start;
                    }
                    if let Some(send) = self.send.get(&id)
                        && snapshot.storage_error.is_none()
                        && snapshot
                            .turns
                            .iter()
                            .any(|turn| turn.request.as_deref() == Some(&send.request))
                    {
                        if let Some(field) = self.fields.get_mut(&id) {
                            let _ = field.draft.accepted(&send.submission);
                            field.focused = true;
                        }
                        self.send.remove(&id);
                    }
                    if self.selected.as_ref() == Some(&id)
                        && self.states.get(&id).is_none_or(|previous| {
                            previous.turns != snapshot.turns
                                || previous.start != snapshot.start
                                || previous.busy != snapshot.busy
                                || previous.partial != snapshot.partial
                        })
                    {
                        self.rows_dirty = true;
                    }
                    self.states.insert(id, snapshot);
                }
                if self.error.is_none()
                    && matches!(command, Command::Archive { .. })
                    && let Some(id) = self.selected.take()
                {
                    self.send.remove(&id);
                }
                if self.selected.is_none()
                    && let Some(id) = self
                        .summaries
                        .iter()
                        .find(|summary| !summary.archived)
                        .map(|summary| summary.id.clone())
                {
                    self.select(&id);
                }
            }
        }
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
        let id = self.selected.clone()?;
        match action {
            Action::Send => {
                if self.send.contains_key(&id) {
                    return None;
                }
                let field = self.field()?;
                let stamp = field.draft.stamp().ok()?;
                let submission = field.draft.submission(view, &stamp, None).ok()?;
                let request = uuid::Uuid::new_v4().simple().to_string();
                let command = Command::Send {
                    chat: id.clone(),
                    request: request.clone(),
                    text: submission.text.clone(),
                };
                self.send.insert(
                    id.clone(),
                    PendingSend {
                        chat: id,
                        request,
                        submission,
                    },
                );
                self.error = None;
                Some(self.request(command))
            }
            Action::Stop => Some(self.request(Command::Stop { chat: id })),
            Action::Retry => {
                self.error = None;
                let command = self
                    .send
                    .get(&id)
                    .map_or(Command::Retry { chat: id }, |send| Command::Send {
                        chat: send.chat.clone(),
                        request: send.request.clone(),
                        text: send.submission.text.clone(),
                    });
                Some(self.request(command))
            }
            Action::Restore => Some(self.request(Command::Restore { chat: id })),
            Action::Archive => Some(self.request(Command::Archive { chat: id })),
            Action::Earlier => {
                let before = self.state()?.start;
                if before == 0 {
                    return None;
                }
                self.reading = true;
                Some(self.request(Command::Read {
                    chat: id,
                    before: Some(before),
                }))
            }
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
            if matches!(event, SurfaceInput::Move { .. }) && !self.transcript.dragging() {
                return false;
            }
            let at_ms = now.saturating_duration_since(self.born).as_millis() as u64;
            if matches!(event, SurfaceInput::Down { .. })
                && let Some(field) = self.field()
            {
                field.input(TextInput::FocusLost, at_ms);
            }
            if let Some(action) = self.transcript.pointer(event, &mut self.fonts) {
                let destination = match action {
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
            return true;
        }
        if resource == COMPOSER {
            if matches!(event, SurfaceInput::Move { .. })
                && self
                    .selected
                    .as_ref()
                    .and_then(|id| self.fields.get(id))
                    .is_none_or(|field| !field.dragging())
            {
                return false;
            }
            let at = now.duration_since(self.born).as_millis() as u64;
            if let Some(id) = self.selected.clone()
                && let Some(field) = self.fields.get_mut(&id)
            {
                field.pointer(event, &mut self.fonts, at);
            }
            return true;
        }
        false
    }
    pub fn next_wake(&self, now: Instant) -> Instant {
        self.poll.max(now + Duration::from_millis(100))
    }
    pub fn version(&self, resource: &str) -> Option<u64> {
        match resource {
            TRANSCRIPT => Some(self.transcript.version()),
            COMPOSER => self
                .selected
                .as_ref()
                .and_then(|id| self.fields.get(id))
                .map(Field::version),
            _ => None,
        }
    }
    pub fn size(&self, resource: &str, available: f32) -> Option<(f32, f32)> {
        let composer_height = self
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
                    self.error = Some(format!("Couldn't lay out conversation: {error}"));
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
            if let Some(id) = &self.selected
                && let Some(field) = self.fields.get_mut(id)
            {
                field.paint(frame, rect, scale, &mut self.fonts);
            }
            return true;
        }
        false
    }
    pub fn cursor(&self) -> Option<(f64, f64)> {
        let field = self.selected.as_ref().and_then(|id| self.fields.get(id))?;
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
            let turns = self
                .state()
                .map(|state| state.turns.clone())
                .unwrap_or_default();
            let mut rows: Vec<Node<()>> = turns
                .iter()
                .enumerate()
                .map(|(index, turn)| {
                    let key = format!("turn-{}", start + index);
                    let entry = self
                        .parsed
                        .entry(key.clone())
                        .or_insert_with(|| (turn.clone(), message(key.clone(), turn)));
                    if entry.0 != *turn {
                        *entry = (turn.clone(), message(key, turn));
                    }
                    entry.1.clone()
                })
                .collect();
            if let Some(state) = self.state() {
                if !state.partial.is_empty() {
                    rows.push(message(
                        "stream".into(),
                        &Turn::assistant(state.partial.clone(), None),
                    ));
                }
                if state.busy {
                    rows.push(Node {
                        key: "working".into(),
                        style: Style::default(),
                        element: Element::Working {
                            label: "OpenAgents is replying…".into(),
                        },
                    });
                }
            }
            if rows.is_empty() {
                rows.push(message("welcome".into(),&Turn::assistant("How can we help?\n\nAsk a question, explore an idea, or work through a problem.",None)));
            }
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
        if let Some(meta) = self
            .state()
            .and_then(|state| state.turns.last())
            .filter(|turn| turn.role == Role::Assistant)
            .and_then(|turn| turn.meta.as_ref())
        {
            for (index, followup) in meta.followups.iter().enumerate() {
                controls.push(button(
                    &format!("chat-followup-{index}"),
                    &followup.label,
                    Action::Followup {
                        text: followup.label.clone(),
                    },
                    !self.busy(),
                ));
            }
        }
        if !controls.is_empty() {
            children.push(stack("chat-reading-controls", Axis::Horizontal, controls));
        }
        let failure = self
            .error
            .clone()
            .or_else(|| self.state().and_then(|state| state.failure.clone()));
        if self
            .state()
            .and_then(|state| state.turns.last())
            .is_some_and(|turn| turn.stopped && turn.role == Role::Assistant)
        {
            children.push(text(
                "chat-stopped",
                "Stopped receiving this reply. The hosted worker may still finish.",
                TextRole::Status,
            ));
            children.push(button(
                "chat-retry-stopped",
                "Retry reply",
                Action::Retry,
                true,
            ));
        }
        if let Some(failure) = failure {
            children.push(text("chat-error", failure, TextRole::Status));
            children.push(button("chat-retry", "Try again", Action::Retry, true));
        }
        stack("chat-body", Axis::Vertical, children)
    }
    pub fn footer(&mut self) -> Node<Intent> {
        let Some(id) = self.selected.clone() else {
            return text(
                "chat-no-selection",
                "Choose New chat to start.",
                TextRole::Body,
            );
        };
        let archived = self
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
fn message(key: String, turn: &Turn) -> Node<()> {
    Node {
        key: key.clone(),
        style: Style::default(),
        element: Element::Message {
            role: if turn.role == Role::User {
                MessageRole::User
            } else {
                MessageRole::Assistant
            },
            note: None,
            children: vec![Node {
                key: format!("{key}-body"),
                style: Style::default(),
                element: Element::Markdown {
                    blocks: rust_native::markdown::parse(&turn.text),
                },
            }],
        },
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
        style: Style::default(),
        element: Element::Button {
            label: label.into(),
            enabled,
            icon: None,
            intent: Intent::Chat { action },
        },
    }
}
