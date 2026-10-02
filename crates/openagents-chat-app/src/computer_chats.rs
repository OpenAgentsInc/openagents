//! Another computer's threads as a chat client backend (#10151).
//!
//! [`Remote`] carries the shared chat service's commands to a paired
//! computer's host over NIP-HOST, through the same [`Link`] the phone's
//! host threads use: the list with `thread.list`, a page with
//! `thread.read`, a message with `thread.send`, a stop with `thread.stop`,
//! and Run Coder with `thread.run`, all under this device's grant. The
//! client (`openagents_chat::client::Client::over_computer`) then streams a
//! reply by reading the thread again while the computer answers, as it does
//! over this computer's own host. OpenAgents Terminal on a laptop follows
//! the desktop's threads this way.
//!
//! What NIP-HOST does not carry is refused in words: a new thread starts on
//! that computer, and archiving, renaming, and pinning happen there.

use std::sync::Arc;

use coder_host::access::thread::{ThreadPage, ThreadRow};
use openagents_chat::basic_chats::{Spawned, Summary};
use openagents_chat::client::{BoxFuture, Host, Migration};
use openagents_chat::router::Caller;
use openagents_chat::service::{Command, Snapshot};

use crate::host_threads::{Link, Refusal};

/// One paired computer's threads over NIP-HOST.
pub struct Remote {
    link: Arc<dyn Link>,
    host: String,
}

impl Remote {
    /// The threads of `host` (its key) over `link`.
    pub fn new(link: Arc<dyn Link>, host: String) -> Self {
        Self { link, host }
    }
}

/// The words for a refusal.
fn words(refusal: Refusal) -> String {
    match refusal {
        Refusal::NotServed => {
            "That computer does not share this with this device, or does not have it.".into()
        }
        Refusal::Refused(why) => why,
        Refusal::Failed => "That computer cannot be reached.".into(),
    }
}

fn summary(row: &ThreadRow) -> Summary {
    Summary {
        id: row.thread.clone(),
        title: row.title.clone(),
        started: row.started,
        updated: row.updated,
        coder: row.coder.as_ref().map(|coder| Spawned {
            host: coder.host.clone(),
            task: coder.task.clone(),
            project: coder.project.clone(),
            at: coder.at,
        }),
        archived: false,
        pinned: row.pinned,
        named: false,
    }
}

/// A page as the service's snapshot, its row included so the client's
/// reader finds the thread's title and Coder link without a list.
fn snapshot(page: &ThreadPage) -> Snapshot {
    let turns: Vec<_> = page.turns.iter().map(crate::host_threads::turn).collect();
    let times: Vec<u64> = turns.iter().filter_map(|turn| turn.at).collect();
    let coder = page.coder.as_ref().map(|coder| Spawned {
        host: coder.host.clone(),
        task: coder.task.clone(),
        project: coder.project.clone(),
        at: coder.at,
    });
    let row = Summary {
        id: page.thread.clone(),
        title: page.title.clone(),
        started: times.iter().copied().min().unwrap_or(0),
        updated: times.iter().copied().max().unwrap_or(0),
        coder: coder.clone(),
        archived: false,
        pinned: false,
        named: false,
    };
    Snapshot {
        chats: vec![row],
        list_total: 1,
        chat: Some(page.thread.clone()),
        start: usize::try_from(page.start).unwrap_or(usize::MAX),
        total: usize::try_from(page.total).unwrap_or(usize::MAX),
        turns,
        busy: page.busy,
        partial: page.partial.clone(),
        failure: page.failure.clone(),
        coder,
        ..Snapshot::default()
    }
}

impl Remote {
    fn read(&self, chat: &str, before: Option<usize>) -> Result<Snapshot, String> {
        let before = before.map(|before| u64::try_from(before).unwrap_or(u64::MAX));
        self.link
            .read(&self.host, chat, before)
            .map(|page| snapshot(&page))
            .map_err(words)
    }

    /// One command, blocking on the link.
    fn apply(&self, command: Command) -> Result<Snapshot, String> {
        match command {
            Command::List {} => {
                let rows = self.link.list(&self.host).map_err(words)?;
                Ok(Snapshot {
                    list_total: rows.len(),
                    chats: rows.iter().map(summary).collect(),
                    ..Snapshot::default()
                })
            }
            // `thread.list` answers whole, so there is no later page.
            Command::ListMore { .. } => Ok(Snapshot::default()),
            Command::Read { chat, before } => self.read(&chat, before),
            Command::Send {
                chat,
                request,
                text,
            } => {
                self.link
                    .send(&self.host, &chat, &request, &text)
                    .map_err(words)?;
                self.read(&chat, None)
            }
            Command::Stop { chat } => {
                let page = self.read(&chat, None)?;
                let request = page.turns.last().and_then(|turn| turn.request.clone());
                self.link
                    .stop(&self.host, &chat, request.as_deref())
                    .map_err(words)?;
                self.read(&chat, None)
            }
            Command::RunCoder { chat } => {
                let task = self.link.run(&self.host, &chat).map_err(words)?;
                let mut page = self.read(&chat, None)?;
                page.coder.get_or_insert(Spawned {
                    host: self.host.clone(),
                    task,
                    project: None,
                    at: None,
                });
                Ok(page)
            }
            Command::Create { .. } => Err(
                "New threads start on that computer. Open one of its threads with Ctrl+T.".into(),
            ),
            _ => Err("That is done on the computer itself.".into()),
        }
    }
}

impl Host for Remote {
    fn apply(&mut self, command: Command, _: Caller) -> BoxFuture<'_, Result<Snapshot, String>> {
        let remote = Self {
            link: self.link.clone(),
            host: self.host.clone(),
        };
        Box::pin(async move {
            tokio::task::spawn_blocking(move || remote.apply(command))
                .await
                .unwrap_or_else(|_| Err("That computer cannot be reached.".into()))
        })
    }

    fn migrate(&mut self, _: &std::path::Path) -> BoxFuture<'_, Migration> {
        Box::pin(async { Migration::Quiet })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_host::access::thread::{ThreadExtras, ThreadRole, ThreadTurn};
    use std::sync::Mutex;

    /// A computer with one thread, which answers each message at once.
    #[derive(Default)]
    struct Computer {
        turns: Mutex<Vec<ThreadTurn>>,
        stopped: Mutex<Vec<Option<String>>>,
    }

    const THREAD: &str = "0123456789abcdef0123456789abcdef";

    impl Link for Computer {
        fn list(&self, _: &str) -> Result<Vec<ThreadRow>, Refusal> {
            Ok(vec![ThreadRow {
                thread: THREAD.into(),
                title: "Deploy notes".into(),
                started: 10,
                updated: 20,
                pinned: true,
                coder: None,
            }])
        }
        fn read(&self, _: &str, thread: &str, _: Option<u64>) -> Result<ThreadPage, Refusal> {
            if thread != THREAD {
                return Err(Refusal::NotServed);
            }
            let turns = self.turns.lock().unwrap().clone();
            Ok(ThreadPage {
                thread: THREAD.into(),
                title: "Deploy notes".into(),
                start: 0,
                total: turns.len() as u64,
                turns,
                busy: false,
                partial: String::new(),
                failure: None,
                coder: None,
                outside: None,
            })
        }
        fn send(&self, _: &str, thread: &str, request: &str, text: &str) -> Result<(), Refusal> {
            if thread != THREAD {
                return Err(Refusal::NotServed);
            }
            let turn = |role, text: &str, request: Option<String>| ThreadTurn {
                role,
                text: text.into(),
                at: Some(30),
                stopped: false,
                model: None,
                request,
                extras: ThreadExtras::default(),
            };
            let mut turns = self.turns.lock().unwrap();
            turns.push(turn(ThreadRole::User, text, Some(request.into())));
            turns.push(turn(ThreadRole::Assistant, &format!("Re: {text}"), None));
            Ok(())
        }
        fn stop(&self, _: &str, _: &str, request: Option<&str>) -> Result<(), Refusal> {
            self.stopped
                .lock()
                .unwrap()
                .push(request.map(str::to_owned));
            Ok(())
        }
        fn run(&self, _: &str, _: &str) -> Result<String, Refusal> {
            Err(Refusal::Refused(
                "This reply has no current Coder offer.".into(),
            ))
        }
    }

    #[test]
    fn commands_become_nip_host_operations() {
        let computer = Arc::new(Computer::default());
        let remote = Remote::new(computer.clone(), "ab".repeat(32));
        let listed = remote.apply(Command::List {}).unwrap();
        assert_eq!(listed.list_total, 1);
        assert_eq!(listed.chats[0].title, "Deploy notes");
        assert!(listed.chats[0].pinned);
        let sent = remote
            .apply(Command::Send {
                chat: THREAD.into(),
                request: "f".repeat(32),
                text: "status?".into(),
            })
            .unwrap();
        assert_eq!(sent.chat.as_deref(), Some(THREAD));
        let texts: Vec<&str> = sent.turns.iter().map(|turn| turn.text.as_str()).collect();
        assert_eq!(texts, ["status?", "Re: status?"]);
        assert_eq!(sent.chats[0].updated, 30);
        remote
            .apply(Command::Stop {
                chat: THREAD.into(),
            })
            .unwrap();
        assert_eq!(*computer.stopped.lock().unwrap(), [None]);
        // What NIP-HOST does not carry is said, not attempted.
        let new = remote
            .apply(Command::Create {
                chat: "1".repeat(32),
            })
            .unwrap_err();
        assert!(new.contains("start on that computer"), "{new}");
        assert!(
            remote
                .apply(Command::Archive {
                    chat: THREAD.into()
                })
                .is_err()
        );
        assert_eq!(
            remote
                .apply(Command::RunCoder {
                    chat: THREAD.into()
                })
                .unwrap_err(),
            "This reply has no current Coder offer."
        );
        assert!(
            remote
                .apply(Command::Read {
                    chat: "2".repeat(32),
                    before: None
                })
                .is_err()
        );
    }

    /// The chat client over another computer streams a reply to a message
    /// in one of its threads, and says where a new thread starts.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_client_continues_another_computers_thread() {
        use openagents_chat::client::{self, Client, Event, Kind, Op, Options, Start};
        let computer = Arc::new(Computer::default());
        let client = Client::over_computer(
            Box::new(Remote::new(computer, "ab".repeat(32))),
            "Desk".into(),
            Options::new(Caller::TERMINAL),
            Arc::new(client::NoCoder),
        );
        assert_eq!(client.kind(), Kind::Computer);
        assert_eq!(client.place(), "Desk");
        let send = |new: bool, thread: &str| Op::Send {
            thread: thread.into(),
            new,
            text: "status?".into(),
            start: Start::Settings,
            timeout: std::time::Duration::from_secs(5),
        };
        let client::Stream { mut events, done } = client.stream(send(false, THREAD));
        let mut seen = Vec::new();
        while let Some(event) = events.recv().await {
            seen.push(event);
        }
        let (client, _) = done.await.unwrap();
        assert!(
            seen.iter().any(
                |event| matches!(event, Event::Reply { reply, .. } if reply.text == "Re: status?")
            ),
            "{seen:?}"
        );
        let (_, ended) = done_of(client.stream(send(true, &"1".repeat(32)))).await;
        assert!(ended.is_err());
    }

    async fn done_of(
        stream: openagents_chat::client::Stream,
    ) -> (
        Vec<openagents_chat::client::Event>,
        Result<openagents_chat::client::Ended, openagents_chat::client::Error>,
    ) {
        let openagents_chat::client::Stream { mut events, done } = stream;
        let mut seen = Vec::new();
        while let Some(event) = events.recv().await {
            seen.push(event);
        }
        (seen, done.await.unwrap().1)
    }
}
