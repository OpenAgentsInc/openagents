//! The overlay's performance instruments: how a frame's time splits between
//! the world and the terminals, how fast each pane's output parses, and how
//! long a key takes to show as a glyph.
//!
//! The prefix, then `?`, shows the numbers in the overlay's header, and
//! while they show the overlay logs one JSON line a second to standard
//! error. A stress run ([`super::stress`]) records every frame instead.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// One frame's time, in milliseconds.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize)]
pub struct Frame {
    /// Since the previous frame finished: what the player sees.
    pub interval_ms: f32,
    /// The frame's CPU time on the main thread, from its start to the
    /// renderer's return.
    pub cpu_ms: f32,
    /// The world: everything on the main thread but the terminal overlay.
    pub world_ms: f32,
    /// Applying the panes' output to their grids.
    pub update_ms: f32,
    /// Building the overlay's vertices.
    pub draw_ms: f32,
    /// Output bytes the panes parsed this frame.
    pub bytes: u64,
}

/// Percentiles of a set of milliseconds.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize)]
pub struct Spread {
    pub median: f32,
    pub p95: f32,
    pub p99: f32,
    pub worst: f32,
}

impl Spread {
    #[must_use]
    pub fn of(values: &[f32]) -> Spread {
        if values.is_empty() {
            return Spread::default();
        }
        let mut sorted = values.to_vec();
        sorted.sort_by(f32::total_cmp);
        let at = |q: f32| {
            let index = ((sorted.len() as f32 - 1.0) * q).round() as usize;
            sorted[index.min(sorted.len() - 1)]
        };
        Spread {
            median: at(0.5),
            p95: at(0.95),
            p99: at(0.99),
            worst: sorted[sorted.len() - 1],
        }
    }
}

/// A second's summary, for the stats line and the log.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct Summary {
    pub frames: usize,
    pub interval: Spread,
    pub world_ms: f32,
    pub update_ms: f32,
    pub draw_ms: f32,
    /// Output parsed per second across panes, in MB.
    pub mb_per_s: f32,
    /// The busiest pane's output per second, in MB.
    pub busiest_mb_per_s: f32,
    /// How fast the emulator parses, in MB per second of parse time.
    pub parse_mb_per_s: f32,
    /// Keypress to the frame that showed its echo, in milliseconds.
    pub latency: Spread,
}

/// A key sent to a pane, waiting for its echo.
#[derive(Clone, Copy, Debug)]
struct Pending {
    at: Instant,
    pane: u64,
    generation: u64,
}

/// The overlay's instruments.
#[derive(Debug)]
pub struct Stats {
    /// The stats line shows, and the overlay logs a summary each second.
    pub shown: bool,
    /// Keep every frame and latency, for a stress run.
    pub record: bool,
    pub frames: Vec<Frame>,
    pub latencies: Vec<f32>,
    last: Option<Instant>,
    window: Vec<Frame>,
    window_latency: Vec<f32>,
    window_start: Option<Instant>,
    /// Per pane, bytes parsed this window.
    window_panes: Vec<u64>,
    window_parse: Duration,
    pub summary: Option<Summary>,
    pending: Option<Pending>,
    echoed: Option<Instant>,
    update: Duration,
    draw: Duration,
    bytes: u64,
    parse: Duration,
    panes: VecDeque<(u64, u64)>,
}

impl Default for Stats {
    fn default() -> Self {
        Stats {
            shown: false,
            record: false,
            frames: Vec::new(),
            latencies: Vec::new(),
            last: None,
            window: Vec::new(),
            window_latency: Vec::new(),
            window_start: None,
            window_panes: Vec::new(),
            window_parse: Duration::ZERO,
            summary: None,
            pending: None,
            echoed: None,
            update: Duration::ZERO,
            draw: Duration::ZERO,
            bytes: 0,
            parse: Duration::ZERO,
            panes: VecDeque::new(),
        }
    }
}

impl Stats {
    /// Whether anything measures, so the overlay skips clock reads when
    /// nothing does.
    #[must_use]
    pub fn active(&self) -> bool {
        self.shown || self.record
    }

    /// A key went to `pane`, whose grid is at `generation`. The first key
    /// waiting for its echo is the one measured.
    pub fn key_sent(&mut self, pane: u64, generation: u64) {
        if self.pending.is_none() && self.echoed.is_none() {
            self.pending = Some(Pending {
                at: Instant::now(),
                pane,
                generation,
            });
        }
    }

    /// The pane `pane` is at `generation` as this frame draws it.
    pub fn drawn(&mut self, pane: u64, generation: u64) {
        if let Some(pending) = self.pending
            && pending.pane == pane
            && generation != pending.generation
        {
            self.pending = None;
            self.echoed = Some(pending.at);
        }
    }

    /// Drops a pending key whose pane went away.
    pub fn forget(&mut self, pane: u64) {
        if self.pending.is_some_and(|p| p.pane == pane) {
            self.pending = None;
        }
    }

    pub fn add_update(&mut self, took: Duration) {
        self.update += took;
    }

    pub fn add_draw(&mut self, took: Duration) {
        self.draw += took;
    }

    /// `pane` parsed `bytes` in `took`.
    pub fn add_parse(&mut self, pane: u64, bytes: u64, took: Duration) {
        if bytes == 0 {
            return;
        }
        self.bytes += bytes;
        self.parse += took;
        self.panes.push_back((pane, bytes));
    }

    /// The frame finished: the renderer returned. `started` is when the
    /// frame began on the main thread.
    pub fn frame_done(&mut self, started: Instant) {
        let now = Instant::now();
        let interval = self.last.map_or(0.0, |last| ms(now - last));
        self.last = Some(now);
        let update = std::mem::take(&mut self.update);
        let draw = std::mem::take(&mut self.draw);
        let cpu = ms(now - started);
        let frame = Frame {
            interval_ms: interval,
            cpu_ms: cpu,
            world_ms: (cpu - ms(update) - ms(draw)).max(0.0),
            update_ms: ms(update),
            draw_ms: ms(draw),
            bytes: std::mem::take(&mut self.bytes),
        };
        let parse = std::mem::take(&mut self.parse);
        if let Some(at) = self.echoed.take() {
            let latency = ms(now - at);
            self.window_latency.push(latency);
            if self.record {
                self.latencies.push(latency);
            }
        }
        if self.record && interval > 0.0 {
            self.frames.push(frame);
        }
        if !self.shown {
            self.panes.clear();
            return;
        }
        let start = *self.window_start.get_or_insert(now);
        if interval > 0.0 {
            self.window.push(frame);
        }
        self.window_parse += parse;
        for (pane, bytes) in self.panes.drain(..) {
            let index = pane as usize;
            if self.window_panes.len() <= index {
                self.window_panes.resize(index + 1, 0);
            }
            self.window_panes[index] += bytes;
        }
        let elapsed = now - start;
        if elapsed >= Duration::from_secs(1) {
            let seconds = elapsed.as_secs_f32();
            let frames = std::mem::take(&mut self.window);
            let latency = std::mem::take(&mut self.window_latency);
            let panes = std::mem::take(&mut self.window_panes);
            let parse = std::mem::take(&mut self.window_parse);
            let bytes: u64 = frames.iter().map(|f| f.bytes).sum();
            let mean = |f: fn(&Frame) -> f32| {
                if frames.is_empty() {
                    0.0
                } else {
                    frames.iter().map(f).sum::<f32>() / frames.len() as f32
                }
            };
            let summary = Summary {
                frames: frames.len(),
                interval: Spread::of(&frames.iter().map(|f| f.interval_ms).collect::<Vec<_>>()),
                world_ms: mean(|f| f.world_ms),
                update_ms: mean(|f| f.update_ms),
                draw_ms: mean(|f| f.draw_ms),
                mb_per_s: bytes as f32 / 1e6 / seconds,
                busiest_mb_per_s: panes.iter().copied().max().unwrap_or(0) as f32 / 1e6 / seconds,
                parse_mb_per_s: if parse.is_zero() {
                    0.0
                } else {
                    bytes as f32 / 1e6 / parse.as_secs_f32()
                },
                latency: Spread::of(&latency),
            };
            if let Ok(line) = serde_json::to_string(&summary) {
                eprintln!("verse: terminal stats {line}");
            }
            self.summary = Some(summary);
            self.window_start = Some(now);
        }
    }

    /// The stats line.
    #[must_use]
    pub fn line(&self) -> String {
        let Some(s) = &self.summary else {
            return "stats: measuring…".into();
        };
        let mut line = format!(
            "{} fps · frame {:.1} p95 {:.1} worst {:.1} ms · world {:.1} · term {:.1}+{:.1} ms · {:.1} MB/s (parse {:.0} MB/s)",
            s.frames,
            s.interval.median,
            s.interval.p95,
            s.interval.worst,
            s.world_ms,
            s.update_ms,
            s.draw_ms,
            s.mb_per_s,
            s.parse_mb_per_s,
        );
        if s.latency.worst > 0.0 {
            line.push_str(&format!(
                " · key→glyph {:.1} p95 {:.1} ms",
                s.latency.median, s.latency.p95
            ));
        }
        line
    }
}

fn ms(duration: Duration) -> f32 {
    duration.as_secs_f32() * 1000.0
}
