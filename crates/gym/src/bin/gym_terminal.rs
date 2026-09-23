//! `gym-terminal`: the Gym's records, read in the terminal.
//!
//! The binary is the shell around [`gym::tui`]. It owns the terminal — raw
//! mode, the alternate screen, the cursor — and it gives all three back on
//! every way out, including a panic. Everything else is the module's.
//!
//! It reads records and runs nothing. There are no doors, no network, and no
//! credentials: this build opens the built-in fixture, and reading a real
//! receipt chain from disk arrives with the store.
//!
//! ```text
//! gym-terminal            # read the fixture in the terminal
//! gym-terminal --print    # write all five views to stdout and exit
//! ```

use std::io::{self, Stdout, Write, stdout};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use crossterm::{cursor, execute};
use gym::terminal_bench;
use gym::terminal_bench_tui;
use gym::tui::{Action, App, KeyLike, Records, View, ladder_from_environment};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

/// The width and height `--print` renders at, for a pipe that has no size.
const PRINT_WIDTH: u16 = 120;
const PRINT_HEIGHT: u16 = 36;

fn main() -> io::Result<()> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments
        .iter()
        .any(|argument| argument == "--terminal-bench")
    {
        return terminal_bench_mode(&arguments);
    }
    match arguments.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        [] => run(),
        ["--print"] => print(),
        ["--help" | "-h"] => {
            println!("{USAGE}");
            Ok(())
        }
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}

const USAGE: &str = "\
gym-terminal: the Gym's records, read in the terminal.

Usage:
  gym-terminal            Read the records in the terminal.
  gym-terminal --print    Write all five decision views to stdout and exit.
  gym-terminal --terminal-bench [--print] [--jobs-dir PATH] [--traces-dir PATH] [--samples-dir PATH]
                            [--runs-dir PATH] [--minitasks-dir PATH] [--no-jobs] [--no-traces]
                            [--no-samples] [--no-runs] [--no-minitasks]
  gym-terminal --help     Print this message.

The default decision-model views open a built-in fixture. Terminal-Bench
views read local Harbor jobs and retained evidence. Neither mode runs a
door or opens a network connection.

Keys:
  1-5            Open the decision scoreboard, families, ladder, row, or chain.
  1-9            Open the Terminal-Bench overview, comparison, attempt,
                 evidence, history, runbooks, Coder One components,
                 requirement maps, or mini-task runs.
  tab, h, l      Walk the views.
  j, k, arrows   Move the cursor.
  g, G           Jump to the first or last item.
  enter          Open the inspector on the selection.
  q, esc         Leave.";

fn terminal_bench_mode(arguments: &[String]) -> io::Result<()> {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let repo =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench");
    let mut jobs = home.map(|path| path.join(".openagents/terminal-bench/jobs"));
    let mut traces = Some(repo.join("traces"));
    let mut samples = Some(repo.join("samples"));
    let mut runs = gym::coder_components::default_runs_dir();
    let mut minitasks = gym::coder_minitasks::default_runs_dir();
    let mut print_only = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--terminal-bench" => {}
            "--print" => print_only = true,
            "--no-jobs" => jobs = None,
            "--no-traces" => traces = None,
            "--no-samples" => samples = None,
            "--no-runs" => runs = None,
            "--no-minitasks" => minitasks = None,
            "--jobs-dir" | "--traces-dir" | "--samples-dir" | "--runs-dir" | "--minitasks-dir" => {
                let Some(path) = arguments.get(index + 1) else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "directory flag needs a path",
                    ));
                };
                match arguments[index].as_str() {
                    "--jobs-dir" => jobs = Some(path.into()),
                    "--traces-dir" => traces = Some(path.into()),
                    "--runs-dir" => runs = Some(path.into()),
                    "--minitasks-dir" => minitasks = Some(path.into()),
                    _ => samples = Some(path.into()),
                }
                index += 1;
            }
            _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, USAGE)),
        }
        index += 1;
    }
    let records =
        terminal_bench::Records::load(jobs.as_deref(), traces.as_deref(), samples.as_deref());
    let components = gym::coder_components::report(runs.as_deref(), &records);
    let requirements = gym::coder_requirements::report(runs.as_deref(), &records);
    let (minitask_runs, minitask_errors) = minitasks
        .as_deref()
        .map(gym::coder_minitasks::load)
        .unwrap_or_default();
    let mut app = terminal_bench_tui::App::new(records)
        .with_components(components)
        .with_requirements(requirements)
        .with_minitasks(minitask_runs, minitask_errors);
    if print_only {
        let mut out = stdout().lock();
        for view in terminal_bench_tui::View::ALL {
            app.open(view);
            writeln!(out, "{}\n", app.to_text(150, app.print_height()))?;
        }
        return Ok(());
    }
    run_tbench(app)
}

fn run_tbench(app: terminal_bench_tui::App) -> io::Result<()> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        hook(info);
    }));
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, cursor::Hide)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(out))?;
    let result = draw_tbench(&mut terminal, app);
    restore();
    let _ = std::panic::take_hook();
    result
}

fn draw_tbench(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    mut app: terminal_bench_tui::App,
) -> io::Result<()> {
    loop {
        terminal.draw(|frame| app.render(frame.area(), frame.buffer_mut()))?;
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            let control = key.modifiers.contains(KeyModifiers::CONTROL);
            match key.code {
                KeyCode::Char('c' | 'd') if control => return Ok(()),
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Char('j') | KeyCode::Down => app.down(),
                KeyCode::Char('k') | KeyCode::Up => app.up(),
                KeyCode::Char('l') | KeyCode::Right | KeyCode::Tab => app.next(),
                KeyCode::Char('h') | KeyCode::Left | KeyCode::BackTab => app.previous(),
                KeyCode::Char('g') | KeyCode::Home => app.home(),
                KeyCode::Char('G') | KeyCode::End => app.end(),
                KeyCode::Enter => app.inspect(),
                KeyCode::Char(digit) => {
                    if let Some(view) = terminal_bench_tui::View::from_digit(digit) {
                        app.open(view);
                    }
                }
                _ => {}
            }
        }
    }
}

/// Writes every view to stdout, so the terminal can be read where no
/// terminal exists.
fn print() -> io::Result<()> {
    let mut app = App::new(Records::fixture(), ladder_from_environment());
    let mut out = stdout().lock();
    for view in View::ALL {
        app.open(view);
        write!(out, "{}", app.to_text(PRINT_WIDTH, PRINT_HEIGHT))?;
        writeln!(out)?;
    }
    Ok(())
}

/// Takes the terminal, draws until the reader leaves, and gives it back.
fn run() -> io::Result<()> {
    // The hook runs before the panic message prints, so the message lands on
    // a terminal that is out of raw mode and off the alternate screen.
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        hook(info);
    }));

    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, cursor::Hide)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(out))?;

    let result = draw(&mut terminal);

    restore();
    // The terminal is already back; drop the hook so a later panic prints
    // the way the runtime meant it to.
    let _ = std::panic::take_hook();
    result
}

/// Leaves raw mode, the alternate screen, and a hidden cursor behind,
/// whatever state they were in. Every step is best effort: a failure here
/// must not hide the error or the panic that brought us.
fn restore() {
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), LeaveAlternateScreen, cursor::Show);
}

/// The read loop: draw, wait for a key, repeat.
fn draw(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
    let mut app = App::new(Records::fixture(), ladder_from_environment());
    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            app.render(area, frame.buffer_mut());
        })?;

        // A resize redraws at the new size and nothing else. Every other
        // event that is not a key press is ignored.
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                let action = key_of(key.code, key.modifiers).map(|key| app.handle_key(key));
                if action == Some(Action::Quit) {
                    return Ok(());
                }
            }
            _ => continue,
        }
    }
}

/// Maps a terminal key onto the reader's own key names.
fn key_of(code: KeyCode, modifiers: KeyModifiers) -> Option<KeyLike> {
    let control = modifiers.contains(KeyModifiers::CONTROL);
    match code {
        KeyCode::Char('c' | 'd') if control => Some(KeyLike::Quit),
        KeyCode::Char('q') | KeyCode::Esc => Some(KeyLike::Quit),
        KeyCode::Char('j') | KeyCode::Down => Some(KeyLike::Down),
        KeyCode::Char('k') | KeyCode::Up => Some(KeyLike::Up),
        KeyCode::Char('l') | KeyCode::Right | KeyCode::Tab => Some(KeyLike::NextView),
        KeyCode::Char('h') | KeyCode::Left | KeyCode::BackTab => Some(KeyLike::PreviousView),
        KeyCode::Char('g') | KeyCode::Home => Some(KeyLike::Home),
        KeyCode::Char('G') | KeyCode::End => Some(KeyLike::End),
        KeyCode::Enter => Some(KeyLike::Inspect),
        KeyCode::Char(digit) => View::from_digit(digit).map(KeyLike::Open),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_keys_reach_the_reader() {
        assert_eq!(
            key_of(KeyCode::Char('q'), KeyModifiers::NONE),
            Some(KeyLike::Quit)
        );
        assert_eq!(
            key_of(KeyCode::Char('c'), KeyModifiers::CONTROL),
            Some(KeyLike::Quit)
        );
        assert_eq!(
            key_of(KeyCode::Down, KeyModifiers::NONE),
            Some(KeyLike::Down)
        );
        assert_eq!(
            key_of(KeyCode::Tab, KeyModifiers::NONE),
            Some(KeyLike::NextView)
        );
        assert_eq!(
            key_of(KeyCode::Enter, KeyModifiers::NONE),
            Some(KeyLike::Inspect)
        );
        assert_eq!(
            key_of(KeyCode::Char('3'), KeyModifiers::NONE),
            Some(KeyLike::Open(View::Ladder))
        );
        assert_eq!(key_of(KeyCode::Char('z'), KeyModifiers::NONE), None);
        // A plain `c` is not a quit; only the control chord is.
        assert_eq!(key_of(KeyCode::Char('c'), KeyModifiers::NONE), None);
    }

    #[test]
    fn the_fixture_opens_with_no_door_and_no_network() {
        let app = App::new(Records::fixture(), ladder_from_environment());
        let text = app.to_text(PRINT_WIDTH, PRINT_HEIGHT);
        assert!(text.contains("fixture"), "{text}");
        assert!(text.contains("scoreboard"), "{text}");
    }
}
