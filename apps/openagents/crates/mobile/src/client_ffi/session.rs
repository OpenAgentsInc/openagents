//! The per-chat FFI object: composer-facing state and every command.
//! Transcript rows never cross here — the layout engine reads
//! [`openagents_client::SessionSnapshot`] in Rust.

use std::sync::Arc;
use std::sync::Mutex;

use openagents_client as zc;
use rust_native::edit::mirror::{Mirror, Stamp as EditorStamp};
use rust_native::view::{Activation, FieldChange};

use super::composer_drafts::ComposerDraftStore;
use super::semantic_bridge::{
    NativeActionResult, NativeViewRegistry, SessionViewIntent, answer_input, lock_registry,
    map_send_outcome,
};
use super::types::{CoreError, CoreResult, SendState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BusyPolicy {
    /// Park it on the shared queue until the turn ends (default).
    Queue,
    /// Deliver mid-turn (steer).
    Steer,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct OutgoingAttachment {
    pub name: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WorktreeSpec {
    /// The repo to branch (the project's folder on the host).
    pub repo_path: String,
    /// Base ref for the fresh `openagents/<name>` branch.
    pub base: String,
    pub space_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct SendRequest {
    pub text: String,
    pub attachments: Vec<OutgoingAttachment>,
    /// Isolated host worktree for this run (new-session "New worktree").
    pub worktree: Option<WorktreeSpec>,
    pub busy: BusyPolicy,
}

impl From<SendRequest> for zc::SendRequest {
    fn from(r: SendRequest) -> Self {
        zc::SendRequest {
            text: r.text,
            attachments: r
                .attachments
                .into_iter()
                .map(|a| zc::OutgoingAttachment {
                    name: a.name,
                    mime_type: a.mime_type,
                    data: a.data,
                })
                .collect(),
            worktree: r.worktree.map(|w| zc::WorktreeSpec {
                repo_path: w.repo_path,
                base: w.base,
                space_id: w.space_id,
            }),
            busy: match r.busy {
                BusyPolicy::Queue => zc::BusyPolicy::Queue,
                BusyPolicy::Steer => zc::BusyPolicy::Steer,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum SendOutcome {
    /// A new turn was requested; the echo's id is `message_id`.
    Started { message_id: String },
    /// Delivered into the live turn.
    Steered { message_id: String },
    /// Parked on the shared queue.
    Queued { queue_id: String },
}

impl From<zc::SendOutcome> for SendOutcome {
    fn from(o: zc::SendOutcome) -> Self {
        match o {
            zc::SendOutcome::Started { message_id } => SendOutcome::Started { message_id },
            zc::SendOutcome::Steered { message_id } => SendOutcome::Steered { message_id },
            zc::SendOutcome::Queued { queue_id } => SendOutcome::Queued { queue_id },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct HostCapabilities {
    pub message_queue: bool,
    pub queue_actions: bool,
    pub queue_attachments: bool,
    pub clean_attachment_text: bool,
    pub queue_edit_lease: bool,
    /// Host ≥ 0.2.12: attachments ride `pending://` refs.
    pub queued_attachments: bool,
    /// The chat's harness steers mid-turn (unknown until a live catalog).
    pub mid_turn_steering: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct HostInfo {
    pub device_id: String,
    pub name: Option<String>,
    pub online: bool,
    pub capabilities: HostCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct LiveStatus {
    /// Display status (an in-flight send reads Working).
    pub indicator: super::types::ChatIndicator,
    /// A turn is actually running on the host (sends queue/steer).
    pub turn_running: bool,
    pub working_since_ms: Option<i64>,
    /// The newest transcript entry is streaming.
    pub streaming: bool,
    /// Stop is meaningful.
    pub can_interrupt: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum QueueGate {
    /// Someone holds an edit lease; the row won't deliver until released.
    Editing {
        owner_device_id: String,
        expires_at_ms: i64,
        mine: bool,
    },
    /// A lease lapsed — a person must review before it sends.
    ReviewRequired { owner_device_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct QueueItem {
    pub id: String,
    /// Raw row text (what the host will send).
    pub text: String,
    /// User-visible text (trailer/Appshot context stripped).
    pub visible_text: String,
    pub attachments: Vec<String>,
    pub hold_for_turn_end: bool,
    pub issued_by: String,
    pub from_this_device: bool,
    pub issued_at_ms: i64,
    pub edited_at_ms: Option<i64>,
    pub gate: Option<QueueGate>,
    /// A Send-now / Remove is awaiting the host's acknowledgement.
    pub action_pending: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum PendingKind {
    Run,
    Steer,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PendingSend {
    /// Client-minted id (the host's user entry reuses it).
    pub message_id: String,
    /// Raw content (attachment trailer included).
    pub text: String,
    pub visible_text: String,
    pub images: Vec<String>,
    pub kind: PendingKind,
    pub sent_at_ms: i64,
    pub state: SendState,
}

impl From<&zc::PendingSend> for PendingSend {
    fn from(p: &zc::PendingSend) -> Self {
        Self {
            message_id: p.message_id.clone(),
            text: p.text.clone(),
            visible_text: p.visible_text.clone(),
            images: p.images.clone(),
            kind: match p.kind {
                zc::PendingKind::Run => PendingKind::Run,
                zc::PendingKind::Steer => PendingKind::Steer,
            },
            sent_at_ms: p.sent_at_ms,
            state: p.state.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct UserInputQuestion {
    pub id: String,
    pub header: String,
    pub question: String,
    pub options: Vec<String>,
    pub multi_select: bool,
    pub prefill: Option<String>,
    pub multiline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct UserInputAnswer {
    pub question_id: String,
    /// Picked option labels (or free text as a single label).
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct InputRequest {
    pub entry_id: String,
    pub request_id: String,
    pub questions: Vec<UserInputQuestion>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RoomState {
    pub connected: bool,
    pub retry_at_ms: Option<i64>,
    /// Graced degradation for this chat.
    pub degraded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct ContextUsage {
    pub tokens: Option<u64>,
    pub window: Option<u64>,
}

/// Everything the composer and its status strip render.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ComposerState {
    pub chat_id: String,
    pub revision: u64,
    pub title: String,
    pub host: HostInfo,
    pub live: LiveStatus,
    /// The shared queue, delivery order.
    pub queue: Vec<QueueItem>,
    /// This device's unadopted sends, oldest first.
    pub pending_sends: Vec<PendingSend>,
    /// Headline state of the oldest pending send.
    pub send_state: Option<SendState>,
    /// A send now would queue rather than deliver promptly.
    pub delivery_degraded: bool,
    pub room: RoomState,
    /// Attachment escort progress (0..1).
    pub transfer_progress: Option<f64>,
    /// Answer this instead of typing.
    pub open_input: Option<InputRequest>,
    /// Last queue action failure.
    pub queue_error: Option<String>,
    /// Scroll-to-own-send target.
    pub last_submitted_message_id: Option<String>,
    pub context_usage: Option<ContextUsage>,
}

impl From<&zc::ComposerState> for ComposerState {
    fn from(c: &zc::ComposerState) -> Self {
        let caps = &c.host.capabilities;
        Self {
            chat_id: c.chat_id.clone(),
            revision: c.revision,
            title: c.title.clone(),
            host: HostInfo {
                device_id: c.host.device_id.clone(),
                name: c.host.name.clone(),
                online: c.host.online,
                capabilities: HostCapabilities {
                    message_queue: caps.message_queue,
                    queue_actions: caps.queue_actions,
                    queue_attachments: caps.queue_attachments,
                    clean_attachment_text: caps.clean_attachment_text,
                    queue_edit_lease: caps.queue_edit_lease,
                    queued_attachments: caps.queued_attachments,
                    mid_turn_steering: caps.mid_turn_steering,
                },
            },
            live: LiveStatus {
                indicator: c.live.indicator.into(),
                turn_running: c.live.turn_running,
                working_since_ms: c.live.working_since_ms,
                streaming: c.live.streaming,
                can_interrupt: c.live.can_interrupt,
            },
            queue: c
                .queue
                .iter()
                .map(|q| QueueItem {
                    id: q.id.clone(),
                    text: q.text.clone(),
                    visible_text: q.visible_text.clone(),
                    attachments: q.attachments.clone(),
                    hold_for_turn_end: q.hold_for_turn_end,
                    issued_by: q.issued_by.clone(),
                    from_this_device: q.from_this_device,
                    issued_at_ms: q.issued_at_ms,
                    edited_at_ms: q.edited_at_ms,
                    gate: q.gate.as_ref().map(|g| match g {
                        zc::QueueGate::Editing {
                            owner_device_id,
                            expires_at_ms,
                            mine,
                        } => QueueGate::Editing {
                            owner_device_id: owner_device_id.clone(),
                            expires_at_ms: *expires_at_ms,
                            mine: *mine,
                        },
                        zc::QueueGate::ReviewRequired { owner_device_id } => {
                            QueueGate::ReviewRequired {
                                owner_device_id: owner_device_id.clone(),
                            }
                        }
                    }),
                    action_pending: q.action_pending,
                })
                .collect(),
            pending_sends: c.pending_sends.iter().map(Into::into).collect(),
            send_state: c.send_state.map(Into::into),
            delivery_degraded: c.delivery_degraded,
            room: RoomState {
                connected: c.room.connected,
                retry_at_ms: c.room.retry_at_ms,
                degraded: c.room.degraded,
            },
            transfer_progress: c.transfer_progress,
            open_input: c.open_input.as_ref().map(|i| InputRequest {
                entry_id: i.entry_id.clone(),
                request_id: i.request_id.clone(),
                questions: i
                    .questions
                    .iter()
                    .map(|q| UserInputQuestion {
                        id: q.id.clone(),
                        header: q.header.clone(),
                        question: q.question.clone(),
                        options: q.options.clone(),
                        multi_select: q.multi_select,
                        prefill: q.prefill.clone(),
                        multiline: q.multiline,
                    })
                    .collect(),
            }),
            queue_error: c.queue_error.clone(),
            last_submitted_message_id: c.last_submitted_message_id.clone(),
            context_usage: c.context_usage.map(|u| ContextUsage {
                tokens: u.tokens,
                window: u.window,
            }),
        }
    }
}

/// Transcript-level facts the platform needs without the rows (loader vs
/// empty state, scroll-to-bottom affordances, the working pill).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TranscriptStatus {
    pub revision: u64,
    /// Content present (false = show a loader, not an empty chat).
    pub hydrated: bool,
    /// Host-written entries.
    pub entry_count: u32,
    pub pending_count: u32,
    pub streaming: bool,
    pub working: bool,
    pub working_since_ms: Option<i64>,
    /// Id of the newest entry (host or echo).
    pub last_entry_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct QueueEditLease {
    pub row_id: String,
    pub lease_id: String,
    /// Row text at lease time.
    pub text: String,
    pub base_text_hash: String,
    pub expires_at_ms: i64,
}

impl From<zc::QueueEditLease> for QueueEditLease {
    fn from(l: zc::QueueEditLease) -> Self {
        Self {
            row_id: l.row_id,
            lease_id: l.lease_id,
            text: l.text,
            base_text_hash: l.base_text_hash,
            expires_at_ms: l.expires_at_ms,
        }
    }
}

impl From<QueueEditLease> for zc::QueueEditLease {
    fn from(l: QueueEditLease) -> Self {
        Self {
            row_id: l.row_id,
            lease_id: l.lease_id,
            text: l.text,
            base_text_hash: l.base_text_hash,
            expires_at_ms: l.expires_at_ms,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum QueueEditStart {
    Acquired {
        lease: QueueEditLease,
    },
    /// Another editor holds the row.
    Locked,
    /// The row left the queue.
    Missing,
    /// Host unreachable / capability missing.
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum QueueEditAction {
    /// Save the new text.
    Commit,
    /// Keep the old text, release the row.
    Cancel,
    /// Delete the row.
    Discard,
    /// Release without changing anything.
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum QueueEditFinish {
    Finished,
    /// The row changed under the lease.
    Conflict,
    Missing,
    /// The lease was superseded or expired.
    Lost,
    Unavailable,
}

/// One open chat. Obtain via `CoreClient.open_session`.
#[derive(uniffi::Object)]
pub struct SessionHandle {
    pub(crate) inner: zc::SessionHandle,
    drafts: Arc<ComposerDraftStore>,
    native_views: std::sync::Mutex<NativeViewRegistry>,
    composer_editor: Mutex<Mirror>,
}

impl SessionHandle {
    pub(crate) fn new(inner: zc::SessionHandle, drafts: Arc<ComposerDraftStore>) -> Arc<Self> {
        Arc::new(Self {
            inner,
            drafts,
            native_views: std::sync::Mutex::new(NativeViewRegistry::default()),
            composer_editor: Mutex::new(Mirror::default()),
        })
    }

    /// The Rust handle (transcript snapshots + subscription for the layout
    /// engine).
    #[allow(dead_code)]
    pub fn handle(&self) -> &zc::SessionHandle {
        &self.inner
    }
}

/// Run an async client call on the client runtime (executor-agnostic).
async fn on_runtime<T, F>(fut: F) -> CoreResult<T>
where
    F: std::future::Future<Output = Result<T, zc::ClientError>> + Send + 'static,
    T: Send + 'static,
{
    zc::runtime::run(fut).await.map_err(Into::into)
}

#[uniffi::export]
impl SessionHandle {
    pub fn chat_id(&self) -> String {
        self.inner.chat_id().to_owned()
    }

    /// Composer + status strip state (pull after `ComposerChanged`).
    pub fn composer(&self) -> ComposerState {
        (&*self.inner.composer()).into()
    }

    /// Save this session's bounded text draft in the signed-in account store.
    pub fn save_composer_draft(&self, text: String) -> CoreResult<()> {
        self.drafts.save(self.inner.chat_id(), &text)
    }

    /// Restore the saved text draft for this session.
    pub fn restore_composer_draft(&self) -> CoreResult<Option<String>> {
        self.drafts.restore(self.inner.chat_id())
    }

    /// Remove this session's saved draft after sending or cancelling it.
    pub fn clear_composer_draft(&self) -> CoreResult<()> {
        self.drafts.save(self.inner.chat_id(), "")
    }

    /// Edit the native composer through Rust Native's UTF-16/IME mirror.
    /// Successful changes are saved in the signed-in account's draft store.
    pub fn composer_editor(
        &self,
        instance: String,
        view_revision: u64,
        request_json: Vec<u8>,
    ) -> CoreResult<Vec<u8>> {
        let mut editor = self.composer_editor.lock().map_err(|_| CoreError::Closed)?;
        if request_json.len() > rust_native::edit::mirror::MAX_REQUEST_BYTES {
            return Err(CoreError::InvalidArgument {
                message: "composer edit exceeds its byte limit".into(),
            });
        }
        let mut request: serde_json::Value =
            serde_json::from_slice(&request_json).map_err(|_| CoreError::InvalidArgument {
                message: "composer edit is invalid".into(),
            })?;
        let token = request
            .get("token")
            .and_then(serde_json::Value::as_str)
            .or_else(|| request.get("stamp")?.get("token")?.as_str())
            .map(str::to_owned)
            .ok_or_else(|| CoreError::InvalidArgument {
                message: "composer edit has no input token".into(),
            })?;
        let limit = lock_registry(&self.native_views)?.composer_max_bytes(
            &instance,
            view_revision,
            &token,
        )?;
        let mounting = request.get("op").and_then(serde_json::Value::as_str) == Some("mount");
        if mounting
            && request.get("max_bytes").and_then(serde_json::Value::as_u64) != Some(limit as u64)
        {
            return Err(CoreError::InvalidArgument {
                message: "composer input does not match its Rust view".into(),
            });
        }
        if mounting && let Some(draft) = self.drafts.restore(self.inner.chat_id())? {
            request["draft"] = serde_json::Value::String(draft);
        }
        let request_json = serde_json::to_vec(&request).map_err(|_| CoreError::Internal {
            message: "composer edit could not be encoded".into(),
        })?;
        let reply = editor.call_json(&request_json);
        let response: serde_json::Value =
            serde_json::from_slice(&reply).map_err(|_| CoreError::Internal {
                message: "composer edit returned invalid state".into(),
            })?;
        if response.get("error").is_none() {
            if let Some(text) = response
                .get("state")
                .and_then(|state| state.get("text"))
                .and_then(serde_json::Value::as_str)
            {
                self.drafts.save(self.inner.chat_id(), text)?;
            }
        }
        Ok(reply)
    }

    /// Submit the current Rust Native composer input through the existing
    /// session send operation. Rust resolves the choice token and busy policy.
    pub fn submit_native_composer(
        &self,
        instance: String,
        view_revision: u64,
        input_token: String,
        editing_lifetime: u64,
        editor_revision: u64,
        choice_token: Option<String>,
        text: String,
        attachments: Vec<OutgoingAttachment>,
    ) -> CoreResult<NativeActionResult> {
        let stamp = EditorStamp {
            token: input_token.clone(),
            lifetime: editing_lifetime,
            revision: editor_revision,
        };
        {
            let editor = self.composer_editor.lock().map_err(|_| CoreError::Closed)?;
            let state = editor.state().ok_or(CoreError::Closed)?;
            if editor.stamp().as_ref() != Some(&stamp) || state.text != text {
                return Err(CoreError::InvalidArgument {
                    message: "composer text changed before it was sent".into(),
                });
            }
        }
        validate_outgoing_attachments(&attachments)?;
        let choice = choice_token.as_deref().unwrap_or(&input_token);
        let busy = lock_registry(&self.native_views)?.composer_busy_policy(
            &instance,
            view_revision,
            choice,
            &text,
        )?;
        let submitted_text = text.clone();
        let request = SendRequest {
            text,
            attachments,
            worktree: None,
            busy,
        };
        let outcome = self.inner.send(request.into())?;
        if let Ok(mut editor) = self.composer_editor.lock() {
            if editor.submitted(&stamp, &submitted_text).unwrap_or(false) {
                let _ = self.drafts.save(self.inner.chat_id(), "");
            }
        }
        Ok(map_send_outcome(outcome.into()))
    }

    /// Validate a Rust Native view whose intents use the closed mobile
    /// session action set. Return its canonical bytes for native mounting.
    pub fn prepare_native_view(&self, view_json: Vec<u8>) -> CoreResult<Vec<u8>> {
        lock_registry(&self.native_views)?.prepare(&view_json)
    }

    /// Mark a mounted view current only after the native adapter applied it.
    pub fn acknowledge_native_view(&self, instance: String, revision: u64) -> CoreResult<()> {
        lock_registry(&self.native_views)?.acknowledge(&instance, revision)
    }

    /// Resolve a field change against the acknowledged Rust Native view.
    /// Only draft fields may change through this bridge.
    pub fn change_native_field(
        &self,
        instance: String,
        revision: u64,
        node: String,
        value: String,
    ) -> CoreResult<NativeActionResult> {
        let event = FieldChange {
            activation: Activation {
                instance,
                revision,
                node,
            },
            value,
        };
        let intent = lock_registry(&self.native_views)?.change_field(&event)?;
        match intent {
            SessionViewIntent::SaveDraft => {
                self.drafts.save(self.inner.chat_id(), &event.value)?;
                Ok(NativeActionResult::DraftSaved)
            }
            _ => Err(CoreError::Unsupported {
                message: "this Rust Native field change is not supported by the mobile session"
                    .into(),
            }),
        }
    }

    /// Resolve an activation against the acknowledged view, then call an
    /// existing session operation. Unknown intents fail during view decoding.
    pub fn activate_native(
        &self,
        instance: String,
        revision: u64,
        node: String,
        text: Option<String>,
        attachments: Vec<OutgoingAttachment>,
    ) -> CoreResult<NativeActionResult> {
        let event = Activation {
            instance,
            revision,
            node,
        };
        let registry = lock_registry(&self.native_views)?;
        let intent = registry.activate(&event)?;
        match intent {
            SessionViewIntent::Interrupt => {
                reject_unused_input(text, attachments)?;
                drop(registry);
                self.inner.interrupt()?;
                Ok(NativeActionResult::Interrupted)
            }
            SessionViewIntent::RespondInput {
                request_id,
                answers,
            } => {
                reject_unused_input(text, attachments)?;
                let (request_id, answers) = answer_input(request_id, answers);
                drop(registry);
                self.inner.respond_input(&request_id, answers)?;
                Ok(NativeActionResult::InputAnswered)
            }
            SessionViewIntent::RequestAttachment => {
                reject_unused_input(text, attachments)?;
                Ok(NativeActionResult::AttachmentPickerRequested)
            }
            SessionViewIntent::SaveDraft => Err(CoreError::Unsupported {
                message: "this Rust Native action is not supported by the mobile session".into(),
            }),
        }
    }

    pub fn transcript_status(&self) -> TranscriptStatus {
        let snap = self.inner.snapshot();
        TranscriptStatus {
            revision: snap.revision,
            hydrated: snap.hydrated,
            entry_count: snap.transcript_len as u32,
            pending_count: snap.pending.len() as u32,
            streaming: snap.streaming,
            working: snap.working,
            working_since_ms: snap.working_since_ms,
            last_entry_id: snap.entries.last().map(|e| e.id.clone()),
        }
    }

    /// Plain text of one entry's text parts (copy / share).
    pub fn message_text(&self, entry_id: String) -> Option<String> {
        let snap = self.inner.snapshot();
        let entry = snap.entry(&entry_id)?;
        let texts: Vec<&str> = entry
            .message
            .parts
            .iter()
            .filter_map(|p| match p {
                zc::MessagePart::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        let joined = texts.join("\n\n");
        Some(if entry.message.role == zc::MessageRole::User {
            zc::attachments::parse_user_message(&joined).text
        } else {
            joined
        })
    }

    /// A transcript view is on/off screen (marks seen on attach).
    pub fn set_view_attached(&self, attached: bool) {
        self.inner.set_view_attached(attached);
    }

    pub fn mark_seen(&self) {
        self.inner.mark_seen();
    }

    /// Send: a new turn when idle; while working, queue (or steer per `busy`).
    pub fn send(&self, request: SendRequest) -> CoreResult<SendOutcome> {
        Ok(self.inner.send(request.into())?.into())
    }

    pub fn interrupt(&self) -> CoreResult<()> {
        Ok(self.inner.interrupt()?)
    }

    pub fn respond_input(
        &self,
        request_id: String,
        answers: Vec<UserInputAnswer>,
    ) -> CoreResult<()> {
        let answers = answers
            .into_iter()
            .map(|a| zc::UserInputAnswer {
                question_id: a.question_id,
                labels: a.labels,
            })
            .collect();
        Ok(self.inner.respond_input(&request_id, answers)?)
    }

    /// Park a message on the shared queue directly. Returns the row id.
    pub fn enqueue(
        &self,
        text: String,
        attachments: Vec<String>,
        hold_for_turn_end: bool,
    ) -> CoreResult<String> {
        Ok(self.inner.enqueue(&text, attachments, hold_for_turn_end)?)
    }

    /// Move a queue row to index `to` (clamped). False when it can't move.
    pub fn move_queued(&self, id: String, to: u32) -> CoreResult<bool> {
        Ok(self.inner.move_queued(&id, to as usize)?)
    }

    /// Nudge a row one slot up (`-1`) or down (`+1`).
    pub fn move_queued_by(&self, id: String, delta: i32) -> CoreResult<bool> {
        Ok(self.inner.move_queued_by(&id, delta)?)
    }

    pub async fn begin_queued_edit(&self, id: String, instance_id: String) -> QueueEditStart {
        let inner = self.inner.clone();
        let result =
            zc::runtime::run(async move { Ok(inner.begin_queued_edit(&id, &instance_id).await) })
                .await;
        match result {
            Ok(zc::QueueEditStart::Acquired(lease)) => QueueEditStart::Acquired {
                lease: lease.into(),
            },
            Ok(zc::QueueEditStart::Locked) => QueueEditStart::Locked,
            Ok(zc::QueueEditStart::Missing) => QueueEditStart::Missing,
            Ok(zc::QueueEditStart::Unavailable) | Err(_) => QueueEditStart::Unavailable,
        }
    }

    pub async fn renew_queued_edit(&self, lease: QueueEditLease) -> bool {
        let inner = self.inner.clone();
        let lease: zc::QueueEditLease = lease.into();
        zc::runtime::run(async move { Ok(inner.renew_queued_edit(&lease).await) })
            .await
            .unwrap_or(false)
    }

    pub async fn finish_queued_edit(
        &self,
        lease: QueueEditLease,
        action: QueueEditAction,
        text: Option<String>,
    ) -> QueueEditFinish {
        let inner = self.inner.clone();
        let lease: zc::QueueEditLease = lease.into();
        let action = match action {
            QueueEditAction::Commit => zc::QueueEditAction::Commit,
            QueueEditAction::Cancel => zc::QueueEditAction::Cancel,
            QueueEditAction::Discard => zc::QueueEditAction::Discard,
            QueueEditAction::Release => zc::QueueEditAction::Release,
        };
        let result =
            zc::runtime::run(
                async move { Ok(inner.finish_queued_edit(&lease, action, text).await) },
            )
            .await;
        match result {
            Ok(zc::QueueEditFinish::Finished) => QueueEditFinish::Finished,
            Ok(zc::QueueEditFinish::Conflict) => QueueEditFinish::Conflict,
            Ok(zc::QueueEditFinish::Missing) => QueueEditFinish::Missing,
            Ok(zc::QueueEditFinish::Lost) => QueueEditFinish::Lost,
            Ok(zc::QueueEditFinish::Unavailable) | Err(_) => QueueEditFinish::Unavailable,
        }
    }

    /// Deliver a queued row now (interrupting the live turn if needed).
    pub async fn send_queued_now(&self, id: String) -> CoreResult<bool> {
        let inner = self.inner.clone();
        on_runtime(async move { inner.send_queued_now(&id).await }).await
    }

    /// A row's primary action: steer text into the live turn (never
    /// interrupts); attachments send now.
    pub async fn deliver_queued_now(&self, id: String) -> CoreResult<bool> {
        let inner = self.inner.clone();
        on_runtime(async move { inner.deliver_queued_now(&id).await }).await
    }

    /// Remove a queued row (applied locally after the host acks).
    pub async fn remove_queued(&self, id: String) -> CoreResult<bool> {
        let inner = self.inner.clone();
        on_runtime(async move { inner.remove_queued(&id).await }).await
    }

    pub fn clear_queue_error(&self) {
        self.inner.clear_queue_error();
    }

    /// "Not delivered — tap to retry".
    pub fn retry_delivery(&self) -> CoreResult<()> {
        Ok(self.inner.retry_delivery()?)
    }
}

fn reject_unused_input(
    text: Option<String>,
    attachments: Vec<OutgoingAttachment>,
) -> CoreResult<()> {
    if text.is_some() || !attachments.is_empty() {
        Err(CoreError::InvalidArgument {
            message: "this action does not accept composer content".into(),
        })
    } else {
        Ok(())
    }
}

fn validate_outgoing_attachments(attachments: &[OutgoingAttachment]) -> CoreResult<()> {
    const MAX_ATTACHMENTS: usize = 10;
    const MAX_TOTAL_BYTES: usize = 48 * 1024 * 1024;
    if attachments.len() > MAX_ATTACHMENTS {
        return Err(CoreError::InvalidArgument {
            message: "too many attachments".into(),
        });
    }
    let mut total = 0usize;
    for attachment in attachments {
        if attachment.name.trim().is_empty()
            || attachment.name.len() > 255
            || attachment.mime_type.trim().is_empty()
            || attachment.mime_type.len() > 255
            || attachment.data.is_empty()
            || attachment.data.len() > zc::attachments::MAX_ATTACHMENT_BYTES
        {
            return Err(CoreError::InvalidArgument {
                message: "attachment metadata or size is invalid".into(),
            });
        }
        total = total.saturating_add(attachment.data.len());
        if total > MAX_TOTAL_BYTES {
            return Err(CoreError::InvalidArgument {
                message: "attachments exceed the combined size limit".into(),
            });
        }
    }
    Ok(())
}
