//! The tracking sink: the landmarker over every frame, one line a frame
//! on the hands socket.
//!
//! The model loads on the first `hands on`, which fetches it when the
//! cache under `~/.openagents/quest/models/` is empty, and stays loaded
//! across `hands off`. The status line the tracker reports is what
//! `status` answers under `hands`.

use crate::fanout::Sink;
use crate::frame::Frame;
use crate::publisher::Publisher;
use coder_hands::landmarker::Landmarker;
use coder_hands::model::{Manifest, ensure_model, model_path};
use coder_hands::wire::Line;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// What the control socket reads and sets.
#[derive(Default)]
pub struct HandsState {
    /// Whether the tracker runs.
    pub on: AtomicBool,
    /// The tracker's status line.
    pub status: Mutex<String>,
    /// Frames the tracker read while on.
    pub tracked: Mutex<u64>,
}

impl HandsState {
    pub fn new() -> Arc<HandsState> {
        let state = HandsState::default();
        state.set_status("off");
        Arc::new(state)
    }

    pub fn set_status(&self, status: impl Into<String>) {
        if let Ok(mut s) = self.status.lock() {
            *s = status.into();
        }
    }

    pub fn status(&self) -> String {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn is_on(&self) -> bool {
        self.on.load(Ordering::Relaxed)
    }

    pub fn turn(&self, on: bool) {
        self.on.store(on, Ordering::Relaxed);
        if !on {
            self.set_status("off");
        } else {
            self.set_status("starting");
        }
    }
}

pub struct HandsSink {
    state: Arc<HandsState>,
    publisher: Publisher,
    net: Option<Landmarker>,
    last_load: Option<Instant>,
    /// The lane's drop count at the line last published, so the next
    /// line can say what the reader missed.
    announced: u64,
}

impl HandsSink {
    pub fn new(state: Arc<HandsState>, publisher: Publisher) -> HandsSink {
        HandsSink {
            state,
            publisher,
            net: None,
            last_load: None,
            announced: 0,
        }
    }

    /// Loads the model, fetching it first when the cache is empty.
    fn load(&mut self) -> Result<(), String> {
        let manifest = Manifest::pinned()?;
        let path = model_path(&manifest)?;
        if !path.exists() {
            self.state.set_status("fetching the hand model");
        }
        ensure_model(&manifest, &path)?;
        self.state.set_status("loading the hand model");
        self.net = Some(Landmarker::load(&path)?);
        Ok(())
    }
}

impl Sink for HandsSink {
    fn name(&self) -> &'static str {
        "hands"
    }

    fn accept(&mut self, frame: &Arc<Frame>, dropped: u64) {
        if !self.state.is_on() {
            return;
        }
        if self.net.is_none() {
            // A load that failed is tried again after a while rather than
            // on every frame, so a host with no network does not fetch
            // thirty times a second.
            let due = self
                .last_load
                .is_none_or(|at| at.elapsed() > Duration::from_secs(10));
            if !due {
                return;
            }
            self.last_load = Some(Instant::now());
            if let Err(err) = self.load() {
                self.state.set_status(err);
                return;
            }
        }
        let Some(net) = self.net.as_mut() else { return };
        match net.infer(&frame.rgb, frame.width, frame.height) {
            Ok(reading) => {
                self.state.set_status(reading.status.clone());
                if let Ok(mut tracked) = self.state.tracked.lock() {
                    *tracked += 1;
                }
                let mut line = Line::from_frame(&reading, frame.timestamp);
                line.dropped = dropped.saturating_sub(self.announced);
                self.announced = dropped;
                self.publisher.publish(&line.render());
            }
            Err(err) => self.state.set_status(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_state_starts_off_and_turning_it_sets_the_status() {
        let state = HandsState::new();
        assert!(!state.is_on());
        assert_eq!(state.status(), "off");
        state.turn(true);
        assert!(state.is_on());
        assert_eq!(state.status(), "starting");
        state.set_status("1 hand(s) detected");
        state.turn(false);
        assert_eq!(state.status(), "off");
    }

    #[test]
    fn an_off_tracker_reads_no_frame() {
        let dir =
            std::env::temp_dir().join(format!("coderos-camera-hands-sink-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let publisher = Publisher::bind(&dir.join("hands.sock")).expect("bind");
        let state = HandsState::new();
        let mut sink = HandsSink::new(Arc::clone(&state), publisher);
        sink.accept(&Arc::new(Frame::solid(1, 4, 4, [0, 0, 0])), 0);
        assert_eq!(*state.tracked.lock().expect("count"), 0);
        assert!(sink.net.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
