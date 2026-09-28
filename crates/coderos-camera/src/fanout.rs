//! One frame in, every output fed.
//!
//! Each sink runs on a thread of its own behind a mailbox that holds one
//! frame. [`Fanout::publish`] drops a frame into every mailbox that is
//! empty and counts the ones that were full, so a sink that falls behind,
//! such as the tracker under a slow model, loses frames rather than
//! holding the camera and the other sinks back. The counts are what
//! `status` reports for each output.

use crate::frame::Frame;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{SyncSender, TrySendError, sync_channel};
use std::thread;

/// One consumer of frames.
pub trait Sink: Send + 'static {
    /// The output's name, as `status` lists it.
    fn name(&self) -> &'static str;
    /// One frame, with the count of frames this lane dropped up to it. A
    /// sink that cannot use the frame returns without it.
    fn accept(&mut self, frame: &Arc<Frame>, dropped: u64);
}

/// What one lane delivered and what it dropped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaneCount {
    pub name: &'static str,
    pub delivered: u64,
    pub dropped: u64,
}

struct Lane {
    name: &'static str,
    tx: SyncSender<(Arc<Frame>, u64)>,
    join: thread::JoinHandle<()>,
    delivered: Arc<AtomicU64>,
    dropped: u64,
}

/// The lanes frames fan out to.
#[derive(Default)]
pub struct Fanout {
    lanes: Vec<Lane>,
}

impl Fanout {
    pub fn new() -> Fanout {
        Fanout::default()
    }

    /// Adds a sink and starts its thread.
    pub fn add(&mut self, mut sink: Box<dyn Sink>) -> Result<(), String> {
        let name = sink.name();
        let (tx, rx) = sync_channel::<(Arc<Frame>, u64)>(1);
        let delivered = Arc::new(AtomicU64::new(0));
        let counter = Arc::clone(&delivered);
        let join = thread::Builder::new()
            .name(format!("camera-{name}"))
            .spawn(move || {
                for (frame, dropped) in rx {
                    sink.accept(&frame, dropped);
                    counter.fetch_add(1, Ordering::Relaxed);
                }
            })
            .map_err(|err| format!("{name} thread: {err}"))?;
        self.lanes.push(Lane {
            name,
            tx,
            join,
            delivered,
            dropped: 0,
        });
        Ok(())
    }

    /// Offers one frame to every lane.
    pub fn publish(&mut self, frame: Arc<Frame>) {
        for lane in &mut self.lanes {
            match lane.tx.try_send((Arc::clone(&frame), lane.dropped)) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
                    lane.dropped += 1;
                }
            }
        }
    }

    /// Ends every lane: the sink threads drain what they hold and exit,
    /// and the loopback writer and every other output close with them.
    /// `stop` waits for each thread, so a sink mid-frame finishes it.
    pub fn stop(&mut self) {
        for lane in self.lanes.drain(..) {
            drop(lane.tx);
            let _ = lane.join.join();
        }
    }

    /// Every lane's counts.
    pub fn counts(&self) -> Vec<LaneCount> {
        self.lanes
            .iter()
            .map(|lane| LaneCount {
                name: lane.name,
                delivered: lane.delivered.load(Ordering::Relaxed),
                dropped: lane.dropped,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::Duration;

    /// The (frame, drops) pairs a test sink remembers.
    type Seen = Arc<Mutex<Vec<(u64, u64)>>>;

    /// A sink that remembers every frame number it saw and the drop
    /// count that came with it, and may dawdle.
    struct Recorder {
        name: &'static str,
        seen: Seen,
        dawdle: Duration,
    }

    impl Sink for Recorder {
        fn name(&self) -> &'static str {
            self.name
        }
        fn accept(&mut self, frame: &Arc<Frame>, dropped: u64) {
            if !self.dawdle.is_zero() {
                thread::sleep(self.dawdle);
            }
            if let Ok(mut seen) = self.seen.lock() {
                seen.push((frame.seq, dropped));
            }
        }
    }

    /// Waits until every lane has delivered or dropped `frames` frames,
    /// which is when the sinks have read everything they will read.
    fn settle(fanout: &Fanout, frames: u64) -> Vec<LaneCount> {
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            let counts = fanout.counts();
            if counts.iter().all(|c| c.delivered + c.dropped == frames) {
                return counts;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the lanes never settled: {counts:?}"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn sink(name: &'static str, dawdle: Duration) -> (Box<dyn Sink>, Seen) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        (
            Box::new(Recorder {
                name,
                seen: Arc::clone(&seen),
                dawdle,
            }),
            seen,
        )
    }

    /// The frame numbers a sink saw, without the drop counts.
    fn seqs(seen: &Seen) -> Vec<u64> {
        seen.lock()
            .expect("seen")
            .iter()
            .map(|(seq, _)| *seq)
            .collect()
    }

    #[test]
    fn every_sink_sees_every_frame_when_it_keeps_up() {
        let mut fanout = Fanout::new();
        let (a, seen_a) = sink("a", Duration::ZERO);
        let (b, seen_b) = sink("b", Duration::ZERO);
        fanout.add(a).expect("a");
        fanout.add(b).expect("b");
        for seq in 0..20 {
            fanout.publish(Arc::new(Frame::solid(seq, 2, 2, [0, 0, 0])));
            // A mailbox holds one frame, so the source paces itself the way
            // a camera at a frame rate does.
            thread::sleep(Duration::from_millis(2));
        }
        let counts = settle(&fanout, 20);
        assert_eq!(seqs(&seen_a), (0..20).collect::<Vec<u64>>());
        assert_eq!(seqs(&seen_b), (0..20).collect::<Vec<u64>>());
        assert_eq!(counts[0].delivered, 20);
        assert_eq!(counts[0].dropped, 0);
    }

    #[test]
    fn a_slow_sink_drops_frames_and_holds_no_other_sink_back() {
        let mut fanout = Fanout::new();
        let (fast, seen_fast) = sink("fast", Duration::ZERO);
        let (slow, seen_slow) = sink("slow", Duration::from_millis(30));
        fanout.add(fast).expect("fast");
        fanout.add(slow).expect("slow");
        let frames = 12;
        for seq in 0..frames {
            fanout.publish(Arc::new(Frame::solid(seq, 2, 2, [0, 0, 0])));
            thread::sleep(Duration::from_millis(3));
        }
        let counts = settle(&fanout, frames);
        let fast_seen = seqs(&seen_fast).len() as u64;
        let slow_seen = seen_slow.lock().expect("slow").clone();
        assert_eq!(fast_seen, frames, "the fast sink saw every frame");
        assert!(
            (slow_seen.len() as u64) < frames,
            "the slow sink dropped some: {slow_seen:?}"
        );
        assert!(!slow_seen.is_empty());
        let slow = counts.iter().find(|c| c.name == "slow").expect("slow lane");
        assert_eq!(slow.delivered, slow_seen.len() as u64);
        assert_eq!(
            slow.delivered + slow.dropped,
            frames,
            "every frame was delivered or dropped"
        );
        let fast = counts.iter().find(|c| c.name == "fast").expect("fast lane");
        assert_eq!(fast.dropped, 0);
        // The count that rides a frame is the lane's drops up to it: the
        // frames between the ones the sink took.
        for (taken, (seq, dropped)) in slow_seen.iter().enumerate() {
            assert_eq!(*dropped, seq - taken as u64);
        }
    }

    #[test]
    fn stop_drains_the_mailbox_and_ends_every_lane() {
        let mut fanout = Fanout::new();
        let (a, seen_a) = sink("a", Duration::ZERO);
        fanout.add(a).expect("a");
        for seq in 0..3 {
            fanout.publish(Arc::new(Frame::solid(seq, 1, 1, [0, 0, 0])));
        }
        fanout.stop();
        assert!(seqs(&seen_a).len() <= 3, "only the frames sent arrive");
        assert!(fanout.counts().is_empty(), "no lanes left to count");
        // A stopped fan-out takes frames into nothing.
        fanout.publish(Arc::new(Frame::solid(9, 1, 1, [0, 0, 0])));
    }

    #[test]
    fn counts_read_while_the_lanes_run() {
        let mut fanout = Fanout::new();
        let (a, _seen) = sink("a", Duration::ZERO);
        fanout.add(a).expect("a");
        fanout.publish(Arc::new(Frame::solid(1, 1, 1, [0, 0, 0])));
        let counts = settle(&fanout, 1);
        assert_eq!(counts.len(), 1);
        assert_eq!(counts[0].name, "a");
        assert_eq!(counts[0].delivered, 1);
    }
}
