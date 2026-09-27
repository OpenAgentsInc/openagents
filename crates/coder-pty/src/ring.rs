//! The bounded replay buffer a host keeps for each terminal.
//!
//! The ring holds a terminal's sequenced frames — output and the final
//! exit — up to a byte ceiling and a frame ceiling, and discards the oldest
//! first. A reader that asks for frames after a sequence number the ring no
//! longer covers gets a [`Missed`] range before the frames it still has, so
//! a client can say what it did not see rather than stitch two ends of the
//! output together as if they were adjacent.

use std::collections::VecDeque;

use crate::wire::{Body, Exit};

/// How many discarded frames' end offsets the ring remembers, so a gap
/// can say how many bytes it covers. Older gaps report an unknown count.
const DISCARDED_INDEX: usize = 1024;

/// One retained frame.
#[derive(Clone, Debug)]
struct Entry {
    seq: u64,
    /// Output bytes the terminal produced before this frame.
    offset: u64,
    body: Body,
}

impl Entry {
    fn len(&self) -> usize {
        match &self.body {
            Body::Output { data, .. } => data.len(),
            _ => 0,
        }
    }
}

/// Frames a reader asked for that the ring had already discarded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Missed {
    pub from: u64,
    pub to: u64,
    /// The output bytes in those frames, or `None` when the ring no longer
    /// remembers where the reader's last frame ended.
    pub bytes: Option<u64>,
}

/// A terminal's replay buffer.
#[derive(Debug)]
pub struct Ring {
    entries: VecDeque<Entry>,
    /// `(seq, end offset)` of recently discarded frames, oldest first.
    discarded: VecDeque<(u64, u64)>,
    held: usize,
    max_bytes: usize,
    max_frames: usize,
    last_seq: u64,
    produced: u64,
}

impl Ring {
    /// A ring holding at most `max_bytes` output bytes in at most
    /// `max_frames` frames. Both are at least one.
    #[must_use]
    pub fn new(max_bytes: usize, max_frames: usize) -> Self {
        Ring {
            entries: VecDeque::new(),
            discarded: VecDeque::new(),
            held: 0,
            max_bytes: max_bytes.max(1),
            max_frames: max_frames.max(1),
            last_seq: 0,
            produced: 0,
        }
    }

    /// Appends output and returns its sequence number.
    pub fn push_output(&mut self, data: Vec<u8>) -> u64 {
        let seq = self.last_seq + 1;
        let len = data.len() as u64;
        self.push(Body::Output { seq, data });
        self.produced += len;
        seq
    }

    /// Appends the terminal's exit and returns its sequence number.
    pub fn push_exit(&mut self, exit: Exit) -> u64 {
        let seq = self.last_seq + 1;
        self.push(Body::Exit { seq, exit });
        seq
    }

    fn push(&mut self, body: Body) {
        let entry = Entry {
            seq: self.last_seq + 1,
            offset: self.produced,
            body,
        };
        self.last_seq = entry.seq;
        self.held += entry.len();
        self.entries.push_back(entry);
        while self.entries.len() > 1
            && (self.held > self.max_bytes || self.entries.len() > self.max_frames)
        {
            if let Some(old) = self.entries.pop_front() {
                self.held -= old.len();
                self.discarded
                    .push_back((old.seq, old.offset + old.len() as u64));
                if self.discarded.len() > DISCARDED_INDEX {
                    self.discarded.pop_front();
                }
            }
        }
    }

    /// The newest sequence number, or zero before the first frame.
    #[must_use]
    pub fn head(&self) -> u64 {
        self.last_seq
    }

    /// The oldest retained sequence number, if any frame is retained.
    #[must_use]
    pub fn first(&self) -> Option<u64> {
        self.entries.front().map(|entry| entry.seq)
    }

    /// Every output byte the terminal produced, retained or not.
    #[must_use]
    pub fn produced(&self) -> u64 {
        self.produced
    }

    /// Output bytes currently retained.
    #[must_use]
    pub fn held(&self) -> usize {
        self.held
    }

    /// What a reader that has everything through `after` is missing, if
    /// the ring discarded any of the frames that follow it.
    #[must_use]
    pub fn missed(&self, after: u64) -> Option<Missed> {
        let first = self.entries.front()?;
        if after + 1 >= first.seq {
            return None;
        }
        let end = if after == 0 {
            Some(0)
        } else {
            self.discarded
                .iter()
                .find(|(seq, _)| *seq == after)
                .map(|(_, end)| *end)
        };
        Some(Missed {
            from: after + 1,
            to: first.seq - 1,
            bytes: end.map(|end| first.offset - end),
        })
    }

    /// The retained frame with sequence number `seq`.
    #[must_use]
    pub fn get(&self, seq: u64) -> Option<&Body> {
        let first = self.entries.front()?.seq;
        let index = usize::try_from(seq.checked_sub(first)?).ok()?;
        self.entries.get(index).map(|entry| &entry.body)
    }

    /// The retained frames after `after`, oldest first.
    pub fn after(&self, after: u64) -> impl Iterator<Item = &Body> {
        self.entries
            .iter()
            .filter(move |entry| entry.seq > after)
            .map(|entry| &entry.body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::Cause;

    #[test]
    fn sequence_numbers_start_at_one_and_increase() {
        let mut ring = Ring::new(1024, 16);
        assert_eq!(ring.head(), 0);
        assert_eq!(ring.push_output(b"a".to_vec()), 1);
        assert_eq!(ring.push_output(b"b".to_vec()), 2);
        let exit = Exit {
            cause: Cause::Exited,
            code: Some(0),
            signal: None,
        };
        assert_eq!(ring.push_exit(exit), 3);
        assert_eq!(
            ring.after(1).filter_map(Body::seq).collect::<Vec<_>>(),
            vec![2, 3]
        );
        assert_eq!(ring.missed(0), None);
    }

    #[test]
    fn a_full_ring_discards_the_oldest_and_reports_what_was_missed() {
        let mut ring = Ring::new(8, 100);
        for _ in 0..5 {
            ring.push_output(b"xxx".to_vec());
        }
        // 15 bytes produced, 8 allowed: frames 1 to 3 went, 4 and 5 remain.
        assert_eq!(ring.first(), Some(4));
        assert_eq!(ring.held(), 6);
        assert_eq!(ring.produced(), 15);
        assert_eq!(
            ring.missed(0),
            Some(Missed {
                from: 1,
                to: 3,
                bytes: Some(9)
            })
        );
        assert_eq!(
            ring.missed(1),
            Some(Missed {
                from: 2,
                to: 3,
                bytes: Some(6)
            })
        );
        assert_eq!(ring.missed(3), None);
        assert_eq!(ring.missed(5), None);
    }

    #[test]
    fn the_frame_ceiling_also_discards() {
        let mut ring = Ring::new(1 << 20, 2);
        for _ in 0..4 {
            ring.push_output(b"x".to_vec());
        }
        assert_eq!(ring.first(), Some(3));
        assert!(ring.get(2).is_none());
        assert_eq!(ring.get(4).and_then(Body::seq), Some(4));
    }
}
