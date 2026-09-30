//! Bounded local performance samples. Records contain no application content.
use std::collections::VecDeque;
use std::io::Write;
use std::sync::mpsc::{SyncSender, sync_channel};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Input,
    Tick,
    Layout,
    Paint,
    Upload,
    Acquire,
    SurfaceOccluded,
    SurfaceTimeout,
    SurfaceOutdated,
    SurfaceLost,
    Present,
    Frame,
    InputToPresent,
}

#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub phase: Phase,
    pub micros: u64,
    pub pixels: u64,
    pub regions: usize,
}

/// One submitted native frame. Durations exclude GPU completion and scanout.
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct FrameTiming {
    pub total_us: u64,
    pub paint_us: u64,
    pub upload_us: u64,
    pub acquire_us: u64,
    pub present_us: u64,
    pub damaged_pixels: u64,
    pub regions: usize,
}

/// A native surface that cannot supply a frame. No pixels are uploaded.
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct FrameSkip {
    pub reason: SkipReason,
    pub acquire_us: u64,
}

#[derive(Clone, Copy, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    Occluded,
    Timeout,
    Outdated,
    Lost,
}

/// Keeps the last 512 samples. An optional bounded writer never blocks rendering.
pub struct Timings {
    samples: VecDeque<Sample>,
    writer: Option<SyncSender<Sample>>,
    pub dropped: u64,
    input: Option<Instant>,
}

impl Default for Timings {
    fn default() -> Self {
        Self {
            samples: VecDeque::with_capacity(512),
            writer: None,
            dropped: 0,
            input: None,
        }
    }
}

impl Timings {
    /// Opt in with `OPENAGENTS_DESKTOP_TIMINGS=/absolute/path.jsonl`.
    /// File opening and writes run on a background thread; nothing is sent remotely.
    pub fn from_env() -> Self {
        let mut timings = Self::default();
        if let Some(path) = std::env::var_os("OPENAGENTS_DESKTOP_TIMINGS") {
            let (send, receive) = sync_channel::<Sample>(512);
            let result = std::thread::Builder::new()
                .name("desktop-timings".into())
                .spawn(move || {
                    let mut options = std::fs::OpenOptions::new();
                    options.create(true).append(true);
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::OpenOptionsExt;
                        options.mode(0o600);
                    }
                    let Ok(mut file) = options.open(path) else {
                        return;
                    };
                    while let Ok(sample) = receive.recv() {
                        if writeln!(
                            file,
                            "{{\"phase\":\"{:?}\",\"us\":{},\"pixels\":{},\"regions\":{}}}",
                            sample.phase, sample.micros, sample.pixels, sample.regions
                        )
                        .is_err()
                        {
                            break;
                        }
                    }
                });
            if result.is_ok() {
                timings.writer = Some(send);
            }
        }
        timings
    }
    pub fn samples(&self) -> &VecDeque<Sample> {
        &self.samples
    }
    pub fn input(&mut self, now: Instant) {
        self.input.get_or_insert(now);
    }
    pub fn presented(&mut self) {
        if let Some(input) = self.input.take() {
            self.record(Phase::InputToPresent, input.elapsed(), 0, 0);
        }
    }
    pub fn record(&mut self, phase: Phase, duration: Duration, pixels: u64, regions: usize) {
        let sample = Sample {
            phase,
            micros: duration.as_micros().min(u64::MAX as u128) as u64,
            pixels,
            regions,
        };
        if self.samples.len() == 512 {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
        if let Some(writer) = &self.writer
            && writer.try_send(sample).is_err()
        {
            self.dropped = self.dropped.saturating_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn samples_and_blocked_writer_stay_bounded() {
        let (writer, _receiver) = sync_channel(1);
        let mut timings = Timings {
            writer: Some(writer),
            ..Timings::default()
        };
        for _ in 0..1000 {
            timings.record(Phase::Paint, Duration::from_micros(8), 40, 1);
        }
        assert_eq!(timings.samples().len(), 512);
        assert_eq!(timings.dropped, 999);
    }
}
