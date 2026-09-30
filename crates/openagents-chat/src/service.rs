//! Local chat operations. A host admits these only on its same-user socket.

use crate::basic_chats::{BasicChats, Summary, Tail};
use crate::basic_coder::Turn;
use serde::{Deserialize, Serialize};

/// A closed local operation; the caller supplies stable IDs for new chats and sends.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    /// A resident host handles this through its admitted Coder broker.
    RunCoder {
        chat: String,
    },
    /// Record that `task` runs for this chat, started on `host` in
    /// `project`: the binding a Coder run this chat asked for keeps, so
    /// every surface can follow it. `host` is a host's key, or `local` for
    /// a run this computer started for the person at it. It grants nothing
    /// and runs nothing; the chat must exist and not be bound already to
    /// another task.
    BindCoder {
        chat: String,
        host: String,
        task: String,
        project: Option<String>,
    },
    List {},
    ListMore {
        after: usize,
        version: u64,
    },
    UseSuggestion {
        chat: String,
        id: String,
    },
    Create {
        chat: String,
    },
    Read {
        chat: String,
        before: Option<usize>,
    },
    Send {
        chat: String,
        request: String,
        text: String,
    },
    Retry {
        chat: String,
    },
    Stop {
        chat: String,
    },
    Rename {
        chat: String,
        title: String,
    },
    Pin {
        chat: String,
        pinned: bool,
    },
    Archive {
        chat: String,
    },
    Restore {
        chat: String,
    },
}

/// A bounded page of a conversation and its current reply.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coder: Option<crate::basic_chats::Spawned>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ready_computer: Option<String>,
    pub chats: Vec<Summary>,
    #[serde(default)]
    pub list_start: usize,
    #[serde(default)]
    pub list_total: usize,
    #[serde(default)]
    pub list_version: u64,
    /// Bounded digests shared with the phone's non-repeated suggestion policy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub used: Vec<String>,
    pub chat: Option<String>,
    /// Index of the oldest included turn; read before this for earlier turns.
    pub start: usize,
    pub total: usize,
    pub turns: Vec<Turn>,
    /// The worker judged that this reply belongs on a computer; this grants no execution.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub computer: bool,
    pub busy: bool,
    pub partial: String,
    pub failure: Option<String>,
    pub storage_error: Option<String>,
}

fn identity(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

/// Apply a validated local operation. Repeated sends bind to their exact text.
///
/// # Errors
/// Rejects invalid IDs, missing conversations, conflicting commands, a busy
/// conversation, or a failed encrypted write. No operation executes commands.
pub fn apply(chats: &mut BasicChats, command: Command, now: u64) -> Result<Snapshot, String> {
    chats.flush_pending();
    chats.settle(now);
    let (id, before) = match command {
        Command::RunCoder { .. } => {
            return Err("This chat service has no admitted Coder broker.".into());
        }
        Command::BindCoder {
            chat,
            host,
            task,
            project,
        } => {
            if !identity(&chat) || chats.get(&chat).is_none() {
                return Err("Chat not found.".into());
            }
            let bounded = |text: &str, max: usize| {
                !text.is_empty()
                    && text.len() <= max
                    && text
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-:".contains(&b))
            };
            if !bounded(&host, 128)
                || !bounded(&task, 128)
                || project.as_deref().is_some_and(|p| !bounded(p, 128))
            {
                return Err("Invalid Coder task.".into());
            }
            if let Some(bound) = chats.get(&chat).and_then(|summary| summary.coder.as_ref())
                && (bound.task != task || bound.host != host)
            {
                return Err("This chat already runs another Coder task.".into());
            }
            chats.spawned_in(&chat, &host, &task, project.as_deref(), now);
            (Some(chat), None)
        }
        Command::List {} => (None, None),
        Command::ListMore { after, version } => {
            let mut page = snapshot(chats, None, None)?;
            if page.list_version != version || after > chats.list().len() {
                return Err("Chat list changed. Refresh it and try again.".into());
            }
            page.list_start = after;
            page.chats =
                chats.list()[after..chats.list().len().min(after.saturating_add(128))].to_vec();
            return Ok(page);
        }
        Command::UseSuggestion { chat, id } => {
            if !identity(&chat) || chats.get(&chat).is_none() {
                return Err("Chat not found.".into());
            }
            if id.is_empty()
                || id.len() > 96
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._@-:".contains(&b))
            {
                return Err("Invalid suggestion ID.".into());
            }
            chats.use_suggestion(&id);
            (Some(chat), None)
        }
        Command::Create { chat } => {
            if !identity(&chat) {
                return Err("Invalid chat ID.".into());
            }
            if chats.get(&chat).is_none() && !chats.create(&chat, now) {
                return Err(
                    "Couldn't save a new chat. Archive an older chat and try again.".into(),
                );
            }
            (Some(chat), None)
        }
        Command::Read { chat, before } => (Some(chat), before),
        Command::Send {
            chat,
            request,
            text,
        } => {
            if !identity(&request) || text.trim().is_empty() || text.len() > 32 * 1024 {
                return Err("The message is empty or too long.".into());
            }
            if !identity(&chat) || chats.get(&chat).is_none() {
                return Err("Chat not found.".into());
            }
            let previous = chats
                .turns(&chat)
                .iter()
                .find(|turn| turn.request.as_deref() == Some(&request));
            if let Some(previous) = previous {
                if previous.text != text.trim() {
                    return Err("This send ID belongs to another message.".into());
                }
            } else if chats.get(&chat).is_some_and(|summary| summary.archived) {
                return Err("Restore this chat before sending a message.".into());
            } else if !chats.send_tagged(&chat, &text, now, Some(request)) {
                return Err(
                    "Couldn't send the message. Wait for the reply or check chat storage.".into(),
                );
            }
            (Some(chat), None)
        }
        Command::Retry { chat } => {
            if !identity(&chat) || chats.get(&chat).is_none() {
                return Err("Chat not found.".into());
            }
            chats.retry(&chat);
            (Some(chat), None)
        }
        Command::Stop { chat } => {
            if !identity(&chat) || chats.get(&chat).is_none() {
                return Err("Chat not found.".into());
            }
            chats.stop(&chat, now);
            (Some(chat), None)
        }
        Command::Rename { chat, title } => {
            if !identity(&chat) {
                return Err("Invalid chat ID.".into());
            }
            chats.rename(&chat, &title)?;
            (Some(chat), None)
        }
        Command::Pin { chat, pinned } => {
            if !identity(&chat) {
                return Err("Invalid chat ID.".into());
            }
            chats.pin(&chat, pinned)?;
            (Some(chat), None)
        }
        Command::Archive { chat } => {
            if !identity(&chat) {
                return Err("Invalid chat ID.".into());
            }
            chats.archive(&chat, now);
            (None, None)
        }
        Command::Restore { chat } => {
            if !identity(&chat) || chats.get(&chat).is_none() {
                return Err("Chat not found.".into());
            }
            chats.restore(&chat);
            (Some(chat), None)
        }
    };
    snapshot(chats, id, before)
}

fn snapshot(
    chats: &mut BasicChats,
    id: Option<String>,
    before: Option<usize>,
) -> Result<Snapshot, String> {
    use std::hash::{Hash, Hasher};
    let mut digest = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_vec(chats.list())
        .map_err(|_| "Couldn't read chat list.")?
        .hash(&mut digest);
    let mut snapshot = Snapshot {
        chats: chats.list()[..chats.list().len().min(128)].to_vec(),
        list_total: chats.list().len(),
        list_start: 0,
        list_version: digest.finish(),
        used: chats.used_markers().to_vec(),
        storage_error: chats.storage_error.clone(),
        ..Snapshot::default()
    };
    if let Some(id) = id {
        if !identity(&id) || chats.get(&id).is_none() {
            return Err("Chat not found.".into());
        }
        let turns = chats.turns(&id);
        snapshot.total = turns.len();
        let end = before.unwrap_or(turns.len()).min(turns.len());
        snapshot.start = end.saturating_sub(16);
        snapshot.turns = turns[snapshot.start..end].to_vec();
        snapshot.storage_error = chats.storage_error.clone();
        snapshot.computer = chats.lane(&id) == Some(crate::basic_coder::Lane::Computer);
        snapshot.coder = chats.get(&id).and_then(|summary| summary.coder.clone());
        snapshot.busy = chats.busy(&id);
        snapshot.partial = chats.partial(&id);
        if let Tail::Failed(why) = chats.tail(&id) {
            snapshot.failure = Some(why);
        }
        snapshot.chat = Some(id);
        // Leave room for the control envelope; escape expansion is included.
        while serde_json::to_vec(&snapshot)
            .map_err(|_| "Couldn't read chat.")?
            .len()
            > 220 * 1024
            && snapshot.turns.len() > 1
        {
            snapshot.turns.remove(0);
            snapshot.start += 1;
        }
        if serde_json::to_vec(&snapshot)
            .map_err(|_| "Couldn't read chat.")?
            .len()
            > 220 * 1024
        {
            return Err("This reply is too large to read over the local connection.".into());
        }
    }
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basic_coder::{Door, Reply, Turn, lock};
    use crate::cache::Cache;
    use crate::router::Context;
    use std::{
        future::Future,
        pin::Pin,
        sync::{Arc, Mutex},
    };

    struct Waiting;
    impl Door for Waiting {
        fn ask(
            &self,
            _: Vec<Turn>,
            _: Context,
            reply: Arc<Mutex<Reply>>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(async move {
                lock(&reply).text = "Some words".into();
                std::future::pending::<()>().await;
            })
        }
    }

    type Call = (Vec<Turn>, Arc<Mutex<Reply>>);
    #[derive(Default)]
    struct Controlled {
        calls: Mutex<Vec<Call>>,
    }
    impl Door for Controlled {
        fn ask(
            &self,
            turns: Vec<Turn>,
            _: Context,
            reply: Arc<Mutex<Reply>>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            self.calls.lock().unwrap().push((turns, reply));
            Box::pin(std::future::pending())
        }
    }

    #[test]
    fn a_chat_binds_one_coder_task_and_keeps_it() {
        let dir = tempfile::tempdir().unwrap();
        let secret = secp256k1::SecretKey::from_byte_array([7; 32]).unwrap();
        let mut chats =
            BasicChats::new(None, None, Some(Cache::open(dir.path(), &secret).unwrap()));
        let id = "e".repeat(32);
        let bind = |task: &str| Command::BindCoder {
            chat: id.clone(),
            host: "local".into(),
            task: task.into(),
            project: Some("scratch".into()),
        };
        assert!(apply(&mut chats, bind("t1"), 5).is_err(), "no such chat");
        apply(&mut chats, Command::Create { chat: id.clone() }, 1).unwrap();
        let snapshot = apply(&mut chats, bind("t1"), 5).unwrap();
        let coder = snapshot.coder.unwrap();
        assert_eq!((coder.host.as_str(), coder.task.as_str()), ("local", "t1"));
        assert_eq!(coder.project.as_deref(), Some("scratch"));
        // Binding the same task again is a no-op; another task is refused.
        assert!(apply(&mut chats, bind("t1"), 6).is_ok());
        assert!(apply(&mut chats, bind("t2"), 7).is_err());
        let odd = Command::BindCoder {
            chat: id.clone(),
            host: "local".into(),
            task: "../x y".into(),
            project: None,
        };
        assert!(apply(&mut chats, odd, 8).is_err());
    }

    #[test]
    fn management_survives_restart_and_list_pages_bind_to_one_revision() {
        let dir = tempfile::tempdir().unwrap();
        let secret = secp256k1::SecretKey::from_byte_array([7; 32]).unwrap();
        let mut chats =
            BasicChats::new(None, None, Some(Cache::open(dir.path(), &secret).unwrap()));
        for n in 0..512 {
            assert!(chats.create(&format!("{n:032x}"), n));
        }
        let id = format!("{:032x}", 15);
        apply(
            &mut chats,
            Command::Rename {
                chat: id.clone(),
                title: "Plan the Rocket".into(),
            },
            513,
        )
        .unwrap();
        apply(
            &mut chats,
            Command::Pin {
                chat: id.clone(),
                pinned: true,
            },
            513,
        )
        .unwrap();
        apply(&mut chats, Command::Archive { chat: id.clone() }, 513).unwrap();
        drop(chats);
        let mut chats =
            BasicChats::new(None, None, Some(Cache::open(dir.path(), &secret).unwrap()));
        assert_eq!(chats.list().len(), 512);
        let row = chats.get(&id).unwrap();
        assert_eq!(row.title, "Plan the Rocket");
        assert!(row.pinned && row.named && row.archived);
        apply(&mut chats, Command::Restore { chat: id.clone() }, 514).unwrap();
        assert!(chats.send(&id, "First message must not replace the name", 514));
        assert_eq!(chats.get(&id).unwrap().title, "Plan the Rocket");
        let first = apply(&mut chats, Command::List {}, 514).unwrap();
        assert_eq!(first.list_total, 512);
        assert_eq!(first.chats.len(), 128);
        let mut ids: std::collections::BTreeSet<_> =
            first.chats.iter().map(|s| s.id.clone()).collect();
        for after in [128, 256, 384] {
            let page = apply(
                &mut chats,
                Command::ListMore {
                    after,
                    version: first.list_version,
                },
                514,
            )
            .unwrap();
            assert_eq!(page.list_start, after);
            assert!(serde_json::to_vec(&page).unwrap().len() < 220 * 1024);
            ids.extend(page.chats.into_iter().map(|s| s.id));
        }
        assert_eq!(ids.len(), 512);
        apply(
            &mut chats,
            Command::Pin {
                chat: id.clone(),
                pinned: false,
            },
            514,
        )
        .unwrap();
        assert!(
            apply(
                &mut chats,
                Command::ListMore {
                    after: 128,
                    version: first.list_version
                },
                514
            )
            .is_err()
        );
        for title in ["", "bad\nname", &"x".repeat(161)] {
            assert!(chats.rename(&id, title).is_err());
        }
        assert!(
            chats
                .get(&id)
                .is_some_and(|row| !row.archived && !row.pinned)
        );
    }

    #[test]
    fn independent_streams_stop_retry_and_late_results_stay_bound_to_their_chat() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let door = Arc::new(Controlled::default());
        let mut chats = BasicChats::new(Some(runtime.handle().clone()), Some(door.clone()), None);
        let a = "a".repeat(32);
        let b = "b".repeat(32);
        for (chat, request, text) in [
            (a.clone(), "1".repeat(32), "First question"),
            (b.clone(), "2".repeat(32), "Other question"),
        ] {
            apply(&mut chats, Command::Create { chat: chat.clone() }, 1).unwrap();
            apply(
                &mut chats,
                Command::Send {
                    chat,
                    request,
                    text: text.into(),
                },
                2,
            )
            .unwrap();
        }
        let replies: Vec<_> = door
            .calls
            .lock()
            .unwrap()
            .iter()
            .map(|(_, reply)| reply.clone())
            .collect();
        lock(&replies[0]).text = "A partial".into();
        lock(&replies[1]).text = "B partial".into();
        let stopped = apply(&mut chats, Command::Stop { chat: a.clone() }, 3).unwrap();
        assert!(!stopped.busy);
        assert!(stopped.turns[1].stopped);
        assert_eq!(stopped.turns[1].text, "A partial");
        lock(&replies[0]).text = "Late old result".into();
        lock(&replies[0]).done = true;
        let retry = apply(&mut chats, Command::Retry { chat: a.clone() }, 4).unwrap();
        assert!(retry.busy);
        assert_eq!(
            retry.turns.len(),
            1,
            "retry keeps exactly one original user message"
        );
        assert_eq!(door.calls.lock().unwrap()[2].0[0].text, "First question");
        assert!(
            door.calls.lock().unwrap()[2]
                .0
                .iter()
                .all(|turn| !turn.stopped)
        );
        let current = door.calls.lock().unwrap()[2].1.clone();
        lock(&current).text = "A finished".into();
        lock(&current).done = true;
        lock(&replies[1]).text = "B finished".into();
        lock(&replies[1]).done = true;
        let a_done = apply(
            &mut chats,
            Command::Read {
                chat: a.clone(),
                before: None,
            },
            5,
        )
        .unwrap();
        let b_done = apply(
            &mut chats,
            Command::Read {
                chat: b,
                before: None,
            },
            5,
        )
        .unwrap();
        assert_eq!(a_done.turns[1].text, "A finished");
        assert_eq!(b_done.turns[1].text, "B finished");
        assert!(!a_done.turns[1].stopped);
        apply(
            &mut chats,
            Command::Send {
                chat: a,
                request: "3".repeat(32),
                text: "Follow up".into(),
            },
            6,
        )
        .unwrap();
        let calls = door.calls.lock().unwrap();
        assert_eq!(
            calls[3]
                .0
                .iter()
                .map(|turn| turn.text.as_str())
                .collect::<Vec<_>>(),
            ["First question", "A finished", "Follow up"]
        );
    }

    #[test]
    fn interrupted_writes_recover_without_duplicate_messages_and_metadata_is_atomic() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let key = secp256k1::SecretKey::from_byte_array([7; 32]).unwrap();
        let open = || Cache::open(dir.path(), &key).unwrap();
        let mut chats = BasicChats::new(
            Some(runtime.handle().clone()),
            Some(Arc::new(Waiting)),
            Some(open()),
        );
        let id = "7".repeat(32);
        apply(&mut chats, Command::Create { chat: id.clone() }, 1).unwrap();
        let path = dir.path().join(format!("basic-{id}.cache"));
        let previous = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        let send = Command::Send {
            chat: id.clone(),
            request: "8".repeat(32),
            text: "Keep this exactly".into(),
        };
        assert!(apply(&mut chats, send.clone(), 2).is_err());
        assert!(!chats.busy(&id));
        assert_eq!(chats.turns(&id).len(), 1);
        std::fs::remove_dir(&path).unwrap();
        std::fs::write(&path, previous).unwrap();
        let saved = apply(&mut chats, send, 3).unwrap();
        assert!(saved.storage_error.is_none());
        assert_eq!(saved.turns.len(), 1);
        // A durable unanswered message has an explicit retry, rather than an automatic resend.
        assert!(saved.failure.is_some());
        apply(&mut chats, Command::Retry { chat: id.clone() }, 4).unwrap();
        for _ in 0..100 {
            if !chats.partial(&id).is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        apply(&mut chats, Command::Stop { chat: id.clone() }, 5).unwrap();
        apply(&mut chats, Command::Archive { chat: id.clone() }, 6).unwrap();
        drop(chats);
        // The record restores its title and archive state even if a crash lost the auxiliary list.
        std::fs::remove_file(dir.path().join("basic-index.cache")).unwrap();
        let mut reopened = BasicChats::new(None, None, Some(open()));
        let recovered = apply(
            &mut reopened,
            Command::Read {
                chat: id,
                before: None,
            },
            7,
        )
        .unwrap();
        assert_eq!(recovered.turns.len(), 2);
        assert_eq!(recovered.chats[0].title, "Keep this exactly");
        assert!(recovered.chats[0].archived);
    }

    #[test]
    fn corrupt_storage_is_reported_and_never_replaced_with_an_empty_chat() {
        let dir = tempfile::tempdir().unwrap();
        let key = secp256k1::SecretKey::from_byte_array([6; 32]).unwrap();
        let open = || Cache::open(dir.path(), &key).unwrap();
        let mut chats = BasicChats::new(None, None, Some(open()));
        let id = "6".repeat(32);
        apply(&mut chats, Command::Create { chat: id.clone() }, 1).unwrap();
        drop(chats);
        let path = dir.path().join(format!("basic-{id}.cache"));
        std::fs::write(&path, "damaged").unwrap();
        let mut chats = BasicChats::new(None, None, Some(open()));
        let read = apply(
            &mut chats,
            Command::Read {
                chat: id.clone(),
                before: None,
            },
            2,
        )
        .unwrap();
        assert!(read.storage_error.is_some());
        assert!(
            apply(
                &mut chats,
                Command::Send {
                    chat: id,
                    request: "7".repeat(32),
                    text: "Don't overwrite".into()
                },
                3
            )
            .is_err()
        );
        assert_eq!(std::fs::read_to_string(path).unwrap(), "damaged");
        std::fs::write(dir.path().join("basic-index.cache"), "damaged index").unwrap();
        let mut chats = BasicChats::new(None, None, Some(open()));
        assert!(
            apply(
                &mut chats,
                Command::Create {
                    chat: "8".repeat(32)
                },
                4
            )
            .is_err()
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("basic-index.cache")).unwrap(),
            "damaged index"
        );
    }

    #[test]
    fn sends_are_exactly_bound_across_relaunch_and_stop_keeps_the_partial() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let key = secp256k1::SecretKey::from_byte_array([9; 32]).unwrap();
        let open = || Cache::open(dir.path(), &key).unwrap();
        let mut chats = BasicChats::new(
            Some(runtime.handle().clone()),
            Some(Arc::new(Waiting)),
            Some(open()),
        );
        let id = "1".repeat(32);
        apply(&mut chats, Command::Create { chat: id.clone() }, 1).unwrap();
        let send = Command::Send {
            chat: id.clone(),
            request: "2".repeat(32),
            text: "Hello".into(),
        };
        assert!(apply(&mut chats, send.clone(), 2).unwrap().busy);
        assert_eq!(apply(&mut chats, send.clone(), 3).unwrap().turns.len(), 1);
        for _ in 0..100 {
            if !chats.partial(&id).is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let stopped = apply(&mut chats, Command::Stop { chat: id.clone() }, 4).unwrap();
        assert!(!stopped.busy);
        assert_eq!(stopped.turns.len(), 2);
        assert_eq!(stopped.turns[1].text, "Some words");
        assert!(stopped.turns[1].stopped);
        drop(chats);
        let mut chats = BasicChats::new(None, None, Some(open()));
        assert_eq!(apply(&mut chats, send, 5).unwrap().turns.len(), 2);
        assert!(
            apply(
                &mut chats,
                Command::Send {
                    chat: id.clone(),
                    request: "2".repeat(32),
                    text: "Different".into()
                },
                6
            )
            .is_err()
        );
        apply(&mut chats, Command::Archive { chat: id }, 7).unwrap();
        assert!(chats.list()[0].archived);
        let chats = BasicChats::new(None, None, Some(open()));
        assert!(chats.list()[0].archived);
    }
}

#[cfg(test)]
mod suggestion_tests {
    use super::*;
    use crate::cache::Cache;
    #[test]
    fn used_suggestion_writes_retry_and_survive_restart() {
        let dir = tempfile::tempdir().unwrap();
        let key = secp256k1::SecretKey::from_byte_array([8; 32]).unwrap();
        let open = || Cache::open(dir.path(), &key).unwrap();
        let mut chats = BasicChats::new(None, None, Some(open()));
        let chat = "a".repeat(32);
        apply(&mut chats, Command::Create { chat: chat.clone() }, 1).unwrap();
        let path = dir.path().join("used-suggestions.cache");
        std::fs::create_dir(&path).unwrap();
        let snapshot = apply(
            &mut chats,
            Command::UseSuggestion {
                chat: chat.clone(),
                id: "answer@2".into(),
            },
            2,
        )
        .unwrap();
        assert!(snapshot.storage_error.is_some());
        assert!(chats.used(Some("answer@3"), &[]));
        std::fs::remove_dir(&path).unwrap();
        let snapshot = apply(&mut chats, Command::Read { chat, before: None }, 3).unwrap();
        assert!(snapshot.storage_error.is_none());
        drop(chats);
        let reopened = BasicChats::new(None, None, Some(open()));
        assert!(reopened.used(Some("answer@1"), &[]));
    }
}
