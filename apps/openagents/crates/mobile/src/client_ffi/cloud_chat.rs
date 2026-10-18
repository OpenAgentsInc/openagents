//! The phone's first screen (#11978, #11979): a chat answered by OpenAgents,
//! over [`openagents_account::cloud_chat::CloudChat`]. It is always the
//! account's chat: the phone signs in silently with its own key (#11995).
//! When the account signs out, the open chat is closed so an account chat
//! never shows signed out.
//! The offline demo has one too ([`CoreCloudChat::demo`]), answered from
//! fixed replies with no server (#11984).
use super::CoreAccount;
use openagents_account::{
    Controller, Phase,
    cloud_chat::{CloudChat, Line, Role, Snapshot},
    cloud_chat_demo::DemoChat,
};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum CloudChatRole {
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct CloudChatLine {
    pub role: CloudChatRole,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct CloudChatSnapshot {
    /// The open chat's id, once its first message was sent.
    pub chat: Option<String>,
    pub lines: Vec<CloudChatLine>,
    /// An answer is being written.
    pub answering: bool,
    /// Text that wasn't sent, to put back in the composer.
    pub unsent: Option<String>,
    pub error: Option<String>,
    /// The open chat is the account's (it shows in the account's chats).
    pub account_chat: bool,
    /// The open chat is pinned in the account's chats.
    pub pinned: bool,
}

impl From<Snapshot> for CloudChatSnapshot {
    fn from(s: Snapshot) -> Self {
        Self {
            chat: s.chat,
            lines: s.lines.into_iter().map(line).collect(),
            answering: s.answering,
            unsent: s.unsent,
            error: s.error,
            account_chat: s.account_chat,
            pinned: s.pinned,
        }
    }
}

fn line(l: Line) -> CloudChatLine {
    CloudChatLine {
        role: match l.role {
            Role::User => CloudChatRole::User,
            Role::Assistant => CloudChatRole::Assistant,
            Role::Tool => CloudChatRole::Tool,
        },
        text: l.text,
    }
}

/// Called on a background thread with each new snapshot: hop to the main
/// thread before touching the interface.
/// Called once with how a pin or archive went: `None` when it worked, else
/// a line to show.
#[uniffi::export(with_foreign)]
pub trait CloudChatDone: Send + Sync {
    fn on_done(&self, error: Option<String>);
}

#[uniffi::export(with_foreign)]
pub trait CloudChatListener: Send + Sync {
    fn on_cloud_chat_changed(&self, snapshot: CloudChatSnapshot);
}

#[derive(uniffi::Object)]
pub struct CoreCloudChat {
    chat: Chat,
    cancel: CancellationToken,
}

/// The live chat, or the offline demo's.
enum Chat {
    Live(Arc<CloudChat>),
    Demo(Arc<DemoChat>),
}

/// Whether the account's phase change closes the open chat: only a
/// sign-out after being signed in does.
fn closes(was_signed_in: bool, phase: &Phase) -> bool {
    was_signed_in && matches!(phase, Phase::SignedOut { .. })
}

impl CoreCloudChat {
    pub(super) fn new(
        controller: Arc<Controller>,
        listener: Arc<dyn CloudChatListener>,
    ) -> Arc<Self> {
        let chat = CloudChat::new(controller.clone());
        let cancel = CancellationToken::new();
        let stop = cancel.clone();
        let owned = chat.clone();
        let mut updates = chat.watch();
        let mut account = controller.watch();
        openagents_client::runtime::shared().spawn(async move {
            let mut signed_in = matches!(account.borrow().phase, Phase::SignedIn { .. });
            listener.on_cloud_chat_changed(owned.snapshot().into());
            loop {
                tokio::select! {
                    biased;
                    _ = stop.cancelled() => break,
                    changed = updates.changed() => {
                        if changed.is_err() { break; }
                        let snapshot = updates.borrow_and_update().clone();
                        listener.on_cloud_chat_changed(snapshot.into());
                    }
                    changed = account.changed() => {
                        if changed.is_err() { break; }
                        let phase = account.borrow_and_update().phase.clone();
                        if closes(signed_in, &phase) {
                            owned.new_chat();
                        }
                        signed_in = matches!(phase, Phase::SignedIn { .. });
                    }
                }
            }
        });
        Arc::new(Self {
            chat: Chat::Live(chat),
            cancel,
        })
    }

    pub(crate) fn send_completion(
        &self,
        text: String,
        lease: Option<openagents_account::Lease>,
    ) -> super::CoreResult<std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>> {
        match &self.chat {
            Chat::Live(chat) => Ok(Box::pin(
                chat.send_for_lease(text, lease.ok_or(super::CoreError::Closed)?),
            )),
            Chat::Demo(chat) => Ok(Box::pin(chat.send(text))),
        }
    }

    pub(crate) fn watch(&self) -> tokio::sync::watch::Receiver<Snapshot> {
        match &self.chat {
            Chat::Live(chat) => chat.watch(),
            Chat::Demo(chat) => chat.watch(),
        }
    }

    pub(crate) fn raw_snapshot(&self) -> Snapshot {
        match &self.chat {
            Chat::Live(chat) => chat.snapshot(),
            Chat::Demo(chat) => chat.snapshot(),
        }
    }
}

impl Drop for CoreCloudChat {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

#[uniffi::export]
impl CoreCloudChat {
    /// The offline demo's chat: fixed replies, streamed, with no server.
    #[uniffi::constructor]
    pub fn demo(listener: Arc<dyn CloudChatListener>) -> Arc<Self> {
        let chat = DemoChat::new(std::time::Duration::from_millis(60));
        let cancel = CancellationToken::new();
        let stop = cancel.clone();
        let mut updates = chat.watch();
        openagents_client::runtime::shared().spawn(async move {
            loop {
                tokio::select! {
                    biased;
                    _ = stop.cancelled() => break,
                    changed = updates.changed() => {
                        if changed.is_err() { break; }
                        let snapshot = updates.borrow_and_update().clone();
                        listener.on_cloud_chat_changed(snapshot.into());
                    }
                }
            }
        });
        Arc::new(Self {
            chat: Chat::Demo(chat),
            cancel,
        })
    }
    pub fn snapshot(&self) -> CloudChatSnapshot {
        self.raw_snapshot().into()
    }
    /// Send `text` in the open chat, or start one. Progress arrives through
    /// the listener.
    pub fn send(&self, text: String) {
        let runtime = openagents_client::runtime::shared();
        match &self.chat {
            Chat::Live(chat) => drop(runtime.spawn(chat.send(text))),
            Chat::Demo(chat) => drop(runtime.spawn(chat.send(text))),
        }
    }
    /// Stop the answer being written.
    pub fn stop(&self) {
        match &self.chat {
            Chat::Live(chat) => drop(openagents_client::runtime::shared().spawn(chat.stop())),
            Chat::Demo(chat) => chat.stop(),
        }
    }
    /// Pin (`true`) or unpin the open chat. `done` hears how it went.
    pub fn set_pinned(&self, pinned: bool, done: Arc<dyn CloudChatDone>) {
        match &self.chat {
            Chat::Live(chat) => {
                let call = chat.set_pinned(pinned);
                drop(openagents_client::runtime::shared().spawn(async move {
                    done.on_done(call.await.err());
                }));
            }
            Chat::Demo(chat) => {
                chat.set_pinned(pinned);
                done.on_done(None);
            }
        }
    }
    /// Archive the open chat, then start a new one. `done` hears how it
    /// went; on an error the chat stays open.
    pub fn archive(&self, done: Arc<dyn CloudChatDone>) {
        match &self.chat {
            Chat::Live(chat) => {
                let call = chat.archive();
                drop(openagents_client::runtime::shared().spawn(async move {
                    done.on_done(call.await.err());
                }));
            }
            Chat::Demo(chat) => {
                chat.archive();
                done.on_done(None);
            }
        }
    }
    /// The chat's lines as plain text, for Copy Transcript.
    pub fn transcript(&self) -> String {
        self.raw_snapshot().transcript()
    }
    /// Start over with an empty chat.
    pub fn new_chat(&self) {
        match &self.chat {
            Chat::Live(chat) => chat.new_chat(),
            Chat::Demo(chat) => chat.new_chat(),
        }
    }
}

#[uniffi::export]
impl CoreAccount {
    /// The chat answered by OpenAgents, as this phone's account.
    pub fn cloud_chat(&self, listener: Arc<dyn CloudChatListener>) -> Arc<CoreCloudChat> {
        CoreCloudChat::new(self.controller().clone(), listener)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_signing_out_closes_the_chat() {
        let account = openagents_account::openagents::Account {
            id: "a".into(),
            label: "A".into(),
            email: None,
            avatar_url: None,
        };
        let signed_in = Phase::SignedIn { account };
        assert!(!closes(false, &signed_in));
        assert!(!closes(true, &signed_in));
        assert!(closes(true, &Phase::SignedOut { notice: None }));
        assert!(!closes(true, &Phase::Restoring));
        assert!(!closes(false, &Phase::SignedOut { notice: None }));
    }

    #[test]
    fn snapshots_carry_every_field() {
        let snapshot = Snapshot {
            chat: Some("c".into()),
            lines: vec![
                Line {
                    role: Role::User,
                    text: "hi".into(),
                },
                Line {
                    role: Role::Assistant,
                    text: "Hello".into(),
                },
            ],
            answering: true,
            unsent: None,
            error: Some("e".into()),
            account_chat: false,
            pinned: true,
        };
        let view = CloudChatSnapshot::from(snapshot);
        assert_eq!(view.chat.as_deref(), Some("c"));
        assert_eq!(
            view.lines.iter().map(|l| l.role).collect::<Vec<_>>(),
            vec![CloudChatRole::User, CloudChatRole::Assistant]
        );
        assert!(view.answering);
        assert_eq!(view.error.as_deref(), Some("e"));
        assert!(view.pinned);
    }

    struct Heard(std::sync::Mutex<Vec<Option<String>>>);
    impl CloudChatDone for Heard {
        fn on_done(&self, error: Option<String>) {
            self.0.lock().unwrap().push(error);
        }
    }
    struct Quiet;
    impl CloudChatListener for Quiet {
        fn on_cloud_chat_changed(&self, _: CloudChatSnapshot) {}
    }

    #[test]
    fn the_demo_pins_copies_and_archives_without_a_server() {
        let chat = CoreCloudChat::demo(Arc::new(Quiet));
        let Chat::Demo(demo) = &chat.chat else {
            unreachable!()
        };
        drop(openagents_client::runtime::shared().block_on(demo.send("Hello".into())));
        let heard = Arc::new(Heard(Default::default()));
        chat.set_pinned(true, heard.clone());
        assert!(chat.snapshot().pinned);
        let transcript = chat.transcript();
        assert!(
            transcript.starts_with("You:\nHello\n\n---\n\nOpenAgents:\n"),
            "{transcript}"
        );
        chat.archive(heard.clone());
        let after = chat.snapshot();
        assert!(after.lines.is_empty() && !after.pinned);
        assert_eq!(chat.transcript(), "");
        assert_eq!(*heard.0.lock().unwrap(), [None, None]);
    }
}
