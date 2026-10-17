//! The phone's first screen (#11978, #11979): a chat answered by OpenAgents,
//! signed in or not, over [`openagents_account::cloud_chat::CloudChat`].
//!
//! Signed out, the chat runs as a visitor with a few messages before a
//! sign-in. When the account signs in, the visitor's chats move to it
//! (`claim`), and the open chat keeps going as the account's. When it signs
//! out, the open chat is closed so an account chat never shows signed out.
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
    /// Signed out: messages left before a sign-in is needed.
    pub left: Option<u32>,
    /// Signed out and out of messages: ask the person to sign in.
    pub sign_in_required: bool,
    /// Text that wasn't sent, to put back in the composer.
    pub unsent: Option<String>,
    pub error: Option<String>,
    /// The open chat is the account's (it shows in the account's chats).
    pub account_chat: bool,
}

impl From<Snapshot> for CloudChatSnapshot {
    fn from(s: Snapshot) -> Self {
        Self {
            chat: s.chat,
            lines: s.lines.into_iter().map(line).collect(),
            answering: s.answering,
            left: s.left,
            sign_in_required: s.sign_in_required,
            unsent: s.unsent,
            error: s.error,
            account_chat: s.account_chat,
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

/// What to do when the account's phase changes.
#[derive(Debug, PartialEq, Eq)]
enum Change {
    Claim,
    Close,
    Nothing,
}

fn change(was_signed_in: bool, phase: &Phase) -> Change {
    match (was_signed_in, matches!(phase, Phase::SignedIn { .. })) {
        (false, true) => Change::Claim,
        (true, false) if matches!(phase, Phase::SignedOut { .. }) => Change::Close,
        _ => Change::Nothing,
    }
}

impl CoreCloudChat {
    pub(super) fn new(
        controller: Arc<Controller>,
        dir: &std::path::Path,
        listener: Arc<dyn CloudChatListener>,
    ) -> Arc<Self> {
        let chat = CloudChat::new(controller.clone(), dir);
        let cancel = CancellationToken::new();
        let stop = cancel.clone();
        let owned = chat.clone();
        let mut updates = chat.watch();
        let mut account = controller.watch();
        openagents_client::runtime::shared().spawn(async move {
            let mut signed_in = matches!(account.borrow().phase, Phase::SignedIn { .. });
            if signed_in {
                owned.claim().await;
            } else {
                owned.resume().await;
            }
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
                        match change(signed_in, &phase) {
                            Change::Claim => {
                                let claiming = owned.clone();
                                openagents_client::runtime::shared().spawn(claiming.claim());
                            }
                            Change::Close => owned.new_chat(),
                            Change::Nothing => {}
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
    /// The chat answered by OpenAgents, signed in or not.
    pub fn cloud_chat(&self, listener: Arc<dyn CloudChatListener>) -> Arc<CoreCloudChat> {
        CoreCloudChat::new(
            self.controller().clone(),
            &self.data_dir().join("cloud-chat"),
            listener,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signing_in_claims_and_signing_out_closes() {
        let account = openagents_account::openagents::Account {
            id: "a".into(),
            label: "A".into(),
            email: None,
            avatar_url: None,
        };
        let signed_in = Phase::SignedIn { account };
        assert_eq!(change(false, &signed_in), Change::Claim);
        assert_eq!(change(true, &signed_in), Change::Nothing);
        assert_eq!(
            change(true, &Phase::SignedOut { notice: None }),
            Change::Close
        );
        assert_eq!(change(true, &Phase::Restoring), Change::Nothing);
        assert_eq!(
            change(false, &Phase::SignedOut { notice: None }),
            Change::Nothing
        );
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
            left: Some(2),
            sign_in_required: false,
            unsent: None,
            error: Some("e".into()),
            account_chat: false,
        };
        let view = CloudChatSnapshot::from(snapshot);
        assert_eq!(view.chat.as_deref(), Some("c"));
        assert_eq!(
            view.lines.iter().map(|l| l.role).collect::<Vec<_>>(),
            vec![CloudChatRole::User, CloudChatRole::Assistant]
        );
        assert_eq!((view.answering, view.left), (true, Some(2)));
        assert_eq!(view.error.as_deref(), Some("e"));
    }

    struct Memory;
    impl openagents_account::openagents::CredentialStore for Memory {
        fn load(
            &self,
            _: &openagents_account::openagents::Deployment,
        ) -> Result<
            Option<openagents_account::openagents::CredentialRecord>,
            openagents_account::openagents::StorageError,
        > {
            Ok(None)
        }
        fn save(
            &self,
            _: &openagents_account::openagents::CredentialRecord,
        ) -> Result<(), openagents_account::openagents::StorageError> {
            Ok(())
        }
        fn clear(
            &self,
            _: &openagents_account::openagents::Deployment,
        ) -> Result<(), openagents_account::openagents::StorageError> {
            Ok(())
        }
    }
    struct Seen(tokio::sync::mpsc::UnboundedSender<CloudChatSnapshot>);
    impl CloudChatListener for Seen {
        fn on_cloud_chat_changed(&self, snapshot: CloudChatSnapshot) {
            let _ = self.0.send(snapshot);
        }
    }

    /// Signed out: the first send mints a visitor, and the server's
    /// sign-in refusal shows as the sign-in prompt with the text kept.
    #[tokio::test]
    async fn a_signed_out_send_reaches_the_sign_in_prompt() {
        use openagents_account::openagents::{Client, Deployment};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                tokio::spawn(async move {
                    let mut request = Vec::new();
                    let mut buf = [0; 4096];
                    while !request.windows(4).any(|v| v == b"\r\n\r\n") {
                        let n = socket.read(&mut buf).await.unwrap();
                        if n == 0 {
                            return;
                        }
                        request.extend_from_slice(&buf[..n]);
                    }
                    let (status, body) = if request.starts_with(b"POST /v1/visitors ") {
                        ("200 OK", r#"{"token":"ovis_fixture","messages":0}"#)
                    } else if request.starts_with(b"POST /v1/threads ") {
                        assert!(
                            String::from_utf8_lossy(&request)
                                .to_ascii_lowercase()
                                .contains("authorization: bearer ovis_fixture")
                        );
                        (
                            "409 Conflict",
                            r#"{"error":{"code":"sign_in_required","message":"Sign in"}}"#,
                        )
                    } else {
                        ("404 Not Found", r#"{"error":{"code":"not_found"}}"#)
                    };
                    let response = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });
        let root = tempfile::tempdir().unwrap();
        let client =
            Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();
        let controller = Controller::new_named(
            client,
            Arc::new(Memory),
            root.path().join("signed-out"),
            "OpenAgents".into(),
        );
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let chat = CoreCloudChat::new(
            controller,
            &root.path().join("cloud-chat"),
            Arc::new(Seen(tx)),
        );
        let first = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(first.lines.is_empty() && !first.sign_in_required);
        chat.send("What can you do?".into());
        let last = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let s = rx.recv().await.unwrap();
                if s.sign_in_required {
                    return s;
                }
            }
        })
        .await
        .unwrap();
        server.abort();
        assert_eq!(last.unsent.as_deref(), Some("What can you do?"));
        assert_eq!(last.left, Some(0));
        assert!(!last.answering && last.lines.is_empty() && last.error.is_none());
    }
}
