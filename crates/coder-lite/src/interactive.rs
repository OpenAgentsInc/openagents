//! Interactive coder-lite TUI session
//!
//! Key handling follows the pattern used in grok-build's ratatui-textarea and
//! grok-pager: destructure `crossterm::event::KeyEvent` by `code` and
//! `modifiers` so control chords do not fall through to plain character input.

use crate::acp;
use crate::runtime::{CoderRuntimeSession, Control};
use crate::tui::{CoderUi, Entry, Role};
use std::sync::mpsc;
use crossterm::{
    event::{
        self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{stderr, stdout};
use std::time::Duration;

pub async fn run_tui() -> Result<(), Box<dyn std::error::Error>> {
    if !atty_is_terminal() {
        println!("Non-interactive terminal detected. Run coder-lite from a TTY.");
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = stdout();
    stdout.execute(EnterAlternateScreen)?;

    let mut stderr = stderr();
    let flags = event::KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
        | event::KeyboardEnhancementFlags::REPORT_EVENT_TYPES
        | event::KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES;
    let _ = crossterm::execute!(stderr, PushKeyboardEnhancementFlags(flags));

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.show_cursor()?;

    let (tx, rx) = mpsc::channel::<Control>();
    let mut ui = CoderUi::new();

    match acp::find_agents().await {
        Ok(agents) => {
            let list = agents
                .iter()
                .map(|a| a.id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            ui.entries.push(Entry {
                role: Role::Notice,
                text: format!("found ACP agents: {}", list),
                output: None,
            });
            ui.agents = agents;
        }
        Err(_) => {
            ui.entries.push(Entry {
                role: Role::Notice,
                text: "found ACP agents: none".to_string(),
                output: None,
            });
        }
    }

    loop {
        while let Ok(control) = rx.try_recv() {
            match control {
                Control::Chunk(chunk) => {
                    // Append text to the current assistant entry.
                    if let Some(last) = ui
                        .entries
                        .iter_mut()
                        .rfind(|e| e.role == Role::Assistant)
                    {
                        last.text.push_str(&chunk);
                        ui.scroll_override = None;
                    }
                }
                Control::Done => ui.loading = false,
                Control::Tool { agent, title } => {
                    if let Some(last) = ui.entries.last_mut() {
                        if last.role == Role::Tool {
                            last.text = format!("delegate {}: {}", agent, title);
                            last.output = Some(Vec::new());
                        } else {
                            ui.entries.push(Entry {
                                role: Role::Tool,
                                text: format!("delegate {}: {}", agent, title),
                                output: Some(Vec::new()),
                            });
                        }
                    } else {
                        ui.entries.push(Entry {
                            role: Role::Tool,
                            text: format!("delegate {}: {}", agent, title),
                            output: Some(Vec::new()),
                        });
                    }
                    ui.scroll_override = None;
                }
                Control::ToolText(chunk) => {
                    if let Some(last) = ui.entries.last_mut() {
                        if last.role == Role::Tool {
                            last.output.get_or_insert_with(Vec::new).push(chunk);
                        }
                    }
                    ui.scroll_override = None;
                }
                Control::ToolDone => {}
            }
        }

        terminal.draw(|f| {
            let size = f.area();
            ui.render(f, size);
        })?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key {
                    KeyEvent {
                        code: KeyCode::Esc,
                        ..
                    }
                    | KeyEvent {
                        code: KeyCode::Char('q' | 'c' | 'd'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => break,
                    KeyEvent {
                        code: KeyCode::Enter,
                        modifiers,
                        ..
                    } => {
                        if modifiers.contains(KeyModifiers::SHIFT) {
                            ui.composer.push('\n');
                        } else if !ui.composer.trim().is_empty() {
                            let prompt = ui.composer.clone();
                            ui.entries.push(Entry {
                                role: Role::You,
                                text: prompt.clone(),
                                output: None,
                            });
                            ui.entries.push(Entry {
                                role: Role::Assistant,
                                text: String::new(),
                                output: None,
                            });
                            ui.composer.clear();
                            ui.scroll_override = None;
                            ui.loading = true;

                            let mut session = CoderRuntimeSession::new();
                            session.agents = ui.agents.clone();
                            let tx = tx.clone();
                            tokio::spawn(async move {
                                let _ = session.execute_turn(&prompt, tx).await;
                            });
                        }
                    }
                    KeyEvent {
                        code: KeyCode::Backspace,
                        ..
                    } => {
                        ui.composer.pop();
                    }
                    KeyEvent {
                        code: KeyCode::Up,
                        ..
                    } => ui.scroll_by(-1),
                    KeyEvent {
                        code: KeyCode::Down,
                        ..
                    } => ui.scroll_by(1),
                    KeyEvent {
                        code: KeyCode::PageUp,
                        ..
                    } => {
                        let page = ui.transcript_height.max(1) as i32;
                        ui.scroll_by(-page);
                    }
                    KeyEvent {
                        code: KeyCode::PageDown,
                        ..
                    } => {
                        let page = ui.transcript_height.max(1) as i32;
                        ui.scroll_by(page);
                    }
                    KeyEvent {
                        code: KeyCode::Char(c),
                        modifiers,
                        ..
                    } if !modifiers.contains(KeyModifiers::CONTROL)
                        && !modifiers.contains(KeyModifiers::ALT) =>
                    {
                        ui.composer.push(c);
                    }
                    _ => {}
                }
            }
        }
    }

    terminal.hide_cursor()?;
    let _ = crossterm::execute!(stderr, PopKeyboardEnhancementFlags);
    disable_raw_mode()?;
    std::io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

fn atty_is_terminal() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stdin())
        && std::io::IsTerminal::is_terminal(&std::io::stdout())
}
