//! Windowed render-lab runner (#12600): presents the lab into a native window
//! through `af-gpu`'s `SurfaceSource` path (a `CAMetalLayer` on macOS, a
//! Wayland or Xlib surface on Linux) and records frame pacing.
//!
//! ```text
//! render-lab-window [--width 1920] [--height 1080] [--frames 600] [--warmup 120]
//!                   [--backend vulkan|metal|...] [--suite compare|quick|heavy|all]
//!                   [--config '{"path":"gpu"}'] [--present fifo,immediate,mailbox]
//! ```
//!
//! For each case and present mode it prints one JSON line with the pacing
//! summary (the interval between consecutive presented frames), the GPU
//! timings, and the final `Stats`. A mode the surface lacks falls back to
//! `fifo`; the line names the mode in effect. `--width` and `--height` are
//! the requested size in physical pixels; the line reports the size the window
//! system granted.

use af_gpu::{PresentMode, SurfaceSource};
use af_render_lab::bench::{config_from, median, percentile, suite};
use af_render_lab::capi::{HostOptions, NativeLab};
use serde_json::{Value, json};
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use winit::window::{Window, WindowAttributes, WindowId, WindowLevel};

struct Args {
    width: u32,
    height: u32,
    frames: u32,
    warmup: u32,
    backend: Option<String>,
    suite: String,
    config: Option<String>,
    present: Vec<PresentMode>,
}

fn args() -> Args {
    let mut a = Args { width: 1920, height: 1080, frames: 600, warmup: 120, backend: None, suite: "quick".into(), config: None, present: vec![PresentMode::Fifo, PresentMode::Immediate] };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || it.next().unwrap_or_else(|| panic!("{flag} needs a value"));
        match flag.as_str() {
            "--width" => a.width = value().parse().expect("--width"),
            "--height" => a.height = value().parse().expect("--height"),
            "--frames" => a.frames = value().parse().expect("--frames"),
            "--warmup" => a.warmup = value().parse().expect("--warmup"),
            "--backend" => a.backend = Some(value()),
            "--suite" => a.suite = value(),
            "--config" => a.config = Some(value()),
            "--present" => a.present = value().split(',').map(|m| PresentMode::parse(m.trim()).unwrap_or_else(|| panic!("unknown present mode {m}"))).collect(),
            "-h" | "--help" => {
                eprintln!("{}", include_str!("render-lab-window.rs").lines().take_while(|l| l.starts_with("//!")).map(|l| l.trim_start_matches("//!")).collect::<Vec<_>>().join("\n"));
                std::process::exit(0);
            }
            other => panic!("unknown argument {other}"),
        }
    }
    a
}

/// One case in one present mode, in progress.
struct Run {
    plan: usize,
    mode: PresentMode,
    frame: u32,
    started: Option<Instant>,
    last: Option<Instant>,
    intervals: Vec<f64>,
    cpu: Vec<f64>,
    gpu: Vec<f64>,
    /// The lab's error list when the run started (it keeps the last 20).
    errors_before: Vec<String>,
}

struct Runner {
    a: Args,
    plan: Vec<(String, Value, PresentMode)>,
    window: Option<Window>,
    #[cfg(target_os = "macos")]
    layer: Option<raw_window_metal::Layer>,
    lab: Option<NativeLab>,
    run: Option<Run>,
    size: [u32; 2],
    refresh_hz: Option<f64>,
    failed: bool,
}

impl Runner {
    fn surface(&mut self, window: &Window) -> Result<SurfaceSource, String> {
        let handle = window.window_handle().map_err(|e| e.to_string())?.as_raw();
        let display = window.display_handle().map_err(|e| e.to_string())?.as_raw();
        match (handle, display) {
            #[cfg(target_os = "macos")]
            (RawWindowHandle::AppKit(h), _) => {
                // SAFETY: the view belongs to `window`, which outlives the layer.
                let layer = unsafe { raw_window_metal::Layer::from_ns_view(h.ns_view) };
                let source = SurfaceSource::MetalLayer(layer.as_ptr().as_ptr());
                self.layer = Some(layer);
                Ok(source)
            }
            (RawWindowHandle::Wayland(h), RawDisplayHandle::Wayland(d)) => Ok(SurfaceSource::Wayland { display: d.display.as_ptr(), surface: h.surface.as_ptr() }),
            (RawWindowHandle::Xlib(h), RawDisplayHandle::Xlib(d)) => {
                let display = d.display.ok_or("Xlib display handle without a display")?;
                // `Window` is a C `unsigned long`, narrower than u64 on some targets.
                #[allow(clippy::unnecessary_cast)]
                let window = h.window as u64;
                Ok(SurfaceSource::Xlib { display: display.as_ptr(), window })
            }
            (h, d) => Err(format!("unsupported window system: {h:?} / {d:?}")),
        }
    }

    fn start_run(&mut self, plan: usize) {
        let (name, overrides, wanted) = &self.plan[plan];
        let lab = self.lab.as_mut().unwrap();
        lab.lab().set_config(config_from(overrides));
        let mode = lab.lab().gpu().set_present_mode(*wanted).unwrap_or(PresentMode::Fifo);
        if mode != *wanted {
            eprintln!("[{name}] {} unsupported; using {}", wanted.name(), mode.name());
        }
        let errors_before = lab.lab().stats().errors.clone();
        self.run = Some(Run { plan, mode, frame: 0, started: None, last: None, intervals: Vec::new(), cpu: Vec::new(), gpu: Vec::new(), errors_before });
    }

    /// Renders one frame of the current run; returns false when every run is done.
    fn step(&mut self) -> bool {
        let Some(run) = self.run.as_mut() else { return false };
        let lab = self.lab.as_mut().unwrap().lab();
        let warmup = self.a.warmup;
        let t = (run.frame as f64) * 16.7;
        let stats = lab.frame(t);
        let now = Instant::now();
        if run.frame == warmup {
            run.started = Some(now);
        } else if run.frame > warmup {
            if let Some(last) = run.last {
                run.intervals.push((now - last).as_secs_f64() * 1000.0);
            }
            run.cpu.push(stats.cpu_frame_ms);
            if stats.gpu_timestamps && stats.gpu_epoch == stats.epoch && stats.gpu.total > 0.0 {
                run.gpu.push(stats.gpu.total);
            }
        }
        run.last = Some(now);
        run.frame += 1;
        if run.frame <= warmup + self.a.frames {
            return true;
        }
        let run = self.run.take().unwrap();
        let wall = (now - run.started.unwrap()).as_secs_f64() * 1000.0;
        let lab = self.lab.as_mut().unwrap().lab();
        lab.gpu().wait_idle();
        let stats = lab.frame(t + 16.7).clone();
        let (name, _, wanted) = &self.plan[run.plan];
        let n = run.intervals.len().max(1) as f64;
        let mean = run.intervals.iter().sum::<f64>() / n;
        let sd = (run.intervals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n).sqrt();
        let p50 = median(run.intervals.clone());
        let hitches = run.intervals.iter().filter(|&&v| v > p50 * 1.5).count();
        // Errors raised during this run (the lab's list is cumulative).
        let skip = run.errors_before.len().min(stats.errors.len());
        let new_errors = if stats.errors.len() < 20 { stats.errors[skip..].to_vec() } else { stats.errors.clone() };
        let result = json!({
            "case": name,
            "present_mode": run.mode.name(),
            "present_mode_requested": wanted.name(),
            "frames": self.a.frames,
            "window": {"size": self.size, "refresh_hz": self.refresh_hz},
            "summary": {
                "frame_ms": wall / self.a.frames as f64,
                "fps": self.a.frames as f64 * 1000.0 / wall,
                "interval_p50_ms": p50,
                "interval_p95_ms": percentile(run.intervals.clone(), 0.95),
                "interval_p99_ms": percentile(run.intervals.clone(), 0.99),
                "interval_max_ms": run.intervals.iter().cloned().fold(0.0, f64::max),
                "interval_sd_ms": sd,
                "hitches_over_1_5x_p50": hitches,
                "cpu_frame_ms": median(run.cpu),
                "gpu_total_ms": median(run.gpu.clone()),
                "gpu_total_p95_ms": percentile(run.gpu.clone(), 0.95),
                "gpu_samples": run.gpu.len(),
            },
            "run_errors": new_errors,
            "stats": stats,
        });
        let s = &result["summary"];
        eprintln!(
            "{name:30} {:9} {:7.2} fps  interval p50 {:6.3} p95 {:6.3} p99 {:6.3} max {:6.3} ms  hitches {:3}  gpu {:6.3} ms  errors {}",
            run.mode.name(),
            s["fps"].as_f64().unwrap_or(0.0),
            s["interval_p50_ms"].as_f64().unwrap_or(0.0),
            s["interval_p95_ms"].as_f64().unwrap_or(0.0),
            s["interval_p99_ms"].as_f64().unwrap_or(0.0),
            s["interval_max_ms"].as_f64().unwrap_or(0.0),
            hitches,
            s["gpu_total_ms"].as_f64().unwrap_or(0.0),
            result["run_errors"].as_array().map_or(0, Vec::len),
        );
        println!("{result}");
        if run.plan + 1 < self.plan.len() {
            self.start_run(run.plan + 1);
            true
        } else {
            false
        }
    }
}

impl ApplicationHandler for Runner {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = WindowAttributes::default()
            .with_title("render lab")
            .with_inner_size(PhysicalSize::new(self.a.width, self.a.height))
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop);
        let window = match event_loop.create_window(attributes) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("render-lab-window: no window ({e})");
                self.failed = true;
                event_loop.exit();
                return;
            }
        };
        let size = window.inner_size();
        self.size = [size.width.max(1), size.height.max(1)];
        self.refresh_hz = window.current_monitor().and_then(|m| m.refresh_rate_millihertz()).map(|mhz| mhz as f64 / 1000.0);
        let host = HostOptions { backend: self.a.backend.clone(), present_mode: Some(self.plan[0].2.name().into()), ..Default::default() };
        let result = self.surface(&window).and_then(|surface| NativeLab::with_surface(self.size[0], self.size[1], config_from(&self.plan[0].1), &host, Some(surface)));
        match result {
            Ok(mut lab) => {
                let adapter = lab.lab().stats().adapter.clone();
                let modes: Vec<&str> = lab.lab().gpu().present_modes().iter().map(|m| m.name()).collect();
                eprintln!(
                    "adapter: {} · {} · {} · {}\nwindow: {}x{} px, scale {}, refresh {:?} Hz, present modes {:?}",
                    adapter.backend, adapter.vendor, adapter.architecture, adapter.description, self.size[0], self.size[1], window.scale_factor(), self.refresh_hz, modes
                );
                self.lab = Some(lab);
                self.window = Some(window);
                self.start_run(0);
            }
            Err(e) => {
                eprintln!("render-lab-window: {e}");
                self.failed = true;
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                eprintln!("resized to {}x{}", size.width, size.height);
                self.size = [size.width.max(1), size.height.max(1)];
                if let Some(lab) = self.lab.as_mut() {
                    lab.lab().resize(self.size[0], self.size[1]);
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.lab.is_some() && !self.step() {
            event_loop.exit();
        }
    }
}

fn main() {
    let a = args();
    let cases: Vec<(String, Value)> = match &a.config {
        Some(text) => vec![("custom".into(), serde_json::from_str(text).expect("--config JSON"))],
        None => suite(&a.suite).into_iter().map(|(n, v)| (n.to_string(), v)).collect(),
    };
    let plan = cases.iter().flat_map(|(name, v)| a.present.iter().map(move |m| (name.clone(), v.clone(), *m))).collect();
    let event_loop = EventLoop::new().unwrap_or_else(|e| {
        eprintln!("render-lab-window: no event loop ({e})");
        std::process::exit(1);
    });
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut runner = Runner {
        a,
        plan,
        window: None,
        #[cfg(target_os = "macos")]
        layer: None,
        lab: None,
        run: None,
        size: [1, 1],
        refresh_hz: None,
        failed: false,
    };
    event_loop.run_app(&mut runner).expect("event loop");
    // The lab holds the surface; drop it before the layer and the window.
    runner.lab = None;
    if runner.failed {
        std::process::exit(1);
    }
}
