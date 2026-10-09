//! The Coder chats list as last seen, kept in the app's encrypted store so a
//! relaunch shows it at once while the computers are read again.
//!
//! Each row keeps the newest verified summary this device saw for a task,
//! the chat's title and last message time from the computer's chat catalog,
//! and the first line and send time of each chat this device started. It is
//! a display cache: live summaries from a host always replace a row, a
//! computer that is no longer added drops its rows, and nothing here grants
//! or starts anything.

use coder_computers::cache::Cache;
use nostr::activity_summary::{ActivitySummary, Attention, Phase, SubjectKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The key the list is stored under.
const KEY: &str = "coder-list";
/// The most rows kept.
pub const MAX_ROWS: usize = 100;

/// A task's phase, as stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stored {
    Queued,
    Running,
    Waiting,
    Completed,
    Failed,
    Cancelled,
    Unknown,
}

impl From<Phase> for Stored {
    fn from(phase: Phase) -> Self {
        match phase {
            Phase::Queued => Self::Queued,
            Phase::Running => Self::Running,
            Phase::Waiting => Self::Waiting,
            Phase::Completed => Self::Completed,
            Phase::Failed => Self::Failed,
            Phase::Cancelled => Self::Cancelled,
            Phase::Unknown => Self::Unknown,
        }
    }
}

impl From<Stored> for Phase {
    fn from(phase: Stored) -> Self {
        match phase {
            Stored::Queued => Self::Queued,
            Stored::Running => Self::Running,
            Stored::Waiting => Self::Waiting,
            Stored::Completed => Self::Completed,
            Stored::Failed => Self::Failed,
            Stored::Cancelled => Self::Cancelled,
            Stored::Unknown => Self::Unknown,
        }
    }
}

/// One task as the list last showed it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    pub host: String,
    pub task: String,
    pub sequence: u64,
    pub phase: Stored,
    pub headline: String,
    /// When the host published the summary; used only to order rows with
    /// no known message time.
    pub updated_at: u64,
    /// The chat's title from the computer's catalog.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Unix seconds of the chat's last message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last: Option<u64>,
}

impl Row {
    pub fn of(summary: &ActivitySummary, title: Option<String>, last: Option<u64>) -> Self {
        Self {
            host: summary.host.clone(),
            task: summary.subject.clone(),
            sequence: summary.sequence,
            phase: summary.phase.into(),
            headline: summary.headline.clone(),
            updated_at: summary.updated_at,
            title,
            last,
        }
    }

    /// The row as a summary for the list. It was verified when first seen.
    pub fn summary(&self) -> ActivitySummary {
        ActivitySummary {
            host: self.host.clone(),
            subject_kind: SubjectKind::Task,
            subject: self.task.clone(),
            sequence: self.sequence,
            phase: self.phase.into(),
            headline: self.headline.clone(),
            attention: Attention::None,
            updated_at: self.updated_at,
        }
    }
}

/// Everything the list keeps across a relaunch.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct List {
    pub rows: Vec<Row>,
    /// The feature card the new chat's carousel last opened on, so the
    /// next open, after a relaunch too, starts on another
    /// ([`crate::carousel::pick_start`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_card: Option<usize>,
    /// The first line of each chat this device started, by task ID.
    #[serde(default)]
    pub titles: BTreeMap<String, String>,
    /// When this device sent each chat's first message, by task ID.
    #[serde(default)]
    pub sent: BTreeMap<String, u64>,
    /// When this device last started a chat in each workspace of each
    /// computer, by host key and workspace label; new chats and the
    /// suggested workspaces follow it.
    #[serde(default)]
    pub used: BTreeMap<String, u64>,
    /// The reply in each conversation that started or continued a Coder
    /// task, by conversation ID, so the conversation still shows its start
    /// card after a relaunch.
    #[serde(default)]
    pub started: BTreeMap<String, Started>,
}

/// The reply that started or continued a conversation's Coder task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Started {
    /// The reply's turn index in the conversation.
    pub reply: usize,
    /// The task it started or continued.
    pub task: String,
    /// Unix seconds when it did.
    pub at: u64,
    /// One short line of how the task's turn ended, once read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
}

/// The most conversations whose start is kept; the oldest go first.
pub const MAX_STARTED: usize = 512;

/// The list and the store it lives in. Without a store it lasts only as
/// long as the app.
pub struct Store {
    cache: Option<Cache>,
    pub list: List,
    saved: List,
}

impl Store {
    pub fn open(cache: Option<Cache>) -> Self {
        let list: List = cache
            .as_ref()
            .and_then(|cache| cache.read(KEY).ok().flatten())
            .unwrap_or_default();
        Self {
            cache,
            saved: list.clone(),
            list,
        }
    }

    /// Write the list when it changed since the last write.
    pub fn save(&mut self) {
        if self.list == self.saved {
            return;
        }
        self.list.rows.truncate(MAX_ROWS);
        // Past the bound, keep only the titles and send times of rows still
        // listed. A chat just started has none yet, so it keeps its own
        // until then.
        if self.list.titles.len() > MAX_ROWS || self.list.sent.len() > MAX_ROWS {
            let listed: std::collections::BTreeSet<String> =
                self.list.rows.iter().map(|row| row.task.clone()).collect();
            self.list.titles.retain(|task, _| listed.contains(task));
            self.list.sent.retain(|task, _| listed.contains(task));
        }
        while self.list.started.len() > MAX_STARTED {
            let Some(oldest) = self
                .list
                .started
                .iter()
                .min_by_key(|(_, started)| started.at)
                .map(|(id, _)| id.clone())
            else {
                break;
            };
            self.list.started.remove(&oldest);
        }
        if let Some(cache) = &self.cache
            && cache.write(KEY, &self.list).is_ok()
        {
            self.saved = self.list.clone();
        }
    }
}
