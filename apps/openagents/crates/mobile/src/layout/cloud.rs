//! Cloud and offline-demo messages enter the same Rust layout worker as live rooms.
use super::*;
use openagents_account::cloud_chat::{Role, Snapshot};
use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub(super) struct SourceGate {
    generation: u64,
    closed: bool,
}
pub(super) fn send(
    gate: &Mutex<SourceGate>,
    tx: &Sender<Msg>,
    generation: u64,
    input: TranscriptInput,
) -> bool {
    // The source check and enqueue share the replacement lock. An old update
    // cannot enter the queue after a replacement's clear/input.
    let gate = gate.lock().unwrap();
    !gate.closed && gate.generation == generation && tx.send(Msg::Input(input)).is_ok()
}
fn input(snapshot: Snapshot) -> TranscriptInput {
    let last = snapshot.lines.len().checked_sub(1);
    let prefix = snapshot.chat.as_deref().unwrap_or("new");
    let entries = snapshot
        .lines
        .into_iter()
        .enumerate()
        .map(|(at, line)| {
            let streaming =
                snapshot.answering && last == Some(at) && matches!(&line.role, Role::Assistant);
            Arc::new(SessionMessageEntry {
                id: format!("cloud:{prefix}:{at}"),
                role: match line.role {
                    Role::User => MessageRole::User,
                    Role::Assistant => MessageRole::Assistant,
                    Role::Tool => MessageRole::System,
                },
                parts: vec![MessagePart::Text {
                    id: "text".into(),
                    text: line.text,
                }],
                created_at: 0,
                device_id: String::new(),
                status: Some(if streaming {
                    MessageStatus::Streaming
                } else {
                    MessageStatus::Complete
                }),
                continuation_of: None,
                duration_ms: None,
            })
        })
        .collect();
    TranscriptInput {
        entries,
        working: snapshot.answering,
        streaming: snapshot.answering,
        ..Default::default()
    }
}
impl TranscriptView {
    pub(super) fn replace_source(&self) -> Option<u64> {
        self.watch.lock().unwrap().take();
        if let Some(stop) = self.cloud_watch.lock().unwrap().take() {
            stop.cancel();
        }
        let mut source = self.source.lock().unwrap();
        if source.closed {
            return None;
        }
        source.generation = source.generation.checked_add(1)?;
        let _ = self.tx.lock().unwrap().send(Msg::Input(Default::default()));
        Some(source.generation)
    }
    pub(super) fn send_source(&self, generation: u64, input: TranscriptInput) -> bool {
        let tx = self.tx.lock().unwrap().clone();
        send(&self.source, &tx, generation, input)
    }
    pub(super) fn close_source(&self) {
        self.watch.lock().unwrap().take();
        if let Some(stop) = self.cloud_watch.lock().unwrap().take() {
            stop.cancel();
        }
        self.source.lock().unwrap().closed = true;
    }
}
#[uniffi::export]
impl TranscriptView {
    /// Follow cloud/default-chat or synthetic-demo messages in Rust, without native row copies.
    pub fn attach_cloud(&self, chat: Arc<crate::client_ffi::CoreCloudChat>) -> bool {
        let _binding = self.source_binding.lock().unwrap();
        let Some(generation) = self.replace_source() else {
            return false;
        };
        let mut changes = chat.watch();
        if !self.send_source(generation, input(changes.borrow_and_update().clone())) {
            return false;
        }
        let stop = CancellationToken::new();
        *self.cloud_watch.lock().unwrap() = Some(stop.clone());
        let source = self.source.clone();
        let tx = self.tx.lock().unwrap().clone();
        openagents_client::runtime::shared().spawn(async move {
            loop {
                tokio::select!{biased;_=stop.cancelled()=>break,changed=changes.changed()=>if changed.is_err(){break}}
                let next=changes.borrow_and_update().clone();
                if !send(&source,&tx,generation,input(next)){break}
            }
        });
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use openagents_account::cloud_chat::Line;
    #[test]
    fn cloud_projection_keeps_rust_order_keys_and_streaming_state() {
        let snapshot = Snapshot {
            chat: Some("synthetic".into()),
            lines: vec![
                Line {
                    role: Role::User,
                    text: "Hello".into(),
                },
                Line {
                    role: Role::Assistant,
                    text: "Hi".into(),
                },
            ],
            answering: true,
            ..Default::default()
        };
        let projected = input(snapshot.clone());
        assert_eq!(projected.entries.len(), 2);
        assert_eq!(projected.entries[0].role, MessageRole::User);
        assert_eq!(projected.entries[1].status, Some(MessageStatus::Streaming));
        let mut next = snapshot;
        next.lines[1].text.push('!');
        assert_eq!(projected.entries[1].id, input(next).entries[1].id);
        assert!(projected.working);
        assert!(input(Snapshot::default()).entries.is_empty());
    }
    #[test]
    fn replacement_and_shutdown_reject_late_source_updates() {
        let (tx, rx) = mpsc::channel();
        let gate = Mutex::new(SourceGate {
            generation: 1,
            closed: false,
        });
        assert!(send(&gate, &tx, 1, Default::default()));
        gate.lock().unwrap().generation = 2;
        assert!(!send(&gate, &tx, 1, Default::default()));
        assert!(send(&gate, &tx, 2, Default::default()));
        gate.lock().unwrap().closed = true;
        assert!(!send(&gate, &tx, 2, Default::default()));
        assert_eq!(rx.try_iter().count(), 2);
    }
}
