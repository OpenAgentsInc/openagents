//! Offline visual check of the terminal overlay over the plaza.
//! Usage: terminal_capture OUTPUT.png [SCALE]
//!
//! Opens the overlay with three panes on real PTYs of this computer, in a
//! temporary directory that is also their home: OpenAgents Terminal when
//! an `openagents` binary is found (else a listing), a shell command with
//! color output and box drawing, and `vim` on a small file. It waits for
//! them to draw and renders one frame, 1280 by 800 points at SCALE pixels
//! a point (default 2).
use std::path::PathBuf;
use std::time::{Duration, Instant};

use verse::runtime::WorldRuntime;
use verse::terminal::Overlay;
use verse::terminal::layout::Axis;
use verse::terminal::pty::Program;

const COLORS: &str = r#"printf '\033[1mcolors\033[0m  '; for c in 31 32 33 34 35 36; do printf "\033[${c}m■ $c \033[0m"; done; echo
printf '\033[48;5;24m 256-color background \033[0m \033[38;2;255;140;0mtruecolor\033[0m\n'
printf '┌──────┬──────┐\n│ box  │ draw │\n├──────┼──────┤\n└──────┴──────┘\n▁▂▃▄▅▆▇█ ░▒▓\n'
ls -la -G /
exec cat"#;

fn sh(script: &str, label: &str) -> Program {
    Program::Command {
        program: "/bin/sh".into(),
        args: vec!["-c".into(), script.into()],
        label: label.into(),
    }
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let output = PathBuf::from(args.next().ok_or("Expected an output PNG path")?);
    let scale: f32 = args.next().map_or(Ok(2.0), |s| {
        s.parse().map_err(|_| format!("{s} is not a number"))
    })?;
    let (width, height) = ((1280.0 * scale) as u32, (800.0 * scale) as u32);
    let root = std::env::temp_dir().join(format!("verse-terminal-{}", std::process::id()));
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    std::fs::write(
        root.join("notes.rs"),
        "// A pane in Verse.\nfn main() {\n    println!(\"hello from vim\");\n}\n",
    )
    .map_err(|e| e.to_string())?;
    let first = Program::openagents_terminal()
        .unwrap_or_else(|| sh("ls -la /usr/bin | head -40; exec cat", "ls"));
    let mut overlay = Overlay::with(&root, "/bin/sh".into(), first);
    overlay.toggle();
    let atlas = verse::ui::Atlas::new((14.0 * scale).round());
    let size = [width as f32, height as f32];
    let mut batch = verse::ui::UiBatch::default();
    overlay.draw(&mut batch, &atlas, size);
    overlay.split(Axis::Columns, &sh(COLORS, "sh colors"));
    overlay.split(
        Axis::Rows,
        &Program::Command {
            program: "/usr/bin/vim".into(),
            args: vec![
                "-u".into(),
                "NONE".into(),
                "+syntax on".into(),
                "+set number".into(),
                "notes.rs".into(),
            ],
            label: "vim notes.rs".into(),
        },
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let mut batch = verse::ui::UiBatch::default();
        overlay.draw(&mut batch, &atlas, size);
        std::thread::sleep(Duration::from_millis(50));
    }
    let mut batch = verse::ui::UiBatch::default();
    overlay.draw(&mut batch, &atlas, size);
    let runtime = WorldRuntime::new();
    let result = verse::render::capture(
        &output,
        width,
        height,
        &runtime.world.mesh,
        runtime.view(width as f32 / height as f32),
        &runtime.dynamic_mesh(),
        &batch,
        &atlas,
    );
    overlay.shutdown();
    let _ = std::fs::remove_dir_all(&root);
    result
}
