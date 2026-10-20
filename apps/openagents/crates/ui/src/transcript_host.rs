//! The desktop's answers to the transcript: its app state and engine
//! connection.

use std::rc::Rc;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use gpui::{App, Entity, Subscription};
use openagents_doc::{SessionMessageEntry, TranscriptBaseline};

use crate::rpc::Rpc;
use crate::state::{AppState, Indicator, TranscriptTextChanged};
use crate::tool_progress::ToolProgress;
use crate::transcript::host::{ChatPlace, TranscriptHost, TurnActivity};
use crate::transcript::{PreparedTranscript, sending_bridge, turn_elapsed_secs};
use crate::workspace_links::FileLinkRoot;

/// The transcript's host over the desktop's [`AppState`].
#[derive(Clone)]
pub struct AppHost(pub Entity<AppState>);

impl AppHost {
    /// A host for transcripts over `state`.
    pub fn shared(state: &Entity<AppState>) -> Rc<dyn TranscriptHost> {
        Rc::new(Self(state.clone()))
    }
}

impl TranscriptHost for AppHost {
    fn selected_chat<'a>(&self, cx: &'a App) -> Option<&'a str> {
        self.0.read(cx).selected_chat.as_deref()
    }

    fn transcript_replayed(&self, cx: &App) -> bool {
        self.0.read(cx).transcript_replayed
    }

    fn transcript_revision(&self, cx: &App) -> u64 {
        self.0.read(cx).transcript_revision
    }

    fn entries<'a>(&self, doc: Option<&str>, cx: &'a App) -> &'a [SessionMessageEntry] {
        let state = self.0.read(cx);
        match doc {
            Some(doc_id) => state.sub_transcript(doc_id),
            None => state.transcript.as_slice(),
        }
    }

    fn pending_echoes<'a>(&self, cx: &'a App) -> &'a [SessionMessageEntry] {
        self.0.read(cx).pending_echoes()
    }

    fn prepared<'a>(&self, chat_id: &str, cx: &'a App) -> Option<&'a Arc<PreparedTranscript>> {
        self.0.read(cx).prepared_transcripts.get(chat_id)
    }

    fn baseline<'a>(&self, chat_id: &str, cx: &'a App) -> Option<&'a Arc<TranscriptBaseline>> {
        self.0.read(cx).transcript_baseline(chat_id)
    }

    fn chat_is_local(&self, chat_id: &str, cx: &App) -> bool {
        self.0.read(cx).chat_is_local(chat_id)
    }

    fn link_roots_revision(&self, cx: &App) -> u64 {
        self.0.read(cx).link_roots_revision
    }

    fn file_link_roots(&self, chat_id: &str, cx: &App) -> Vec<FileLinkRoot> {
        self.0.read(cx).file_link_roots(chat_id)
    }

    fn chat_place(&self, chat_id: Option<&str>, cx: &App) -> Option<ChatPlace> {
        let state = self.0.read(cx);
        chat_id
            .and_then(|id| state.chats.iter().find(|chat| chat.id == id))
            .or_else(|| state.selected_chat_row())
            .map(|chat| ChatPlace {
                device_id: chat.device_id.clone(),
                cwd: chat.cwd.clone(),
            })
    }

    fn local_device_id<'a>(&self, cx: &'a App) -> Option<&'a str> {
        self.0.read(cx).local_device_id.as_deref()
    }

    fn transfer_percent(&self, upload_id: &str, cx: &App) -> Option<u8> {
        self.0.read(cx).transfer_percent(upload_id)
    }

    fn upload_progress_percent(&self, cx: &App) -> Option<u8> {
        self.0.read(cx).upload_progress_percent()
    }

    fn send_undelivered(&self, chat_id: &str, now: DateTime<Utc>, cx: &App) -> bool {
        self.0.read(cx).send_undelivered(chat_id, now)
    }

    /// Re-kick every delivery road engine-side (fresh chat2 socket, host
    /// nudge, delivery escorts) and restart the grace clock so the trailer
    /// returns to Sending/Queued while the retry runs.
    fn retry_send(&self, chat_id: &str, cx: &mut App) {
        let engine = self.0.read(cx).engine().cloned();
        self.0.update(cx, |s, cx| {
            s.retry_pending_send(chat_id, chrono::Utc::now());
            cx.notify();
        });
        if let Some(engine) = engine {
            let chat_id = chat_id.to_owned();
            cx.spawn(async move |_| {
                let params = serde_json::json!({ "chatId": chat_id });
                if let Err(err) = engine
                    .client()
                    .call(openagents_rpc::methods::RETRY_DELIVERY, params)
                    .await
                {
                    tracing::warn!(error = %err, "delivery retry RPC failed");
                }
            })
            .detach();
        }
    }

    fn turn_activity(&self, chat_id: &str, now: DateTime<Utc>, cx: &App) -> Option<TurnActivity> {
        let state = self.0.read(cx);
        if state.indicator_for(chat_id, now) != Indicator::Working {
            return None;
        }
        // During the send→turn window the session row's `started_at`
        // still belongs to the PREVIOUS turn — a timer based on the
        // send counted the round-trip and then restarted when the
        // turn actually began (user report). Bridge it as "Sending…"
        // with no timer instead; the word + timer start with the
        // turn.
        let turn_started = state.session_for(chat_id).and_then(|s| s.started_at);
        let send_started = state.pending_send_started(chat_id, now);
        let sending = sending_bridge(send_started, turn_started);
        // Degraded delivery path: the send is a durable local write
        // waiting on connectivity — say so instead of faking
        // progress. (The overlay holds while degraded, so this line
        // owns the surface until the ack or the failed state.)
        let queued = sending && state.chat_delivery_degraded(chat_id);
        let chat_created = state
            .chats
            .iter()
            .find(|c| c.id == chat_id)
            .map(|c| c.created_at.timestamp_millis());
        let elapsed_secs = turn_elapsed_secs(
            turn_started.map(|t| t.timestamp_millis()),
            chat_created,
            now.timestamp_millis(),
        );
        // While sending, the session row still carries the PREVIOUS
        // turn: the bridge keys on its own send instead, so it never
        // rolls out of that turn's word or an earlier send's.
        let turn_ms =
            if sending { send_started } else { turn_started }.map_or(0, |t| t.timestamp_millis());
        Some(TurnActivity {
            sending,
            queued,
            elapsed_secs,
            turn_ms,
        })
    }

    fn running_tool(&self, now: DateTime<Utc>, cx: &App) -> Option<ToolProgress> {
        let state = self.0.read(cx);
        crate::tool_progress::running(&state.transcript, &state.chats, now)
    }

    fn rpc(&self, cx: &App) -> Option<Rpc> {
        self.0.read(cx).engine().map(|engine| engine.rpc())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn watch(
        &self,
        changed: Rc<dyn Fn(Option<&str>, &mut App)>,
        cx: &mut App,
    ) -> Vec<Subscription> {
        let on_text = changed.clone();
        vec![
            cx.observe(&self.0, move |_, cx| changed(None, cx)),
            cx.subscribe(&self.0, move |_, event: &TranscriptTextChanged, cx| {
                on_text(Some(&event.doc_id), cx)
            }),
        ]
    }
}
