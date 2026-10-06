//! Terminal frames in sequence order, whatever order a transport delivers.
//!
//! A direct channel delivers one attachment's frames in order. A relay
//! delivers each frame as its own artifact, and retained artifacts can come
//! back in any order after a reconnect. [`Ordered`] holds frames that arrive
//! early and applies each once the frames before it have arrived, so the
//! terminal state sees a transport reordering as order, never as loss.

use std::collections::BTreeMap;

use coder_pty::client::{Applied, TerminalState};
use coder_pty::wire::{Body, Frame};

/// The most frames held while waiting for an earlier one.
pub const HELD_MAX: usize = 4096;

/// A terminal state fed in sequence order.
#[derive(Debug)]
pub struct Ordered {
    state: TerminalState,
    held: BTreeMap<u64, Frame>,
}

impl Ordered {
    /// Order frames for `state`.
    #[must_use]
    pub fn new(state: TerminalState) -> Self {
        Self {
            state,
            held: BTreeMap::new(),
        }
    }

    /// The terminal state.
    #[must_use]
    pub fn state(&self) -> &TerminalState {
        &self.state
    }

    /// How many frames wait for an earlier one.
    #[must_use]
    pub fn held(&self) -> usize {
        self.held.len()
    }

    /// Accept one frame and apply every frame that is now next. A frame
    /// that is neither next nor a duplicate waits; once [`HELD_MAX`] frames
    /// wait, the earliest is applied anyway and the state reports the loss.
    pub fn push(&mut self, frame: Frame) -> Vec<Applied> {
        self.push_frames(frame)
            .into_iter()
            .map(|(applied, _)| applied)
            .collect()
    }

    /// As [`Ordered::push`], and returns each frame applied with what
    /// applying it did, in order. An emulator feeds the data of each
    /// [`Applied::Output`] frame.
    pub fn push_frames(&mut self, frame: Frame) -> Vec<(Applied, Frame)> {
        let position = match &frame.body {
            Body::Output { seq, .. } | Body::Exit { seq, .. } => Some(*seq),
            Body::Gap { from, .. } => Some(*from),
            // An effect or a typist change is a notice, applied when it
            // arrives.
            Body::Detached { .. } | Body::Effect { .. } | Body::Typist { .. } => None,
        };
        let mut applied = Vec::new();
        match position {
            Some(position) if position > self.state.resume_after() + 1 => {
                self.held.entry(position).or_insert(frame);
                if self.held.len() > HELD_MAX
                    && let Some((_, earliest)) = self.held.pop_first()
                {
                    applied.push((self.state.apply(&earliest), earliest));
                }
            }
            _ => applied.push((self.state.apply(&frame), frame)),
        }
        while let Some(entry) = self.held.first_entry() {
            let next = self.state.resume_after() + 1;
            if *entry.key() > next {
                break;
            }
            let frame = entry.remove();
            applied.push((self.state.apply(&frame), frame));
        }
        applied
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_pty::wire::TerminalRef;

    fn output(seq: u64, text: &str) -> Frame {
        Frame::new(
            TerminalRef {
                generation: "a".repeat(64),
                terminal: "b".repeat(64),
            },
            "c".repeat(64),
            Body::Output {
                seq,
                data: text.as_bytes().to_vec(),
            },
        )
    }

    #[test]
    fn reordered_frames_apply_in_sequence() {
        let terminal = output(1, "").terminal;
        let mut ordered = Ordered::new(TerminalState::new(terminal, 10, 80));
        assert!(ordered.push(output(3, "c")).is_empty());
        assert!(ordered.push(output(2, "b")).is_empty());
        assert_eq!(ordered.held(), 2);
        let applied = ordered.push(output(1, "a"));
        assert_eq!(applied.len(), 3);
        assert!(applied.iter().all(|a| matches!(a, Applied::Output { .. })));
        assert_eq!(ordered.state().resume_after(), 3);
        assert_eq!(ordered.push(output(2, "b")), vec![Applied::Duplicate]);
        assert!(ordered.state().screen().text().contains("abc"));
        assert!(!ordered.state().behind());
    }
}
