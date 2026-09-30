//! Desktop chat presentation and local drafts over host-owned conversation state.
use crate::chat_action::Action;
use crate::chrome::{Chat, Section, State};
use crate::control::ControlResult;
use crate::model::{Intent, Request};
use openagents_chat::basic_coder::{Role, Turn};
use openagents_chat::service::{Command, Snapshot};
use openagents_chat_app::projection::{self, Appearance, Projection, Reply};
use openagents_chat_app::session::Session;
use openagents_chat_app::task_chat::{self, Action as TaskAction};
use rust_native::style::{Color, Space, Style, TextWeight};
use rust_native::{Axis, Element, Glyph, Icon, MessageRole, Node, TextRole, ValidatedView};
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

pub const COMMAND_QUERY: &str = "composer:command-query";
pub const SEARCH: &str = "composer:chat-search";
pub const RENAME: &str = "composer:chat-rename";

pub struct Panel {
    commands: openagents_chat_app::commands::Overlay,
    saved: openagents_chat_app::retained::Session,
    saved_visible: bool,
    saved_project: Option<String>,
    command_query: Field,
    command_token: String,
    menu_point: Option<(f32, f32)>,
    menu_navigation: bool,
    search: Field,
    rename: Option<(String, Field)>,
    rename_pending: Option<(u64, String, String)>,
    rename_focus: usize,
    aux_rect: Option<PxRect>,
    session: Session,
    ids: BTreeMap<String, u64>,
    fields: BTreeMap<String, Field>,
    submissions: BTreeMap<String, (String, Submission)>,
    tasks: BTreeMap<String, task_chat::Session>,
    task_submissions: BTreeMap<(String, String), Submission>,
    task_editor: BTreeMap<String, u64>,
    born: Instant,
    pub viewport: (f32, f32, f32),
    pub column_width: f32,
    pub transcript: Transcript,
    fonts: Fonts,
    transcript_rows: Vec<Arc<Node<()>>>,
    projection: Projection,
    transcript_size: (f32, f32),
    composer_rect: Option<PxRect>,
    rows_dirty: bool,
    activated: Vec<String>,
    press_revision: Option<(u64, u64)>,
    queued: Vec<Request>,
    navigation: Option<openagents_chat::router::Screen>,
    notice: Option<String>,
    waker: Option<rust_native_desktop::Waker>,
    image_input: Option<(
        String,
        rust_native_desktop::composer::Stamp,
        std::sync::mpsc::Receiver<crate::chat_images::Result>,
    )>,
}

impl Panel {
    pub fn new(now: Instant) -> Self {
        let transcript = chat_transcript();
        Self {
            commands: openagents_chat_app::commands::Overlay::default(),
            saved: openagents_chat_app::retained::Session::default(),
            saved_visible: false,
            saved_project: None,
            command_query: chat_field("Find a command or chat…"),
            command_token: String::new(),
            menu_point: None,
            menu_navigation: false,
            search: search_field(),
            rename: None,
            rename_pending: None,
            rename_focus: 0,
            aux_rect: None,
            session: Session::new(now),
            ids: BTreeMap::new(),
            fields: BTreeMap::new(),
            submissions: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_submissions: BTreeMap::new(),
            task_editor: BTreeMap::new(),
            born: now,
            viewport: (1200.0, 840.0, 1.0),
            column_width: 768.0,
            transcript,
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
            image_input: None,
        }
    }
    fn import_image(&mut self, source: crate::chat_images::Source) {
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
                    self.notice =
                        Some(result.err().unwrap_or_else(|| {
                            "Image added. Hosted chat accepts text only.".into()
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
        self.waker = Some(waker);
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
                let (width, height) = self.transcript_size;
                if width > 0.0 && height > 0.0 {
                    let _ =
                        self.transcript
                            .update_shared(self.transcript_rows.clone(), width, height);
                }
                self.rows_dirty = false;
            }
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
                            |_| "Continue with recent loaded context · up to 16 KiB",
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
        if self.saved_visible {
            return self
                .saved
                .open_more()
                .map(|(ticket, request)| Request::Saved { ticket, request });
        }
        let at_ms = now.saturating_duration_since(self.born).as_millis() as u64;
        self.poll_images(at_ms);
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
        outcome.or_else(|| {
            let chat = self.session.selected.clone()?;
            let (ticket, request) = self.tasks.get_mut(&chat)?.tick(now)?;
            Some(Request::TaskChat {
                chat,
                ticket,
                request,
            })
        })
    }
    fn busy(&self) -> bool {
        self.task()
            .map_or_else(|| self.session.busy(), task_chat::Session::busy)
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
            if let Some(reason) = self.session.images.hosted_send_refusal(&chat) {
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
        if revision != task.revision {
            self.rows_dirty = true;
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
        if let Some(field) = previous.and_then(|id| self.fields.get_mut(&id)) {
            field.input(TextInput::FocusLost, 0);
        }
        if let Some(id) = &self.session.selected {
            let field = self
                .fields
                .entry(id.clone())
                .or_insert_with(|| chat_field("Message OpenAgents…"));
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
        state.projects.clear();
        let listed = openagents_chat_app::chat_list::search(&self.session.summaries, &state.search);
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
                self.rename = None;
            }
        }
        let previous = self.session.selected.clone();
        let revision = self.session.revision;
        let accepted = self
            .session
            .outcome(ticket, result.map_err(|error| error.to_string()));
        if let Some(snapshot) = self.session.state()
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
            Action::Command { key } => {
                let registry = self.registry();
                let entries = self.commands.entries(&registry);
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
            Action::Palette | Action::Menu | Action::DismissOverlay | Action::Command { .. } => {
                None
            }
            Action::Card { key } => {
                if let Some(action) = self.task().and_then(|task| task.actions.get(&key)).cloned() {
                    return self.task_action(action, view, now);
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
                    openagents_chat_app::cards::Effect::None => None,
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
                if self.task().is_some() {
                    return self.task_action(TaskAction::Send, view, now);
                }
                if let Some(reason) = self.session.images.hosted_send_refusal(&id) {
                    self.notice = Some(reason.into());
                    return None;
                }
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
                if let Some(wake) = self.waker.clone() {
                    field.start(wake);
                }
                field.focused = true;
                // The initial title mounts with the next semantic view.
                self.rename_focus = 0;
                self.rename = Some((title, field));
                None
            }
            Action::CancelRename => {
                self.rename = None;
                None
            }
            Action::SaveName => {
                let title = self.rename.as_ref()?.1.text().to_owned();
                self.save_name(id, title)
            }
            Action::Stop => {
                if self.task().is_some() {
                    self.task_action(TaskAction::Stop, view, now)
                } else {
                    Some(self.request(Command::Stop { chat: id }))
                }
            }
            Action::Retry => {
                if self.task().is_some() {
                    self.task_action(TaskAction::Retry, view, now)
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
    pub fn search_focused(&self) -> bool {
        self.search.focused
    }
    pub fn aux_focused(&self) -> bool {
        self.modal() || self.search.focused
    }
    fn registry(&self) -> Vec<openagents_chat_app::commands::Entry> {
        openagents_chat_app::commands::registry(
            &self.session.summaries,
            if self.saved_visible {
                None
            } else {
                self.session.selected.as_deref()
            },
            !self.saved_visible && self.busy(),
        )
    }
    fn open_commands(&mut self, kind: openagents_chat_app::commands::Kind) {
        self.menu_point = None;
        self.menu_navigation = false;
        self.rename = None;
        self.search.focused = false;
        if let Some(field) = self.field() {
            field.focused = false;
        }
        self.commands.open(kind);
        self.command_token = uuid::Uuid::new_v4().simple().to_string();
        self.command_query = chat_field("Find a command or chat…");
        self.command_query.focused = true;
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
        self.command_query.focused = false;
        self.rename = None;
        self.rename_pending = None;
        if let Some(field) = self.field() {
            field.focused = true;
        }
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
        let Some(action) = openagents_chat_app::commands::shortcut(key, *command, *shift, scope)
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
    fn run_command(
        &mut self,
        action: openagents_chat_app::commands::Action,
        view: &ValidatedView<Intent>,
        now: Instant,
    ) {
        use openagents_chat_app::commands::Action as C;
        let request = match action {
            C::NewChat => Some(self.new_chat()),
            C::Search => {
                if let Some(field) = self.field() {
                    field.focused = false;
                }
                self.search.focused = true;
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
                    let entries = self.commands.entries(&self.registry());
                    self.commands
                        .navigate_entries(*key == "ArrowUp" || (*key == "Tab" && *shift), &entries);
                    return FieldAction::Edited;
                }
                if *key == "Enter" {
                    let entries = self.commands.entries(&self.registry());
                    if let Some(entry) = entries.get(self.commands.selected).filter(|e| e.enabled) {
                        self.activated.push(format!("command:{}", entry.key));
                    }
                    return FieldAction::Edited;
                }
            }
            let result = self.command_query.input(event, at);
            let query = self.command_query.text().to_owned();
            if self.commands.query != query {
                self.commands.query = query;
                self.commands.selected = 0;
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
                && self.rename_focus == 2
            {
                self.close_overlay();
                return FieldAction::Edited;
            }
        }
        if let Some((_, field)) = &mut self.rename {
            let result = if self.rename_focus == 1
                && matches!(&event, TextInput::Key { key: "Enter", .. })
            {
                FieldAction::Send
            } else {
                field.input(event, at)
            };
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
            if matches!(event, SurfaceInput::Down { .. })
                && let Some(field) = self.field()
            {
                field.input(TextInput::FocusLost, at_ms);
            }
            if let Some(action) = self.transcript.pointer(event, &mut self.fonts) {
                let destination = match action {
                    rust_native_desktop::transcript::Action::Activate(key) => {
                        if self.press_revision.take()
                            == Some((
                                self.session.revision,
                                self.task().map_or(0, |task| task.revision),
                            ))
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
        self.task().map_or_else(
            || self.session.next_wake(now),
            |task| self.session.next_wake(now).min(task.next_wake(now)),
        )
    }
    pub fn version(&self, resource: &str) -> Option<u64> {
        match resource {
            COMMAND_QUERY => Some(self.command_query.version()),
            SEARCH => Some(self.search.version()),
            RENAME => self.rename.as_ref().map(|(_, field)| field.version()),
            TRANSCRIPT => Some(self.transcript.version()),
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
            RENAME => Some((available, 56.0)),
            TRANSCRIPT => Some((
                available,
                (self.viewport.1
                    - if available < 620.0 { 288.0 } else { 240.0 }
                    - composer_height
                    - self.image_height(available))
                .max(40.0),
            )),
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
                width: match kind {
                    Kind::Palette => 560,
                    Kind::Menu => 216,
                    Kind::ConfirmArchive => 360,
                },
                placement: if *kind == Kind::Menu {
                    self.menu_point.map_or(
                        OverlayPlacement::TopRight { top: 40, right: 10 },
                        |(x, y)| OverlayPlacement::At { x, y },
                    )
                } else {
                    OverlayPlacement::Center
                },
                scrim: (*kind != Kind::Menu).then_some(Color {
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
        self.overlay_layout()?;
        let mut button = button("chat-latest", "↓  Scroll to bottom", Action::Latest, true);
        button.style.background = Some(Color::rgb(32, 32, 32));
        button.style.text_size = Some(13);
        button.style.line_height = Some(18);
        button.style.button_padding = Some([12, 5]);
        button.style.radius = Some(15);
        let mut pill = stack("chat-latest-pill", Axis::Vertical, vec![button]);
        pill.style.gap = Some(Space::None);
        pill.style.radius = Some(15);
        pill.style.border = Some(openagents_chat_app::visual::BORDER);
        Some(pill)
    }
    fn command_panel(&mut self) -> Node<Intent> {
        if let Some(kind) = &self.commands.kind {
            let entries: Vec<_> = self
                .commands
                .entries(&self.registry())
                .into_iter()
                .filter(|entry| {
                    *kind != openagents_chat_app::commands::Kind::Menu
                        || entry.enabled
                        || entry.action != openagents_chat_app::commands::Action::Restore
                })
                .collect();
            let label = match kind {
                openagents_chat_app::commands::Kind::Palette => "Commands",
                openagents_chat_app::commands::Kind::Menu => "Chat menu",
                openagents_chat_app::commands::Kind::ConfirmArchive => "Archive this conversation?",
            };
            let mut rows = vec![];
            if *kind == openagents_chat_app::commands::Kind::ConfirmArchive {
                let mut heading = text("command-heading", label, TextRole::Body);
                heading.style.text_size = Some(13);
                heading.style.line_height = Some(20);
                heading.style.padding_points = Some([12, 12, 12, 12]);
                rows.push(heading);
            }
            if *kind == openagents_chat_app::commands::Kind::Palette {
                let query = Node {
                    key: "command-query".into(),
                    style: Style::default(),
                    element: Element::Composer {
                        token: self.command_token.clone(),
                        placeholder: "Find a command or chat…".into(),
                        max_bytes: 128,
                        enabled: true,
                        busy: false,
                        stop: None,
                        choices: vec![],
                        draft: Some(self.commands.query.clone()),
                        focus: true,
                    },
                };
                let mut escape = button("command-close", "Esc", Action::DismissOverlay, true);
                escape.style.text_size = Some(11);
                escape.style.line_height = Some(14);
                escape.style.button_padding = Some([6, 2]);
                escape.style.radius = Some(4);
                escape.style.foreground = Some(openagents_chat_app::visual::MUTED);
                escape.style.background = Some(Color::rgb(32, 32, 32));
                let mut header = stack(
                    "command-search-header",
                    Axis::Horizontal,
                    vec![query, escape],
                );
                header.style.padding_points = Some([8, 16, 8, 16]);
                header.style.gap_points = Some(10);
                rows.push(header);
            }
            let mut items = vec![];
            if entries.is_empty() {
                items.push(text(
                    "command-none",
                    "No matching commands.",
                    TextRole::Status,
                ));
            }
            let visible = if *kind == openagents_chat_app::commands::Kind::Palette {
                ((self.viewport.1 - 180.0) / 32.0).clamp(3.0, 10.0) as usize
            } else {
                entries.len()
            };
            for index in self.commands.window_at(entries.len(), visible) {
                let entry = &entries[index];
                use openagents_chat_app::commands::{Action as C, Kind};
                let label = if *kind == Kind::Menu {
                    match entry.action {
                        C::Rename => "Rename…",
                        C::Pin if entry.label.starts_with("Unpin") => "Unpin",
                        C::Pin => "Pin",
                        C::Archive => "Archive",
                        C::Restore => "Unarchive",
                        _ => &entry.label,
                    }
                } else {
                    &entry.label
                };
                let mut row = button(
                    &format!("command-{}", entry.key),
                    label,
                    Action::Command {
                        key: entry.key.clone(),
                    },
                    entry.enabled,
                );
                row.style.align = Some(rust_native::style::TextAlign::Start);
                row.style.text_size = Some(13);
                row.style.line_height = Some(18);
                row.style.button_padding = Some([
                    8,
                    if *kind == openagents_chat_app::commands::Kind::Palette {
                        4
                    } else {
                        6
                    },
                ]);
                row.style.min_height = Some(30);
                row.style.radius = Some(if *kind == openagents_chat_app::commands::Kind::Palette {
                    10
                } else {
                    7
                });
                row.style.background = Some(
                    if index == self.commands.selected
                        && (*kind != Kind::Menu || self.menu_navigation)
                    {
                        openagents_chat_app::visual::SELECTED
                    } else {
                        Color::rgb(16, 16, 16)
                    },
                );
                items.push(row);
            }
            let mut results = stack("command-results", Axis::Vertical, items);
            results.style.padding_points = Some([4, 4, 4, 4]);
            results.style.gap_points = Some(2);
            rows.push(results);
            if *kind == openagents_chat_app::commands::Kind::Palette {
                let mut hint = text(
                    "command-navigation-hint",
                    "↑↓ Navigate    ↵ Select    Esc Close",
                    TextRole::Status,
                );
                hint.style.text_size = Some(11);
                hint.style.line_height = Some(16);
                hint.style.padding_points = Some([7, 16, 7, 16]);
                rows.push(hint);
            }
            let mut panel = stack("command-panel", Axis::Vertical, rows);
            panel.style.background = Some(Color::rgb(16, 16, 16));
            panel.style.border = Some(openagents_chat_app::visual::BORDER);
            panel.style.radius = Some(if *kind == openagents_chat_app::commands::Kind::Palette {
                16
            } else {
                12
            });
            panel.style.gap = Some(Space::None);
            return panel;
        }
        stack("command-panel", Axis::Vertical, vec![])
    }
    pub fn body(&mut self) -> Node<Intent> {
        if self.saved_visible {
            return self.saved_body();
        }
        let start = self.state().map_or(0, |state| state.start);
        if self.rows_dirty {
            let task_rows = self
                .session
                .selected
                .as_ref()
                .and_then(|id| self.tasks.get_mut(id))
                .map(task_chat::Session::rows);
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
                if rows.is_empty() {
                    rows.push(Arc::new(message("welcome".into(),&Turn::assistant("How can we help?\n\nAsk a question, explore an idea, or work through a problem.",None))));
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
                rows.extend(
                    self.session
                        .cards
                        .rows_with(snapshot, busy, self.session.error.as_deref())
                        .into_iter()
                        .map(Arc::new),
                );
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
                let size = self.transcript_size;
                if size.0 > 0.0 && size.1 > 0.0 {
                    let _ =
                        self.transcript
                            .update_shared(self.transcript_rows.clone(), size.0, size.1);
                }
            }
            self.rows_dirty = false;
        }
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
        let mut body = stack("chat-body", Axis::Vertical, children);
        body.style.fill_height = Some(true);
        body
    }
    pub fn footer(&mut self) -> Node<Intent> {
        if self.saved_visible {
            return self.saved_footer();
        }
        if let Some((title, field)) = &self.rename {
            return stack(
                "chat-rename-controls",
                Axis::Vertical,
                vec![
                    Node {
                        key: "chat-rename".into(),
                        style: Style::default(),
                        element: Element::Composer {
                            token: format!(
                                "rename-{}",
                                self.session.selected.as_deref().unwrap_or("")
                            ),
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
                            focus: true,
                        },
                    },
                    stack(
                        "chat-rename-buttons",
                        Axis::Horizontal,
                        vec![
                            button("chat-save-name", "Save title", Action::SaveName, true),
                            button("chat-cancel-name", "Cancel", Action::CancelRename, true),
                        ],
                    ),
                ],
            );
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
        let placeholder = task.map_or("Message OpenAgents…", task_chat::Session::placeholder);
        if let Some(field) = self.fields.get_mut(&id) {
            field.set_placeholder(placeholder);
            field.set_unframed(true);
        }
        let draft = self.fields.get(&id).map(|field| {
            if field.draft.editor().is_none() {
                task.and_then(task_chat::Session::editing_text)
                    .unwrap_or(field.text())
                    .to_owned()
            } else {
                field.text().to_owned()
            }
        });
        let task_mode = task.map(task_chat::Session::mode);
        let task_ready = task.is_none_or(|task| task.summary.is_some());
        let enabled = draft.as_ref().is_some_and(|text| !text.trim().is_empty())
            || !self.session.images.get(&id).is_empty();
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
        let mut buttons = vec![
            icon_button(
                "chat-attach",
                "Attach image",
                Action::AttachImage,
                !busy,
                Glyph::Paperclip,
                false,
            ),
            text("chat-toolbar-space", "", TextRole::Status),
        ];
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
        buttons.push(if busy && task.is_none() {
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
        for image in self.session.images.get(&id) {
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
        let has_previews = !previews.is_empty();
        if has_previews {
            content.push(stack("image-previews", Axis::Wrap, previews));
        }
        let field_width = (self.column_width - 114.0).max(1.0);
        let compact = !has_previews
            && task.is_none_or(|task| !task.active())
            && self.fields.get(&id).is_some_and(|field| {
                !field.text().contains('\n') && field.content_line_count(field_width - 16.0) == 1
            });
        if let Some(field) = self.fields.get_mut(&id) {
            field
                .set_metrics(composer_metrics(compact))
                .expect("valid composer metrics");
        }
        let mut card = if compact {
            let attach = buttons.remove(0);
            buttons.remove(0); // Expanded-only flexible spacer.
            let send = buttons.pop().expect("composer send control");
            let mut card = stack(
                "chat-composer-card",
                Axis::Horizontal,
                vec![attach, composer, send],
            );
            card.style.padding_start = Some(Space::Sm);
            card.style.padding_end = Some(Space::Sm);
            card.style.gap = Some(Space::Xs);
            card
        } else {
            let mut toolbar = stack("chat-send-controls", Axis::Horizontal, buttons);
            toolbar.style.padding_points = Some([2, 8, 8, 8]);
            toolbar.style.min_height = Some(42);
            stack(
                "chat-composer-card",
                Axis::Vertical,
                vec![composer, toolbar],
            )
        };
        card.style.background = Some(openagents_chat_app::visual::COMPOSER);
        card.style.radius = Some(26);
        card.style.border = Some(openagents_chat_app::visual::COMPOSER_BORDER);
        card.style.gap = Some(if compact { Space::Xs } else { Space::None });
        content.push(card);
        let mut footer = stack("chat-footer", Axis::Vertical, content);
        footer.style.padding_start = Some(Space::Md);
        footer.style.padding_end = Some(Space::Md);
        footer
    }
}
fn chat_transcript() -> Transcript {
    let mut transcript = Transcript::default();
    transcript.set_font_family(rust_native::layout::display::FontFamily::Geist);
    transcript
        .set_metrics(openagents_chat_app::visual::TRANSCRIPT)
        .expect("valid chat metrics");
    transcript.set_palette(&openagents_chat_app::visual::COLORS);
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
    let mut field = chat_field("Filter sessions…");
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
    field.set_font_family(rust_native::layout::display::FontFamily::Geist);
    field
        .set_metrics(composer_metrics(true))
        .expect("valid composer metrics");
    field.set_colors(
        openagents_chat_app::visual::TEXT,
        openagents_chat_app::visual::FAINT,
        Color::rgb(129, 140, 248),
    );
    field
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
        style: Style {
            background: Some(openagents_chat_app::visual::SELECTED),
            foreground: Some(openagents_chat_app::visual::TEXT),
            weight: Some(TextWeight::Normal),
            ..Style::default()
        },
        element: Element::Button {
            label: label.into(),
            enabled,
            icon: None,
            intent: Intent::Chat { action },
        },
    }
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
    node.style.background = Some(if primary {
        openagents_chat_app::visual::TEXT
    } else {
        Color {
            alpha: 0,
            ..Color::rgb(0, 0, 0)
        }
    });
    node.style.foreground = Some(if primary {
        openagents_chat_app::visual::SIDEBAR
    } else {
        openagents_chat_app::visual::MUTED
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
}
