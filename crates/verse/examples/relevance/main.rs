//! The relevance visualizer: takes a GitHub issue from this repository,
//! picks candidate files, and asks several decision backends side by side
//! "Is this file relevant to solving the issue?", one System One `noul`
//! per file through `crates/jev`. It streams each answer with its
//! probability and latency, scores every backend against the fix when the
//! issue has one, and prints a comparison table; with `--visual` it shows
//! the run as a Verse scene. See `docs/verse/relevance-visualizer.md`.
//!
//! ```sh
//! cargo run -p verse --example relevance -- --random --visual
//! cargo run -p verse --example relevance -- --issue 11106 --backends jev,ollama-flash
//! ```
mod cases;
mod run;
mod scene;
mod source;

use cases::{Backend, Case, LaneStatus, Rng, THRESHOLD, backends, ranking, secs};
use run::{Board, Event};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

const USAGE: &str = "relevance [--issue N | --random [--open]] [--files K] [--backends a,b,..]
          [--backend NAME --model clef-flash|clef] [--conc C] [--timeout SECS] [--seed S]
          [--visual] [--capture OUT.png [--size WxH] [--scale 2] [--wait SECS]]

lanes: jev, ollama-flash, ollama-clef, llamacpp-flash, llamacpp-27b, coderos-4080, psionic
       (also clef-ollama, clef-llamacpp with --model)
default: a random closed issue with a fix on main, 12 files, the default lanes
         (every lane but llamacpp-27b)";

#[derive(Clone, Debug)]
struct Options {
    issue: Option<u64>,
    open: bool,
    files: usize,
    backends: Vec<Backend>,
    conc: usize,
    timeout: Duration,
    seed: u64,
    visual: bool,
    capture: Option<PathBuf>,
    size: [u32; 2],
    scale: f32,
    wait: f64,
}

fn options() -> Result<Options, String> {
    let mut args = std::env::args().skip(1);
    let mut o = Options {
        issue: None,
        open: false,
        files: 12,
        backends: Vec::new(),
        conc: 1,
        timeout: Duration::from_secs(180),
        seed: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.as_nanos() as u64),
        visual: false,
        capture: None,
        size: [2880, 1720],
        scale: 2.0,
        wait: 300.0,
    };
    let (mut list, mut model): (Option<String>, Option<String>) = (None, None);
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
        let number = |s: String| s.parse::<u64>().map_err(|_| format!("{s} is not a number"));
        match arg.as_str() {
            "--issue" => o.issue = Some(number(value("--issue")?)?),
            "--random" => o.issue = None,
            "--open" => o.open = true,
            "--files" => o.files = number(value("--files")?)?.clamp(1, 40) as usize,
            "--backends" | "--backend" => list = Some(value("--backends")?),
            "--model" => model = Some(value("--model")?),
            "--conc" => o.conc = number(value("--conc")?)?.clamp(1, 8) as usize,
            "--timeout" => o.timeout = Duration::from_secs(number(value("--timeout")?)?),
            "--seed" => o.seed = number(value("--seed")?)?,
            "--visual" => o.visual = true,
            "--capture" => o.capture = Some(value("--capture")?.into()),
            "--scale" => {
                let v = value("--scale")?;
                o.scale = v.parse().map_err(|_| format!("{v} is not a number"))?;
            }
            "--wait" => o.wait = number(value("--wait")?)? as f64,
            "--size" => {
                let s = value("--size")?;
                let (w, h) = s.split_once('x').ok_or("--size is WxH")?;
                o.size = [number(w.into())? as u32, number(h.into())? as u32];
            }
            "-h" | "--help" => return Err(USAGE.into()),
            other => return Err(format!("unknown argument {other}\n\n{USAGE}")),
        }
    }
    o.backends = backends(
        list.as_deref().unwrap_or(cases::DEFAULT_BACKENDS),
        model.as_deref(),
    )?;
    Ok(o)
}

/// Loads a case: the issue asked for, or a random closed one with a fix
/// (a random open one with `--open`).
fn load(
    root: &Path,
    issue: Option<u64>,
    open: bool,
    files: usize,
    seed: u64,
) -> Result<Case, String> {
    let main = source::main_rev(root);
    let mut rng = Rng::new(seed);
    let (issue, fix) = match issue {
        Some(n) => {
            let issue = source::issue(n)?;
            let fix = if issue.state == "CLOSED" {
                source::fix(root, &main, n)?
            } else {
                None
            };
            (issue, fix)
        }
        None if open => (source::random_open(&mut rng)?, None),
        None => {
            let (issue, fix) = source::random_closed(root, &main, &mut rng)?;
            (issue, Some(fix))
        }
    };
    source::case(root, issue, fix, files, seed)
}

fn main() {
    let result = options().and_then(|o| {
        let root = source::root()?;
        if o.visual || o.capture.is_some() {
            visual::run(o, root)
        } else {
            cli(o, &root)
        }
    });
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(2);
    }
}

fn header(case: &Case) {
    println!(
        "ISSUE #{} [{}] {}",
        case.issue.number, case.issue.state, case.issue.title
    );
    match &case.fix {
        Some(f) => println!(
            "ground truth: {} file(s) changed by {} (files read at {})",
            f.files.len(),
            f.commits
                .iter()
                .map(|c| &c[..9.min(c.len())])
                .collect::<Vec<_>>()
                .join(", "),
            case.rev.chars().take(10).collect::<String>()
        ),
        None => println!(
            "no fix commit on main: no ground truth (files read at {})",
            case.rev
        ),
    }
    for (i, c) in case.candidates.iter().enumerate() {
        println!("  f{:<2} {:<7} {}", i + 1, c.origin.label(), c.path);
    }
}

fn cli(o: Options, root: &Path) -> Result<(), String> {
    eprintln!("loading the case...");
    let case = Arc::new(load(root, o.issue, o.open, o.files, o.seed)?);
    header(&case);
    let mut board = Board::new(case.clone(), &o.backends);
    let run = run::start(case.clone(), o.backends.clone(), o.conc, o.timeout, o.seed);
    let started = Instant::now();
    println!();
    while !board.finished() {
        let Ok(event) = run.events.recv() else {
            break;
        };
        board.apply(&event);
        let id = |lane: usize| board.lanes[lane].backend.id;
        match &event {
            Event::Offline { lane, why } => println!("[{:<14}] offline: {why}", id(*lane)),
            Event::Decision {
                lane,
                file,
                p,
                latency,
                ..
            } => {
                let truth = match board.labels[*file] {
                    Some(true) => "  (fix)",
                    _ => "",
                };
                let said = if *p >= THRESHOLD { "RELEVANT" } else { "-" };
                println!(
                    "[{:<14}] {:.3} {:<8} {:>6}  {}{truth}",
                    id(*lane),
                    p,
                    said,
                    secs(*latency),
                    case.candidates[*file].path
                );
            }
            Event::Failed { lane, file, error } => println!(
                "[{:<14}] error on {}: {}",
                id(*lane),
                case.candidates[*file].path,
                error.chars().take(160).collect::<String>()
            ),
            Event::Note { lane, text } => println!("[{:<14}] {text}", id(*lane)),
            Event::Started { .. } | Event::Done { .. } => {}
        }
    }
    println!(
        "\nall lanes finished in {:.1}s\n",
        started.elapsed().as_secs_f64()
    );
    print!("{}", table(&board));
    Ok(())
}

/// The comparison: the ranking with every lane's probability, then each
/// lane's speed and, with ground truth, its precision and recall.
fn table(board: &Board) -> String {
    use std::fmt::Write;
    let case = &board.case;
    let mut s = String::new();
    let lanes: Vec<&cases::Lane> = board.lanes.iter().collect();
    let _ = write!(s, "{:>4} {:>5} {:<5} ", "rank", "mean", "truth");
    for l in &lanes {
        let _ = write!(s, "{:>8} ", &l.backend.id[..l.backend.id.len().min(8)]);
    }
    let _ = writeln!(s, " file");
    for (i, row) in ranking(&board.lanes, case.candidates.len())
        .iter()
        .enumerate()
    {
        let truth = match board.labels[row.file] {
            Some(true) => "fix",
            Some(false) => "-",
            None => "?",
        };
        let mean = row.mean.map_or("    -".into(), |m| format!("{m:.3}"));
        let _ = write!(s, "{:>4} {:>5} {:<5} ", i + 1, mean, truth);
        for p in &row.p {
            let _ = write!(s, "{:>8} ", p.map_or("-".into(), |p| format!("{p:.3}")));
        }
        let split = if row.disagree { " split" } else { "" };
        let _ = writeln!(s, " {}{split}", case.candidates[row.file].path);
    }
    let _ = writeln!(
        s,
        "\n{:<15} {:>6} {:>7} {:>7} {:>7} {:>6} {:>6} {:>6} {:>4}  model",
        "lane", "done", "dec/s", "p50", "p90", "prec", "recall", "acc", "err"
    );
    for l in &lanes {
        if let LaneStatus::Offline(why) = &l.status {
            let _ = writeln!(s, "{:<15} offline: {why}", l.backend.id);
            continue;
        }
        if l.answered() == 0 {
            let why = l
                .last_error
                .as_deref()
                .or(l.note.as_deref())
                .unwrap_or("no answer");
            let _ = writeln!(
                s,
                "{:<15} 0/{}  {}",
                l.backend.id,
                case.candidates.len(),
                why.chars().take(160).collect::<String>()
            );
            continue;
        }
        let q = l.quality(&board.labels);
        let pct = |v: Option<f64>| v.map_or("-".into(), |v| format!("{:.0}%", v * 100.0));
        let _ = writeln!(
            s,
            "{:<15} {:>6} {:>7} {:>7} {:>7} {:>6} {:>6} {:>6} {:>4}  {} at {}",
            l.backend.id,
            format!("{}/{}", l.answered(), case.candidates.len()),
            l.rate().map_or("-".into(), |r| format!("{r:.2}")),
            l.p50().map_or("-".into(), secs),
            l.p90().map_or("-".into(), secs),
            pct(q.and_then(|q| q.precision)),
            pct(q.and_then(|q| q.recall)),
            pct(q.map(|q| q.accuracy)),
            l.errors,
            l.backend.model,
            l.backend.base
        );
    }
    let _ = writeln!(
        s,
        "\nthreshold {THRESHOLD}; dec/s is one lane's answers over its own wall time"
    );
    s
}

mod visual {
    use super::*;
    use std::sync::Arc as StdArc;
    use verse::imported::{Renderer, WindowPresenter};
    use verse::ui::Atlas;
    use winit::{
        application::ApplicationHandler,
        event::{ElementState, WindowEvent},
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        keyboard::{KeyCode, PhysicalKey},
        window::{Window, WindowId},
    };

    /// Everything the scene shows, apart from the GPU.
    struct Viz {
        o: Options,
        root: PathBuf,
        enabled: Vec<bool>,
        board: Option<Board>,
        run: Option<run::Run>,
        lane_ids: Vec<usize>,
        angles: Vec<f32>,
        arrivals: scene::Arrivals,
        loading: Option<mpsc::Receiver<Result<Case, String>>>,
        message: String,
        clock: Instant,
        run_started: f32,
        run_ended: Option<f32>,
        camera: scene::Camera,
        seed: u64,
        printed: bool,
    }

    impl Viz {
        fn new(o: Options, root: PathBuf) -> Self {
            let enabled = vec![true; o.backends.len()];
            let seed = o.seed;
            Self {
                o,
                root,
                enabled,
                board: None,
                run: None,
                lane_ids: vec![],
                angles: vec![],
                arrivals: scene::Arrivals(vec![]),
                loading: None,
                message: String::new(),
                clock: Instant::now(),
                run_started: 0.0,
                run_ended: None,
                camera: scene::Camera {
                    yaw: 0.6,
                    distance: 27.0,
                },
                seed,
                printed: false,
            }
        }

        fn t(&self) -> f32 {
            self.clock.elapsed().as_secs_f32()
        }

        /// Loads a case on a worker thread: the issue named on the command
        /// line first, then random ones.
        fn fetch(&mut self, issue: Option<u64>) {
            let (tx, rx) = mpsc::channel();
            let (root, open, files, seed) =
                (self.root.clone(), self.o.open, self.o.files, self.seed);
            std::thread::spawn(move || {
                let _ = tx.send(load(&root, issue, open, files, seed));
            });
            self.loading = Some(rx);
            self.message = match issue {
                Some(n) => format!("fetching issue #{n} and its files..."),
                None if self.o.open => "picking a random open issue...".into(),
                None => "picking a random closed issue with a fix...".into(),
            };
        }

        /// Starts the enabled lanes on the current case.
        fn start(&mut self) {
            let Some(case) = self.board.as_ref().map(|b| b.case.clone()) else {
                return;
            };
            self.print_table();
            self.run = None;
            let chosen: Vec<(usize, Backend)> = self
                .o
                .backends
                .iter()
                .cloned()
                .enumerate()
                .filter(|(i, _)| self.enabled[*i])
                .collect();
            let registry = cases::registry();
            self.lane_ids = chosen
                .iter()
                .map(|(_, b)| registry.iter().position(|r| r.id == b.id).unwrap_or(0))
                .collect();
            let backends: Vec<Backend> = chosen.into_iter().map(|(_, b)| b).collect();
            self.board = Some(Board::new(case.clone(), &backends));
            self.arrivals =
                scene::Arrivals(vec![vec![None; case.candidates.len()]; backends.len()]);
            self.angles = scene::layout(&case);
            self.run_started = self.t();
            self.run_ended = None;
            self.printed = false;
            if !backends.is_empty() {
                self.run = Some(run::start(
                    case,
                    backends,
                    self.o.conc,
                    self.o.timeout,
                    self.seed,
                ));
            }
            self.message.clear();
        }

        fn print_table(&mut self) {
            if let Some(board) = &self.board
                && !self.printed
                && board.lanes.iter().any(|l| l.answered() > 0)
            {
                println!();
                header(&board.case);
                println!();
                print!("{}", table(board));
                self.printed = true;
            }
        }

        /// Takes what arrived since the last frame.
        fn pump(&mut self) {
            if let Some(rx) = &self.loading
                && let Ok(result) = rx.try_recv()
            {
                self.loading = None;
                match result {
                    Ok(case) => {
                        self.board = Some(Board::new(Arc::new(case), &[]));
                        self.start();
                    }
                    Err(e) => self.message = format!("could not load a case: {e}"),
                }
            }
            let now = self.t();
            if let (Some(run), Some(board)) = (&self.run, &mut self.board) {
                while let Ok(event) = run.events.try_recv() {
                    if let Event::Decision { lane, file, .. } = &event {
                        self.arrivals.0[*lane][*file] = Some(now);
                    }
                    board.apply(&event);
                }
                if board.finished() && self.run_ended.is_none() {
                    self.run_ended = Some(now);
                }
            }
            if self.run_ended.is_some() {
                self.print_table();
            }
        }

        fn key(&mut self, key: KeyCode) -> bool {
            match key {
                KeyCode::Escape => return false,
                KeyCode::KeyR if self.loading.is_none() => {
                    self.seed = self
                        .seed
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    self.print_table();
                    self.run = None;
                    self.fetch(None);
                }
                KeyCode::Space if self.loading.is_none() => {
                    self.seed = self.seed.wrapping_add(1);
                    self.start();
                }
                KeyCode::ArrowLeft => self.camera.yaw -= 0.25,
                KeyCode::ArrowRight => self.camera.yaw += 0.25,
                KeyCode::ArrowUp => self.camera.distance = (self.camera.distance - 3.0).max(16.0),
                KeyCode::ArrowDown => self.camera.distance = (self.camera.distance + 3.0).min(70.0),
                key => {
                    let digits = [
                        KeyCode::Digit1,
                        KeyCode::Digit2,
                        KeyCode::Digit3,
                        KeyCode::Digit4,
                        KeyCode::Digit5,
                        KeyCode::Digit6,
                        KeyCode::Digit7,
                        KeyCode::Digit8,
                        KeyCode::Digit9,
                    ];
                    if let Some(i) = digits.iter().position(|d| *d == key)
                        && i < self.enabled.len()
                        && self.loading.is_none()
                    {
                        self.enabled[i] = !self.enabled[i];
                        self.start();
                    }
                }
            }
            true
        }

        /// One frame at `size` physical pixels; the overlay is laid out in
        /// points (`size / fonts.scale`) with glyphs rasterized at physical
        /// pixels, so text is drawn 1:1 on a Retina display.
        fn frame(
            &mut self,
            size: [u32; 2],
            fonts: &Fonts,
        ) -> (
            verse::render::View,
            Vec<verse_engine::presentation::Instance>,
            verse::ui::UiBatch,
            [f32; 2],
        ) {
            let t = self.t();
            self.camera.yaw += 0.0011;
            let aspect = size[0] as f32 / size[1].max(1) as f32;
            let (view, vp) = self.camera.view(aspect);
            let overlay = [size[0] as f32 / fonts.scale, size[1] as f32 / fonts.scale];
            let world = self.board.as_ref().map_or_else(Vec::new, |b| {
                scene::instances(b, &self.lane_ids, &self.angles, &self.arrivals, t)
            });
            let elapsed = self.run_ended.unwrap_or(t) - self.run_started;
            let hud = scene::Hud {
                backends: &self.o.backends,
                enabled: &self.enabled,
                message: &self.message,
                elapsed,
            };
            let board = self.board.as_ref();
            let ui = scene::overlay(board, &self.angles, vp, overlay, &fonts.layout, &hud);
            (view, world, ui, overlay)
        }
    }

    /// The HUD font at a display's backing scale: the bitmap the renderer
    /// samples is rasterized at physical pixels (13 points times the scale),
    /// and `layout` measures in points, as `verse`'s own window does
    /// (`ui_atlas` in `crates/verse/src/app.rs`).
    pub struct Fonts {
        pub raster: Atlas,
        pub layout: Atlas,
        pub scale: f32,
    }

    impl Fonts {
        pub fn new(scale: f32) -> Self {
            let scale = if scale.is_finite() {
                scale.clamp(1.0, 4.0)
            } else {
                1.0
            };
            let raster = Atlas::new((13.0 * scale).round());
            let layout = raster
                .layout_at_scale(scale)
                .unwrap_or_else(|| Atlas::new(13.0));
            Self {
                raster,
                layout,
                scale,
            }
        }
    }

    fn renderer(dir: &Path, size: [u32; 2], atlas: &Atlas) -> Result<Renderer, String> {
        let pack = scene::pack(dir)?;
        Renderer::new(
            pack,
            dir,
            size[0],
            size[1],
            atlas,
            &scene::static_instances(),
        )
    }

    pub fn run(o: Options, root: PathBuf) -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!("verse-relevance-{}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut viz = Viz::new(o.clone(), root);
        viz.fetch(o.issue);
        let result = if let Some(path) = o.capture.clone() {
            capture(&mut viz, &dir, &Fonts::new(o.scale), &path)
        } else {
            window(viz, &dir)
        };
        let _ = std::fs::remove_dir_all(&dir);
        result
    }

    /// Runs the case without a window, then writes one frame: at the end of
    /// the run, or after `--wait` seconds.
    fn capture(viz: &mut Viz, dir: &Path, fonts: &Fonts, path: &Path) -> Result<(), String> {
        let size = viz.o.size;
        let mut renderer = renderer(dir, size, &fonts.raster)?;
        let started = Instant::now();
        loop {
            viz.pump();
            let done = viz.run_ended.is_some_and(|e| viz.t() - e > 1.2);
            if done || started.elapsed().as_secs_f64() > viz.o.wait {
                break;
            }
            if viz.loading.is_none() && viz.board.is_none() {
                return Err(std::mem::take(&mut viz.message));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        viz.camera.yaw = 0.6;
        let (view, world, ui, overlay) = viz.frame(size, fonts);
        renderer.set_overlay_size(overlay[0], overlay[1]);
        scene::capture(&mut renderer, view, &world, &ui, size[0], size[1], path)?;
        viz.print_table();
        println!("wrote {}", path.display());
        Ok(())
    }

    struct App {
        viz: Viz,
        fonts: Fonts,
        dir: PathBuf,
        window: Option<StdArc<Window>>,
        renderer: Option<Renderer>,
        presenter: Option<WindowPresenter>,
        error: Option<String>,
        keys: HashSet<KeyCode>,
    }

    fn window(viz: Viz, dir: &Path) -> Result<(), String> {
        let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
        event_loop.set_control_flow(ControlFlow::Poll);
        let mut app = App {
            viz,
            fonts: Fonts::new(1.0),
            dir: dir.to_owned(),
            window: None,
            renderer: None,
            presenter: None,
            error: None,
            keys: HashSet::new(),
        };
        event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
        app.viz.print_table();
        app.error.map_or(Ok(()), Err)
    }

    impl App {
        /// (Re)builds the renderer for the window's backing scale: its glyph
        /// bitmap is fixed at build time, so a move to a display of another
        /// scale rebuilds it.
        fn open(&mut self, scale: f32) -> Result<(), String> {
            let window = self.window.clone().ok_or("no window")?;
            self.fonts = Fonts::new(scale);
            let size = window.inner_size();
            self.presenter = None;
            self.renderer = None;
            let renderer = renderer(&self.dir, [size.width, size.height], &self.fonts.raster)?;
            self.presenter = Some(renderer.attach_window(window)?);
            self.renderer = Some(renderer);
            Ok(())
        }

        fn draw(&mut self) -> Result<(), String> {
            let window = self.window.clone().ok_or("no window")?;
            let renderer = self.renderer.as_mut().ok_or("no renderer")?;
            if renderer.recover_if_lost(&self.fonts.raster)? {
                self.presenter = Some(renderer.attach_window(window.clone())?);
            }
            let size = window.inner_size();
            if size.width == 0 || size.height == 0 {
                return Ok(());
            }
            renderer.resize(size.width, size.height)?;
            self.viz.pump();
            let (view, world, ui, overlay) = self.viz.frame([size.width, size.height], &self.fonts);
            renderer.set_overlay_size(overlay[0], overlay[1]);
            renderer.draw_live(view, &world, &ui, &scene::lighting())?;
            let presenter = self.presenter.as_mut().ok_or("no presenter")?;
            renderer.present_window(presenter, [size.width, size.height])
        }
    }

    impl ApplicationHandler for App {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.window.is_some() {
                return;
            }
            let result = (|| -> Result<(), String> {
                let window = StdArc::new(
                    event_loop
                        .create_window(
                            Window::default_attributes()
                                .with_title("Verse - Relevance")
                                .with_inner_size(winit::dpi::LogicalSize::new(1440.0, 860.0)),
                        )
                        .map_err(|e| e.to_string())?,
                );
                self.window = Some(window.clone());
                self.open(window.scale_factor() as f32)
            })();
            if let Err(e) = result {
                self.error = Some(e);
                event_loop.exit();
            }
        }

        fn about_to_wait(&mut self, _: &ActiveEventLoop) {
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }

        fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                    if (scale_factor as f32 - self.fonts.scale).abs() > 0.01
                        && let Err(e) = self.open(scale_factor as f32)
                    {
                        self.error = Some(e);
                        event_loop.exit();
                    }
                }
                WindowEvent::RedrawRequested => {
                    if let Err(e) = self.draw() {
                        self.error = Some(e);
                        event_loop.exit();
                    }
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    let PhysicalKey::Code(key) = event.physical_key else {
                        return;
                    };
                    if event.state != ElementState::Pressed {
                        self.keys.remove(&key);
                        return;
                    }
                    if !self.keys.insert(key) {
                        return;
                    }
                    if !self.viz.key(key) {
                        event_loop.exit();
                    }
                }
                _ => {}
            }
        }
    }
}
