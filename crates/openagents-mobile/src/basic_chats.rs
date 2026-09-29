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
use crate::router::{Context, Meta};
use coder_computers::cache::Cache;
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
const MAX_TALKS: usize = 200;
/// The most turns one conversation keeps; the oldest go first.
const MAX_TURNS: usize = 400;
/// The most bytes of turns one conversation keeps, inside the store's
/// 192 KiB item bound.
const MAX_TALK_BYTES: usize = 150 * 1024;

/// The task a conversation started on a computer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Spawned {
    pub host: String,
    pub task: String,
}

/// One conversation's row in the list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Summary {
    pub id: String,
    pub title: String,
    pub started: u64,
    /// When its last message was sent or answered.
    pub updated: u64,
    #[serde(default)]
    pub coder: Option<Spawned>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Saved {
    turns: Vec<Turn>,
}

/// A reply streaming into a conversation.
struct Stream {
    reply: Arc<Mutex<Reply>>,
    handle: Option<JoinHandle<()>>,
    markdown: IncrementalMarkdown,
}

/// What an open conversation shows below its turns.
pub(crate) enum Tail {
    None,
    /// Waiting for the first words.
    Thinking,
    /// The reply so far, parsed for display.
    Streaming(Vec<rust_native::markdown::Block>),
    /// Why the last message got no reply.
    Failed(String),
}

pub(crate) struct BasicChats {
    runtime: Option<Handle>,
    door: Option<Arc<dyn Door>>,
    store: Option<Cache>,
    index: Vec<Summary>,
    turns: BTreeMap<String, Vec<Turn>>,
    streams: BTreeMap<String, Stream>,
    failures: BTreeMap<String, String>,
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
}

/// A new chat's suggestions this few are shown as they are, unranked.
const RANK_AT_LEAST: usize = 2;

impl BasicChats {
    /// Conversations kept in `store`, answered through `door` on `runtime`.
    pub(crate) fn new(
        runtime: Option<Handle>,
        door: Option<Arc<dyn Door>>,
        store: Option<Cache>,
    ) -> Self {
        let index: Vec<Summary> = store
            .as_ref()
            .and_then(|store| store.read("basic-index").ok().flatten())
            .unwrap_or_default();
        Self {
            runtime,
            door,
            store,
            index,
            turns: BTreeMap::new(),
            streams: BTreeMap::new(),
            failures: BTreeMap::new(),
            lanes: BTreeMap::new(),
            ranking: None,
            rank_allowed: false,
            context: Context::default(),
        }
    }

    /// No store and no door, as in tests of other surfaces.
    pub(crate) fn empty() -> Self {
        Self::new(None, None, None)
    }

    /// Every conversation, newest first.
    pub(crate) fn list(&self) -> &[Summary] {
        &self.index
    }

    pub(crate) fn get(&self, id: &str) -> Option<&Summary> {
        self.index.iter().find(|summary| summary.id == id)
    }

    /// The conversation's turns, read from the store the first time.
    pub(crate) fn turns(&mut self, id: &str) -> &[Turn] {
        if !self.turns.contains_key(id) {
            let saved: Saved = self
                .store
                .as_ref()
                .and_then(|store| store.read(&item(id)).ok().flatten())
                .unwrap_or_default();
            self.turns.insert(id.to_owned(), saved.turns);
        }
        self.turns.get(id).map_or(&[], Vec::as_slice)
    }

    /// Whether a reply is streaming into any conversation, or the worker is
    /// ranking the suggestions.
    pub(crate) fn streaming(&self) -> bool {
        !self.streams.is_empty()
            || self
                .ranking
                .as_ref()
                .is_some_and(|(_, reply)| !lock(reply).ended())
    }

    /// Keep the way to the worker open while the tab shows, and let the
    /// next new chat's suggestions be ranked once.
    pub(crate) fn warm(&mut self) {
        self.rank_allowed = true;
        if let (Some(door), Some(runtime)) = (&self.door, &self.runtime) {
            door.warm(runtime);
        }
    }

    /// Close the way to the worker once no reply waits on it.
    pub(crate) fn rest(&self) {
        if let Some(door) = &self.door {
            door.rest();
        }
    }

    /// What the next turn tells the worker about the phone: whether a
    /// computer is ready, and the app's build.
    pub(crate) fn set_context(&mut self, context: Context) {
        self.context = context;
    }

    /// What the router said about the last reply of `id`: the streaming
    /// reply's, while one streams, else the last answer's.
    pub(crate) fn last_meta(&self, id: &str) -> Option<Meta> {
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
    pub(crate) fn shared(&mut self, id: &str) -> Option<SharedChat> {
        let turns = self.turns(id);
        let skip = turns.len().saturating_sub(MAX_CHAT_TURNS);
        let turns: Vec<ChatTurn> = turns[skip..].iter().map(chat_turn).collect();
        (!turns.is_empty()).then_some(SharedChat {
            reason: ShareReason::Shared,
            turns,
        })
    }

    /// Where the worker's judgment placed the last reply of `id`.
    pub(crate) fn lane(&self, id: &str) -> Option<Lane> {
        self.lanes.get(id).copied()
    }

    /// Ask the worker once to order a new chat's suggestions (ID and
    /// label), when there are enough to order, this set was not asked
    /// about already, and the tab has shown since the last rank job.
    pub(crate) fn want_rank(&mut self, candidates: Vec<(String, String)>) {
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
        runtime.spawn(rung(door.rank(candidates, reply.clone())));
        self.ranking = Some((set, reply));
    }

    /// Order `items` by the worker's ranking when it answered for exactly
    /// their IDs; otherwise, or when it failed, leave the phone's order.
    pub(crate) fn rank_order<T>(&self, items: &mut [(String, T)]) {
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
    pub(crate) fn busy(&self, id: &str) -> bool {
        self.streams.contains_key(id)
    }

    /// Start a conversation with `text` and ask for the reply.
    pub(crate) fn start(&mut self, text: &str, now: u64) -> Option<String> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        let id = uuid::Uuid::new_v4().simple().to_string();
        let title: String = text
            .lines()
            .next()
            .unwrap_or(text)
            .chars()
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
        self.send(&id, text, now);
        Some(id)
    }

    /// Add the user's message to `id` and ask for the reply. A message
    /// while a reply streams waits for it: the composer is busy then.
    pub(crate) fn send(&mut self, id: &str, text: &str, now: u64) -> bool {
        let text = text.trim();
        if text.is_empty() || self.busy(id) || self.get(id).is_none() {
            return false;
        }
        self.turns(id);
        if let Some(turns) = self.turns.get_mut(id) {
            turns.push(Turn::user(text));
        }
        self.touch(id, now);
        self.save(id);
        self.ask(id);
        true
    }

    /// Ask again for the reply to the last message, after a failure.
    pub(crate) fn retry(&mut self, id: &str) {
        let last_is_user = self
            .turns(id)
            .last()
            .is_some_and(|turn| turn.role == Role::User);
        if last_is_user && !self.busy(id) {
            self.ask(id);
        }
    }

    /// Stop the reply streaming into `id`. What streamed is kept as the
    /// reply.
    pub(crate) fn stop(&mut self, id: &str, now: u64) {
        let Some(mut stream) = self.streams.remove(id) else {
            return;
        };
        if let Some(handle) = stream.handle.take() {
            handle.abort();
        }
        let (text, meta) = {
            let reply = lock(&stream.reply);
            (reply.text.clone(), reply.meta.clone())
        };
        if !text.trim().is_empty() {
            self.answer(id, text, meta, now);
        }
    }

    fn ask(&mut self, id: &str) {
        self.failures.remove(id);
        self.lanes.remove(id);
        let reply = Arc::new(Mutex::new(Reply::default()));
        let turns = self.turns.get(id).cloned().unwrap_or_default();
        let handle = match (&self.door, &self.runtime) {
            (Some(door), Some(runtime)) => {
                Some(runtime.spawn(rung(door.ask(turns, self.context.clone(), reply.clone()))))
            }
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
    pub(crate) fn settle(&mut self, now: u64) -> bool {
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
                None => self.answer(&id, reply.text, reply.meta, now),
                Some(failure) => {
                    self.failures.insert(id, failure.describe());
                }
            }
        }
        changed
    }

    fn answer(&mut self, id: &str, text: String, meta: Meta, now: u64) {
        self.turns(id);
        if let Some(turns) = self.turns.get_mut(id) {
            turns.push(Turn::assistant(text, (!meta.is_empty()).then_some(meta)));
        }
        self.touch(id, now);
        self.save(id);
    }

    /// What shows below the conversation's turns.
    pub(crate) fn tail(&self, id: &str) -> Tail {
        if let Some(stream) = self.streams.get(id) {
            let blocks = stream.markdown.display_blocks();
            return if blocks.is_empty() {
                Tail::Thinking
            } else {
                Tail::Streaming(blocks.into_owned())
            };
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
    pub(crate) fn spawned(&mut self, id: &str, host: &str, task: &str, now: u64) {
        if let Some(summary) = self.index.iter_mut().find(|summary| summary.id == id) {
            summary.coder = Some(Spawned {
                host: host.to_owned(),
                task: task.to_owned(),
            });
        }
        self.touch(id, now);
        self.save_index();
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
        if let Some(turns) = self.turns.get_mut(id) {
            // Keep what the store can hold: the newest turns.
            while turns.len() > MAX_TURNS
                || turns.len() > 1
                    && turns.iter().map(|turn| turn.text.len()).sum::<usize>() > MAX_TALK_BYTES
            {
                turns.remove(0);
            }
            if let Some(store) = &self.store {
                let _ = store.write(
                    &item(id),
                    &Saved {
                        turns: turns.clone(),
                    },
                );
            }
        }
        self.save_index();
    }

    fn save_index(&self) {
        if let Some(store) = &self.store {
            let _ = store.write("basic-index", &self.index);
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
fn item(id: &str) -> String {
    format!("basic-{id}")
}

/// The context a computer's Coder starts with when the person runs Coder
/// from a conversation: its title as the first line (the task's title), then
/// the conversation so far, newest turns kept, within `limit` bytes.
pub(crate) fn handoff(title: &str, turns: &[Turn], limit: usize) -> String {
    let title: String = title
        .lines()
        .next()
        .unwrap_or("Chat")
        .chars()
        .take(80)
        .collect();
    let head = format!(
        "{title}\n\nContinue this conversation from the OpenAgents app on this \
         computer. The conversation so far:\n\n"
    );
    let mut parts: Vec<String> = vec![];
    let mut bytes = head.len();
    for turn in turns.iter().rev() {
        let who = match turn.role {
            Role::User => "User",
            Role::Assistant => "Coder",
        };
        let part = format!("{who}: {}\n\n", turn.text.trim());
        if bytes + part.len() > limit {
            if parts.is_empty() {
                // The newest message alone is too long: keep its start.
                let room = limit.saturating_sub(bytes);
                let mut end = room.min(part.len());
                while !part.is_char_boundary(end) {
                    end -= 1;
                }
                parts.push(part[..end].to_owned());
            }
            break;
        }
        bytes += part.len();
        parts.push(part);
    }
    parts.reverse();
    format!("{head}{}", parts.concat()).trim_end().to_owned()
}

/// `job`, then a ring (`wake`), so its answer shows at once.
async fn rung(job: impl std::future::Future<Output = ()>) {
    job.await;
    crate::wake::ring();
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
            text.starts_with("Tests\n\nContinue this conversation"),
            "{text}"
        );
        assert!(text.contains("Coder: Sure."), "{text}");
        assert!(text.ends_with("User: Run the tests in my repo."), "{text}");
        assert!(!text.contains("xxx"), "{text}");
        assert!(text.len() <= 300);
        let alone = handoff("Tests", &turns[..1], 200);
        assert!(alone.len() <= 200 && alone.contains("User: xx"), "{alone}");
    }
}
