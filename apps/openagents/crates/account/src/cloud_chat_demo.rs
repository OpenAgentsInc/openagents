//! The offline demo's chat answered by OpenAgents (#11984): the same
//! [`Snapshot`] as [`crate::cloud_chat::CloudChat`], answered from a fixed
//! list of replies streamed a few words at a time, with no server. It is
//! what the demo and the marketing screenshots show; nothing in it is sent
//! anywhere.
use crate::cloud_chat::{Line, Role, Snapshot};
use std::{
    future::Future,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::watch;

/// The demo's replies, in order; the last repeats.
pub const REPLIES: &[&str] = &[
    "We can answer questions, explain and write code, and start coding tasks. \
     Connect your own computer and Coder works in your projects while you follow \
     along here.",
    "Yes. Tell us the repository and what's wrong, and a coding task takes it from \
     there: it reads the code, makes the change, runs the tests, and shows you the \
     diff before anything is merged.",
    "Anytime. Ask another question or start a coding task when you're ready.",
];

pub struct DemoChat {
    state: Mutex<(u64, Snapshot)>,
    updates: watch::Sender<Snapshot>,
    /// Time between streamed pieces of a reply.
    pace: Duration,
}

impl DemoChat {
    pub fn new(pace: Duration) -> Arc<Self> {
        let (updates, _) = watch::channel(Snapshot::default());
        Arc::new(Self {
            state: Mutex::new((0, Snapshot::default())),
            updates,
            pace,
        })
    }

    pub fn watch(&self) -> watch::Receiver<Snapshot> {
        self.updates.subscribe()
    }

    pub fn snapshot(&self) -> Snapshot {
        self.state.lock().unwrap().1.clone()
    }

    fn change(&self, generation: u64, edit: impl FnOnce(&mut Snapshot)) -> bool {
        let mut s = self.state.lock().unwrap();
        if s.0 != generation {
            return false;
        }
        edit(&mut s.1);
        self.updates.send_replace(s.1.clone());
        true
    }

    pub fn new_chat(&self) {
        let mut s = self.state.lock().unwrap();
        s.0 += 1;
        s.1 = Snapshot::default();
        self.updates.send_replace(s.1.clone());
    }

    /// Pin or unpin, on this phone only: the demo has no account.
    pub fn set_pinned(&self, pinned: bool) {
        let mut s = self.state.lock().unwrap();
        s.1.pinned = pinned;
        self.updates.send_replace(s.1.clone());
    }

    /// Archive: the demo keeps nothing, so it only starts over.
    pub fn archive(&self) {
        self.new_chat();
    }

    /// Stop the reply being written; what's written so far stays.
    pub fn stop(&self) {
        let mut s = self.state.lock().unwrap();
        s.0 += 1;
        s.1.answering = false;
        self.updates.send_replace(s.1.clone());
    }

    /// Answer `text` with the next reply, streamed.
    pub fn send(self: &Arc<Self>, text: String) -> impl Future<Output = ()> + Send + 'static {
        let this = self.clone();
        async move {
            let text = text.trim().to_owned();
            let (generation, reply) = {
                let mut s = this.state.lock().unwrap();
                if text.is_empty() || s.1.answering {
                    return;
                }
                s.0 += 1;
                let asked = s.1.lines.iter().filter(|l| l.role == Role::User).count();
                let reply = REPLIES[asked.min(REPLIES.len() - 1)];
                s.1.chat.get_or_insert_with(|| "demo-cloud-chat".into());
                s.1.answering = true;
                s.1.error = None;
                s.1.lines.push(Line {
                    role: Role::User,
                    text,
                });
                s.1.lines.push(Line {
                    role: Role::Assistant,
                    text: String::new(),
                });
                this.updates.send_replace(s.1.clone());
                (s.0, reply)
            };
            let words: Vec<&str> = reply.split_inclusive(' ').collect();
            for piece in words.chunks(4) {
                tokio::time::sleep(this.pace).await;
                let piece = piece.concat();
                if !this.change(generation, |s| {
                    if let Some(last) = s.lines.last_mut() {
                        last.text.push_str(&piece);
                    }
                }) {
                    return;
                }
            }
            this.change(generation, |s| s.answering = false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn answers_in_order_and_streams_the_whole_reply() {
        let chat = DemoChat::new(Duration::ZERO);
        chat.send("What can you do?".into()).await;
        let s = chat.snapshot();
        assert_eq!(s.chat.as_deref(), Some("demo-cloud-chat"));
        assert!(!s.answering);
        assert_eq!(s.lines[1].text, REPLIES[0]);
        chat.send("Can you fix a bug?".into()).await;
        chat.send("Thanks!".into()).await;
        chat.send("And again".into()).await;
        let s = chat.snapshot();
        assert_eq!(s.lines.len(), 8);
        assert_eq!(s.lines[3].text, REPLIES[1]);
        assert_eq!(s.lines[7].text, REPLIES[2]);
        assert!(crate::own_computer::answered(&s));
        chat.new_chat();
        assert_eq!(chat.snapshot(), Snapshot::default());
    }

    #[tokio::test]
    async fn stopping_keeps_what_was_written() {
        let chat = DemoChat::new(Duration::from_millis(20));
        let sending = tokio::spawn(chat.send("What can you do?".into()));
        tokio::time::sleep(Duration::from_millis(30)).await;
        chat.stop();
        sending.await.unwrap();
        let s = chat.snapshot();
        assert!(!s.answering);
        assert!(s.lines[1].text.len() < REPLIES[0].len());
    }

    #[test]
    fn the_replies_read_plainly() {
        for reply in REPLIES {
            assert!(oa_copy::violations(reply, &[]).is_empty(), "{reply}");
        }
    }
}
