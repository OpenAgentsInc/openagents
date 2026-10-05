//! A scripted stress run of the terminal overlay over a heavy world: the
//! `terminal_stress` example opens Everglade's town, opens the overlay with
//! a typing pane and `busy` panes of heavy output, keeps Wind Walls and
//! Meteor Swarm going, types a key every [`KEY_EVERY`], and records every
//! frame and every key's echo latency for [`Plan::seconds`] after a
//! warm-up. It writes a JSON report and ends Verse.
//!
//! Read `docs/verse/verification/2026-10-05-terminal-performance/`.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::pty::Program;
use super::stats::{Frame, Spread};

/// How often the run types into the typing pane.
pub const KEY_EVERY: Duration = Duration::from_millis(150);
/// How often the run raises a Wind Wall.
const WIND_EVERY: Duration = Duration::from_millis(1500);
/// How often the run casts Meteor Swarm on the town.
const METEOR_EVERY: Duration = Duration::from_secs(6);

/// What one run measures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// Panes of heavy output, beside the typing pane.
    pub busy: usize,
    /// Seconds recorded after the warm-up.
    pub seconds: u32,
    /// Seconds after the panes open before recording starts.
    pub warmup: u32,
    /// Where the JSON report goes.
    pub out: PathBuf,
    /// Cast Wind Walls and Meteor Swarm while recording.
    pub spells: bool,
    /// A free-form label the report carries, such as "before".
    pub label: String,
}

/// What the app does next for the run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Open the overlay with [`Driver::programs`].
    Open,
    WindWall,
    MeteorSwarm,
    /// Type one key into the focused pane.
    Key(char),
    /// Recording ended: write [`Driver::report`] and quit.
    Finish,
}

/// The run's clock and what it has done.
#[derive(Debug)]
pub struct Driver {
    pub plan: Plan,
    opened: Option<Instant>,
    recording: Option<Instant>,
    next_key: Instant,
    next_wind: Instant,
    next_meteor: Instant,
    typed: usize,
    root: PathBuf,
}

/// A run's report.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Report {
    pub label: String,
    pub busy_panes: usize,
    pub seconds: f32,
    pub spells: bool,
    pub frames: usize,
    pub interval_ms: Spread,
    pub cpu_ms: Spread,
    pub world_ms: Spread,
    pub update_ms: Spread,
    pub draw_ms: Spread,
    /// Output parsed per second across panes, in MB.
    pub mb_per_s: f32,
    pub latency_ms: Spread,
    pub latency_samples: usize,
    pub frames_over_16_7_ms: usize,
    /// The five longest frames, with how their time split.
    pub worst_frames: Vec<Frame>,
}

impl Driver {
    #[must_use]
    pub fn new(plan: Plan) -> Self {
        let now = Instant::now();
        let root =
            std::env::temp_dir().join(format!("verse-terminal-stress-{}", std::process::id()));
        Driver {
            plan,
            opened: None,
            recording: None,
            next_key: now,
            next_wind: now,
            next_meteor: now,
            typed: 0,
            root,
        }
    }

    /// The scratch directory the panes run in; it is also their home.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Whether recording has started.
    #[must_use]
    pub fn recording(&self) -> bool {
        self.recording.is_some()
    }

    /// The next action once the world is ready, or `None` for this frame.
    pub fn step(&mut self, ready: bool) -> Vec<Action> {
        let now = Instant::now();
        let mut actions = Vec::new();
        let Some(opened) = self.opened else {
            if ready {
                self.opened = Some(now);
                actions.push(Action::Open);
            }
            return actions;
        };
        if self.recording.is_none() {
            if now.duration_since(opened) >= Duration::from_secs(self.plan.warmup.into()) {
                self.recording = Some(now);
                self.next_key = now;
                self.next_wind = now;
                self.next_meteor = now;
            } else {
                return actions;
            }
        }
        let recording = self.recording.unwrap_or(now);
        if now.duration_since(recording) >= Duration::from_secs(self.plan.seconds.into()) {
            actions.push(Action::Finish);
            return actions;
        }
        if now >= self.next_key {
            self.next_key = now + KEY_EVERY;
            // A line at a time, so cat's echo stays on screen.
            self.typed += 1;
            let key = if self.typed % 40 == 0 {
                '\r'
            } else {
                char::from(b'a' + (self.typed % 26) as u8)
            };
            actions.push(Action::Key(key));
        }
        if self.plan.spells {
            if now >= self.next_wind {
                self.next_wind = now + WIND_EVERY;
                actions.push(Action::WindWall);
            }
            if now >= self.next_meteor {
                self.next_meteor = now + METEOR_EVERY;
                actions.push(Action::MeteorSwarm);
            }
        }
        actions
    }

    /// Writes the panes' input files and returns what they run: the typing
    /// pane (`cat`) first, then `busy` panes cycling through `yes`, a large
    /// file, a colored build log, `seq`, and `top`.
    ///
    /// # Errors
    /// The scratch directory cannot be written.
    pub fn programs(&self) -> Result<Vec<Program>, String> {
        std::fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        let big = self.root.join("big.txt");
        let mut text = String::new();
        for i in 0..200_000 {
            text.push_str(&format!(
                "{i:>7} the quick brown fox jumps over the lazy dog — λ → ✓ {}\n",
                "lorem ipsum dolor sit amet ".repeat(1 + i % 3)
            ));
        }
        std::fs::write(&big, text).map_err(|e| e.to_string())?;
        let log = self.root.join("build.log");
        let mut text = String::new();
        for i in 0..20_000 {
            match i % 7 {
                0 => text.push_str(&format!(
                    "\x1b[1m\x1b[32m   Compiling\x1b[0m crate-{i} v0.{}.{} (/work/crates/crate-{i})\n",
                    i % 9,
                    i % 13
                )),
                1 => text.push_str(&format!(
                    "\x1b[1m\x1b[33mwarning\x1b[0m\x1b[1m: unused variable: `x{i}`\x1b[0m\n\x1b[38;5;12m  --> \x1b[0msrc/lib.rs:{}:9\n",
                    i % 400
                )),
                2 => text.push_str(&format!(
                    "\x1b[38;2;120;200;255mtest\x1b[0m tests::case_{i} ... \x1b[32mok\x1b[0m\n"
                )),
                _ => text.push_str(&format!(
                    "\x1b[2m[{:>4}/{}] building object build/obj/file_{i}.o\x1b[0m\n",
                    i % 1000,
                    1000
                )),
            }
        }
        std::fs::write(&log, text).map_err(|e| e.to_string())?;
        let sh = |script: String, label: &str| Program::Command {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), script],
            label: label.into(),
        };
        let workloads = [
            sh("exec yes 'verse terminal stress: yes'".into(), "yes"),
            sh(
                format!("while :; do cat '{}'; done", big.display()),
                "cat big.txt",
            ),
            sh(
                format!("while :; do cat '{}'; done", log.display()),
                "build log",
            ),
            sh("while :; do seq 1 10000000; done".into(), "seq"),
            sh("exec top -s 1 -o cpu".into(), "top"),
        ];
        let mut programs = vec![sh("stty -icanon; exec cat".into(), "typing")];
        programs.extend((0..self.plan.busy).map(|i| workloads[i % workloads.len()].clone()));
        Ok(programs)
    }

    /// The report over `frames` and `latencies`.
    #[must_use]
    pub fn report(&self, frames: &[Frame], latencies: &[f32]) -> Report {
        let spread = |f: fn(&Frame) -> f32| Spread::of(&frames.iter().map(f).collect::<Vec<_>>());
        let seconds: f32 = frames.iter().map(|f| f.interval_ms).sum::<f32>() / 1000.0;
        let bytes: u64 = frames.iter().map(|f| f.bytes).sum();
        Report {
            label: self.plan.label.clone(),
            busy_panes: self.plan.busy,
            seconds,
            spells: self.plan.spells,
            frames: frames.len(),
            interval_ms: spread(|f| f.interval_ms),
            cpu_ms: spread(|f| f.cpu_ms),
            world_ms: spread(|f| f.world_ms),
            update_ms: spread(|f| f.update_ms),
            draw_ms: spread(|f| f.draw_ms),
            mb_per_s: if seconds > 0.0 {
                bytes as f32 / 1e6 / seconds
            } else {
                0.0
            },
            latency_ms: Spread::of(latencies),
            latency_samples: latencies.len(),
            frames_over_16_7_ms: frames.iter().filter(|f| f.interval_ms > 16.7).count(),
            worst_frames: {
                let mut worst = frames.to_vec();
                worst.sort_by(|a, b| b.interval_ms.total_cmp(&a.interval_ms));
                worst.truncate(5);
                worst
            },
        }
    }

    /// Removes the scratch directory.
    pub fn clean(&self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
