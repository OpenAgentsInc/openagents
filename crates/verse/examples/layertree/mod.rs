//! LayerTree's event-driven Verse scene, shared by the viewer and film shots.

pub mod escape;
pub mod model;
pub mod scene;
pub mod watch;

use model::{Layout, Run, Snapshot};
use std::{collections::VecDeque, path::Path};

struct Playback {
    frames: VecDeque<u64>,
    seconds: f32,
    elapsed: f32,
}

pub struct Set {
    pub run: Run,
    pub state: Snapshot,
    pub layout: Layout,
    pub previous: Option<(Snapshot, Layout)>,
    pub phase: f32,
    pub time: f32,
    pub compare_progress: bool,
    playback: Option<Playback>,
}

impl Set {
    pub fn load(path: &Path, at: Option<u64>) -> Result<Self, String> {
        let run = Run::load(path)?;
        let state = run.snapshot(at.unwrap_or(run.events.len() as u64 - 1))?;
        let layout = Layout::new(&state)?;
        Ok(Self {
            run,
            state,
            layout,
            previous: None,
            phase: 1.0,
            time: 0.0,
            compare_progress: false,
            playback: None,
        })
    }

    pub fn play(&mut self, from: u64, to: Option<u64>, rate: f32) -> Result<(), String> {
        let to = to.unwrap_or(self.run.events.len() as u64 - 1);
        if from > to || to >= self.run.events.len() as u64 {
            return Err("Choose an event range within the run.".into());
        }
        if !rate.is_finite() || !(0.01..=60.0).contains(&rate) {
            return Err("Choose a playback rate from 0.01 to 60 events per second.".into());
        }
        self.state = self.run.snapshot(from)?;
        self.layout = Layout::new(&self.state)?;
        self.previous = None;
        self.time = 0.0;
        let frames = (from + 1..=to).collect();
        self.playback = Some(Playback {
            frames,
            seconds: rate.recip(),
            elapsed: 0.0,
        });
        Ok(())
    }

    #[allow(dead_code)] // Used by the viewer's frame sequence capture.
    pub fn duration(&self) -> f32 {
        self.playback
            .as_ref()
            .map_or(0.0, |p| (p.frames.len() + 1) as f32 * p.seconds - p.elapsed)
    }

    pub fn advance(&mut self, dt: f32) {
        let dt = dt.max(0.0);
        self.time += dt;
        if let Some(playback) = &mut self.playback {
            playback.elapsed += dt;
            while playback.elapsed >= playback.seconds && !playback.frames.is_empty() {
                playback.elapsed -= playback.seconds;
                let seq = playback.frames.pop_front().unwrap();
                // Run::load validates the complete replay. Keep only the prior
                // frame so a large payout log does not clone the tree per event.
                let previous = (self.state.clone(), self.layout.clone());
                self.state
                    .apply(&self.run.events[seq as usize])
                    .expect("The run was checked when it was loaded.");
                self.layout = Layout::new(&self.state)
                    .expect("The run's balances were checked when it was loaded.");
                self.previous = Some(previous);
            }
            self.phase = (playback.elapsed / playback.seconds).min(1.0);
        }
    }

    pub fn next_height(&self) -> Option<u32> {
        self.playback
            .as_ref()?
            .frames
            .front()
            .and_then(|seq| self.run.events[*seq as usize].height)
    }

    pub fn event(&self) -> &model::Event {
        &self.run.events[self.state.seq as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn playback_uses_checked_snapshots_and_stops_at_the_requested_event() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/verse/fixtures/layertree-transitions.jsonl");
        let mut set = Set::load(&path, None).unwrap();
        set.play(0, None, 1.0).unwrap();
        let last = set.run.events.len() as u64 - 1;
        for seq in 0..=last {
            assert_eq!(set.state.seq, seq);
            assert_eq!(
                set.state.output_amount,
                set.state.leaves.values().map(|l| l.amount).sum::<u64>()
            );
            assert_eq!(
                set.layout.nodes[&(set.layout.depth, 0)].sum,
                set.state.output_amount
            );
            for node in set.layout.nodes.values().filter(|n| n.height > 0) {
                let children: u64 = [0, 1]
                    .iter()
                    .map(|side| {
                        set.layout
                            .nodes
                            .get(&(node.height - 1, node.index * 2 + side))
                            .map_or(0, |n| n.sum)
                    })
                    .sum();
                assert_eq!(node.sum, children);
            }
            if seq < last {
                set.advance(1.0);
            }
        }
        set.advance(50.0);
        assert_eq!(set.state.seq, last);
        assert_eq!(set.phase, 1.0);
        let before_reject = set.run.snapshot(last - 1).unwrap();
        assert_eq!(set.state.leaves, before_reject.leaves);
        assert_eq!(set.state.root, before_reject.root);
        set.play(0, Some(0), 1.0).unwrap();
        set.advance(1.0);
        assert_eq!(set.state.seq, 0);
        let text = std::fs::read_to_string(&path)
            .unwrap()
            .replace("\"synthetic\":true", "\"synthetic\":false");
        assert!(Run::parse(&text, "unmarked".into()).is_err());
        assert!(set.play(1, Some(0), 1.0).is_err());
        assert!(set.play(0, None, f32::NAN).is_err());
    }
}
