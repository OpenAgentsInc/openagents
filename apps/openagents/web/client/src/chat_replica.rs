//! One chat's chat2 room as the browser keeps it (#12224): the protocol half
//! of a read-mostly member. It speaks the room's binary frames
//! ([`openagents_doc::chat_frames`], the codec `openagents-sync` uses on the
//! desktop and phone), imports the room's rows into an
//! [`openagents_doc::SessionDoc`], and reads the transcript from it the way
//! the desktop does. [`crate::chat_room`] carries it over a WebSocket.
//!
//! The browser writes nothing to the room: a reply goes to the chat's
//! computer through the device room, as the composer already sends it
//! (#12267), and comes back here as rows.
//!
//! Catch-up: a fresh member with a checkpoint in the room reads the
//! checkpoint first (the site's `/v1/sync/chat2/{id}/checkpoint`, since a
//! room token opens only the socket), then asks for the rows after it.

use openagents_doc::chat_frames::{self as wire, frame_type};
use openagents_doc::{SessionDoc, SessionMessageEntry};
use serde_json::json;

/// What a frame asks of the connection.
#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    /// Nothing to do.
    Nothing,
    /// The transcript changed.
    Changed,
    /// Send these bytes.
    Send(Vec<u8>),
    /// Read the room's checkpoint, then call [`ChatReplica::checkpoint`].
    Checkpoint,
}

pub struct ChatReplica {
    doc: SessionDoc,
    device: String,
    /// The last room row this member holds.
    cursor: u64,
    /// The checkpoint the hello named, while it is being read.
    checkpoint_seq: u64,
    /// The rows up to the room's head have arrived at least once.
    caught_up: bool,
}

impl ChatReplica {
    pub fn new(device: impl Into<String>) -> Self {
        Self {
            doc: SessionDoc::from_doc(loro::LoroDoc::new()),
            device: device.into(),
            cursor: 0,
            checkpoint_seq: 0,
            caught_up: false,
        }
    }

    /// The first frame on a new connection.
    pub fn hello(&self) -> Vec<u8> {
        wire::encode(
            frame_type::HELLO,
            &json!({"cursor": self.cursor, "device": self.device}),
            &[],
        )
    }

    fn rows_after(&self, after: u64) -> Vec<u8> {
        wire::encode(
            frame_type::ROWS_REQ,
            &json!({"after": after, "excludeOwn": false}),
            &[],
        )
    }

    /// A probe for a quiet room.
    pub fn probe(&self) -> Vec<u8> {
        wire::encode(frame_type::PROBE, &json!({}), &[])
    }

    /// Applies one server frame. Anything unreadable is left out.
    pub fn apply(&mut self, bytes: &[u8]) -> Step {
        let Some(frame) = wire::decode(bytes) else {
            return Step::Nothing;
        };
        match frame.kind {
            frame_type::STATE => {
                let Ok(state) = serde_json::from_value::<wire::StateHeader>(frame.header) else {
                    return Step::Nothing;
                };
                // Rows below the floor are gone: read the checkpoint first.
                let compacted = self.cursor < state.seq_floor;
                let fresh = self.cursor == 0 && state.checkpoint_seq > 0;
                if (compacted || fresh) && state.checkpoint_size > 0 {
                    self.checkpoint_seq = state.checkpoint_seq;
                    Step::Checkpoint
                } else {
                    Step::Send(self.rows_after(self.cursor))
                }
            }
            frame_type::ROW => {
                let Ok(row) = serde_json::from_value::<wire::RowHeader>(frame.header) else {
                    return Step::Nothing;
                };
                if self.doc.doc().import(&frame.payload).is_err() {
                    return Step::Nothing;
                }
                self.cursor = self.cursor.max(row.seq);
                Step::Changed
            }
            frame_type::ROWS_DONE => {
                if let Ok(done) = serde_json::from_value::<wire::RowsDoneHeader>(frame.header) {
                    self.cursor = self.cursor.max(done.head_seq);
                }
                self.caught_up = true;
                Step::Changed
            }
            _ => Step::Nothing,
        }
    }

    /// The checkpoint's bytes (none when the room has none after all); the
    /// rows after it are asked for next.
    pub fn checkpoint(&mut self, bytes: Option<&[u8]>) -> Step {
        let after = match bytes {
            Some(bytes) if self.doc.doc().import(bytes).is_ok() => {
                self.cursor = self.cursor.max(self.checkpoint_seq);
                self.cursor
            }
            _ => self.cursor,
        };
        Step::Send(self.rows_after(after))
    }

    /// Whether the room's rows have arrived at least once.
    pub fn caught_up(&self) -> bool {
        self.caught_up
    }

    /// Host-reported context occupancy, alongside the transcript (#12270).
    pub fn context_usage(&self) -> Option<openagents_proto::ContextUsage> {
        self.doc.context_usage()
    }

    /// The chat's transcript, as the desktop reads it.
    pub fn entries(&self) -> Vec<SessionMessageEntry> {
        self.doc.read_entries().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_doc::{MessagePart, MessageRole, MessageStatus};

    fn entry(id: &str, text: &str) -> SessionMessageEntry {
        SessionMessageEntry {
            id: id.into(),
            role: MessageRole::User,
            parts: vec![MessagePart::Text {
                id: "t".into(),
                text: text.into(),
            }],
            created_at: 1,
            device_id: "host".into(),
            status: Some(MessageStatus::Complete),
            continuation_of: None,
            duration_ms: None,
        }
    }

    fn row(seq: u64, payload: &[u8]) -> Vec<u8> {
        wire::encode(
            frame_type::ROW,
            &json!({"seq": seq, "device": "host", "batchId": format!("b{seq}")}),
            payload,
        )
    }

    fn state(head: u64, floor: u64, checkpoint: u64, size: u64) -> Vec<u8> {
        wire::encode(
            frame_type::STATE,
            &json!({"headSeq": head, "seqFloor": floor, "checkpointSeq": checkpoint, "checkpointSize": size}),
            &[],
        )
    }

    #[test]
    fn rows_after_the_hello_build_the_transcript() {
        let host = SessionDoc::init("c1").unwrap();
        host.push_message(&entry("m1", "hello")).unwrap();
        host.update_context_usage(Some(50_000), Some(200_000))
            .unwrap();
        let first = host.doc().export(loro::ExportMode::all_updates()).unwrap();
        let mut replica = ChatReplica::new("web-1");
        let hello = wire::decode(&replica.hello()).unwrap();
        assert_eq!(hello.kind, frame_type::HELLO);
        assert_eq!(hello.header["cursor"], 0);
        let Step::Send(ask) = replica.apply(&state(1, 0, 0, 0)) else {
            panic!("rows are asked for");
        };
        let ask = wire::decode(&ask).unwrap();
        assert_eq!(ask.kind, frame_type::ROWS_REQ);
        assert_eq!(ask.header["after"], 0);
        assert_eq!(replica.apply(&row(1, &first)), Step::Changed);
        let done = wire::encode(frame_type::ROWS_DONE, &json!({"headSeq": 1}), &[]);
        assert_eq!(replica.apply(&done), Step::Changed);
        assert!(replica.caught_up());
        let entries = replica.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "m1");
        // A reconnect resumes after the rows it holds.
        let hello = wire::decode(&replica.hello()).unwrap();
        assert_eq!(hello.header["cursor"], 1);
        assert_eq!(replica.context_usage().unwrap().fraction(), Some(0.25));
        // Context-only changes arrive without adding a transcript entry.
        host.update_context_usage(Some(75_000), None).unwrap();
        let update = host.doc().export(loro::ExportMode::all_updates()).unwrap();
        assert_eq!(replica.apply(&row(2, &update)), Step::Changed);
        assert_eq!(replica.context_usage().unwrap().fraction(), Some(0.375));
        assert_eq!(replica.entries().len(), 1);
    }

    #[test]
    fn a_compacted_room_is_read_from_its_checkpoint_first() {
        let host = SessionDoc::init("c1").unwrap();
        host.push_message(&entry("m1", "from the checkpoint"))
            .unwrap();
        let snapshot = host.export_snapshot().unwrap();
        let mut replica = ChatReplica::new("web-1");
        assert_eq!(replica.apply(&state(9, 5, 7, 100)), Step::Checkpoint);
        let Step::Send(ask) = replica.checkpoint(Some(&snapshot)) else {
            panic!("rows are asked for");
        };
        assert_eq!(wire::decode(&ask).unwrap().header["after"], 7);
        assert_eq!(replica.entries().len(), 1);
        // No checkpoint after all: rows from where it is.
        let mut empty = ChatReplica::new("web-2");
        assert_eq!(empty.apply(&state(3, 0, 2, 10)), Step::Checkpoint);
        let Step::Send(ask) = empty.checkpoint(None) else {
            panic!("rows are asked for");
        };
        assert_eq!(wire::decode(&ask).unwrap().header["after"], 0);
    }

    #[test]
    fn unreadable_frames_change_nothing() {
        let mut replica = ChatReplica::new("web-1");
        assert_eq!(replica.apply(b"nope"), Step::Nothing);
        assert_eq!(replica.apply(&row(1, b"not loro")), Step::Nothing);
        assert!(replica.entries().is_empty());
    }
}
