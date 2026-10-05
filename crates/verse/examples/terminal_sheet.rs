//! Renders the smart terminal's sheet through the live demo flow, offscreen.
//! Usage: terminal_sheet OUTDIR FIXTURE [window|grid] [SCALE]
//!
//! Runs the shared terminal (`terminal-core` and `terminal-gfx`) on a real
//! zsh in a scratch home, with the `openagents` helper beside this example
//! answering live, and writes one PNG per stage to OUTDIR: `idle`, `failed`
//! (the fixture's `cargo test`), `asked` (the answer and its pending
//! proposal), `confirmed` (after ENTER), `help` (F1), and `program` (a
//! full-screen program). `window` draws the standalone sheet, 1200 by 800
//! points; `grid` draws the same sheet anchored over the plaza. Keys are
//! key events delivered in this process, not from a keyboard. The run's
//! scratch home, with its threads, is removed at the end.
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use verse::runtime::WorldRuntime;
use verse::terminal::pty::Program;
use verse::terminal::{KeyIn, Mount, Overlay};
use winit::keyboard::{Key, KeyCode, NamedKey};

struct Run {
    overlay: Overlay,
    atlas: verse::ui::Atlas,
    size: [f32; 2],
    out: PathBuf,
    log: Vec<serde_json::Value>,
}

impl Run {
    fn draw(&mut self) -> verse::ui::UiBatch {
        let started = Instant::now();
        let mut batch = verse::ui::UiBatch::default();
        self.overlay.draw(&mut batch, &mut self.atlas, self.size);
        self.overlay.frame_done(started);
        batch
    }
    fn wait(&mut self, seconds: f32, done: impl Fn(&Overlay) -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs_f32(seconds);
        while Instant::now() < deadline {
            self.draw();
            if done(&self.overlay) {
                // Two more frames, so what changed is drawn.
                self.draw();
                self.draw();
                return true;
            }
            std::thread::sleep(Duration::from_millis(16));
        }
        false
    }
    fn key(&mut self, code: KeyCode, named: NamedKey) {
        let mut key = KeyIn {
            code,
            logical: Key::Named(named),
            text: None,
            plain: None,
            pressed: true,
            repeat: false,
            synthetic: false,
        };
        self.overlay.key(&key);
        key.pressed = false;
        self.overlay.key(&key);
        self.draw();
    }
    fn line(&mut self, text: &str) {
        self.overlay.paste(text);
        self.draw();
        self.key(KeyCode::Enter, NamedKey::Enter);
    }
    fn capture(&mut self, name: &str) -> Result<(), String> {
        let batch = self.draw();
        let path = self.out.join(format!("{name}.png"));
        let runtime = WorldRuntime::new();
        let [w, h] = self.size;
        verse::render::capture(
            &path,
            w as u32,
            h as u32,
            &runtime.world.mesh,
            runtime.view(w / h),
            &runtime.dynamic_mesh(),
            &batch,
            &self.atlas,
        )?;
        self.log
            .push(serde_json::json!({"stage": name, "png": path}));
        eprintln!("captured {}", path.display());
        Ok(())
    }
}

fn last_block_done(overlay: &Overlay) -> bool {
    let core = &overlay.core;
    core.focus_id()
        .and_then(|id| core.panes.get(&id))
        .and_then(|pane| pane.session.blocks.records.back())
        .is_some_and(|block| block.end.is_some())
        && !core.paper_running()
}

fn blocks(overlay: &Overlay) -> usize {
    let core = &overlay.core;
    core.focus_id()
        .and_then(|id| core.panes.get(&id))
        .map_or(0, |pane| pane.session.blocks.records.len())
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().ok_or("Expected an output directory")?);
    let fixture = PathBuf::from(args.next().ok_or("Expected the demo fixture's path")?);
    let mount = args.next().unwrap_or_else(|| "window".into());
    let scale: f32 = args.next().map_or(Ok(2.0), |s| {
        s.parse().map_err(|_| format!("{s} is not a number"))
    })?;
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let root = std::env::temp_dir().join(format!("terminal-sheet-{}", std::process::id()));
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    // The scratch home's zsh finds this computer's Rust toolchain.
    let exports: String = ["RUSTUP_HOME", "CARGO_HOME", "CARGO_TARGET_DIR"]
        .iter()
        .filter_map(|name| Some(format!("export {name}='{}'\n", std::env::var(name).ok()?)))
        .collect();
    std::fs::write(root.join(".zshenv"), exports).map_err(|e| e.to_string())?;
    let mut overlay = Overlay::with(&root, "/bin/zsh".into(), Program::Shell);
    let (size, px) = if mount == "grid" {
        overlay.mount = Mount::Overlay;
        ([1440.0 * scale, 900.0 * scale], 14.0)
    } else {
        overlay.mount = Mount::Window;
        let [w, h] = verse::terminal::SHEET_POINTS;
        ([w * scale, h * scale], 16.0)
    };
    overlay.scale = scale;
    overlay.toggle();
    let mut atlas = verse::ui::Atlas::new((px * scale).round());
    atlas.reserve_glyphs(verse::terminal::GLYPH_ROWS)?;
    let mut run = Run {
        overlay,
        atlas,
        size,
        out: out.clone(),
        log: Vec::new(),
    };
    let result = demo(&mut run, &fixture);
    let log = serde_json::json!({
        "mount": mount,
        "scale": scale,
        "entries": run.overlay.core.paper.entries.iter().map(|e| format!("{e:?}")).collect::<Vec<_>>(),
        "door": run.overlay.core.paper.door,
        "threads": run.overlay.core.smart.threads.values().collect::<Vec<_>>(),
        "stages": run.log,
        "result": result.as_ref().err(),
        "keys": "key events delivered in this process (not a keyboard)",
    });
    std::fs::write(
        out.join(format!("{mount}-run.json")),
        serde_json::to_vec_pretty(&log).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    run.overlay.shutdown();
    let _ = std::fs::remove_dir_all(&root);
    result
}

fn demo(run: &mut Run, fixture: &Path) -> Result<(), String> {
    if !run.wait(10.0, |o| {
        o.core
            .focus_id()
            .and_then(|id| o.core.panes.get(&id))
            .is_some_and(|pane| pane.session.blocks.at_prompt)
    }) {
        return Err("the shell never reached its prompt".into());
    }
    run.capture("idle")?;
    latency(run);
    run.line(&format!("cd {}", fixture.display()));
    run.wait(5.0, last_block_done);
    let before = blocks(&run.overlay);
    run.line("cargo test");
    if !run.wait(240.0, |o| blocks(o) > before && last_block_done(o)) {
        return Err("cargo test did not finish".into());
    }
    run.capture("failed")?;
    run.line("why did that fail");
    if !run.wait(150.0, |o| {
        o.core.smart.pending.is_some()
            || (o.core.smart.workers.is_empty()
                && o.core
                    .paper
                    .entries
                    .iter()
                    .any(|e| matches!(e, verse::terminal::paper::Entry::Answer(_))))
    }) {
        eprintln!("no proposal arrived");
    }
    run.capture("asked")?;
    if run.overlay.core.smart.pending.is_some() {
        let before = blocks(&run.overlay);
        run.key(KeyCode::Enter, NamedKey::Enter);
        if run.overlay.core.smart.pending.is_some() {
            // A command that may change files takes a second CONFIRM.
            run.capture("confirm-again")?;
            run.key(KeyCode::Enter, NamedKey::Enter);
        }
        run.wait(240.0, |o| blocks(o) > before && last_block_done(o));
        // The result goes back to the same thread; wait for its answer.
        run.wait(150.0, |o| {
            o.core.smart.workers.is_empty() && o.core.smart.execution.is_none()
        });
        run.capture("confirmed")?;
    }
    run.key(KeyCode::F1, NamedKey::F1);
    run.capture("help")?;
    run.key(KeyCode::F1, NamedKey::F1);
    run.line("top -s 1 -n 20");
    run.wait(4.0, |o| {
        o.core
            .focus_id()
            .and_then(|id| o.core.panes.get(&id))
            .is_some_and(|pane| pane.session.vt.alternate_screen())
    });
    run.wait(6.0, |_| false);
    run.capture("program")?;
    run.overlay.paste("q");
    run.wait(3.0, last_block_done);
    Ok(())
}

/// Key-to-glyph time on the input line: from one key event to the next
/// frame's finished drawing (the sheet laid out and every vertex built),
/// before GPU submission and the display's refresh.
fn latency(run: &mut Run) {
    let mut samples = Vec::new();
    for index in 0..400 {
        let c = char::from(b'a' + (index % 26) as u8);
        let started = Instant::now();
        run.overlay.key(&KeyIn {
            code: KeyCode::KeyA,
            logical: Key::Character(c.to_string().into()),
            text: Some(c.to_string().into()),
            plain: None,
            pressed: true,
            repeat: false,
            synthetic: false,
        });
        run.draw();
        samples.push(started.elapsed().as_secs_f64() * 1000.0);
        if index % 40 == 39 {
            run.overlay.paste("");
            for _ in 0..40 {
                run.key(KeyCode::Backspace, NamedKey::Backspace);
            }
        }
    }
    samples.sort_by(f64::total_cmp);
    let at = |q: f64| samples[((samples.len() - 1) as f64 * q).round() as usize];
    let report = serde_json::json!({
        "measure": "key event handled to the next frame's sheet drawn (CPU), before GPU submission",
        "samples": samples.len(),
        "p50_ms": at(0.5),
        "p95_ms": at(0.95),
        "max_ms": samples.last(),
    });
    eprintln!("latency {report}");
    run.log.push(report);
}
