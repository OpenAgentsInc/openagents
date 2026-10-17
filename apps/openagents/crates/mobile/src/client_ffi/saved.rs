//! Native controls over the desktop's account-bound saved-history models.
use super::{CoreError, CoreResult};
use openagents_account::{
    Lease, Snapshot as AccountUpdate,
    history::{History, LoadState},
    openagents::{AttachmentRef, ChatSource, MessageRole, SavedMessage},
    reader::{Reader, text_part},
    turns::Turns,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio_util::sync::CancellationToken;

#[derive(Clone, uniffi::Enum)]
pub enum SavedLoadState {
    Idle,
    Loading,
    Ready,
    Failed { message: String, stale: bool },
}
impl From<LoadState> for SavedLoadState {
    fn from(value: LoadState) -> Self {
        match value {
            LoadState::Idle => Self::Idle,
            LoadState::Loading => Self::Loading,
            LoadState::Ready => Self::Ready,
            LoadState::Failed { message, stale } => Self::Failed { message, stale },
        }
    }
}
#[derive(Clone, uniffi::Record)]
pub struct SavedChatRow {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub group: String,
}
#[derive(Clone, uniffi::Record)]
pub struct SavedProjectRow {
    pub id: String,
    pub name: String,
}
#[derive(Clone, uniffi::Record)]
pub struct SavedAttachment {
    pub key: String,
    pub name: String,
    pub available: bool,
}
#[derive(Clone, uniffi::Record)]
pub struct SavedMessageRow {
    pub id: String,
    pub role: String,
    pub preview: String,
}
#[derive(Clone, uniffi::Record)]
pub struct SavedMessagePart {
    pub role: String,
    pub text: String,
    pub part: u64,
    pub parts: u64,
    pub files: Vec<SavedAttachment>,
}
#[derive(Clone, uniffi::Record)]
pub struct SavedTranscript {
    pub can_reply: bool,
    pub answering: bool,
    pub stopping: bool,
    pub retry_send: bool,
    pub send_error: Option<String>,
    pub accepted: u64,
    pub title: Option<String>,
    pub messages: Vec<SavedMessageRow>,
    pub captures: Vec<SavedAttachment>,
    pub first: u64,
    pub received: u64,
    pub has_more: bool,
    pub continue_required: bool,
    pub smaller_page: bool,
    pub reload_required: bool,
    pub login_required: bool,
    pub access_lost: bool,
    pub state: SavedLoadState,
    pub download_name: Option<String>,
    pub download_state: SavedLoadState,
}
#[derive(Clone, uniffi::Record)]
pub struct SavedHistorySnapshot {
    pub signed_in: bool,
    pub login_required: bool,
    pub archived: bool,
    pub project: Option<String>,
    pub rows: Vec<SavedChatRow>,
    pub projects: Vec<SavedProjectRow>,
    pub has_more: bool,
    pub more_projects: bool,
    pub state: SavedLoadState,
    pub projects_state: SavedLoadState,
    pub transcript: SavedTranscript,
}
#[derive(Clone, uniffi::Record)]
pub struct SavedFileBytes {
    pub name: String,
    pub bytes: Vec<u8>,
}
#[derive(Clone, uniffi::Record)]
pub struct SavedFilePart {
    pub text: Option<String>,
    pub part: u64,
    pub parts: u64,
}
#[derive(Clone, uniffi::Enum)]
pub enum SavedHistoryAction {
    MoreChats,
    RefreshChats,
    ToggleArchived,
    MoreProjects,
    RefreshProjects,
    SelectProject { id: Option<String> },
    Open { id: String },
    Close,
    MoreMessages,
    RefreshMessages,
    ContinueReading,
    SmallerPage,
    Download { key: String },
    CloseDownload,
    Send { text: String },
    RetrySend,
    StopAnswer,
}
#[uniffi::export(with_foreign)]
pub trait SavedHistoryListener: Send + Sync {
    fn on_saved_history_changed(&self);
}

#[derive(uniffi::Object)]
pub struct CoreSavedHistory {
    lease: Lease,
    history: Arc<History>,
    reader: Arc<Reader>,
    turns: Arc<Turns>,
    archived: AtomicBool,
    visible: AtomicBool,
    foreground: AtomicBool,
    cancel: CancellationToken,
    listener: Arc<dyn SavedHistoryListener>,
}
fn unavailable() -> CoreError {
    CoreError::Auth {
        message: "Sign in again to see your chats.".into(),
    }
}
fn failed(_: impl std::fmt::Display) -> CoreError {
    CoreError::NotFound {
        message: "This saved content is no longer available. Refresh and try again.".into(),
    }
}
fn role(message: &SavedMessage) -> String {
    match message.role {
        MessageRole::User => "You",
        MessageRole::Assistant => "Assistant",
        MessageRole::Tool => "Tool",
        MessageRole::Thinking => "Thinking",
        MessageRole::Other(_) => "Message",
    }
    .into()
}
fn body(message: &SavedMessage) -> String {
    let mut text = message.text.clone();
    if let Some(tool) = &message.tool {
        text.push_str(&format!(
            "\n\n{} — {} ({})\n\nInput\n{}\n\nOutput\n{}",
            tool.name,
            tool.summary,
            tool.status.as_str(),
            tool.input,
            tool.output
        ));
    }
    if !message.extra.is_empty() {
        text.push_str("\n\nSaved details\n");
        text.push_str(&serde_json::to_string_pretty(&message.extra).unwrap_or_default());
    }
    text
}
fn message_files(message: &SavedMessage) -> Vec<SavedAttachment> {
    message
        .files
        .iter()
        .map(|file| SavedAttachment {
            key: format!("file:{}:{}", message.id, file.id),
            name: file.name.clone(),
            available: file.download_ref().is_some(),
        })
        .collect()
}
impl CoreSavedHistory {
    pub(crate) fn new(
        lease: Lease,
        mut account: tokio::sync::watch::Receiver<AccountUpdate>,
        listener: Arc<dyn SavedHistoryListener>,
    ) -> Arc<Self> {
        let history = History::new();
        let reader = Reader::new();
        let turns = Turns::new(reader.clone());
        let mut turn_updates = turns.watch();
        history.reset(Some(lease.clone()));
        let mut lists = history.watch();
        let mut messages = reader.watch();
        let this = Arc::new(Self {
            lease: lease.clone(),
            history: history.clone(),
            reader: reader.clone(),
            turns: turns.clone(),
            archived: AtomicBool::new(false),
            visible: AtomicBool::new(false),
            foreground: AtomicBool::new(true),
            cancel: CancellationToken::new(),
            listener: listener.clone(),
        });
        let cancel = this.cancel.clone();
        let weak = Arc::downgrade(&this);
        openagents_client::runtime::shared().spawn(async move {
            listener.on_saved_history_changed();
            let mut timer = tokio::time::interval(std::time::Duration::from_secs(2));
            timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => break,
                    changed = account.changed() => {
                        if changed.is_err() || !lease.is_current() {
                            history.reset(None); reader.clear(); turns.clear();
                            listener.on_saved_history_changed(); break;
                        }
                    }
                    _ = timer.tick() => {
                        reader.poll().await;
                        if weak.upgrade().is_some_and(|s|s.visible.load(Ordering::SeqCst) && s.foreground.load(Ordering::SeqCst)) {
                            openagents_client::runtime::shared().spawn(turns.follow());
                        }
                    },
                    changed = turn_updates.changed() => {
                        if changed.is_err() {break;}
                        listener.on_saved_history_changed();
                    },
                    changed = lists.changed() => {
                        if changed.is_err() { break; }
                        listener.on_saved_history_changed();
                    }
                    changed = messages.changed() => {
                        if changed.is_err() { break; }
                        listener.on_saved_history_changed();
                    }
                }
            }
        });
        openagents_client::runtime::shared().spawn(this.history.more());
        openagents_client::runtime::shared().spawn(this.history.more_projects());
        this
    }
    fn current(&self) -> bool {
        !self.cancel.is_cancelled() && self.lease.is_current()
    }
    fn refresh_lists(&self) {
        let runtime = openagents_client::runtime::shared();
        if self.history.refresh().is_ok() {
            runtime.spawn(self.history.more());
        }
        if self.history.refresh_projects().is_ok() {
            runtime.spawn(self.history.more_projects());
        }
    }
    fn attachment(&self, key: &str) -> Option<(AttachmentRef, String)> {
        let snapshot = self.reader.snapshot();
        for message in snapshot.messages.iter() {
            for file in &message.files {
                if key == format!("file:{}:{}", message.id, file.id) {
                    return file.download_ref().cloned().map(|r| (r, file.name.clone()));
                }
            }
        }
        snapshot
            .captures
            .iter()
            .find(|c| key == format!("capture:{}", c.id))
            .and_then(|c| c.download_ref().cloned().map(|r| (r, c.name.clone())))
    }
}
#[uniffi::export]
impl CoreSavedHistory {
    /// Render account history through the same Rust layout engine as computer sessions.
    pub fn render_transcript(&self, view: Arc<crate::layout::TranscriptView>) -> CoreResult<()> {
        if !self.lease.is_current() {
            view.set_input(Default::default());
            return Err(unavailable());
        }
        let snapshot = self.reader.snapshot();
        view.set_input(crate::layout::TranscriptInput {
            entries: self
                .reader
                .target()
                .map_or_else(
                    || openagents_account::chat_entries::entries(&snapshot.messages, snapshot.epoch),
                    |target| {
                        openagents_account::chat_entries::entries_for_chat(
                            &snapshot.messages,
                            snapshot.epoch,
                            &target.chat,
                        )
                    },
                )
                .into_iter()
                .map(Arc::new)
                .collect(),
            ..Default::default()
        });
        Ok(())
    }

    /// Download only references in the currently authorized transcript.
    pub async fn attachment_bytes(&self, path: String) -> CoreResult<Vec<u8>> {
        if !self.current() {
            return Err(unavailable());
        }
        let reader = self.reader.clone();
        openagents_client::runtime::shared()
            .spawn(async move { reader.attachment(&path).await })
            .await
            .map_err(failed)?
            .map_err(failed)
    }

    pub fn attachments(&self) -> Vec<SavedAttachment> {
        if !self.current() {
            return vec![];
        }
        let snapshot = self.reader.snapshot();
        let mut files: Vec<_> = snapshot.messages.iter().flat_map(message_files).collect();
        files.extend(snapshot.captures.iter().map(|capture| SavedAttachment {
            key: format!("capture:{}", capture.id),
            name: capture.name.clone(),
            available: capture.download_ref().is_some(),
        }));
        files
    }

    pub fn snapshot(&self) -> SavedHistorySnapshot {
        let active = self.current();
        let h = if active {
            self.history.snapshot()
        } else {
            Default::default()
        };
        let r = if active {
            self.reader.snapshot()
        } else {
            Default::default()
        };
        let turns = if active {
            self.turns.snapshot()
        } else {
            Default::default()
        };
        let archived = self.archived.load(Ordering::SeqCst);
        let rows = h
            .visible_rows(archived)
            .into_iter()
            .map(|row| {
                let source = match row.source {
                    ChatSource::Web => "Web",
                    ChatSource::Terminal => "Terminal",
                    ChatSource::Phone => "Phone",
                    ChatSource::Other(_) => "Saved chat",
                };
                let group = if row.pinned_unix.is_some() {
                    "Pinned".into()
                } else {
                    h.projects
                        .iter()
                        .find(|p| Some(p.id()) == row.project.as_deref())
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| "Chats".into())
                };
                SavedChatRow {
                    id: row.chat.id().into(),
                    title: row.title,
                    subtitle: format!("{source} · {} messages", row.messages),
                    group,
                }
            })
            .collect();
        SavedHistorySnapshot {
            signed_in: active && h.signed_in,
            login_required: h.login_required,
            archived,
            project: h.project.map(|p| p.name),
            rows,
            projects: h
                .projects
                .into_iter()
                .filter(|p| p.shared)
                .map(|p| SavedProjectRow {
                    id: p.id().into(),
                    name: p.name,
                })
                .collect(),
            has_more: h.has_more,
            more_projects: h.more_projects,
            state: h.state.into(),
            projects_state: h.projects_state.into(),
            transcript: SavedTranscript {
                can_reply: active
                    && r.summary
                        .as_ref()
                        .is_some_and(|s| s.source == ChatSource::Web),
                answering: turns.busy,
                stopping: turns.stopping,
                retry_send: turns.retry,
                send_error: turns.error,
                accepted: turns.accepted,
                title: r.summary.map(|s| s.title),
                messages: r
                    .messages
                    .iter()
                    .map(|m| SavedMessageRow {
                        id: m.id.clone(),
                        role: role(m),
                        preview: m.text.chars().take(512).collect(),
                    })
                    .collect(),
                captures: r
                    .captures
                    .iter()
                    .map(|c| SavedAttachment {
                        key: format!("capture:{}", c.id),
                        name: c.name.clone(),
                        available: c.download_ref().is_some(),
                    })
                    .collect(),
                first: r.first as u64,
                received: r.received as u64,
                has_more: r.has_more,
                continue_required: r.continue_required,
                smaller_page: r.smaller_page,
                reload_required: r.reload_required,
                login_required: r.login_required,
                access_lost: r.access_lost,
                state: r.state.into(),
                download_name: r.download.as_ref().map(|d| d.name.clone()),
                download_state: r
                    .download
                    .map(|d| d.state.into())
                    .unwrap_or(SavedLoadState::Idle),
            },
        }
    }
    /// Native view visibility controls following; navigation never grants write access.
    pub fn follow(&self, visible: bool) {
        self.visible.store(visible, Ordering::SeqCst);
        if visible && self.foreground.load(Ordering::SeqCst) {
            openagents_client::runtime::shared().spawn(self.turns.resume());
        } else {
            self.turns.pause();
        }
        self.reader
            .set_following(visible && self.foreground.load(Ordering::SeqCst));
    }
    pub fn set_foreground(&self, active: bool) {
        let was_active = self.foreground.swap(active, Ordering::SeqCst);
        if active && self.visible.load(Ordering::SeqCst) {
            openagents_client::runtime::shared().spawn(self.turns.resume());
        } else {
            self.turns.pause();
        }
        self.reader
            .set_following(active && self.visible.load(Ordering::SeqCst));
        if active && !was_active && self.current() {
            self.refresh_lists();
        }
    }
    pub fn action(&self, action: SavedHistoryAction) -> CoreResult<()> {
        if !self.current() {
            return Err(unavailable());
        }
        let runtime = openagents_client::runtime::shared();
        match action {
            SavedHistoryAction::MoreChats => {
                runtime.spawn(self.history.more());
            }
            SavedHistoryAction::RefreshChats => {
                self.refresh_lists();
            }
            SavedHistoryAction::ToggleArchived => {
                self.archived.fetch_xor(true, Ordering::SeqCst);
                self.listener.on_saved_history_changed();
            }
            SavedHistoryAction::MoreProjects => {
                runtime.spawn(self.history.more_projects());
            }
            SavedHistoryAction::RefreshProjects => {
                self.history.refresh_projects().map_err(failed)?;
                runtime.spawn(self.history.more_projects());
            }
            SavedHistoryAction::SelectProject { id } => {
                let snapshot = self.history.snapshot();
                let project = id
                    .as_ref()
                    .map(|id| {
                        snapshot
                            .projects
                            .iter()
                            .find(|p| p.id() == id && p.shared)
                            .ok_or_else(unavailable)
                    })
                    .transpose()?;
                self.turns.clear();
                self.reader.clear();
                self.history.project(project).map_err(failed)?;
                self.archived.store(false, Ordering::SeqCst);
                runtime.spawn(self.history.more());
            }
            SavedHistoryAction::Open { id } => {
                self.turns.clear();
                self.reader
                    .open_latest(self.history.existing(&id).ok_or_else(unavailable)?)
                    .map_err(failed)?;
                runtime.spawn(self.reader.more());
            }
            SavedHistoryAction::Close => {
                self.turns.clear();
                self.reader.clear();
            }
            SavedHistoryAction::MoreMessages => {
                runtime.spawn(self.reader.more());
            }
            SavedHistoryAction::RefreshMessages => {
                self.reader.refresh().map_err(failed)?;
                runtime.spawn(self.reader.more());
            }
            SavedHistoryAction::ContinueReading => {
                self.reader.continue_window();
                runtime.spawn(self.reader.more());
            }
            SavedHistoryAction::SmallerPage => {
                self.reader.smaller_page();
                runtime.spawn(self.reader.more());
            }
            SavedHistoryAction::Download { key } => {
                let (reference, name) = self.attachment(&key).ok_or_else(unavailable)?;
                runtime.spawn(self.reader.download(reference, name));
            }
            SavedHistoryAction::CloseDownload => self.reader.close_download(),
            SavedHistoryAction::Send { text } => {
                runtime.spawn(self.turns.submit(text));
            }
            SavedHistoryAction::RetrySend => {
                runtime.spawn(self.turns.retry());
            }
            SavedHistoryAction::StopAnswer => {
                runtime.spawn(self.turns.stop());
            }
        }
        Ok(())
    }
    pub fn message(&self, id: String, part: u64) -> CoreResult<SavedMessagePart> {
        if !self.current() {
            return Err(unavailable());
        }
        let snapshot = self.reader.snapshot();
        let message = snapshot
            .messages
            .iter()
            .find(|m| m.id == id)
            .ok_or_else(unavailable)?;
        let (text, parts, part) = text_part(&body(message), part.try_into().unwrap_or(usize::MAX));
        Ok(SavedMessagePart {
            role: role(message),
            text,
            part: part as u64,
            parts: parts as u64,
            files: message_files(message),
        })
    }
    pub fn downloaded_file(&self) -> CoreResult<SavedFileBytes> {
        if !self.current() {
            return Err(unavailable());
        }
        let snapshot = self.reader.snapshot();
        let download = snapshot.download.ok_or_else(unavailable)?;
        let bytes = self
            .reader
            .bytes_for_save(snapshot.epoch, &download.reference)
            .ok_or_else(unavailable)?;
        Ok(SavedFileBytes {
            name: download.name,
            bytes: bytes.as_ref().clone(),
        })
    }
    pub fn downloaded_file_part(&self, part: u64) -> CoreResult<SavedFilePart> {
        if !self.current() {
            return Err(unavailable());
        }
        let snapshot = self.reader.snapshot();
        let download = snapshot.download.ok_or_else(unavailable)?;
        let bytes = self
            .reader
            .bytes_for_save(snapshot.epoch, &download.reference)
            .ok_or_else(unavailable)?;
        let Some(text) = std::str::from_utf8(&bytes).ok() else {
            return Ok(SavedFilePart {
                text: None,
                part: 0,
                parts: 1,
            });
        };
        let (text, parts, part) = text_part(text, part.try_into().unwrap_or(usize::MAX));
        Ok(SavedFilePart {
            text: Some(text),
            part: part as u64,
            parts: parts as u64,
        })
    }
    pub fn shutdown(&self) {
        self.cancel.cancel();
        self.turns.clear();
        self.history.reset(None);
        self.reader.clear();
        self.listener.on_saved_history_changed();
    }
}
impl Drop for CoreSavedHistory {
    fn drop(&mut self) {
        self.cancel.cancel();
        self.turns.clear();
        self.history.reset(None);
        self.reader.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_parts_preserve_unicode_and_clamp_a_stale_part() {
        let text = "🦀é".repeat(40_000);
        let (_, count, _) = text_part(&text, 0);
        let joined: String = (0..count).map(|n| text_part(&text, n).0).collect();
        assert_eq!(joined, text);
        assert_eq!(text_part(&text, usize::MAX).2, count - 1);
    }
}
