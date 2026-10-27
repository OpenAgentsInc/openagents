//! Native render-lab runner: renders each configuration offscreen on native
//! `webgpu.h` and prints one JSON line per case with the web lab's `Stats` and
//! summary timings.
//!
//! ```text
//! render-lab-native [--width 1920] [--height 1080] [--frames 300] [--warmup 60]
//!                   [--backend vulkan|metal|...] [--suite compare|quick|heavy|all]
//!                   [--config '{"path":"gpu"}'] [--capture DIR] [--verbose]
//! ```
//!
//! `--config` runs one case (merged over the defaults) instead of a suite.
//! `heavy` is the city-scale scene (#12600): ten times the street scene's
//! instances, five times its crowd, seen from the overview camera.
//! `--capture DIR` writes each case's last frame as a PPM image.

use af_render_lab::capi::{HostOptions, NativeLab};
use af_render_lab::bench::{config_from, median, percentile, suite};
use af_render_lab::{Config, Stats};
use serde_json::{Value, json};
use std::time::Instant;

struct Args {
    width: u32,
    height: u32,
    frames: u32,
    warmup: u32,
    backend: Option<String>,
    suite: String,
    config: Option<String>,
    capture: Option<String>,
    verbose: bool,
}

fn args() -> Args {
    let mut a = Args { width: 1920, height: 1080, frames: 300, warmup: 60, backend: None, suite: "compare".into(), config: None, capture: None, verbose: false };
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
            "--capture" => a.capture = Some(value()),
            "--verbose" => a.verbose = true,
            "-h" | "--help" => {
                eprintln!("{}", include_str!("render-lab-native.rs").lines().take_while(|l| l.starts_with("//!")).map(|l| l.trim_start_matches("//!")).collect::<Vec<_>>().join("\n"));
                std::process::exit(0);
            }
            other => panic!("unknown argument {other}"),
        }
    }
    a
}

fn write_ppm(path: &str, w: u32, h: u32, rgba: &[u8]) {
    let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
    for px in rgba.as_chunks::<4>().0 {
        out.extend_from_slice(&px[..3]);
    }
    std::fs::write(path, out).unwrap_or_else(|e| panic!("write {path}: {e}"));
}

fn run_case(native: &mut NativeLab, name: &str, config: Config, a: &Args) -> Value {
    let lab = native.lab();
    let trace = |step: &str| {
        if a.verbose {
            eprintln!("[{name}] {step}");
        }
    };
    lab.set_config(config);
    trace("configured");
    for i in 0..a.warmup {
        lab.frame(i as f64 * 16.7);
        trace(&format!("warmup frame {i}"));
    }
    lab.gpu().wait_idle();
    trace("warmed up");
    let mut cpu = Vec::new();
    let mut encode = Vec::new();
    let mut cull = Vec::new();
    let mut gpu = Vec::new();
    let mut intervals = Vec::new();
    let start = Instant::now();
    let mut last = start;
    for i in 0..a.frames {
        let stats: &Stats = lab.frame((a.warmup + i) as f64 * 16.7);
        if a.verbose {
            eprintln!("[{name}] frame {i} errors {:?}", stats.errors);
        }
        cpu.push(stats.cpu_frame_ms);
        encode.push(stats.main_encode_ms);
        cull.push(stats.cpu_cull_ms);
        if stats.gpu_timestamps && stats.gpu_epoch == stats.epoch && stats.gpu.total > 0.0 {
            gpu.push(stats.gpu.total);
        }
        let now = Instant::now();
        intervals.push((now - last).as_secs_f64() * 1000.0);
        last = now;
    }
    lab.gpu().wait_idle();
    let wall_ms = start.elapsed().as_secs_f64() * 1000.0;
    // A last frame after the drain, so counters and GPU timings are current.
    lab.gpu().wait_idle();
    let stats = lab.frame((a.warmup + a.frames) as f64 * 16.7).clone();
    if let Some(dir) = &a.capture
        && let Some((w, h, rgba)) = lab.capture() {
            std::fs::create_dir_all(dir).ok();
            write_ppm(&format!("{dir}/{name}.ppm"), w, h, &rgba);
        }
    json!({
        "case": name,
        "frames": a.frames,
        "summary": {
            "frame_ms": wall_ms / a.frames as f64,
            "fps": a.frames as f64 * 1000.0 / wall_ms,
            "interval_p95_ms": percentile(intervals, 0.95),
            "cpu_frame_ms": median(cpu),
            "main_encode_ms": median(encode),
            "cpu_cull_ms": median(cull),
            "gpu_total_ms": median(gpu.clone()),
            "gpu_total_p95_ms": percentile(gpu.clone(), 0.95),
            "gpu_samples": gpu.len(),
        },
        "stats": stats,
    })
}

fn main() {
    let a = args();
    let cases: Vec<(String, Value)> = match &a.config {
        Some(text) => vec![("custom".into(), serde_json::from_str(text).expect("--config JSON"))],
        None => suite(&a.suite).into_iter().map(|(n, v)| (n.to_string(), v)).collect(),
    };
    let host = HostOptions { backend: a.backend.clone(), ..Default::default() };
    let first = config_from(&cases[0].1);
    let mut native = NativeLab::new(a.width, a.height, first, &host).unwrap_or_else(|e| {
        eprintln!("render-lab-native: {e}");
        std::process::exit(1);
    });
    let adapter = native.lab().stats().adapter.clone();
    eprintln!("adapter: {} · {} · {} · {}", adapter.backend, adapter.vendor, adapter.architecture, adapter.description);
    for (name, overrides) in &cases {
        let result = run_case(&mut native, name, config_from(overrides), &a);
        let s = &result["summary"];
        eprintln!(
            "{name:32} frame {:7.3} ms  gpu {:7.3} ms  cpu {:6.3} ms  encode {:6.3} ms  draws {}  errors {}",
            s["frame_ms"].as_f64().unwrap_or(0.0),
            s["gpu_total_ms"].as_f64().unwrap_or(0.0),
            s["cpu_frame_ms"].as_f64().unwrap_or(0.0),
            s["main_encode_ms"].as_f64().unwrap_or(0.0),
            result["stats"]["commands"]["draws"],
            result["stats"]["errors"].as_array().map_or(0, Vec::len),
        );
        println!("{result}");
    }
}
