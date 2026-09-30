//! Offline native performance fixture. Never reaches the owner's host or home.
use crate::shell::DesktopApp;
use openagents_chat::service::Snapshot;
use openagents_desktop::model::Intent;
use rust_native::ValidatedView;
use rust_native_desktop::timing::FrameTiming;
use rust_native_desktop::{App, Frame, PxRect, Theme, WindowLayout};
use serde::Serialize;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Warm,
    Scroll,
    Streaming,
    Sidebar,
    Idle,
}
impl Phase {
    fn next(self) -> Option<Self> {
        match self {
            Self::Warm => Some(Self::Scroll),
            Self::Scroll => Some(Self::Streaming),
            Self::Streaming => Some(Self::Sidebar),
            Self::Sidebar => Some(Self::Idle),
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
#[derive(Serialize)]
struct Sample {
    phase: Phase,
    frame: FrameTiming,
    step_us: u64,
    rows: usize,
    visible_rows: usize,
    relaid: usize,
}
#[derive(Serialize)]
struct PhaseResult {
    phase: Phase,
    seconds: f64,
    cpu_percent: Option<f64>,
    peak_rss_bytes: Option<u64>,
}
#[derive(Serialize)]
struct Report {
    schema: &'static str,
    platform: &'static str,
    points: (f32, f32),
    scale: f32,
    backdrop: bool,
    rows: usize,
    chats: usize,
    samples: Vec<Sample>,
    phases: Vec<PhaseResult>,
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
        if let WindowLayout::Split(ref mut split) = layout {
            split.leading_width = 280.0;
        }
        layout
    }
    fn view(&self) -> &ValidatedView<Intent> {
        self.app.view()
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
        if matches!(self.phase, Phase::Warm)
            && now.duration_since(self.phase_started) > Duration::from_secs(1)
            || matches!(self.phase, Phase::Idle)
                && now.duration_since(self.phase_started) > Duration::from_secs(5)
            || !matches!(self.phase, Phase::Warm | Phase::Idle) && self.steps >= 120
        {
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
        if self.frames >= 5 && !matches!(self.phase, Phase::Warm) && self.samples.len() < 600 {
            self.samples.push(Sample {
                phase: self.phase,
                frame,
                step_us: self.last_step_us,
                rows,
                visible_rows,
                relaid,
            });
        }
        self.frames += 1;
        self.last_step_us = 0;
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
    if report.samples.is_empty() {
        return Err("no presented frames were measured".into());
    }
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
            platform: std::env::consts::OS,
            points: self.points,
            scale: self.scale,
            backdrop: self.backdrop,
            rows: 3300,
            chats: 500,
            samples: std::mem::take(&mut self.fixture.samples),
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
    fn exit_requested(&self) -> bool {
        self.fixture.exit_requested()
    }
}
