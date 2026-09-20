//! Two lanes from a working task to a draw loop: a reliable one for what
//! happened, a lossy one for what it looked like while happening.
//!
//! A turn reports two kinds of thing. Control events — a command proposed,
//! a command's outcome, a refusal, the turn's completion — each change what
//! the transcript says, and losing one silently misreports the turn. Text
//! events — streamed reply deltas — only change what the live preview
//! shows, and the finished reply arrives whole anyway. So [`Feed::send`]
//! never drops a control event: the channel is unbounded and the send
//! cannot fail while the draw loop lives. Text rides the same channel, in
//! order with the controls, but only up to [`TEXT_BACKLOG_MAX`] bytes of
//! it may be in flight; past that the delta is dropped and counted, and the
//! draw loop reads the count with [`Inbox::take_dropped`] so the loss is
//! drawn rather than hidden.
//!
//! [`Inbox::drain`] takes everything ready and merges adjacent text, so a
//! frame does one `push_str` per burst instead of one per token.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::sync::mpsc;

/// The most bytes of text the draw loop may be behind before deltas are
/// dropped rather than queued. A reply longer than this still arrives
/// whole with its completion event.
pub const TEXT_BACKLOG_MAX: usize = 1 << 20;

/// Which lane an event travels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    /// Must arrive: an outcome, a refusal, a completion.
    Control,
    /// May be coalesced or, under pressure, dropped: a preview delta.
    Text,
}

/// An event a [`Feed`] carries.
pub trait Event {
    /// Which lane this event travels.
    fn lane(&self) -> Lane;
    /// How many bytes of text this event carries; counts against
    /// [`TEXT_BACKLOG_MAX`]. Zero for controls.
    fn text_len(&self) -> usize;
    /// Merges `next` into `self` if both are text of a kind that
    /// concatenates. Returns whether it did.
    fn coalesce(&mut self, next: &Self) -> bool;
}

/// What became of a sent event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sent {
    /// Queued for the draw loop.
    Delivered,
    /// A text event dropped for backlog; `bytes` is how much it carried.
    Dropped { bytes: usize },
    /// The draw loop is gone. Only a control event reports this; a text
    /// event to a gone loop is dropped without comment.
    Closed,
}

struct Pressure {
    backlog: AtomicUsize,
    dropped: AtomicUsize,
}

/// The sending side. Clone it for each task that reports.
pub struct Feed<T> {
    tx: mpsc::UnboundedSender<T>,
    pressure: Arc<Pressure>,
}

impl<T> Clone for Feed<T> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            pressure: Arc::clone(&self.pressure),
        }
    }
}

/// The receiving side, owned by the draw loop.
pub struct Inbox<T> {
    rx: mpsc::UnboundedReceiver<T>,
    pressure: Arc<Pressure>,
}

/// A connected feed and inbox.
pub fn channel<T: Event>() -> (Feed<T>, Inbox<T>) {
    let (tx, rx) = mpsc::unbounded_channel();
    let pressure = Arc::new(Pressure {
        backlog: AtomicUsize::new(0),
        dropped: AtomicUsize::new(0),
    });
    (
        Feed {
            tx,
            pressure: Arc::clone(&pressure),
        },
        Inbox { rx, pressure },
    )
}

impl<T: Event> Feed<T> {
    /// Sends `event`. A control event is queued or, when the inbox is
    /// gone, reported `Closed`; it is never dropped for pressure. A text
    /// event is queued while the backlog has room and dropped, counted,
    /// otherwise.
    pub fn send(&self, event: T) -> Sent {
        match event.lane() {
            Lane::Control => match self.tx.send(event) {
                Ok(()) => Sent::Delivered,
                Err(_) => Sent::Closed,
            },
            Lane::Text => {
                let bytes = event.text_len();
                let pressure = &self.pressure;
                let queued = pressure.backlog.load(Ordering::Acquire);
                if queued.saturating_add(bytes) > TEXT_BACKLOG_MAX {
                    pressure.dropped.fetch_add(bytes, Ordering::AcqRel);
                    return Sent::Dropped { bytes };
                }
                pressure.backlog.fetch_add(bytes, Ordering::AcqRel);
                match self.tx.send(event) {
                    Ok(()) => Sent::Delivered,
                    Err(_) => {
                        pressure.backlog.fetch_sub(bytes, Ordering::AcqRel);
                        Sent::Dropped { bytes }
                    }
                }
            }
        }
    }
}

impl<T: Event> Inbox<T> {
    /// The next event, in send order; `None` when every feed is gone.
    pub async fn recv(&mut self) -> Option<T> {
        let event = self.rx.recv().await?;
        self.account(&event);
        Some(event)
    }

    /// Everything ready now, in order, with adjacent text merged.
    pub fn drain(&mut self) -> Vec<T> {
        let mut batch: Vec<T> = Vec::new();
        while let Ok(event) = self.rx.try_recv() {
            self.account(&event);
            let merged = batch.last_mut().is_some_and(|last| last.coalesce(&event));
            if !merged {
                batch.push(event);
            }
        }
        batch
    }

    /// How many text bytes were dropped since the last call.
    pub fn take_dropped(&self) -> usize {
        self.pressure.dropped.swap(0, Ordering::AcqRel)
    }

    /// How many text bytes are queued and not yet received.
    pub fn backlog(&self) -> usize {
        self.pressure.backlog.load(Ordering::Acquire)
    }

    fn account(&self, event: &T) {
        if event.lane() == Lane::Text {
            self.pressure
                .backlog
                .fetch_sub(event.text_len(), Ordering::AcqRel);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    enum Item {
        Outcome(u32),
        Text(String),
    }

    impl Event for Item {
        fn lane(&self) -> Lane {
            match self {
                Item::Outcome(_) => Lane::Control,
                Item::Text(_) => Lane::Text,
            }
        }

        fn text_len(&self) -> usize {
            match self {
                Item::Outcome(_) => 0,
                Item::Text(text) => text.len(),
            }
        }

        fn coalesce(&mut self, next: &Self) -> bool {
            match (self, next) {
                (Item::Text(mine), Item::Text(theirs)) => {
                    mine.push_str(theirs);
                    true
                }
                _ => false,
            }
        }
    }

    #[test]
    fn every_control_event_arrives_in_order_with_nobody_reading() {
        let (feed, mut inbox) = channel::<Item>();
        for n in 0..100_000 {
            assert_eq!(feed.send(Item::Outcome(n)), Sent::Delivered);
        }
        let batch = inbox.drain();
        assert_eq!(batch.len(), 100_000);
        assert!(
            batch
                .iter()
                .enumerate()
                .all(|(index, item)| *item == Item::Outcome(index as u32))
        );
    }

    #[test]
    fn text_past_the_backlog_is_dropped_and_counted_while_controls_pass() {
        let (feed, mut inbox) = channel::<Item>();
        let chunk = "x".repeat(TEXT_BACKLOG_MAX / 4);
        for _ in 0..4 {
            assert_eq!(feed.send(Item::Text(chunk.clone())), Sent::Delivered);
        }
        assert_eq!(inbox.backlog(), TEXT_BACKLOG_MAX);
        assert_eq!(
            feed.send(Item::Text("one more".into())),
            Sent::Dropped { bytes: 8 }
        );
        assert_eq!(feed.send(Item::Outcome(7)), Sent::Delivered);
        assert_eq!(
            feed.send(Item::Text("again".into())),
            Sent::Dropped { bytes: 5 }
        );
        assert_eq!(inbox.take_dropped(), 13);
        assert_eq!(inbox.take_dropped(), 0);

        let batch = inbox.drain();
        assert_eq!(
            batch.len(),
            2,
            "four chunks merge into one text, then the outcome"
        );
        assert_eq!(batch[0].text_len(), TEXT_BACKLOG_MAX);
        assert_eq!(batch[1], Item::Outcome(7));
        assert_eq!(inbox.backlog(), 0);
        assert_eq!(feed.send(Item::Text("room again".into())), Sent::Delivered);
    }

    #[test]
    fn drain_keeps_text_and_controls_in_order() {
        let (feed, mut inbox) = channel::<Item>();
        feed.send(Item::Text("{\"v\":1".into()));
        feed.send(Item::Text(",\"commands\":[]}".into()));
        feed.send(Item::Outcome(1));
        feed.send(Item::Text("done".into()));
        let batch = inbox.drain();
        assert_eq!(
            batch,
            [
                Item::Text("{\"v\":1,\"commands\":[]}".into()),
                Item::Outcome(1),
                Item::Text("done".into()),
            ]
        );
    }

    #[test]
    fn a_control_event_to_a_gone_inbox_says_so() {
        let (feed, inbox) = channel::<Item>();
        drop(inbox);
        assert_eq!(feed.send(Item::Outcome(1)), Sent::Closed);
        assert_eq!(
            feed.send(Item::Text("late".into())),
            Sent::Dropped { bytes: 4 }
        );
    }

    #[tokio::test]
    async fn recv_accounts_for_text_it_hands_over() {
        let (feed, mut inbox) = channel::<Item>();
        feed.send(Item::Text("abc".into()));
        assert_eq!(inbox.backlog(), 3);
        assert_eq!(inbox.recv().await, Some(Item::Text("abc".into())));
        assert_eq!(inbox.backlog(), 0);
        drop(feed);
        assert_eq!(inbox.recv().await, None);
    }
}
