//! Local chat operations. A host admits these only on its same-user socket.

use crate::basic_chats::{BasicChats, Summary, Tail};
use crate::basic_coder::Turn;
use serde::{Deserialize, Serialize};

/// A closed local operation; the caller supplies stable IDs for new chats and sends.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    List {},
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
    pub chats: Vec<Summary>,
    pub chat: Option<String>,
    /// Index of the oldest included turn; read before this for earlier turns.
    pub start: usize,
    pub total: usize,
    pub turns: Vec<Turn>,
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
    chats.settle(now);
    let (id, before) = match command {
        Command::List {} => (None, None),
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
    let mut snapshot = Snapshot {
        chats: chats.list().to_vec(),
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
