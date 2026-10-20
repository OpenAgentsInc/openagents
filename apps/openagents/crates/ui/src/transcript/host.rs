//! What the transcript reads from the app around it, and the few things it
//! asks of it. The desktop answers from its app state and engine
//! connection; another host answers from whatever holds its chats.

use std::rc::Rc;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use gpui::{App, Subscription};
use openagents_doc::{SessionMessageEntry, TranscriptBaseline};

use super::PreparedTranscript;
use crate::rpc::Rpc;
use crate::tool_progress::ToolProgress;
use crate::workspace_links::FileLinkRoot;

/// Where a chat runs: its computer and its working folder.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChatPlace {
    pub device_id: String,
    pub cwd: Option<String>,
}

/// A working turn as its trailer shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TurnActivity {
    /// The send hasn't started a turn yet.
    pub sending: bool,
    /// The send waits on the connection.
    pub queued: bool,
    /// Seconds since the turn (or the chat) started.
    pub elapsed_secs: Option<i64>,
    /// When the turn, or the send while sending, started (ms).
    pub turn_ms: i64,
}

pub trait TranscriptHost: 'static {
    /// The chat the window shows.
    fn selected_chat<'a>(&self, cx: &'a App) -> Option<&'a str>;
    /// Whether the selected chat's history has arrived.
    fn transcript_replayed(&self, cx: &App) -> bool;
    /// Changes whenever the selected chat's entries change.
    fn transcript_revision(&self, cx: &App) -> u64;
    /// The selected chat's entries, or a subagent document's.
    fn entries<'a>(&self, doc: Option<&str>, cx: &'a App) -> &'a [SessionMessageEntry];
    /// The selected chat's sent messages the engine hasn't echoed yet.
    fn pending_echoes<'a>(&self, cx: &'a App) -> &'a [SessionMessageEntry];
    /// Rows built off the main thread for `chat_id`, when there are any.
    fn prepared<'a>(&self, chat_id: &str, cx: &'a App) -> Option<&'a Arc<PreparedTranscript>>;
    /// The entries `chat_id` had when it was opened.
    fn baseline<'a>(&self, chat_id: &str, cx: &'a App) -> Option<&'a Arc<TranscriptBaseline>>;
    /// Whether `chat_id` runs on this computer.
    fn chat_is_local(&self, chat_id: &str, cx: &App) -> bool;
    /// Changes whenever a file link's roots may have moved.
    fn link_roots_revision(&self, cx: &App) -> u64;
    /// The checkouts a file link from `chat_id` may open against.
    fn file_link_roots(&self, chat_id: &str, cx: &App) -> Vec<FileLinkRoot>;
    /// Where `chat_id` runs; the selected chat's place when `chat_id` is
    /// `None` or unknown.
    fn chat_place(&self, chat_id: Option<&str>, cx: &App) -> Option<ChatPlace>;
    /// This computer's device id.
    fn local_device_id<'a>(&self, cx: &'a App) -> Option<&'a str>;
    /// How far one upload has crossed to its computer.
    fn transfer_percent(&self, upload_id: &str, cx: &App) -> Option<u8>;
    /// How far the sending message's uploads have crossed.
    fn upload_progress_percent(&self, cx: &App) -> Option<u8>;
    /// Whether `chat_id`'s last send failed to arrive.
    fn send_undelivered(&self, chat_id: &str, now: DateTime<Utc>, cx: &App) -> bool;
    /// Resend `chat_id`'s undelivered message.
    fn retry_send(&self, chat_id: &str, cx: &mut App);
    /// `chat_id`'s working turn, when it has one.
    fn turn_activity(&self, chat_id: &str, now: DateTime<Utc>, cx: &App) -> Option<TurnActivity>;
    /// The selected chat's running tool.
    fn running_tool(&self, now: DateTime<Utc>, cx: &App) -> Option<ToolProgress>;
    /// The engine's methods, while connected.
    fn rpc(&self, cx: &App) -> Option<Rpc>;
    /// This host as its concrete type, for the app's own extensions.
    fn as_any(&self) -> &dyn std::any::Any;
    /// Call `changed` whenever what this host answers changes: with a
    /// document's id when only its text changed, `None` for anything else.
    fn watch(&self, changed: Rc<dyn Fn(Option<&str>, &mut App)>, cx: &mut App)
    -> Vec<Subscription>;
}
