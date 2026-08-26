//! Interactive coder-lite TUI session
//!
//! Key handling follows the pattern used in grok-build's ratatui-textarea and
//! grok-pager: destructure `crossterm::event::KeyEvent` by `code` and
//! `modifiers` so control chords do not fall through to plain character input.

use crate::acp;
use crate::export::{export_trajectory, git_info};
use crate::runtime::{CoderRuntimeSession, Control};
use crate::tui::{CoderUi, Entry, Role, ToolCall};
use crossterm::{
    ExecutableCommand,
    event::{
        self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, PopKeyboardEnhancementFlags,
        PushKeyboardEnhancementFlags,
    },
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::env;
use std::io::{stderr, stdout};
use std::sync::mpsc;
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
            ui.entries.push(Entry::new(
                Role::Notice,
                format!("found ACP agents: {}", list),
            ));
            ui.agents = agents;
        }
        Err(_) => {
            ui.entries
                .push(Entry::new(Role::Notice, "found ACP agents: none"));
        }
    }

    let (repo, branch) = git_info().unwrap_or(("unknown".to_string(), "unknown".to_string()));
    ui.repo = repo;
    ui.branch = branch;
    ui.model = env::var("OPENAGENTS_MODEL").unwrap_or_default();

    loop {
        while let Ok(control) = rx.try_recv() {
            match control {
                Control::Chunk(chunk) => {
                    // Append text to the current assistant entry.
                    if let Some(last) = ui.entries.iter_mut().rfind(|e| e.role == Role::Assistant) {
                        last.push_text(&chunk);
                        ui.scroll_override = None;
                    }
                }
                Control::Done => {
                    // Tell the markdown engine the stream closed so it flushes
                    // any bytes held back at a chunk boundary.
                    if let Some(last) = ui.entries.iter_mut().rfind(|e| e.role == Role::Assistant) {
                        last.finish_text();
                    }
                    ui.loading = false;
                }
                Control::Tool {
                    function_name,
                    arguments,
                    title,
                } => {
                    let parsed = serde_json::from_str(&arguments)
                        .unwrap_or_else(|_| serde_json::json!({ "unparsed_arguments": arguments }));
                    let call_id = format!("call-{}", ui.entries.len());
                    let agent = parsed
                        .get("agent")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let mut entry = Entry::tool_call(format!("delegate {}: {}", agent, title));
                    entry.tool = Some(ToolCall {
                        call_id,
                        function_name,
                        arguments: parsed,
                        output: None,
                        error: None,
                    });
                    ui.entries.push(entry);
                    ui.scroll_override = None;
                }
                Control::ToolTitle(title) => {
                    if let Some(last) = ui.entries.last_mut() {
                        if last.role == Role::Tool {
                            let agent = last
                                .text
                                .split_whitespace()
                                .nth(1)
                                .and_then(|s| s.strip_suffix(':'))
                                .unwrap_or("unknown")
                                .to_string();
                            last.text = format!("delegate {}: {}", agent, title);
                        }
                    }
                    ui.scroll_override = None;
                }
                Control::ToolText(chunk) => {
                    if let Some(last) = ui.entries.last_mut() {
                        if last.role == Role::Tool {
                            last.output.get_or_insert_with(String::new).push_str(&chunk);
                            if let Some(ref mut tool) = last.tool {
                                tool.output = last.output.clone();
                            }
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

        // ratatui has no hyperlink concept, so repaint the link runs as OSC 8
        // sequences over the frame it just flushed. `emit` re-reads the text
        // out of the buffer, so this can never change what a cell says.
        if !ui.links.is_empty() {
            let buffer = terminal.current_buffer_mut().clone();
            let mut out = std::io::stdout();
            let _ = crate::osc8::emit(&mut out, &ui.links, &buffer);
            let cursor = terminal.get_cursor_position()?;
            terminal.set_cursor_position(cursor)?;
        }

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key {
                    KeyEvent {
                        code: KeyCode::Esc, ..
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
                            ui.composer.clear();
                            ui.scroll_override = None;

                            if prompt.trim() == "/export" {
                                ui.entries.push(Entry::new(Role::You, prompt));
                                let model = ui.model.clone();
                                let result =
                                    export_trajectory(&ui.entries, &model, &ui.repo, &ui.branch);
                                ui.entries.push(Entry::new(
                                    Role::Notice,
                                    format!(
                                        "exported {} steps to {} (copied: {})",
                                        result.steps, result.path, result.copied
                                    ),
                                ));
                            } else {
                                ui.entries.push(Entry::new(Role::You, prompt.clone()));
                                ui.entries.push(Entry::new(Role::Assistant, String::new()));
                                ui.loading = true;

                                let mut session = CoderRuntimeSession::new();
                                session.agents = ui.agents.clone();
                                let tx = tx.clone();
                                tokio::spawn(async move {
                                    let _ = session.execute_turn(&prompt, tx).await;
                                });
                            }
                        }
                    }
                    KeyEvent {
                        code: KeyCode::Backspace,
                        ..
                    } => {
                        ui.composer.pop();
                    }
                    KeyEvent {
                        code: KeyCode::Up, ..
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
