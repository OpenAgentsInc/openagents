//! Interactive coder-lite TUI session
//!
//! Key handling follows the pattern used in grok-build's ratatui-textarea and
//! grok-pager: destructure `crossterm::event::KeyEvent` by `code` and
//! `modifiers` so control chords do not fall through to plain character input.

use crate::acp;
use crate::acp_harness::{AcpEvent, AcpHarness};
use crate::runtime::{CoderRuntimeSession, Control};
use crate::tui::{CoderUi, Entry, Role};
use std::sync::mpsc;
use std::path::PathBuf;
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

#[derive(Debug, Clone)]
pub enum DelegateControl {
    Tool { agent: String, title: String },
    Text(String),
    Done,
    Error(String),
}

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
    let (delegate_tx, delegate_rx) = mpsc::channel::<DelegateControl>();
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
                    if let Some(last) = ui.entries.last_mut() {
                        last.text.push_str(&chunk);
                        ui.scroll_override = None;
                    }
                }
                Control::Done => ui.loading = false,
            }
        }

        while let Ok(delegate) = delegate_rx.try_recv() {
            match delegate {
                DelegateControl::Tool { agent, title } => {
                    ui.entries.push(Entry {
                        role: Role::Tool,
                        text: format!("delegate {}: {}", agent, title),
                        output: Some(Vec::new()),
                    });
                    ui.scroll_override = None;
                }
                DelegateControl::Text(chunk) => {
                    if let Some(last) = ui.entries.last_mut() {
                        if last.role == Role::Tool {
                            last.output.get_or_insert_with(Vec::new).push(chunk);
                        }
                    }
                    ui.scroll_override = None;
                }
                DelegateControl::Done => {}
                DelegateControl::Error(message) => {
                    ui.entries.push(Entry {
                        role: Role::Notice,
                        text: format!("delegate error: {}", message),
                        output: None,
                    });
                    ui.scroll_override = None;
                }
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
                            ui.composer.clear();
                            ui.scroll_override = None;

                            if let Some(rest) = prompt.strip_prefix("/delegate") {
                                let rest = rest.trim_start();
                                if let Some((agent_id, task)) = rest.split_once(' ') {
                                    let agent_id = agent_id.to_string();
                                    let task = task.trim().to_string();
                                    let agent = ui.agents.iter().find(|a| a.id == agent_id).cloned();
                                    ui.entries.push(Entry {
                                        role: Role::You,
                                        text: prompt,
                                        output: None,
                                    });
                                    match agent {
                                        Some(agent) => {
                                            ui.entries.push(Entry {
                                                role: Role::Tool,
                                                text: format!("delegate {}: starting", agent_id),
                                                output: Some(Vec::new()),
                                            });
                                            let cwd =
                                                std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                                            let delegate_tx = delegate_tx.clone();
                                            tokio::spawn(async move {
                                                let harness = AcpHarness {
                                                    command: agent.command,
                                                    args: agent.args,
                                                };
                                                let result = harness
                                                    .run(&task, &cwd, |event| match event {
                                                        AcpEvent::Tool { kind: _, title } => {
                                                            let _ = delegate_tx.send(
                                                                DelegateControl::Tool {
                                                                    agent: agent_id.clone(),
                                                                    title,
                                                                },
                                                            );
                                                        }
                                                        AcpEvent::Text { chunk } => {
                                                            let _ = delegate_tx
                                                                .send(DelegateControl::Text(chunk));
                                                        }
                                                        _ => {}
                                                    })
                                                    .await;
                                                if let Err(e) = result {
                                                    let _ = delegate_tx
                                                        .send(DelegateControl::Error(e.to_string()));
                                                } else {
                                                    let _ = delegate_tx.send(DelegateControl::Done);
                                                }
                                            });
                                        }
                                        None => {
                                            ui.entries.push(Entry {
                                                role: Role::Notice,
                                                text: format!("unknown ACP agent: {}", agent_id),
                                                output: None,
                                            });
                                        }
                                    }
                                } else {
                                    ui.entries.push(Entry {
                                        role: Role::Notice,
                                        text: "usage: /delegate <agent> <prompt>".to_string(),
                                        output: None,
                                    });
                                }
                            } else {
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
                                ui.loading = true;

                                let mut session = CoderRuntimeSession::new();
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
