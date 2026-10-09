//! Basic conversations: chats with the hosted basic Coder, kept on the
//! phone in the app's encrypted store.
//!
//! A conversation is a list of turns. Sending a message adds the user's
//! turn and asks the [`Door`] for the reply in the background; the reply
//! streams into a [`Reply`] that the Coder tab draws as it grows, and joins
//! the turns when it ends. A failed reply leaves no turn: the chat shows why
//! and offers to try again. When the person runs Coder on a computer from a
//! conversation, the task it started is remembered with it.

use crate::basic_coder::{self, Door, Lane, Reply, Role, Turn, lock};
use crate::cache::Cache;
use crate::router::{Context, Meta};
use playtest::report::{
    ChatRole, ChatTurn, MAX_CHAT_TEXT_CHARS, MAX_CHAT_TURNS, ShareReason, SharedChat,
};
use rust_native::markdown::IncrementalMarkdown;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use tokio::runtime::Handle;
use tokio::task::JoinHandle;

/// The most conversations kept; the oldest go first.
const MAX_TALKS: usize = 512;
/// The most turns one conversation keeps; the oldest go first.
const MAX_TURNS: usize = 400;
/// The most bytes of turns one conversation keeps, inside the store's
/// 192 KiB item bound.
const MAX_TALK_BYTES: usize = 150 * 1024;

/// The task a conversation started on a computer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spawned {
    pub host: String,
    pub task: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<u64>,
}

/// One conversation's row in the list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    pub id: String,
    pub title: String,
    pub started: u64,
    /// When its last message was sent or answered.
    pub updated: u64,
    #[serde(default)]
    pub coder: Option<Spawned>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pinned: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub named: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct Saved {
    pub(crate) turns: Vec<Turn>,
    /// The record and its list metadata commit together. Older records omit it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) summary: Option<Summary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) lane: Option<Lane>,
}

/// A reply streaming into a conversation.
struct Stream {
    reply: Arc<Mutex<Reply>>,
    handle: Option<JoinHandle<()>>,
    markdown: IncrementalMarkdown,
}

/// What an open conversation shows below its turns.
pub enum Tail {
    None,
    /// Waiting for the first words.
    Thinking,
    /// The reply so far, parsed for display.
    Streaming(Vec<rust_native::markdown::Block>),
    /// Why the last message got no reply.
    Failed(String),
}

pub struct BasicChats {
    wake: crate::Wake,
    /// The last encrypted storage failure, if any.
    pub storage_error: Option<String>,
    storage_errors: BTreeMap<String, String>,
    dirty: BTreeSet<String>,
    dirty_index: bool,
    dirty_used: bool,
    corrupt_index: bool,
    runtime: Option<Handle>,
    door: Option<Arc<dyn Door>>,
    store: Option<Cache>,
    index: Vec<Summary>,
    turns: BTreeMap<String, Vec<Turn>>,
    streams: BTreeMap<String, Stream>,
    failures: BTreeMap<String, String>,
    /// Threads whose last reply failed because the relay could not be
    /// reached ([`basic_coder::Failure::Transport`]): a retry may succeed.
    unreached: BTreeSet<String>,
    /// Where the worker's judgment placed each conversation's last reply.
    lanes: BTreeMap<String, Lane>,
    /// The worker's ordering of a new chat's suggestions, for one set of
    /// candidate IDs.
    ranking: Option<(BTreeSet<String>, Arc<Mutex<Reply>>)>,
    /// A rank job may go: once each time the tab shows, since it is metered
    /// as a message.
    rank_allowed: bool,
    /// What the next turn tells the worker about the phone.
    context: Context,
    /// The suggestions used on this device, oldest first: `id:<id>` for a
    /// suggestion tapped or a prepared answer shown, and `words:<digest>`
    /// for the words of a message sent. Kept in the store, so a used
    /// suggestion stays hidden after a relaunch.
    used: Vec<String>,
}

/// The most conversations [`BasicChats::route_counts`] reads.
pub const MAX_ROUTE_CHATS: usize = 1_000;

/// The most used-suggestion marks kept; the oldest go first.
const MAX_USED: usize = 512;
/// A message longer than this is never a suggestion's words, so its words
/// are not kept.
const MAX_SUGGESTION_CHARS: usize = 120;
/// The store key of the used suggestions.
const USED_KEY: &str = "used-suggestions";

/// Whether a suggestion's stable ID or normalized words have been used.
pub fn suggestion_used(markers: &[String], id: Option<&str>, words: &[&str]) -> bool {
    id.is_some_and(|id| markers.contains(&id_mark(id)))
        || words
            .iter()
            .filter_map(|text| words_mark(text))
            .any(|mark| markers.contains(&mark))
}

/// The mark of a suggestion's ID: a bank ID without its `@version`, so a
/// new version of an answer is the same suggestion.
pub fn id_mark(id: &str) -> String {
    format!("id:{}", id.split('@').next().unwrap_or(id))
}

/// The mark of a message's words: a digest of them lowercased, without
/// apostrophes, and with every run of other punctuation or spaces as one
/// space, so "who are you" is the words of "Who are you?". None for
/// nothing to say or a message too long to be a suggestion.
pub fn words_mark(text: &str) -> Option<String> {
    use sha2::{Digest, Sha256};
    if text.chars().count() > MAX_SUGGESTION_CHARS {
        return None;
    }
    let mut plain = String::new();
    for c in text.chars().flat_map(char::to_lowercase) {
        if c == '\'' || c == '\u{2019}' {
            continue;
        }
        if c.is_alphanumeric() {
            plain.push(c);
        } else if !plain.is_empty() && !plain.ends_with(' ') {
            plain.push(' ');
        }
    }
    let plain = plain.trim_end();
    if plain.is_empty() {
        return None;
    }
    let digest = Sha256::digest(plain.as_bytes());
    let hex: String = digest[..8].iter().map(|b| format!("{b:02x}")).collect();
    Some(format!("words:{hex}"))
}

/// A new chat's suggestions this few are shown as they are, unranked.
const RANK_AT_LEAST: usize = 2;

impl BasicChats {
    /// Conversations kept in `store`, answered through `door` on `runtime`.
    pub fn new(runtime: Option<Handle>, door: Option<Arc<dyn Door>>, store: Option<Cache>) -> Self {
        let loaded = store
            .as_ref()
            .map_or(Ok(None), |store| store.read("basic-index"));
        let corrupt_index = loaded.is_err();
        let mut storage_errors = BTreeMap::new();
        if let Err(error) = &loaded {
            storage_errors.insert("basic-index".into(), error.clone());
        }
        let mut index: Vec<Summary> = loaded.ok().flatten().unwrap_or_default();
        // Recover a record committed before a crash that interrupted the list write.
        if let Some(store) = &store
            && let Ok(keys) = store.keys("basic-")
        {
            for key in keys.into_iter().filter(|key| key.len() == 38) {
                if let Ok(Some(saved)) = store.read::<Saved>(&key)
                    && let Some(summary) = saved.summary
                    && key == item(&summary.id)
                {
                    if let Some(previous) = index.iter_mut().find(|row| row.id == summary.id) {
                        if summary.updated >= previous.updated {
                            *previous = summary;
                        }
                    } else {
                        index.push(summary);
                    }
                }
            }
            index.sort_by_key(|row| std::cmp::Reverse(row.updated));
            index.truncate(MAX_TALKS);
        }
        let used: Vec<String> = store
            .as_ref()
            .and_then(|store| store.read(USED_KEY).ok().flatten())
            .unwrap_or_default();
        Self {
            wake: Arc::new(|| {}),
            storage_error: storage_errors.values().next().cloned(),
            storage_errors,
            dirty: BTreeSet::new(),
            dirty_index: false,
            dirty_used: false,
            corrupt_index,
            runtime,
            door,
            store,
            index,
            turns: BTreeMap::new(),
            streams: BTreeMap::new(),
            failures: BTreeMap::new(),
            unreached: BTreeSet::new(),
            lanes: BTreeMap::new(),
            ranking: None,
            rank_allowed: false,
            context: Context::default(),
            used,
        }
    }

    /// Notify the caller when a job ends.
    pub fn with_wake(mut self, wake: crate::Wake) -> Self {
        self.wake = wake;
        self
    }

    /// Whether the suggestion `id`, or one whose chip reads or sends any of
    /// `words`, was used on this device.
    pub fn used(&self, id: Option<&str>, words: &[&str]) -> bool {
        suggestion_used(&self.used, id, words)
    }

    /// Bounded suggestion digests; adapters never need the original words.
    pub fn used_markers(&self) -> &[String] {
        &self.used
    }

    /// Mark the suggestion `id` used: it never shows again on this device.
    pub fn use_suggestion(&mut self, id: &str) {
        self.mark(id_mark(id));
    }

    fn mark(&mut self, mark: String) {
        if self.used.contains(&mark) {
            return;
        }
        self.used.push(mark);
        while self.used.len() > MAX_USED {
            self.used.remove(0);
        }
        self.dirty_used = true;
        self.save_used();
    }

    fn save_used(&mut self) {
        match self
            .store
            .as_ref()
            .map_or(Ok(()), |store| store.write(USED_KEY, &self.used))
        {
            Ok(()) => {
                self.dirty_used = false;
                self.storage_errors.remove(USED_KEY);
            }
            Err(error) => {
                self.storage_errors.insert(USED_KEY.into(), error);
            }
        }
        self.storage_error = self.storage_errors.values().next().cloned();
    }

    /// No store and no door, as in tests of other surfaces.
    pub fn empty() -> Self {
        Self::new(None, None, None)
    }

    /// Every conversation, newest first.
    pub fn list(&self) -> &[Summary] {
        &self.index
    }

    pub fn get(&self, id: &str) -> Option<&Summary> {
        self.index.iter().find(|summary| summary.id == id)
    }

    /// The conversation's turns, read from the store the first time.
    pub fn turns(&mut self, id: &str) -> &[Turn] {
        if !self.turns.contains_key(id) {
            let key = item(id);
            let loaded = self
                .store
                .as_ref()
                .map_or(Ok(None), |store| store.read::<Saved>(&key));
            let saved = match loaded {
                Ok(saved) => {
                    self.storage_errors.remove(&key);
                    self.storage_error = self.storage_errors.values().next().cloned();
                    match saved {
                        Some(saved) => saved,
                        None if self.get(id).is_some() && self.store.is_some() => {
                            self.storage_errors
                                .insert(key, "Saved conversation is missing.".into());
                            self.storage_error = self.storage_errors.values().next().cloned();
                            return &[];
                        }
                        None => Saved::default(),
                    }
                }
                Err(error) => {
                    self.storage_errors.insert(key, error);
                    self.storage_error = self.storage_errors.values().next().cloned();
                    return &[];
                }
            };
            if let Some(lane) = saved.lane {
                self.lanes.insert(id.to_owned(), lane);
            }
            self.turns.insert(id.to_owned(), saved.turns);
        }
        self.turns.get(id).map_or(&[], Vec::as_slice)
    }

    /// How many replies of the saved conversations took each route, from
    /// the typed judgments their turns keep (`meta.route`), for the Map
    /// page (#10085). Reads the store without keeping the turns, and at
    /// most [`MAX_ROUTE_CHATS`] conversations, newest first. The counts
    /// are this device's; nothing here sends them anywhere.
    pub fn route_counts(&self) -> BTreeMap<String, u64> {
        let mut counts = BTreeMap::new();
        let mut count = |turns: &[Turn]| {
            for turn in turns {
                if turn.role == crate::basic_coder::Role::Assistant
                    && let Some(route) = turn.meta.as_ref().and_then(|meta| meta.route.as_deref())
                {
                    *counts.entry(route.to_owned()).or_insert(0) += 1;
                }
            }
        };
        for summary in self.index.iter().take(MAX_ROUTE_CHATS) {
            if let Some(turns) = self.turns.get(&summary.id) {
                count(turns);
            } else if let Some(store) = &self.store
                && let Ok(Some(saved)) = store.read::<Saved>(&item(&summary.id))
            {
                count(&saved.turns);
            }
        }
        counts
    }

    /// Whether a reply is streaming into any conversation, or the worker is
    /// ranking the suggestions.
    pub fn streaming(&self) -> bool {
        !self.streams.is_empty()
            || self
                .ranking
                .as_ref()
                .is_some_and(|(_, reply)| !lock(reply).ended())
    }

    /// Keep the way to the worker open while the tab shows, and let the
    /// next new chat's suggestions be ranked once.
    pub fn warm(&mut self) {
        self.rank_allowed = true;
        if let (Some(door), Some(runtime)) = (&self.door, &self.runtime) {
            door.warm(runtime);
        }
    }

    /// Close the way to the worker once no reply waits on it.
    pub fn rest(&self) {
        if let Some(door) = &self.door {
            door.rest();
        }
    }

    /// What the next turn tells the worker about the phone: whether a
    /// computer is ready, and the app's build.
    pub fn set_context(&mut self, context: Context) {
        self.context = context;
    }

    /// The context the next turn carries.
    #[must_use]
    pub fn context(&self) -> &Context {
        &self.context
    }

    /// What the router said about the last reply of `id`: the streaming
    /// reply's, while one streams, else the last answer's.
    pub fn last_meta(&self, id: &str) -> Option<Meta> {
        if let Some(stream) = self.streams.get(id) {
            let meta = lock(&stream.reply).meta.clone();
            return (!meta.is_empty()).then_some(meta);
        }
        self.turns
            .get(id)?
            .last()
            .filter(|turn| turn.role == Role::Assistant)?
            .meta
            .clone()
    }

    /// The whole conversation as the tester would share it for evaluation,
    /// newest messages kept, each within the report's bounds.
    pub fn shared(&mut self, id: &str) -> Option<SharedChat> {
        let turns = self.turns(id);
        let skip = turns.len().saturating_sub(MAX_CHAT_TURNS);
        let turns: Vec<ChatTurn> = turns[skip..].iter().map(chat_turn).collect();
        (!turns.is_empty()).then_some(SharedChat {
            reason: ShareReason::Shared,
            turns,
        })
    }

    /// Where the worker's judgment placed the last reply of `id`.
    pub fn lane(&self, id: &str) -> Option<Lane> {
        self.lanes.get(id).copied()
    }

    /// Ask the worker once to order a new chat's suggestions (ID and
    /// label), when there are enough to order, this set was not asked
    /// about already, and the tab has shown since the last rank job.
    pub fn want_rank(&mut self, candidates: Vec<(String, String)>) {
        if candidates.len() < RANK_AT_LEAST || !self.rank_allowed {
            return;
        }
        let set: BTreeSet<String> = candidates.iter().map(|(id, _)| id.clone()).collect();
        if self
            .ranking
            .as_ref()
            .is_some_and(|(asked, _)| *asked == set)
        {
            return;
        }
        let (Some(door), Some(runtime)) = (&self.door, &self.runtime) else {
            return;
        };
        self.rank_allowed = false;
        let reply = Arc::new(Mutex::new(Reply::default()));
        runtime.spawn(rung(
            door.rank(candidates, reply.clone()),
            self.wake.clone(),
        ));
        self.ranking = Some((set, reply));
    }

    /// Order `items` by the worker's ranking when it answered for exactly
    /// their IDs; otherwise, or when it failed, leave the phone's order.
    pub fn rank_order<T>(&self, items: &mut [(String, T)]) {
        let Some((set, reply)) = &self.ranking else {
            return;
        };
        if set.len() != items.len() || !items.iter().all(|(id, _)| set.contains(id)) {
            return;
        }
        let reply = lock(reply);
        if !reply.done || reply.ranked.is_empty() {
            return;
        }
        items.sort_by_key(|(id, _)| {
            reply
                .ranked
                .iter()
                .position(|ranked| ranked == id)
                .unwrap_or(usize::MAX)
        });
    }

    /// Whether a reply is streaming into `id`.
    pub fn busy(&self, id: &str) -> bool {
        self.streams.contains_key(id)
    }

    /// Create an empty local conversation with a caller-selected stable ID.
    pub fn create(&mut self, id: &str, now: u64) -> bool {
        if self.corrupt_index || self.get(id).is_some() {
            return false;
        }
        if self.index.len() >= MAX_TALKS {
            let Some(oldest) = self.index.iter().rposition(|summary| summary.archived) else {
                return false;
            };
            let gone = self.index.remove(oldest);
            self.turns.remove(&gone.id);
        }
        self.index.insert(
            0,
            Summary {
                id: id.into(),
                title: "New chat".into(),
                started: now,
                updated: now,
                coder: None,
                archived: false,
                pinned: false,
                named: false,
            },
        );
        self.turns.insert(id.into(), vec![]);
        self.save(id);
        self.storage_error.is_none()
    }

    /// Archive locally without deleting the encrypted conversation.
    pub fn archive(&mut self, id: &str, now: u64) {
        self.stop(id, now);
        if let Some(summary) = self.index.iter_mut().find(|summary| summary.id == id) {
            summary.archived = true;
        }
        self.turns(id);
        self.save(id);
    }

    /// Save a title independently of the first message.
    pub fn rename(&mut self, id: &str, title: &str) -> Result<(), String> {
        let title = title.trim();
        if title.is_empty() || title.len() > 160 || title.chars().any(char::is_control) {
            return Err("Choose a title of 1 to 160 bytes without line breaks.".into());
        }
        self.turns(id);
        let summary = self
            .index
            .iter_mut()
            .find(|summary| summary.id == id)
            .ok_or("Chat not found.")?;
        summary.title = title.into();
        summary.named = true;
        self.save(id);
        self.storage_error.clone().map_or(Ok(()), Err)
    }

    /// Pinning affects list presentation and grants no task authority.
    pub fn pin(&mut self, id: &str, pinned: bool) -> Result<(), String> {
        self.turns(id);
        let summary = self
            .index
            .iter_mut()
            .find(|summary| summary.id == id)
            .ok_or("Chat not found.")?;
        summary.pinned = pinned;
        self.save(id);
        self.storage_error.clone().map_or(Ok(()), Err)
    }

    /// Restore an archived conversation to the current list.
    pub fn restore(&mut self, id: &str) {
        if let Some(summary) = self.index.iter_mut().find(|summary| summary.id == id) {
            summary.archived = false;
        }
        self.turns(id);
        self.save(id);
    }

    /// The current reply text, without completing the turn.
    pub fn partial(&self, id: &str) -> String {
        self.streams
            .get(id)
            .map(|stream| lock(&stream.reply).text.clone())
            .unwrap_or_default()
    }

    /// Start a conversation with `text` and ask for the reply.
    pub fn start(&mut self, text: &str, now: u64) -> Option<String> {
        self.start_tagged(text, now, None)
    }

    /// [`BasicChats::start`], its first message carrying the local command
    /// ID `request`, as [`BasicChats::send_tagged`].
    pub fn start_tagged(
        &mut self,
        text: &str,
        now: u64,
        request: Option<String>,
    ) -> Option<String> {
        let text = text.trim();
        if self.corrupt_index || text.is_empty() {
            return None;
        }
        let id = uuid::Uuid::new_v4().simple().to_string();
        let title: String = text
            .lines()
            .next()
            .unwrap_or(text)
            .chars()
            .scan(0, |bytes, ch| {
                *bytes += ch.len_utf8();
                (*bytes <= 160).then_some(ch)
            })
            .take(80)
            .collect();
        self.index.insert(
            0,
            Summary {
                id: id.clone(),
                title,
                started: now,
                updated: now,
                coder: None,
                archived: false,
                pinned: false,
                named: false,
            },
        );
        while self.index.len() > MAX_TALKS {
            if let Some(gone) = self.index.pop() {
                self.turns.remove(&gone.id);
                if let Some(mut stream) = self.streams.remove(&gone.id)
                    && let Some(handle) = stream.handle.take()
                {
                    handle.abort();
                }
            }
        }
        self.turns.insert(id.clone(), vec![]);
        self.send_tagged(&id, text, now, request);
        Some(id)
    }

    /// Add the user's message to `id` and ask for the reply. A message
    /// while a reply streams waits for it: the composer is busy then.
    pub fn send(&mut self, id: &str, text: &str, now: u64) -> bool {
        self.send_tagged(id, text, now, None)
    }

    /// Send once for a local command ID, also remembered across a relaunch.
    pub fn send_tagged(&mut self, id: &str, text: &str, now: u64, request: Option<String>) -> bool {
        let text = text.trim();
        if text.is_empty() || text.len() > 32 * 1024 || self.busy(id) || self.get(id).is_none() {
            return false;
        }
        self.turns(id);
        if self.corrupt_index || self.storage_errors.contains_key(&item(id)) {
            return false;
        }
        if let Some(turns) = self.turns.get_mut(id) {
            let mut turn = Turn::user(text);
            turn.request = request;
            turn.at = Some(now);
            turns.push(turn);
        }
        // Words sent are a suggestion used, tapped or typed.
        if let Some(mark) = words_mark(text) {
            self.mark(mark);
        }
        if self.get(id).is_some_and(|summary| !summary.named)
            && self.turns.get(id).is_some_and(|turns| turns.len() == 1)
            && let Some(summary) = self.index.iter_mut().find(|summary| summary.id == id)
        {
            summary.title = text
                .lines()
                .next()
                .unwrap_or(text)
                .chars()
                .scan(0, |bytes, ch| {
                    *bytes += ch.len_utf8();
                    (*bytes <= 160).then_some(ch)
                })
                .take(80)
                .collect();
        }
        self.touch(id, now);
        self.save(id);
        if self.storage_error.is_some() {
            return false;
        }
        self.ask(id);
        true
    }

    /// Add `text` as a reply after `id`'s last turn, asking the worker
    /// nothing: a dispatch plan's run result (#10183). `false` while a
    /// reply streams, for empty or overlong text, or a missing chat.
    pub fn note(&mut self, id: &str, text: &str, now: u64) -> bool {
        let text = text.trim();
        if text.is_empty() || text.len() > 32 * 1024 || self.busy(id) || self.get(id).is_none() {
            return false;
        }
        self.turns(id);
        if self.corrupt_index || self.storage_errors.contains_key(&item(id)) {
            return false;
        }
        if let Some(turns) = self.turns.get_mut(id) {
            let mut turn = Turn::assistant(text, None);
            turn.at = Some(now);
            turns.push(turn);
        }
        self.touch(id, now);
        self.save(id);
        self.storage_error.is_none()
    }

    /// Ask the worker for one combined summary of a dispatch plan's ended
    /// `runs` (#10183): the context carries them for this reply only, and
    /// the reply streams in as a send's does. `false` while a reply
    /// streams or for a missing chat.
    pub fn summarize(&mut self, id: &str, runs: Vec<serde_json::Value>) -> bool {
        if self.busy(id) || self.get(id).is_none() {
            return false;
        }
        self.turns(id);
        let kept = std::mem::replace(&mut self.context.runs, runs);
        self.ask(id);
        self.context.runs = kept;
        true
    }

    /// Ask again for the reply to the last message, after a failure.
    pub fn retry(&mut self, id: &str) {
        if self.busy(id) || self.get(id).is_some_and(|summary| summary.archived) {
            return;
        }
        if self.turns(id).last().is_some_and(|turn| turn.stopped) {
            let turns = self.turns.get_mut(id).expect("loaded turns");
            if turns
                .last()
                .is_some_and(|turn| turn.role == Role::Assistant)
            {
                turns.pop();
            } else if let Some(turn) = turns.last_mut() {
                turn.stopped = false;
            }
        }
        let last_is_user = self
            .turns(id)
            .last()
            .is_some_and(|turn| turn.role == Role::User);
        if last_is_user && !self.busy(id) {
            self.save(id);
            if self.storage_error.is_none() {
                self.ask(id);
            }
        }
    }

    /// Stop the reply streaming into `id`. What streamed is kept as the
    /// reply.
    pub fn stop(&mut self, id: &str, now: u64) {
        let Some(mut stream) = self.streams.remove(id) else {
            return;
        };
        if let Some(handle) = stream.handle.take() {
            handle.abort();
        }
        let (text, meta, model, complete) = {
            let reply = lock(&stream.reply);
            (
                reply.text.clone(),
                reply.meta.clone(),
                reply.model.clone(),
                reply.done && reply.failure.is_none(),
            )
        };
        if !text.trim().is_empty() {
            self.answer(id, text, meta, model, now);
            if !complete {
                if let Some(turn) = self.turns.get_mut(id).and_then(|turns| turns.last_mut()) {
                    turn.stopped = true;
                }
                self.save(id);
            }
        } else {
            if let Some(turn) = self.turns.get_mut(id).and_then(|turns| turns.last_mut()) {
                turn.stopped = true;
            }
            self.save(id);
            self.failures.insert(
                id.into(),
                "Stopped showing this reply. OpenAgents may still finish it.".into(),
            );
        }
    }

    fn ask(&mut self, id: &str) {
        self.failures.remove(id);
        self.unreached.remove(id);
        self.lanes.remove(id);
        let reply = Arc::new(Mutex::new(Reply::default()));
        let turns = self.turns.get(id).cloned().unwrap_or_default();
        let handle = match (&self.door, &self.runtime) {
            (Some(door), Some(runtime)) => Some(runtime.spawn(rung(
                door.ask(turns, self.context.clone(), reply.clone()),
                self.wake.clone(),
            ))),
            _ => {
                lock(&reply).failure = Some(basic_coder::Failure::Transport(
                    "no chat service in this build".into(),
                ));
                None
            }
        };
        self.streams.insert(
            id.to_owned(),
            Stream {
                reply,
                handle,
                markdown: IncrementalMarkdown::default(),
            },
        );
    }

    /// Move every ended reply into its conversation, and keep the rest's
    /// Markdown current. Returns whether anything changed.
    pub fn settle(&mut self, now: u64) -> bool {
        let mut changed = false;
        let ids: Vec<String> = self.streams.keys().cloned().collect();
        for id in ids {
            let Some(stream) = self.streams.get_mut(&id) else {
                continue;
            };
            let reply = lock(&stream.reply).clone();
            if let Some(lane) = reply.lane
                && self.lanes.insert(id.clone(), lane) != Some(lane)
            {
                changed = true;
            }
            if reply.text.len() != stream.markdown.source().len() {
                stream.markdown.set(&reply.text);
                changed = true;
            }
            if !reply.ended() {
                continue;
            }
            self.streams.remove(&id);
            changed = true;
            match reply.failure {
                None => self.answer(&id, reply.text, reply.meta, reply.model, now),
                Some(failure) => {
                    if matches!(failure, basic_coder::Failure::Transport(_)) {
                        self.unreached.insert(id.clone());
                    }
                    self.failures.insert(id, failure.describe());
                }
            }
        }
        changed
    }

    fn answer(&mut self, id: &str, text: String, meta: Meta, model: Option<String>, now: u64) {
        // A prepared answer shown is its suggestion used: the chip for it
        // would only show the same answer again.
        if let Some(answer) = meta.answer.as_deref() {
            self.use_suggestion(answer);
        }
        self.turns(id);
        if let Some(turns) = self.turns.get_mut(id) {
            let mut turn = Turn::assistant(text, (!meta.is_empty()).then_some(meta));
            turn.at = Some(now);
            turn.model = model;
            turns.push(turn);
        }
        self.touch(id, now);
        self.save(id);
    }

    /// Whether the last reply to `id` failed because the relay could not
    /// be reached, so asking again may succeed once it can.
    pub fn unreached(&self, id: &str) -> bool {
        self.unreached.contains(id) && self.failures.contains_key(id)
    }

    /// What shows below the conversation's turns.
    pub fn tail(&self, id: &str) -> Tail {
        if let Some(stream) = self.streams.get(id) {
            let blocks = stream.markdown.display_blocks();
            return if blocks.is_empty() {
                Tail::Thinking
            } else {
                Tail::Streaming(blocks.into_owned())
            };
        }
        if self
            .turns
            .get(id)
            .and_then(|turns| turns.last())
            .is_some_and(|turn| turn.role == Role::User && turn.stopped)
        {
            return Tail::Failed(
                "Stopped showing this reply. OpenAgents may still finish it.".into(),
            );
        }
        match self.failures.get(id) {
            Some(why) => Tail::Failed(why.clone()),
            // A message whose reply never came, as after a relaunch: it
            // can be asked again, so the chat never ends on a dead end.
            None if self
                .turns
                .get(id)
                .and_then(|turns| turns.last())
                .is_some_and(|turn| turn.role == Role::User) =>
            {
                Tail::Failed(basic_coder::Failure::Silent.describe())
            }
            None => Tail::None,
        }
    }

    /// Remember the task `id` started on a computer.
    pub fn spawned(&mut self, id: &str, host: &str, task: &str, now: u64) {
        self.spawned_in(id, host, task, None, now);
    }

    /// Remember the host's workspace label for list grouping.
    pub fn spawned_in(
        &mut self,
        id: &str,
        host: &str,
        task: &str,
        project: Option<&str>,
        now: u64,
    ) {
        if let Some(summary) = self.index.iter_mut().find(|summary| summary.id == id) {
            summary.coder = Some(Spawned {
                host: host.to_owned(),
                task: task.to_owned(),
                project: project.map(|label| {
                    let mut end = label.len().min(128);
                    while !label.is_char_boundary(end) {
                        end -= 1;
                    }
                    label[..end].to_owned()
                }),
                at: Some(now),
            });
        }
        self.touch(id, now);
        self.turns(id);
        self.save(id);
    }

    /// Move `id` to the top of the list with the time of its last message.
    fn touch(&mut self, id: &str, now: u64) {
        if let Some(at) = self.index.iter().position(|summary| summary.id == id) {
            let mut summary = self.index.remove(at);
            summary.updated = now;
            self.index.insert(0, summary);
        }
    }

    fn save(&mut self, id: &str) {
        if self.corrupt_index
            || self.storage_errors.contains_key(&item(id)) && !self.dirty.contains(id)
        {
            return;
        }
        self.dirty.insert(id.into());
        let summary = self.get(id).cloned();
        if let Some(turns) = self.turns.get_mut(id) {
            // Count escape expansion once per turn, retaining whole newest turns.
            let sizes: Vec<usize> = turns
                .iter()
                .map(|turn| {
                    serde_json::to_vec(turn).map_or(usize::MAX / MAX_TURNS, |bytes| bytes.len())
                })
                .collect();
            let overhead = serde_json::to_vec(&Saved {
                turns: vec![],
                summary: summary.clone(),
                lane: self.lanes.get(id).copied(),
            })
            .map_or(MAX_TALK_BYTES, |bytes| bytes.len());
            let mut bytes = sizes.iter().sum::<usize>() + overhead + turns.len().saturating_sub(1);
            let mut remove = 0;
            while turns.len() - remove > MAX_TURNS
                || turns.len() - remove > 1 && bytes > MAX_TALK_BYTES
            {
                bytes = bytes.saturating_sub(sizes[remove] + 1);
                remove += 1;
            }
            turns.drain(..remove);
            let key = item(id);
            let result = self.store.as_ref().map_or(Ok(()), |store| {
                store.write(
                    &key,
                    &Saved {
                        turns: turns.clone(),
                        summary,
                        lane: self.lanes.get(id).copied(),
                    },
                )
            });
            match result {
                Ok(()) => {
                    self.storage_errors.remove(&key);
                    self.dirty.remove(id);
                }
                Err(error) => {
                    self.storage_errors.insert(key, error);
                }
            }
        }
        self.save_index();
    }

    // Records contain their own summary. The bounded index is a warm head;
    // opening the cache recovers the remaining summaries from those records.
    fn save_index(&mut self) {
        if !self.corrupt_index {
            match self.store.as_ref().map_or(Ok(()), |store| {
                store.write("basic-index", &self.index[..self.index.len().min(128)])
            }) {
                Ok(()) => {
                    self.storage_errors.remove("basic-index");
                    self.dirty_index = false;
                }
                Err(error) => {
                    self.storage_errors.insert("basic-index".into(), error);
                    self.dirty_index = true;
                }
            }
        }
        self.storage_error = self.storage_errors.values().next().cloned();
    }

    /// Take in a whole conversation kept elsewhere, as it is: its ID,
    /// title, times, turns (with their router metadata and send IDs), lane,
    /// and Coder link (`crate::migrate`). `Ok(false)` when this store
    /// already holds it, in the list or as a record, which is left
    /// untouched. On a failed write nothing is kept in memory, so a later
    /// attempt writes it again. The host imports Claude Code and Codex
    /// sessions the same way (`coder_host::sessions`).
    pub fn adopt(
        &mut self,
        summary: Summary,
        turns: Vec<Turn>,
        lane: Option<Lane>,
    ) -> Result<bool, String> {
        let id = summary.id.clone();
        let key = item(&id);
        if self.corrupt_index {
            return Err("This store's list can't be read.".into());
        }
        if self.get(&id).is_some() {
            return Ok(false);
        }
        if let Some(store) = &self.store {
            match store.read::<Saved>(&key) {
                Ok(None) => {}
                Ok(Some(_)) => return Ok(false),
                Err(error) => return Err(error),
            }
        }
        let at = self
            .index
            .iter()
            .position(|row| row.updated <= summary.updated)
            .unwrap_or(self.index.len());
        self.index.insert(at, summary);
        self.turns.insert(id.clone(), turns);
        if let Some(lane) = lane {
            self.lanes.insert(id.clone(), lane);
        }
        self.save(&id);
        if let Some(error) = self.storage_errors.get(&key).cloned() {
            self.index.retain(|row| row.id != id);
            self.turns.remove(&id);
            self.lanes.remove(&id);
            self.dirty.remove(&id);
            self.storage_errors.remove(&key);
            self.storage_error = self.storage_errors.values().next().cloned();
            return Err(error);
        }
        Ok(true)
    }

    #[cfg(test)]
    pub(crate) fn set_turns_for_test(&mut self, id: &str, turns: Vec<Turn>, lane: Option<Lane>) {
        self.turns.insert(id.to_owned(), turns);
        if let Some(lane) = lane {
            self.lanes.insert(id.to_owned(), lane);
        }
        self.save(id);
    }

    /// Retry interrupted writes without appending or resending a message.
    pub fn flush_pending(&mut self) {
        if self.dirty_used {
            self.save_used();
        }
        for id in self.dirty.iter().cloned().collect::<Vec<_>>() {
            self.save(&id);
        }
        if self.dirty_index {
            self.save_index();
        }
    }
}

/// One message as a shared chat carries it: its words within the report's
/// bound, and, for a reply, its prepared answer, tier, and judgment.
fn chat_turn(turn: &Turn) -> ChatTurn {
    let meta = turn.meta.as_ref();
    ChatTurn {
        role: match turn.role {
            Role::User => ChatRole::User,
            Role::Assistant => ChatRole::Assistant,
        },
        text: turn.text.chars().take(MAX_CHAT_TEXT_CHARS).collect(),
        answer: meta.and_then(|meta| meta.answer.clone()),
        tier: meta.and_then(|meta| meta.tier.clone()),
        judgment: meta.and_then(|meta| meta.judgment.clone()),
    }
}

/// The store key of a conversation's turns.
pub(crate) fn item(id: &str) -> String {
    format!("basic-{id}")
}

/// The line every handoff prompt carries after its title, which the phone
/// recognizes to show the prompt as one line instead of the whole
/// conversation again.
pub const HANDOFF_MARK: &str =
    "Continue this conversation from the OpenAgents app on this computer.";

/// Where a handoff came from, as a message's note: provenance kept beside
/// the person's words, never pasted in front of them (#10076).
pub const HANDOFF_NOTE: &str = "Continued from the OpenAgents app";

/// The words the person asked with, from a handoff prompt [`handoff`]
/// wrote, wherever it appears (the echo of what was sent, and the
/// computer's transcript's first message): the message that asked for the
/// work, once, never the conversation pasted back at the person and never
/// prefixed with where it came from, which is [`HANDOFF_NOTE`]'s. A prompt
/// with no request part reads as its title. A message that is not a
/// handoff reads as `None`. This reads only the shape [`handoff`] itself
/// writes, after the route is chosen.
pub fn handoff_request(text: &str) -> Option<String> {
    let (head, rest) = text.split_once(HANDOFF_MARK)?;
    let request = rest
        .strip_prefix(" The request:\n\n")
        .map(|request| {
            [HANDOFF_ROUTING, HANDOFF_CONTEXT]
                .iter()
                .fold(request, |request, lead| {
                    request.split_once(lead).map_or(request, |(asked, _)| asked)
                })
                .trim()
        })
        .filter(|request| !request.is_empty());
    let title = head.lines().next().unwrap_or("").trim();
    Some(
        request
            .unwrap_or(if title.is_empty() { "Chat" } else { title })
            .to_owned(),
    )
}

/// How many turns before the request a handoff carries as context.
pub const HANDOFF_CONTEXT_TURNS: usize = 6;

/// The lead of a handoff's routing section ([`handoff_routed`]): what the
/// engine is told about how its run was started, after the request.
pub const HANDOFF_ROUTING: &str = "\n\nHow this run started:\n\n";

/// The lead of a handoff's context section: the turns before the request.
pub const HANDOFF_CONTEXT: &str = "\n\nEarlier in the conversation, for context:\n\n";

/// The context a computer's Coder starts with when the person runs Coder
/// from a conversation (#10073): `title` as the first line (the task's
/// title), then the message that asked for the work, the newest user turn,
/// whole or as much of it as fits, then at most
/// [`HANDOFF_CONTEXT_TURNS`] turns before it, newest kept, as context, all
/// within `limit` bytes. Replies after the request, such as the offer that
/// started Coder, are not carried: they are ours, not the person's ask.
pub fn handoff(title: &str, turns: &[Turn], limit: usize) -> String {
    handoff_routed(title, turns, limit, None)
}

/// [`handoff`] with a routing section after the request (#10084): what the
/// engine is told about how its run was started, such as
/// [`crate::delegation::routing`]. The section is kept whole: the request
/// is cut first, then the context.
pub fn handoff_routed(title: &str, turns: &[Turn], limit: usize, routing: Option<&str>) -> String {
    let routing = routing
        .map(str::trim)
        .filter(|routing| !routing.is_empty())
        .map(|routing| format!("{HANDOFF_ROUTING}{routing}"))
        .unwrap_or_default();
    let total = limit;
    let limit = total.saturating_sub(routing.len());
    let title: String = title
        .lines()
        .next()
        .unwrap_or("Chat")
        .chars()
        .take(80)
        .collect();
    let asked = turns.iter().rposition(|turn| turn.role == Role::User);
    let head = format!("{title}\n\n{HANDOFF_MARK}");
    let Some(asked) = asked else {
        return format!("{}{routing}", clip(&head, limit));
    };
    let request = format!("{head} The request:\n\n{}", turns[asked].text.trim());
    if request.len() >= limit {
        return format!("{}{routing}", clip(&request, limit));
    }
    let request = format!("{request}{routing}");
    let limit = total;
    let lead = HANDOFF_CONTEXT;
    let mut bytes = request.len() + lead.len();
    let mut parts: Vec<String> = vec![];
    for turn in turns[..asked].iter().rev().take(HANDOFF_CONTEXT_TURNS) {
        let who = match turn.role {
            Role::User => "User",
            Role::Assistant => "Coder",
        };
        let part = format!("{who}: {}\n\n", turn.text.trim());
        if bytes + part.len() > limit {
            break;
        }
        bytes += part.len();
        parts.push(part);
    }
    if parts.is_empty() {
        return request;
    }
    parts.reverse();
    format!("{request}{lead}{}", parts.concat())
        .trim_end()
        .to_owned()
}

/// The handoff's title: the first line of the message that asked for the
/// work (the newest user turn), at most 80 characters, or `fallback`, the
/// chat's title, when there is none (#10073).
pub fn handoff_title(fallback: &str, turns: &[Turn]) -> String {
    turns
        .iter()
        .rev()
        .find(|turn| turn.role == Role::User)
        .and_then(|turn| turn.text.lines().find(|line| !line.trim().is_empty()))
        .map(|line| line.trim().chars().take(80).collect::<String>())
        .filter(|line| !line.is_empty())
        .unwrap_or_else(|| fallback.to_owned())
}

/// `text`'s first `limit` bytes, cut at a character boundary.
fn clip(text: &str, limit: usize) -> String {
    let mut end = limit.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

/// `job`, then a ring (`wake`), so its answer shows at once.
async fn rung(job: impl std::future::Future<Output = ()>, wake: crate::Wake) {
    job.await;
    wake();
}

impl Drop for BasicChats {
    fn drop(&mut self) {
        for stream in self.streams.values_mut() {
            if let Some(handle) = stream.handle.take() {
                handle.abort();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::pin::Pin;

    /// Answers every message with its reverse, in two partials.
    struct Echo;

    impl Door for Echo {
        fn ask(
            &self,
            turns: Vec<Turn>,
            _context: Context,
            reply: Arc<Mutex<Reply>>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(async move {
                let text: String = turns.last().unwrap().text.chars().rev().collect();
                let mut reply = lock(&reply);
                reply.text = text;
                reply.done = true;
            })
        }
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    fn settle(chats: &mut BasicChats, runtime: &tokio::runtime::Runtime, now: u64) {
        for _ in 0..50 {
            runtime.block_on(tokio::task::yield_now());
            chats.settle(now);
            if !chats.streaming() {
                return;
            }
        }
        panic!("the reply never ended");
    }

    #[test]
    fn retention_counts_json_escapes_and_keeps_whole_messages() {
        let dir = tempfile::tempdir().unwrap();
        let key = secp256k1::SecretKey::from_byte_array([5; 32]).unwrap();
        let mut chats = BasicChats::new(None, None, Some(Cache::open(dir.path(), &key).unwrap()));
        let id = "5".repeat(32);
        assert!(chats.create(&id, 1));
        let text = "\u{0001}".repeat(20 * 1024);
        chats.turns.insert(
            id.clone(),
            vec![Turn::user(&text), Turn::assistant(text.clone(), None)],
        );
        chats.save(&id);
        assert!(chats.storage_error.is_none());
        assert_eq!(chats.turns(&id).len(), 1);
        assert_eq!(chats.turns(&id)[0].text, text);
        drop(chats);
        let mut reopened =
            BasicChats::new(None, None, Some(Cache::open(dir.path(), &key).unwrap()));
        assert_eq!(reopened.turns(&id)[0].text, text);
    }

    #[test]
    fn a_conversation_keeps_its_turns_across_a_relaunch() {
        let runtime = runtime();
        let dir = tempfile::tempdir().unwrap();
        let secret = secp256k1::SecretKey::from_byte_array([0x31; 32]).unwrap();
        let store = || Cache::open(dir.path(), &secret).ok();
        let mut chats = BasicChats::new(
            Some(runtime.handle().clone()),
            Some(Arc::new(Echo)),
            store(),
        );
        let id = chats.start("hello\nsecond line", 10).unwrap();
        assert!(chats.busy(&id));
        settle(&mut chats, &runtime, 11);
        assert!(chats.send(&id, "abc", 12));
        settle(&mut chats, &runtime, 13);
        let again = BasicChats::new(None, None, store());
        assert_eq!(again.list().len(), 1);
        assert_eq!(again.list()[0].title, "hello");
        assert_eq!(again.list()[0].updated, 13);
        let mut again = again;
        let texts: Vec<&str> = again.turns(&id).iter().map(|t| t.text.as_str()).collect();
        assert_eq!(
            texts,
            ["hello\nsecond line", "enil dnoces\nolleh", "abc", "cba"]
        );
    }

    #[test]
    fn route_counts_read_every_saved_chat_without_keeping_its_turns() {
        let dir = tempfile::tempdir().unwrap();
        let secret = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
        let store = || Cache::open(dir.path(), &secret).ok();
        let mut chats = BasicChats::new(None, None, store());
        for (n, routes) in [("a", vec!["meta", "meta"]), ("b", vec!["work.dispatch"])] {
            let id = n.repeat(32);
            chats.create(&id, 1);
            let mut turns = vec![];
            for route in routes {
                turns.push(Turn::user("q"));
                turns.push(Turn::assistant(
                    "a",
                    Some(crate::router::Meta {
                        route: Some(route.into()),
                        ..Default::default()
                    }),
                ));
            }
            // A user turn never counts, even with a route on it.
            turns.push(Turn {
                meta: Some(crate::router::Meta {
                    route: Some("end".into()),
                    ..Default::default()
                }),
                ..Turn::user("bye")
            });
            chats.turns.insert(id.clone(), turns);
            chats.dirty.insert(id);
        }
        chats.flush_pending();
        drop(chats);
        let mut chats = BasicChats::new(None, None, store());
        let counts = chats.route_counts();
        assert_eq!(
            counts,
            BTreeMap::from([("meta".to_string(), 2), ("work.dispatch".to_string(), 1)])
        );
        assert!(chats.turns.is_empty(), "the turns are read, not kept");
        let snapshot =
            crate::service::apply(&mut chats, crate::service::Command::Routes {}, 2).unwrap();
        assert_eq!(snapshot.routes, counts);
        assert!(snapshot.chat.is_none(), "no conversation opens");
    }

    #[test]
    fn computer_judgment_and_full_project_binding_survive_restart() {
        let dir = tempfile::tempdir().unwrap();
        let secret = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
        let store = || Cache::open(dir.path(), &secret).ok();
        let mut chats = BasicChats::new(None, None, store());
        let id = "b".repeat(32);
        chats.create(&id, 1);
        chats.turns.insert(
            id.clone(),
            vec![
                Turn::user("fix the flaky test in openagents"),
                Turn::assistant("Ready for Coder.", None),
            ],
        );
        chats.lanes.insert(id.clone(), Lane::Computer);
        let project = "project-".to_owned() + &"長".repeat(35);
        chats.spawned_in(&id, &"a".repeat(64), &"c".repeat(64), Some(&project), 10);
        assert!(chats.storage_error.is_none());
        drop(chats);
        let mut chats = BasicChats::new(None, None, store());
        let snapshot = crate::service::apply(
            &mut chats,
            crate::service::Command::Read {
                chat: id.clone(),
                before: None,
            },
            11,
        )
        .unwrap();
        assert!(snapshot.computer);
        let coder = snapshot.coder.unwrap();
        assert_eq!(coder.project.as_deref(), Some(project.as_str()));
        assert_eq!(coder.at, Some(10));
        chats.spawned_in(&id, &coder.host, &coder.task, Some(&"長".repeat(100)), 12);
        assert_eq!(
            chats
                .get(&id)
                .unwrap()
                .coder
                .as_ref()
                .unwrap()
                .project
                .as_ref()
                .unwrap()
                .len(),
            126
        );
    }

    #[test]
    fn a_failed_reply_leaves_no_turn_and_can_be_tried_again() {
        let runtime = runtime();
        let mut chats = BasicChats::new(Some(runtime.handle().clone()), None, None);
        let id = chats.start("hi", 1).unwrap();
        settle(&mut chats, &runtime, 2);
        assert!(matches!(chats.tail(&id), Tail::Failed(_)));
        assert_eq!(chats.turns(&id).len(), 1);
        chats.door = Some(Arc::new(Echo));
        chats.retry(&id);
        settle(&mut chats, &runtime, 3);
        assert!(matches!(chats.tail(&id), Tail::None));
        assert_eq!(chats.turns(&id).len(), 2);
    }

    #[test]
    fn the_handoff_keeps_the_newest_turns_within_its_bound() {
        let turns = vec![
            Turn::user("x".repeat(500)),
            Turn::assistant("Sure.", None),
            Turn::user("Run the tests in my repo."),
        ];
        let text = handoff("Tests", &turns, 300);
        assert!(
            text.starts_with(
                "Tests\n\nContinue this conversation from the OpenAgents app on this computer. \
                 The request:\n\nRun the tests in my repo."
            ),
            "{text}"
        );
        assert!(text.ends_with("Coder: Sure."), "{text}");
        assert!(!text.contains("xxx"), "{text}");
        assert!(text.len() <= 300);
        let alone = handoff("Tests", &turns[..1], 200);
        assert!(
            alone.len() <= 200 && alone.contains("request:\n\nxx"),
            "{alone}"
        );
    }

    /// The owner's chat on 2026-09-30 (#10073): Coder must get the message
    /// that asked for the work, titled by it, not the chat's first message,
    /// and not the reply after it.
    #[test]
    fn the_handoff_leads_with_the_message_that_asked_for_the_work() {
        let mut turns = vec![
            Turn::user("who are you"),
            Turn::assistant("We are OpenAgents.", None),
            Turn::user("who can you delegate to"),
            Turn::assistant("We delegate to Coder, our coding agent.", None),
            Turn::user("do a test delegation now"),
            Turn::assistant("We'd test Project map with its published test set.", None),
        ];
        let title = handoff_title("who are you", &turns);
        assert_eq!(title, "do a test delegation now");
        let text = handoff(&title, &turns, 16 * 1024);
        assert!(
            text.starts_with("do a test delegation now\n\n"),
            "the request titles the task: {text}"
        );
        let request = text
            .find("The request:\n\ndo a test delegation now")
            .unwrap();
        let context = text.find("User: who are you").unwrap();
        assert!(request < context, "the request comes before the context");
        assert!(
            !text.contains("Project map"),
            "the reply after it is not carried"
        );
        assert_eq!(
            handoff_request(&text).as_deref(),
            Some("do a test delegation now")
        );
        // Context is bounded to the turns just before the request.
        turns.splice(0..0, (0..20).map(|n| Turn::user(format!("old {n}"))));
        let text = handoff(&title, &turns, 16 * 1024);
        assert!(
            text.contains("old 18") && !text.contains("old 17"),
            "{text}"
        );
        // No user turn: the chat's title.
        assert_eq!(handoff_title("Chat", &[]), "Chat");
    }

    /// A handoff prompt shows as the person's own message, once, wherever
    /// it appears, without the conversation pasted back or a prefix saying
    /// where it came from (#10076); an ordinary message is not one.
    #[test]
    fn a_handoff_reads_as_the_message_that_asked() {
        let turns = vec![
            Turn::user("who are you"),
            Turn::assistant("We are OpenAgents.", None),
            Turn::user("Run the tests in my repo"),
        ];
        let text = handoff("Run the tests in my repo", &turns, 16 * 1024);
        assert!(text.contains("Earlier in the conversation"), "{text}");
        assert_eq!(
            handoff_request(&text).as_deref(),
            Some("Run the tests in my repo")
        );
        // A long request, cut to the limit, still reads as itself.
        let long = vec![Turn::user("x".repeat(5_000))];
        let text = handoff("Run the tests", &long, 16 * 1024);
        assert_eq!(handoff_request(&text), Some("x".repeat(5_000)));
        // No user turn: the title.
        assert_eq!(
            handoff_request(&handoff("Run the tests", &[], 16 * 1024)).as_deref(),
            Some("Run the tests")
        );
        assert!(handoff_request("Run the tests in my repo").is_none());
        assert!(!HANDOFF_NOTE.contains(':'));
    }
}
