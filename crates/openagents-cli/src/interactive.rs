//! Interactive coder TUI session, diff rendering, keybindings & live transcript

use crate::cli::CoderArgs;
use crate::tui::BoxFrame;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::stdout;
use std::time::Duration;

pub async fn run_tui(args: CoderArgs, _token: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting interactive Coder session...");
    if !atty_is_terminal() {
        println!("Non-interactive terminal detected. Running basic prompt mode.");
        if let Some(prompt) = args.prompt {
            println!("User prompt: {}", prompt);
            println!("Coder response: Interactive session initialized in non-TTY mode.");
        }
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let frame = BoxFrame::new("openagents coder");
    let mut transcript = String::from("Welcome to OpenAgents Coder (Rust CLI v0.1.0)

Type your instructions or press Esc to exit.");
    if let Some(p) = args.prompt {
        transcript.push_str(&format!("

User: {}", p));
        transcript.push_str("
Agent: Ready to execute.");
    }

    loop {
        terminal.draw(|f| {
            let size = f.area();
            frame.render(f, size, &transcript);
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        break;
                    }
                    KeyCode::Esc => {
                        break;
                    }
                    KeyCode::Tab => {
                        transcript.push_str("
[Toggled reasoning effort]");
                    }
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    std::io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

fn atty_is_terminal() -> bool {
    // Check if stdin / stdout is terminal or running in harness
    std::io::IsTerminal::is_terminal(&std::io::stdin()) && std::io::IsTerminal::is_terminal(&std::io::stdout())
}
