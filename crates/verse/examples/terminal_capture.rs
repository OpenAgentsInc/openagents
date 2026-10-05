//! Offline visual check of the terminal overlay over the plaza.
//! Usage: terminal_capture OUTPUT.png [SCALE] [SCENE]
//!
//! Opens the overlay with panes on real PTYs of this computer, in a
//! temporary directory that is also their home, waits for them to draw,
//! and renders one frame, 1280 by 800 points at SCALE pixels a point
//! (default 2). SCENE picks what the panes show:
//!
//! - `panes` (the default): OpenAgents Terminal when an `openagents`
//!   binary is found (else a listing), a shell command with color output
//!   and box drawing, and `vim` on a small file, with the hotbar button's
//!   card.
//! - `unicode`: CJK and emoji, a braille graph, powerline separators, the
//!   text renditions, true color, and `top`.
//! - `select`: a selection made by dragging, a copy-mode search match, and
//!   the stats line.
use std::path::PathBuf;
use std::time::{Duration, Instant};

use verse::runtime::WorldRuntime;
use verse::terminal::layout::Axis;
use verse::terminal::pty::Program;
use verse::terminal::{KeyIn, Overlay};
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey};

const COLORS: &str = r#"printf '\033[1mcolors\033[0m  '; for c in 31 32 33 34 35 36; do printf "\033[${c}m■ $c \033[0m"; done; echo
printf '\033[48;5;24m 256-color background \033[0m \033[38;2;255;140;0mtruecolor\033[0m\n'
printf '┌──────┬──────┐\n│ box  │ draw │\n├──────┼──────┤\n└──────┴──────┘\n▁▂▃▄▅▆▇█ ░▒▓\n'
ls -la -G /
exec cat"#;

const RENDITIONS: &str = r#"printf 'CJK  世界你好 こんにちは 한국어\n'
printf 'emoji  😀 🚀 ✨ 🔒  symbols ✓ ✗ → ⇒ ∑ λ Ж ★ ♥\n'
printf 'combining  e\314\201 a\314\210 n\314\203\n'
printf '\033[1mbold\033[0m \033[2mdim\033[0m \033[3mitalic\033[0m \033[4munderline\033[0m \033[9mstrike\033[0m \033[7minverse\033[0m \033]8;;https://openagents.com\033\\link\033]8;;\033\\\n'
printf '\033[48;5;238m\033[38;5;255m main \033[0m\033[38;5;238m\356\202\260\033[0m \033[38;5;45m~/work\033[0m \033[38;5;45m\356\202\261\033[0m \356\202\240 trunk\n'
i=0; while [ $i -lt 48 ]; do r=$((i*5)); printf "\033[48;2;${r};$((240-r));160m \033[0m"; i=$((i+1)); done; echo
i=16; while [ $i -lt 64 ]; do printf "\033[48;5;${i}m \033[0m"; i=$((i+1)); done; echo
cat graph.txt
exec cat"#;

fn sh(script: &str, label: &str) -> Program {
    Program::Command {
        program: "/bin/sh".into(),
        args: vec!["-c".into(), script.into()],
        label: label.into(),
    }
}

/// A braille graph like `btop` draws: two series, eight rows of dots.
fn braille_graph() -> String {
    let width = 44;
    let rows = 6;
    let levels = rows * 4;
    let height = |x: usize, phase: f32| {
        let t = x as f32 * 0.21 + phase;
        ((t.sin() * 0.45 + (t * 2.7).cos() * 0.2 + 0.55).clamp(0.0, 1.0) * levels as f32) as usize
    };
    let mut out = String::from("cpu ▕ braille graph\n");
    for row in 0..rows {
        let base = (rows - 1 - row) * 4;
        for col in 0..width {
            let mut bits = 0u32;
            for (side, x) in [(0, col * 2), (1, col * 2 + 1)] {
                let h = height(x, 0.0);
                for dot in 0..4 {
                    // Dots fill from the bottom of the cell up.
                    if h > base + dot {
                        bits |= match (side, dot) {
                            (0, 0) => 0x40,
                            (0, 1) => 0x04,
                            (0, 2) => 0x02,
                            (0, 3) => 0x01,
                            (1, 0) => 0x80,
                            (1, 1) => 0x20,
                            (1, 2) => 0x10,
                            _ => 0x08,
                        };
                    }
                }
            }
            out.push(char::from_u32(0x2800 + bits).unwrap_or(' '));
        }
        out.push('\n');
    }
    out
}

fn press(overlay: &mut Overlay, code: KeyCode, text: &str) {
    overlay.key(&KeyIn {
        code,
        logical: Key::Character(text.into()),
        text: Some(text.into()),
        plain: Some(text.into()),
        pressed: true,
    });
}

fn prefix(overlay: &mut Overlay) {
    overlay.modifiers(ModifiersState::CONTROL);
    press(overlay, KeyCode::KeyB, "b");
    overlay.modifiers(ModifiersState::empty());
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let output = PathBuf::from(args.next().ok_or("Expected an output PNG path")?);
    let scale: f32 = args.next().map_or(Ok(2.0), |s| {
        s.parse().map_err(|_| format!("{s} is not a number"))
    })?;
    let scene = args.next().unwrap_or_else(|| "panes".into());
    let (width, height) = ((1280.0 * scale) as u32, (800.0 * scale) as u32);
    let root = std::env::temp_dir().join(format!("verse-terminal-{}", std::process::id()));
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    std::fs::write(
        root.join("notes.rs"),
        "// A pane in Verse.\nfn main() {\n    println!(\"hello from vim\");\n}\n",
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(root.join("graph.txt"), braille_graph()).map_err(|e| e.to_string())?;
    let first = match scene.as_str() {
        "unicode" => sh(RENDITIONS, "sh unicode"),
        "select" => sh(
            "printf 'cargo test -p verse --lib -- terminal\\n   Compiling coder-vt v0.1.0\\n   Compiling verse v0.1.0\\n    Finished test profile in 41.2s\\ntest result: ok. 24 passed; 0 failed\\nwarning: unused import in render.rs\\n'; exec cat",
            "sh build",
        ),
        _ => Program::openagents_terminal()
            .unwrap_or_else(|| sh("ls -la /usr/bin | head -40; exec cat", "ls")),
    };
    let mut overlay = Overlay::with(&root, "/bin/sh".into(), first);
    overlay.toggle();
    let mut atlas = verse::ui::Atlas::new((14.0 * scale).round());
    atlas.reserve_glyphs(verse::terminal::GLYPH_ROWS)?;
    let size = [width as f32, height as f32];
    let mut batch = verse::ui::UiBatch::default();
    overlay.draw(&mut batch, &mut atlas, size);
    match scene.as_str() {
        "unicode" => overlay.split(Axis::Columns, &sh("exec top -s 1 -o cpu", "top")),
        "select" => overlay.split(
            Axis::Columns,
            &sh(
                "for i in $(seq 1 60); do echo \"log line $i: request served in $((i*7 % 90)) ms\"; done; exec cat",
                "sh log",
            ),
        ),
        _ => {
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
        }
    }
    let draw_for = |overlay: &mut Overlay, atlas: &mut verse::ui::Atlas, seconds: f32| {
        let deadline = Instant::now() + Duration::from_secs_f32(seconds);
        while Instant::now() < deadline {
            let started = Instant::now();
            let mut batch = verse::ui::UiBatch::default();
            overlay.draw(&mut batch, atlas, size);
            overlay.frame_done(started);
            std::thread::sleep(Duration::from_millis(16));
        }
    };
    draw_for(&mut overlay, &mut atlas, 3.0);
    if scene == "select" {
        // Search the log pane's scrollback for "line 42", then select the
        // build pane's test summary by dragging.
        prefix(&mut overlay);
        press(&mut overlay, KeyCode::Slash, "/");
        for c in "line 42".chars() {
            press(&mut overlay, KeyCode::KeyL, &c.to_string());
        }
        overlay.key(&KeyIn {
            code: KeyCode::Enter,
            logical: Key::Named(NamedKey::Enter),
            text: None,
            plain: None,
            pressed: true,
        });
        let area = Overlay::area_for(size, verse::terminal::draw::cell_size(&atlas));
        let cell = verse::terminal::draw::cell_size(&atlas);
        let inner = verse::terminal::draw::inner(
            verse::terminal::layout::Rect::new(area.x, area.y, area.w / 2.0, area.h),
            cell,
        );
        let at = |row: f32, col: f32| {
            [
                inner.x + (col + 0.5) * cell[0],
                inner.y + (row + 0.5) * cell[1],
            ]
        };
        overlay.press(at(1.0, 3.0));
        overlay.pointer(at(4.0, 20.0));
        overlay.release(at(4.0, 20.0));
        overlay.stats.shown = true;
        draw_for(&mut overlay, &mut atlas, 1.3);
    }
    let mut batch = verse::ui::UiBatch::default();
    if scene == "panes" {
        // The hotbar button, with the pointer resting on it to show its card.
        let button = Overlay::button_for(size, scale, None);
        overlay.button = Some(button);
        overlay.pointer([button.x + button.w / 2.0, button.y + button.h / 2.0]);
    }
    overlay.draw(&mut batch, &mut atlas, size);
    if let Some(text) = overlay.selected_text() {
        eprintln!("selected: {text:?}");
    }
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
