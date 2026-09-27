//! The Computers screens on the terminal adapter, over the offline fixture.
//!
//! ```sh
//! cargo run -p coder-computers --example terminal            # interactive
//! cargo run -p coder-computers --example terminal -- --print # each screen as text
//! ```
//!
//! Tab and the arrow keys move focus, Enter or Space activates, and `q` or
//! Esc quits. When a screen asks for input, type it and press Enter; Esc
//! cancels. The fixture contacts no host, relay, or SSH server.
use coder_computers::synthetic::Synthetic;
use coder_computers::{Capabilities, Computers, Platform};
use coder_terminal::native::{Focus, render};
use coder_terminal::{Guard, Ladder};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Wrap};
use std::io;

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn open() -> Result<Computers, String> {
    Computers::new(
        Box::new(Synthetic::fixture(Platform::Terminal, now)),
        Capabilities {
            platform: Platform::Terminal,
            camera: false,
        },
        "computers:terminal",
    )
    .map_err(|error| error.to_string())
}

fn print() -> Result<(), String> {
    let mut computers = open()?;
    let ladder = Ladder::new(coder_terminal::Colors::None);
    for press in [
        None,
        Some("first-run-continue"),
        Some("tab-add"),
        Some("tab-activity"),
        Some("tab-computers"),
        Some("host-0-access"),
    ] {
        if let Some(node) = press {
            let view = computers.view().ok_or("no view")?.view();
            let activation = rust_native::Activation {
                instance: view.instance.clone(),
                revision: view.revision,
                node: node.into(),
            };
            computers
                .activate(&activation)
                .map_err(|refusal| refusal.reason())?;
        }
        let view = computers.view().ok_or("no view")?.view();
        println!(
            "===== {:?} (revision {})",
            computers.screen(),
            view.revision
        );
        for line in render(view, ladder, None).lines {
            let text: String = line
                .spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect();
            println!("{text}");
        }
    }
    Ok(())
}

fn interactive() -> Result<(), String> {
    let mut computers = open()?;
    let guard = Guard::full_screen().map_err(|error| error.to_string())?;
    guard.arm_panic_hook();
    let mut terminal =
        Terminal::new(CrosstermBackend::new(io::stdout())).map_err(|error| error.to_string())?;
    let ladder = Ladder::from_environment();
    let mut focus: Option<String> = None;
    let mut draft = String::new();
    loop {
        let view = computers.view().ok_or("no view")?.view().clone();
        let mut keys = Focus::new(&view, focus.as_deref());
        let input = computers.input().cloned();
        terminal
            .draw(|frame| {
                let drawn = render(&view, ladder, keys.current());
                let mut lines = drawn.lines;
                if let Some(input) = &input {
                    lines.push(Line::raw(""));
                    lines.push(Line::raw(format!("{}: {draft}_", input.label)));
                }
                let height = frame.area().height as usize;
                let scroll = drawn.focus_line.unwrap_or(0).saturating_sub(height / 2);
                frame.render_widget(
                    Paragraph::new(lines)
                        .wrap(Wrap { trim: false })
                        .scroll((u16::try_from(scroll).unwrap_or(u16::MAX), 0)),
                    frame.area(),
                );
            })
            .map_err(|error| error.to_string())?;
        let Event::Key(key) = event::read().map_err(|error| error.to_string())? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        if let Some(input) = &input {
            match key.code {
                KeyCode::Esc => {
                    let _ = computers.cancel_input(&input.token);
                    draft.clear();
                }
                KeyCode::Enter => {
                    let _ = computers.submit(&input.token, &draft);
                    draft.clear();
                }
                KeyCode::Backspace => {
                    draft.pop();
                }
                KeyCode::Char(c) if draft.len() < input.max_bytes => draft.push(c),
                _ => {}
            }
            continue;
        }
        if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
            break;
        }
        if let Some(activation) = keys.handle(key) {
            // A refusal shows on the screen as its notice.
            let _ = computers.activate(&activation);
        }
        focus = keys.current().map(str::to_owned);
    }
    drop(terminal);
    guard.restore().map_err(|error| error.to_string())
}

fn main() {
    let result = if std::env::args().any(|arg| arg == "--print") {
        print()
    } else {
        interactive()
    };
    if let Err(error) = result {
        eprintln!("terminal example: {error}");
        std::process::exit(1);
    }
}
