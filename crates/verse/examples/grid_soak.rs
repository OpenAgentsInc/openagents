//! Simulated phone, desktop, and browser players for the Grid soak
//! (#10589), each joining a relay's world as its platform does and drawing
//! the Grid offscreen through the engine renderer every frame.
//!
//! ```sh
//! cargo run --release -p verse --example grid_soak -- \
//!   --relay ws://127.0.0.1:7457 --seconds 1800 --out DIR
//! ```
//!
//! Each client has a fresh in-memory key and its platform's presence
//! profile: the phone and the browser publish at
//! [`PublishIntervals::mobile`] and draw the crowd one moving interval plus
//! 300 ms in the past, as `coder-mobile` and `everglade-web` do; the
//! desktop publishes at [`PublishIntervals::desktop`] with the default
//! crowd delay. Each strafes left and right at the spawn, facing the
//! plaza, and renders at its platform's size: 1179 by 2556 (phone), 1920
//! by 1080 (desktop), and 1280 by 800 (browser).
//!
//! A frame's cost is its wall time from the session tick to a completed
//! readback, which upper-bounds what an on-screen frame costs; the
//! renderer's own GPU wait and readback copy are recorded beside it. The
//! three clients render in turn on one GPU, so the loop's interval is not
//! any client's frame time.
//!
//! `DIR/frames.ndjson` gets one line a second per client, `DIR/summary.json`
//! the totals and the frame budget verdicts, and `DIR/<client>-start.png`
//! and `DIR/<client>-end.png` the client's first and last full minute's
//! view. Exits 1 when a budget, the crowd, or a refusal fails.

use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use verse::controller::InputState;
use verse::grid_frame;
use verse::grid_pack;
use verse::identity::{self, Identity};
use verse::imported::Renderer;
use verse::runtime::WorldRuntime;
use verse::session::{PublishIntervals, Session, Status};
use verse::ui::{Atlas, UiBatch};
use verse_engine::lighting::Lighting;

/// Added to the moving interval for the phone's and browser's crowd delay.
const PRESENCE_MARGIN: Duration = Duration::from_millis(300);
/// Seconds of strafing in each direction.
const STRAFE_SECONDS: f32 = 3.0;

struct Platform {
    name: &'static str,
    width: u32,
    height: u32,
    budget_ms: f64,
    intervals: PublishIntervals,
    delayed: bool,
}

const PLATFORMS: [(&str, u32, u32, f64); 3] = [
    ("phone", 1179, 2556, 33.3),
    ("desktop", 1920, 1080, 16.7),
    ("browser", 1280, 800, 33.3),
];

struct Client {
    platform: Platform,
    runtime: WorldRuntime,
    session: Session,
    renderer: Renderer,
    lighting: Lighting,
    last: Instant,
    second: Vec<f64>,
    all: Vec<f64>,
    gpu_wait: Vec<f64>,
    readback: Vec<f64>,
    peers_min: Option<usize>,
    peers_max: usize,
    peer_samples: Vec<usize>,
    pixels: Option<Vec<u8>>,
}

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("grid_soak: {error}");
            std::process::exit(2);
        }
    }
}

fn run() -> Result<i32, String> {
    let mut relay = None;
    let mut seconds = 60u64;
    let mut out = None;
    let mut world = verse::session::BARE_WORLD;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} takes a value"));
        match arg.as_str() {
            "--relay" => relay = Some(value()?),
            "--seconds" => {
                seconds = value()?
                    .parse()
                    .map_err(|_| "--seconds takes a number".to_owned())?;
            }
            "--out" => out = Some(PathBuf::from(value()?)),
            "--world" => world = Box::leak(value()?.into_boxed_str()),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let relay = relay.ok_or("--relay is required")?;
    let out = out.ok_or("--out is required")?;
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let mut frames = std::fs::File::create(out.join("frames.ndjson")).map_err(|e| e.to_string())?;

    let atlas = Atlas::new(16.0);
    let mut clients = Vec::new();
    for (name, width, height, budget_ms) in PLATFORMS {
        let desktop = name == "desktop";
        let platform = Platform {
            name,
            width,
            height,
            budget_ms,
            intervals: if desktop {
                PublishIntervals::desktop()
            } else {
                PublishIntervals::mobile()
            },
            delayed: !desktop,
        };
        let id = Identity::from_secret(&format!("soak-{name}"), identity::random_secret())?;
        let mut session = Session::start_presence(id, &relay, world)?;
        session.set_display_name(Some(&format!("soak-{name}")));
        session.set_publish_intervals(platform.intervals)?;
        if platform.delayed {
            session
                .crowd
                .set_delay(platform.intervals.moving + PRESENCE_MARGIN);
            session.crowd.set_live_only(true);
        }
        let pack = grid_pack::load_pinned()?;
        let statics = grid_frame::statics(&pack);
        let renderer = Renderer::new(
            pack,
            &grid_pack::pinned_dir(),
            width,
            height,
            &atlas,
            &statics,
        )?;
        let runtime = WorldRuntime::bare();
        let lighting = grid_frame::lighting(&runtime.atmosphere());
        eprintln!(
            "{name}: {} at {width}x{height}, budget {budget_ms} ms",
            session.pubkey()
        );
        clients.push(Client {
            platform,
            runtime,
            session,
            renderer,
            lighting,
            last: Instant::now(),
            second: Vec::new(),
            all: Vec::new(),
            gpu_wait: Vec::new(),
            readback: Vec::new(),
            peers_min: None,
            peers_max: 0,
            peer_samples: Vec::new(),
            pixels: None,
        });
    }

    let start = Instant::now();
    let deadline = start + Duration::from_secs(seconds);
    let mut window = start;
    let mut captured_start = false;
    let ui = UiBatch::default();
    while Instant::now() < deadline {
        let elapsed = start.elapsed().as_secs_f32();
        let left = (elapsed / STRAFE_SECONDS) as u64 % 2 == 0;
        let input = InputState {
            strafe_left: left,
            strafe_right: !left,
            ..InputState::default()
        };
        for client in &mut clients {
            let began = Instant::now();
            let dt = (began - client.last).as_secs_f32().min(0.1);
            client.last = began;
            client.runtime.tick(&input, dt);
            client.session.tick_world(began, &mut client.runtime);
            let peers = client.session.crowd.figures(began, dt);
            let shown = peers
                .iter()
                .filter(|p| p.role == "avatar" && p.online)
                .count();
            let dynamic = grid_frame::dynamic(&client.runtime, &peers, &[]);
            let aspect = client.platform.width as f32 / client.platform.height as f32;
            let pixels = client.renderer.draw(
                client.runtime.view(aspect),
                &dynamic,
                &ui,
                &client.lighting,
            )?;
            let cost = began.elapsed().as_secs_f64() * 1000.0;
            client.second.push(cost);
            client.all.push(cost);
            let timings = &client.renderer.last_timings;
            client.gpu_wait.push(timings.gpu_wait_ms);
            client.readback.push(timings.readback_copy_ms);
            client.peer_samples.push(shown);
            // The crowd is counted once it has filled: after the first minute.
            if began.duration_since(start) >= Duration::from_secs(60) {
                client.peers_min = Some(client.peers_min.map_or(shown, |m| m.min(shown)));
                client.peers_max = client.peers_max.max(shown);
                client.pixels = Some(pixels);
            }
        }
        let now = Instant::now();
        if !captured_start && clients.iter().all(|client| client.pixels.is_some()) {
            captured_start = true;
            for client in &mut clients {
                if let Some(pixels) = client.pixels.take() {
                    write_png(&out, client, "start", &pixels)?;
                }
            }
        }
        if now.duration_since(window) >= Duration::from_secs(1) {
            window = now;
            for client in &mut clients {
                let mut second = std::mem::take(&mut client.second);
                second.sort_by(f64::total_cmp);
                let line = json!({
                    "at_s": now.duration_since(start).as_secs(),
                    "client": client.platform.name,
                    "frames": second.len(),
                    "p50_ms": quantile(&second, 0.5),
                    "p95_ms": quantile(&second, 0.95),
                    "max_ms": second.last().copied(),
                    "peers_shown": client.peer_samples.last().copied(),
                    "online": matches!(client.session.status, Status::Online),
                    "frames_published": client.session.frames_published(),
                    "refusals": client.session.refusals(),
                    "last_refusal": client.session.last_refusal(),
                });
                writeln!(frames, "{line}").map_err(|e| e.to_string())?;
            }
        }
    }
    let mut failed = false;
    let mut rows = Vec::new();
    for client in &mut clients {
        if let Some(pixels) = client.pixels.take() {
            write_png(&out, client, "end", &pixels)?;
        }
        let row = summary(client);
        failed |= row["pass"] != true;
        eprintln!("{}", row);
        rows.push(row);
        let runtime = &client.runtime;
        client.session.leave(&runtime.player, &runtime.agent);
    }
    let summary = json!({
        "relay": relay,
        "world": world,
        "seconds": seconds,
        "measured_s": start.elapsed().as_secs(),
        "adapter": "offscreen engine renderer, readback every frame",
        "clients": rows,
        "pass": !failed,
    });
    std::fs::write(
        out.join("summary.json"),
        serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())? + "\n",
    )
    .map_err(|e| e.to_string())?;
    Ok(i32::from(failed))
}

fn summary(client: &Client) -> Value {
    let mut all = client.all.clone();
    all.sort_by(f64::total_cmp);
    let mut gpu = client.gpu_wait.clone();
    gpu.sort_by(f64::total_cmp);
    let mut readback = client.readback.clone();
    readback.sort_by(f64::total_cmp);
    let p95 = quantile(&all, 0.95).unwrap_or(f64::INFINITY);
    let refusals = client.session.refusals();
    let last_refusal = client.session.last_refusal().map(str::to_owned);
    let rate_limited = last_refusal
        .as_deref()
        .is_some_and(|r| r.starts_with("rate-limited:"));
    let within_budget = p95 < client.platform.budget_ms;
    json!({
        "client": client.platform.name,
        "pubkey": client.session.pubkey(),
        "size": [client.platform.width, client.platform.height],
        "moving_ms": client.platform.intervals.moving.as_millis() as u64,
        "crowd_delayed": client.platform.delayed,
        "frames": all.len(),
        "frame_p50_ms": quantile(&all, 0.5),
        "frame_p95_ms": p95,
        "frame_p99_ms": quantile(&all, 0.99),
        "frame_max_ms": all.last().copied(),
        "gpu_wait_p95_ms": quantile(&gpu, 0.95),
        "readback_p95_ms": quantile(&readback, 0.95),
        "budget_ms": client.platform.budget_ms,
        "within_budget": within_budget,
        "peers_shown_min_after_60s": client.peers_min,
        "peers_shown_max_after_60s": client.peers_max,
        "frames_published": client.session.frames_published(),
        "refusals": refusals,
        "last_refusal": last_refusal,
        "rate_limited": rate_limited,
        "pass": within_budget && refusals == 0 && client.peers_min.is_some_and(|n| n > 0),
    })
}

fn quantile(sorted: &[f64], q: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let index = ((sorted.len() - 1) as f64 * q).round() as usize;
    Some((sorted[index] * 100.0).round() / 100.0)
}

fn write_png(
    out: &std::path::Path,
    client: &Client,
    when: &str,
    pixels: &[u8],
) -> Result<(), String> {
    let path = out.join(format!("{}-{when}.png", client.platform.name));
    let file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(file, client.platform.width, client.platform.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(pixels)
        .map_err(|e| e.to_string())
}
