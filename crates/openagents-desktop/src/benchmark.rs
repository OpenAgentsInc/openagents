//! Offline native performance fixture. Never reaches the owner's host or home.
use crate::shell::DesktopApp;
use openagents_chat::service::Snapshot;
use openagents_desktop::chat_action::Action as ChatAction;
use openagents_desktop::model::Intent;
use rust_native::ValidatedView;
use rust_native_desktop::timing::{FrameSkip, FrameTiming};
use rust_native_desktop::{App, Frame, PxRect, Theme, WindowLayout};
use serde::Serialize;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Warm,
    Scroll,
    Streaming,
    Sidebar,
    Composer,
    Commands,
    ChatMenu,
    Idle,
}
impl Phase {
    fn finished(self, elapsed: Duration, steps: usize, frames: usize) -> bool {
        match self {
            Self::Warm => elapsed >= Duration::from_secs(1),
            Self::Idle => elapsed >= Duration::from_secs(5),
            _ => (steps >= 120 && frames >= 120) || elapsed >= Duration::from_secs(10),
        }
    }
    fn next(self) -> Option<Self> {
        match self {
            Self::Warm => Some(Self::Scroll),
            Self::Scroll => Some(Self::Streaming),
            Self::Streaming => Some(Self::Sidebar),
            Self::Sidebar => Some(Self::Composer),
            Self::Composer => Some(Self::Commands),
            Self::Commands => Some(Self::ChatMenu),
            Self::ChatMenu => Some(Self::Idle),
            Self::Idle => None,
        }
    }
}

#[derive(Clone, Copy, Default, Serialize)]
struct Resources {
    cpu_seconds: Option<f64>,
    peak_rss_bytes: Option<u64>,
}
fn resources() -> Resources {
    #[cfg(unix)]
    {
        let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
        // The operating system writes one rusage value for this process.
        if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } == 0 {
            let usage = unsafe { usage.assume_init() };
            let cpu_seconds = (usage.ru_utime.tv_sec + usage.ru_stime.tv_sec) as f64
                + (usage.ru_utime.tv_usec + usage.ru_stime.tv_usec) as f64 / 1_000_000.0;
            let rss = usage.ru_maxrss.max(0) as u64;
            return Resources {
                cpu_seconds: Some(cpu_seconds),
                peak_rss_bytes: Some(if cfg!(target_os = "macos") {
                    rss
                } else {
                    rss * 1024
                }),
            };
        }
    }
    Resources::default()
}
#[derive(Clone, Serialize)]
struct Sample {
    phase: Phase,
    frame: FrameTiming,
    step_us: u64,
    rows: usize,
    visible_rows: usize,
    relaid: usize,
}
#[derive(Serialize)]
struct Skipped {
    phase: Phase,
    frame: FrameSkip,
}
#[derive(Serialize)]
struct PhaseResult {
    phase: Phase,
    steps: usize,
    submitted_frames: usize,
    seconds: f64,
    cpu_percent: Option<f64>,
    peak_rss_bytes: Option<u64>,
}
#[derive(Serialize)]
struct Report {
    schema: &'static str,
    complete: bool,
    platform: &'static str,
    points: (f32, f32),
    scale: f32,
    backdrop: bool,
    rows: usize,
    chats: usize,
    samples: Vec<Sample>,
    /// First submitted frames after opening the command palette and chat menu.
    openings: Vec<Sample>,
    skipped: Vec<Skipped>,
    skipped_dropped: u64,
    phases: Vec<PhaseResult>,
}
fn check_coverage(samples: &[Sample], openings: &[Sample]) -> Result<(), String> {
    for (phase, minimum) in [
        (Phase::Scroll, 90),
        (Phase::Streaming, 90),
        (Phase::Sidebar, 90),
        (Phase::Composer, 90),
        (Phase::Commands, 90),
        (Phase::ChatMenu, 20),
    ] {
        let count = samples
            .iter()
            .filter(|sample| sample.phase == phase)
            .count();
        if count < minimum {
            return Err(format!(
                "incomplete native benchmark: {phase:?} has {count} submitted samples; need {minimum}"
            ));
        }
    }
    if ![Phase::Commands, Phase::ChatMenu]
        .into_iter()
        .all(|phase| openings.iter().any(|sample| sample.phase == phase))
    {
        return Err("incomplete native benchmark: missing first menu frames".into());
    }
    Ok(())
}

struct Fixture {
    app: DesktopApp,
    snapshot: Snapshot,
    phase: Phase,
    started: Instant,
    phase_started: Instant,
    phase_cpu: Resources,
    next: Instant,
    steps: usize,
    frames: usize,
    last_step_us: u64,
    samples: Vec<Sample>,
    openings: Vec<Sample>,
    skipped: Vec<Skipped>,
    skipped_dropped: u64,
    results: Vec<PhaseResult>,
    done: bool,
    sidebar: f32,
}
impl Fixture {
    fn new() -> Self {
        let now = Instant::now();
        let (app, snapshot) = DesktopApp::performance_fixture(3300, 500, now);
        Self {
            app,
            snapshot,
            phase: Phase::Warm,
            started: now,
            phase_started: now,
            phase_cpu: resources(),
            next: now,
            steps: 0,
            frames: 0,
            last_step_us: 0,
            samples: Vec::with_capacity(500),
            openings: Vec::with_capacity(2),
            skipped: Vec::with_capacity(512),
            skipped_dropped: 0,
            results: vec![],
            done: false,
            sidebar: 0.0,
        }
    }
    fn advance(&mut self, now: Instant) {
        let resource = resources();
        let seconds = now.duration_since(self.phase_started).as_secs_f64();
        self.results.push(PhaseResult {
            phase: self.phase,
            steps: self.steps,
            submitted_frames: self.frames,
            seconds,
            cpu_percent: resource
                .cpu_seconds
                .zip(self.phase_cpu.cpu_seconds)
                .map(|(after, before)| (after - before) / seconds.max(0.001) * 100.0),
            peak_rss_bytes: resource.peak_rss_bytes,
        });
        if let Some(phase) = self.phase.next() {
            self.phase = phase;
            self.phase_started = now;
            self.phase_cpu = resource;
            self.steps = 0;
            self.frames = 0;
            match phase {
                Phase::Commands => self.app.activate(
                    Intent::Chat {
                        action: ChatAction::Palette,
                    },
                    now,
                ),
                Phase::ChatMenu => self.app.activate(
                    Intent::Chat {
                        action: ChatAction::Menu,
                    },
                    now,
                ),
                Phase::Idle => self.app.activate(
                    Intent::Chat {
                        action: ChatAction::DismissOverlay,
                    },
                    now,
                ),
                _ => {}
            }
            if matches!(phase, Phase::Idle) {
                self.app.text_input(
                    rust_native_desktop::input::TextInput::Commit(&"draft line\n".repeat(1000)),
                    now,
                );
            }
        } else {
            self.done = true;
        }
    }
}
impl App for Fixture {
    type Intent = Intent;
    fn title(&self) -> String {
        "OpenAgents — offline performance fixture".into()
    }
    fn theme(&self) -> Theme {
        self.app.theme()
    }
    fn window_layout(&self) -> WindowLayout {
        let mut layout = self.app.window_layout();
        if let WindowLayout::Split(ref mut split)
        | WindowLayout::HeaderSplit { ref mut split, .. } = layout
        {
            split.leading_width = 280.0;
        }
        layout
    }
    fn view(&self) -> &ValidatedView<Intent> {
        self.app.view()
    }
    fn overlay_layout(&self) -> Option<rust_native_desktop::OverlayLayout> {
        self.app.overlay_layout()
    }
    fn activate(&mut self, _: Intent, _: Instant) {}
    fn start(&mut self, _: rust_native_desktop::Waker) {}
    fn tick(&mut self, now: Instant) -> Option<Instant> {
        if self.done {
            return None;
        }
        if now < self.next {
            return Some(self.next);
        }
        let start = Instant::now();
        if self.phase.finished(
            now.duration_since(self.phase_started),
            self.steps,
            self.frames,
        ) {
            self.advance(now);
        }
        if self.done {
            return None;
        }
        match self.phase {
            Phase::Scroll => self.app.performance_scroll(48.0, now),
            Phase::Streaming => {
                self.snapshot.busy = true;
                self.snapshot.partial.push_str("Next streamed word. ");
                // Synthetic source publication time: every sample reaches the shared read path.
                self.app.performance_stream(
                    self.snapshot.clone(),
                    self.started + Duration::from_secs(self.steps as u64 + 10),
                );
            }
            Phase::Sidebar => self.sidebar += 30.0,
            Phase::Composer => {
                self.app
                    .text_input(rust_native_desktop::input::TextInput::Commit(" x"), now);
            }
            Phase::Commands => {
                self.app.text_input(
                    rust_native_desktop::input::TextInput::Key {
                        key: "a",
                        text: None,
                        command: true,
                        alt: false,
                        shift: false,
                    },
                    now,
                );
                self.app.text_input(
                    rust_native_desktop::input::TextInput::Commit(
                        ["", "saved", "chat", "new"][self.steps % 4],
                    ),
                    now,
                );
            }
            Phase::ChatMenu => {
                self.app.text_input(
                    rust_native_desktop::input::TextInput::Key {
                        key: "ArrowDown",
                        text: None,
                        command: false,
                        alt: false,
                        shift: false,
                    },
                    now,
                );
            }
            Phase::Warm | Phase::Idle => self.app.performance_idle(),
        }
        self.steps += 1;
        self.last_step_us = start.elapsed().as_micros() as u64;
        self.next = now
            + if matches!(self.phase, Phase::Idle) {
                Duration::from_secs(1)
            } else {
                Duration::from_millis(16)
            };
        Some(self.next)
    }
    fn leading_scroll(&self) -> Option<f32> {
        Some(self.sidebar)
    }
    fn viewport(&mut self, w: f32, h: f32, scale: f32) {
        self.app.viewport(w, h, scale)
    }
    fn surface_size(&self, r: &str, a: f32) -> Option<(f32, f32)> {
        self.app.surface_size(r, a)
    }
    fn surface_version(&self, r: &str) -> Option<u64> {
        self.app.surface_version(r)
    }
    fn paint_surface(&mut self, r: &str, f: &mut Frame, p: PxRect) {
        self.app.paint_surface(r, f, p)
    }
    fn frame_presented(&mut self, frame: FrameTiming) {
        let (rows, visible_rows, relaid) = self.app.performance_counts();
        let sample = Sample {
            phase: self.phase,
            frame,
            step_us: self.last_step_us,
            rows,
            visible_rows,
            relaid,
        };
        if self.frames == 0
            && matches!(self.phase, Phase::Commands | Phase::ChatMenu)
            && self.openings.len() < 2
        {
            self.openings.push(sample.clone());
        }
        if (5..125).contains(&self.frames) && !matches!(self.phase, Phase::Warm | Phase::Idle) {
            self.samples.push(sample);
        }
        self.frames += 1;
        self.last_step_us = 0;
    }
    fn frame_skipped(&mut self, frame: FrameSkip) {
        if self.skipped.len() < 512 {
            self.skipped.push(Skipped {
                phase: self.phase,
                frame,
            });
        } else {
            self.skipped_dropped += 1;
        }
    }
    fn exit_requested(&self) -> bool {
        self.done
    }
}

pub fn run(directory: &Path, minimum: bool, scale: f32, backdrop: bool) -> Result<(), String> {
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let fixture = Fixture::new();
    let report = std::sync::Arc::new(std::sync::Mutex::new(None));
    let app = Reporting {
        fixture,
        report: report.clone(),
        points: if minimum {
            (760.0, 540.0)
        } else {
            (1200.0, 840.0)
        },
        scale,
        backdrop,
    };
    let points = app.points;
    let options = rust_native_desktop::window::Options {
        pixel_size: Some(((points.0 * scale) as u32, (points.1 * scale) as u32)),
        render_scale: Some(scale),
        fixture_frontmost: true,
        min_size: (1.0, 1.0),
        ..Default::default()
    };
    #[cfg(not(windows))]
    if backdrop {
        // Refused loopback connection; render the real empty Verse, with no public relay.
        rust_native_desktop::window::run_with_backdrop(
            app,
            options,
            Box::new(openagents_desktop::backdrop::GridBackdrop::new(
                "ws://127.0.0.1:9",
                Box::new(|| false),
            )),
        )?;
    } else {
        rust_native_desktop::window::run(app, options)?;
    }
    #[cfg(windows)]
    rust_native_desktop::window::run(app, options)?;
    let report = report
        .lock()
        .map_err(|_| "fixture report lock failed")?
        .take()
        .ok_or("fixture ended without a report")?;
    let bytes = serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?;
    std::fs::write(directory.join("native.json"), bytes).map_err(|error| error.to_string())?;
    check_coverage(&report.samples, &report.openings)?;
    println!("wrote offline native timings to {}", directory.display());
    Ok(())
}
struct Reporting {
    fixture: Fixture,
    report: std::sync::Arc<std::sync::Mutex<Option<Report>>>,
    points: (f32, f32),
    scale: f32,
    backdrop: bool,
}
impl Drop for Reporting {
    fn drop(&mut self) {
        *self
            .report
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(Report {
            schema: "openagents.desktop.performance.v1",
            complete: check_coverage(&self.fixture.samples, &self.fixture.openings).is_ok(),
            platform: std::env::consts::OS,
            points: self.points,
            scale: self.scale,
            backdrop: self.backdrop,
            rows: 3300,
            chats: 500,
            samples: std::mem::take(&mut self.fixture.samples),
            openings: std::mem::take(&mut self.fixture.openings),
            skipped: std::mem::take(&mut self.fixture.skipped),
            skipped_dropped: self.fixture.skipped_dropped,
            phases: std::mem::take(&mut self.fixture.results),
        });
    }
}

impl App for Reporting {
    type Intent = Intent;
    fn title(&self) -> String {
        self.fixture.title()
    }
    fn theme(&self) -> Theme {
        self.fixture.theme()
    }
    fn window_layout(&self) -> WindowLayout {
        self.fixture.window_layout()
    }
    fn view(&self) -> &ValidatedView<Intent> {
        self.fixture.view()
    }
    fn overlay_layout(&self) -> Option<rust_native_desktop::OverlayLayout> {
        self.fixture.overlay_layout()
    }
    fn activate(&mut self, i: Intent, n: Instant) {
        self.fixture.activate(i, n)
    }
    fn tick(&mut self, n: Instant) -> Option<Instant> {
        self.fixture.tick(n)
    }
    fn leading_scroll(&self) -> Option<f32> {
        self.fixture.leading_scroll()
    }
    fn viewport(&mut self, w: f32, h: f32, s: f32) {
        self.fixture.viewport(w, h, s)
    }
    fn surface_size(&self, r: &str, a: f32) -> Option<(f32, f32)> {
        self.fixture.surface_size(r, a)
    }
    fn surface_version(&self, r: &str) -> Option<u64> {
        self.fixture.surface_version(r)
    }
    fn paint_surface(&mut self, r: &str, f: &mut Frame, p: PxRect) {
        self.fixture.paint_surface(r, f, p)
    }
    fn frame_presented(&mut self, t: FrameTiming) {
        self.fixture.frame_presented(t)
    }
    fn frame_skipped(&mut self, t: FrameSkip) {
        self.fixture.frame_skipped(t)
    }
    fn exit_requested(&self) -> bool {
        self.fixture.exit_requested()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extra_backdrop_frames_cannot_starve_later_phases() {
        let mut fixture = Fixture::new();
        for phase in [
            Phase::Scroll,
            Phase::Streaming,
            Phase::Sidebar,
            Phase::Composer,
            Phase::Commands,
            Phase::ChatMenu,
        ] {
            fixture.phase = phase;
            fixture.frames = 0;
            for _ in 0..1000 {
                fixture.frame_presented(FrameTiming::default());
            }
            assert_eq!(
                fixture
                    .samples
                    .iter()
                    .filter(|sample| sample.phase == phase)
                    .count(),
                120
            );
        }
        assert_eq!(fixture.samples.len(), 720);
        assert!(check_coverage(&fixture.samples, &fixture.openings).is_ok());
    }

    #[test]
    fn phases_wait_for_submitted_frames_and_bound_occlusion() {
        for phase in [
            Phase::Scroll,
            Phase::Streaming,
            Phase::Sidebar,
            Phase::Composer,
            Phase::Commands,
            Phase::ChatMenu,
        ] {
            assert!(!phase.finished(Duration::from_secs(2), 120, 36));
            assert!(!phase.finished(Duration::from_secs(2), 36, 120));
            assert!(phase.finished(Duration::from_secs(3), 150, 120));
            assert!(!phase.finished(Duration::from_secs(9), 600, 0));
            assert!(phase.finished(Duration::from_secs(10), 600, 0));
        }
        assert!(!Phase::Warm.finished(Duration::from_millis(999), 120, 120));
        assert!(Phase::Warm.finished(Duration::from_secs(1), 0, 0));
        assert!(!Phase::Idle.finished(Duration::from_secs(4), 120, 120));
        assert!(Phase::Idle.finished(Duration::from_secs(5), 0, 0));
    }

    fn sample(phase: Phase) -> Sample {
        Sample {
            phase,
            frame: FrameTiming::default(),
            step_us: 0,
            rows: 3300,
            visible_rows: 10,
            relaid: 0,
        }
    }

    #[test]
    fn coverage_refuses_occluded_runs_and_missing_opening_frames() {
        let mut samples = vec![];
        for phase in [
            Phase::Scroll,
            Phase::Streaming,
            Phase::Sidebar,
            Phase::Composer,
            Phase::Commands,
            Phase::ChatMenu,
        ] {
            samples.extend((0..115).map(|_| sample(phase)));
        }
        let openings = vec![sample(Phase::Commands), sample(Phase::ChatMenu)];
        assert!(check_coverage(&samples, &openings).is_ok());
        assert!(check_coverage(&samples, &[]).is_err());
        samples.retain(|sample| sample.phase != Phase::Sidebar);
        assert!(
            check_coverage(&samples, &openings)
                .unwrap_err()
                .contains("Sidebar has 0")
        );
    }
}
