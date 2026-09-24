//! `gym-terminal`: the Gym's records, read in the terminal.
//!
//! The binary is the shell around [`gym::tui`]. It owns the terminal — raw
//! mode, the alternate screen, the cursor — and it gives all three back on
//! every way out, including a panic. Everything else is the module's.
//!
//! It reads records and runs nothing. The decision views have no doors, no
//! network, and no credentials: this build opens the built-in fixture, and
//! reading a real receipt chain from disk arrives with the store. The one
//! exception is the Runs pane's learning order: when you choose it, the
//! pane asks Jev about finished runs it hasn't judged yet, with the key in
//! `TYPESAFE_API_KEY` or `~/.openagents/jev.json`. `--no-jev` turns that off.
//!
//! ```text
//! gym-terminal            # read the fixture in the terminal
//! gym-terminal --print    # write all five views to stdout and exit
//! gym-terminal --terminal-bench   # recent Terminal-Bench runs, then the expert views
//! ```

use std::io::{self, Stdout, Write, stdout};

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseButton, MouseEventKind,
};
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
                            [--checks-dir PATH] [--no-samples] [--no-runs] [--no-minitasks]
                            [--no-checks] [--no-jev] [--no-index] [--head-to-head]
                            [--task TASK [--left TEXT] [--right TEXT|pass|best] [--at SECONDS]]
  gym-terminal --help     Print this message.

--task opens head-to-head replay already playing one pair on TASK. --left
and --right pick the attempt on each side whose identity or description
contains TEXT; --right pass picks the newest public attempt that passed and
is on this computer, and --right best the cheapest passing one. In an open
run, w does the same: it compares that run with the cheapest public attempt
that passed its task.

The default decision-model views open a built-in fixture. Terminal-Bench
views read local Harbor jobs and retained evidence. Neither mode calls a
decision model, except that the Runs pane asks Jev when you sort by what is
worth learning from; --no-jev stops that too.

Terminal-Bench views open from the startup index in ~/.openagents/gym/index:
a run or attempt whose files are unchanged since the last start is read
from it, and only new and changed ones are parsed. --no-index parses
everything and keeps nothing. GYM_STARTUP_TIMES=1 prints each loading
stage's time to stderr.

Terminal-Bench opens on the Runs pane: recent runs in plain words.
  arrows, j, k   Move.
  enter          Open a run's summary.
  t              Open, or switch to, the run's transcript.
  enter, space   In a transcript, open or close the selected step.
  e              In a transcript, open or close every step.
  d              In a summary, show the details experts use.
  /              Search by task, agent, or batch.
  a, o, c        Filter by agent, filter by outcome, clear the filters.
  l              Switch between newest first and most worth learning from
                 first. Jev judges each finished run once; the choice is
                 remembered in ~/.openagents/gym/runs-pane.json.
  x              Mark the run, or the selected transcript step, as bad:
                 type a note, move with the arrows, and press tab to tag
                 a judgment; enter saves, esc cancels.
  v              Mark the run fine: you read it and nothing is wrong.
  u              Remove the mark on the run or the selected step. Marks
                 are kept in ~/.openagents/gym/marks/marks.jsonl, the
                 store `gym runs mark` writes.
  esc            Go back.
  q              Leave.

Expert views:
  1-5            Open the decision scoreboard, families, ladder, row, or chain.
  1-9, 0         Open the Terminal-Bench overview, comparison, attempt,
                 evidence, history, runbooks, Coder One components,
                 requirement maps, mini-task runs, or the selected
                 attempt's executor system prompt.
  b, m, r        Open the briefings, the outcome matrix, or the router.
  f              Open the live view: attempts in progress, read again every
                 two seconds while it is open.
  w              Open the experiment pulse: every targeted experiment, and
                 the chosen one's arms, stopping verdict, component health,
                 and notable trials, read again every two seconds. Code
                 only; enter on an experiment shows its pulse.
  tab, h, l      Walk the views.
  j, k, arrows   Move the cursor.
  g, G           Jump to the first or last item.
  enter          Open the inspector on the selection.
  esc            Go back to the Runs pane.
  q              Leave.";

fn terminal_bench_mode(arguments: &[String]) -> io::Result<()> {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let repo =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench");
    let mut jobs = home.map(|path| path.join(".openagents/terminal-bench/jobs"));
    let mut traces = Some(repo.join("traces"));
    let mut samples = Some(repo.join("samples"));
    let mut runs = gym::coder_components::default_runs_dir();
    let mut minitasks = gym::coder_minitasks::default_runs_dir();
    let mut checks = gym::coder_coverage::default_dir();
    let mut print_only = false;
    let mut head_to_head = false;
    let (mut task, mut left, mut right) = (None, None, None);
    let mut at: Option<i64> = None;
    let mut jev = true;
    let mut index_dir = gym::index::default_dir();
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
            "--no-checks" => checks = None,
            "--no-jev" => jev = false,
            "--no-index" => index_dir = None,
            "--head-to-head" => head_to_head = true,
            "--at" => {
                let seconds = arguments
                    .get(index + 1)
                    .and_then(|value| value.parse::<f64>().ok())
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidInput, "--at needs seconds")
                    })?;
                at = Some((seconds * 1000.0) as i64);
                index += 1;
            }
            "--task" | "--left" | "--right" => {
                let Some(value) = arguments.get(index + 1).cloned() else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("{} needs a value", arguments[index]),
                    ));
                };
                match arguments[index].as_str() {
                    "--task" => task = Some(value),
                    "--left" => left = Some(value),
                    _ => right = Some(value),
                }
                head_to_head = true;
                index += 1;
            }
            "--jobs-dir" | "--traces-dir" | "--samples-dir" | "--runs-dir" | "--minitasks-dir"
            | "--checks-dir" => {
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
                    "--checks-dir" => checks = Some(path.into()),
                    _ => samples = Some(path.into()),
                }
                index += 1;
            }
            _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, USAGE)),
        }
        index += 1;
    }
    if !print_only {
        eprintln!("gym-terminal: loading Terminal-Bench records, runs, and replays...");
    }
    let mut clock = Stopwatch::from_environment();
    let index = index_dir.as_deref();
    let records = terminal_bench::Records::load_indexed(
        jobs.as_deref(),
        traces.as_deref(),
        samples.as_deref(),
        index,
    );
    clock.lap("records");
    let components = gym::coder_components::report_indexed(runs.as_deref(), &records, index)
        .with_repair(gym::coder_repair::default_dir().as_deref());
    clock.lap("components");
    let requirements = gym::coder_requirements::report(runs.as_deref(), &records);
    clock.lap("requirements");
    let (minitask_runs, minitask_errors) = minitasks
        .as_deref()
        .map(gym::coder_minitasks::load)
        .unwrap_or_default();
    clock.lap("minitasks");
    let briefing = gym::coder_briefing::report(runs.as_deref(), &records);
    clock.lap("briefing");
    let live = gym::coder_live::Sources {
        minitasks: minitasks.clone(),
        jobs: jobs.clone(),
        ..gym::coder_live::Sources::default()
    };
    let (studies, study_errors) = gym::coder_study::load(&gym::coder_study::default_dirs());
    clock.lap("studies");
    let runs = gym::runs::Catalog::load(gym::runs::Sources {
        jobs: jobs.clone(),
        traces: traces.clone(),
        index: index_dir.clone(),
        ..gym::runs::Sources::standard()
    });
    clock.lap("runs catalog");
    clock.note(&format!(
        "index: {} attempts and {} runs read from it, {} and {} parsed",
        records.index.hits,
        runs.index_stats().hits,
        records.index.parsed,
        runs.index_stats().parsed
    ));
    let app = terminal_bench_tui::App::new(records);
    clock.lap("expert views");
    let coverage = checks
        .as_deref()
        .map(gym::coder_coverage::load_dir)
        .unwrap_or_default();
    clock.lap("coverage");
    let pane = terminal_bench_tui_runs(runs, jev && !print_only);
    clock.lap("runs pane");
    let mut app = app
        .with_components(components)
        .with_requirements(requirements)
        .with_minitasks(minitask_runs, minitask_errors)
        .with_briefing(briefing)
        .with_coverage(coverage)
        .with_live(live)
        .with_pulse(gym::terminal_bench_experiment::default_dir(), jobs.clone())
        .with_studies(studies, study_errors)
        .with_runs(pane);
    clock.lap("assembly");
    clock.total();
    if let Some(task) = &task {
        app.open_replay_on(task, left.as_deref(), right.as_deref())
            .map_err(|message| io::Error::new(io::ErrorKind::NotFound, message))?;
        if let Some(millis) = at {
            app.seek_replay(millis);
        }
    } else if head_to_head {
        app.open_replay();
    }
    if print_only && head_to_head {
        println!("{}", app.to_text(150, 40));
        return Ok(());
    }
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

/// Startup timings on stderr when `GYM_STARTUP_TIMES` is set: each
/// stage's time since the last, then the total.
struct Stopwatch {
    start: std::time::Instant,
    last: std::time::Instant,
    on: bool,
}

impl Stopwatch {
    fn from_environment() -> Self {
        let now = std::time::Instant::now();
        Stopwatch {
            start: now,
            last: now,
            on: std::env::var_os("GYM_STARTUP_TIMES").is_some(),
        }
    }

    fn lap(&mut self, stage: &str) {
        let now = std::time::Instant::now();
        if self.on {
            eprintln!(
                "gym-terminal: {stage:<14} {:>8.1} ms",
                (now - self.last).as_secs_f64() * 1e3
            );
        }
        self.last = now;
    }

    fn note(&self, text: &str) {
        if self.on {
            eprintln!("gym-terminal: {text}");
        }
    }

    fn total(&self) {
        if self.on {
            eprintln!(
                "gym-terminal: {:<14} {:>8.1} ms",
                "total",
                self.start.elapsed().as_secs_f64() * 1e3
            );
        }
    }
}

/// The Runs pane over `catalog`, with the rankings kept under
/// `~/.openagents/gym/learning` and, when `jev` holds, hosted Jev to rank
/// what isn't ranked yet.
fn terminal_bench_tui_runs(catalog: gym::runs::Catalog, jev: bool) -> gym::runs_tui::Pane {
    use gym::runs_learning::{Judge, Store, default_dir};
    let judge = if jev {
        Judge::from_environment()
    } else {
        Judge::Off("--no-jev turns Jev off".to_owned())
    };
    let prefs = default_dir().and_then(|dir| dir.parent().map(|gym| gym.join("runs-pane.json")));
    gym::runs_tui::Pane::new(catalog)
        .with_learning(
            Store::open(default_dir()),
            judge,
            gym::terminal_bench_reference::Reference::checked(),
            prefs,
        )
        .with_marks(
            gym::runs_marks::Marks::open(gym::runs_marks::default_dir()),
            gym::runs_marks::default_author(),
        )
}

/// A terminal key as the Runs pane reads it.
fn runs_key(code: KeyCode) -> Option<gym::runs_tui::Key> {
    use gym::runs_tui::Key;
    Some(match code {
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Back,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Tab => Key::Tab,
        KeyCode::Char(c) => Key::Char(c),
        _ => return None,
    })
}

fn run_tbench(app: terminal_bench_tui::App) -> io::Result<()> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        hook(info);
    }));
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, cursor::Hide, EnableMouseCapture)?;
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
    let mut read_at = std::time::Instant::now();
    let mut replay_at = std::time::Instant::now();
    loop {
        app.advance_replay(replay_at.elapsed());
        replay_at = std::time::Instant::now();
        terminal.draw(|frame| app.render(frame.area(), frame.buffer_mut()))?;
        if app.replaying() && !event::poll(std::time::Duration::from_millis(50))? {
            continue;
        }
        // While Coder One answers a question, its events are read every
        // quarter second; the runs themselves still every two.
        if !app.replaying() && app.asking() && !event::poll(std::time::Duration::from_millis(250))?
        {
            app.poll_ask();
            if read_at.elapsed() >= std::time::Duration::from_secs(2) {
                app.refresh_live();
                read_at = std::time::Instant::now();
            }
            continue;
        }
        // The live view reads its attempts again every two seconds; every
        // other view waits for a key.
        if !app.replaying()
            && !app.asking()
            && app.follows()
            && !event::poll(std::time::Duration::from_secs(2))?
        {
            app.refresh_live();
            read_at = std::time::Instant::now();
            continue;
        }
        let read = event::read()?;
        // The wheel scrolls text and a click opens a step, in the Runs pane
        // and in head-to-head replay.
        if let Event::Mouse(mouse) = &read {
            if app.view() == terminal_bench_tui::View::Runs {
                let (column, row) = (mouse.column, mouse.row);
                let key = match mouse.kind {
                    MouseEventKind::ScrollUp => Some(gym::runs_tui::Key::WheelUp { column, row }),
                    MouseEventKind::ScrollDown => {
                        Some(gym::runs_tui::Key::WheelDown { column, row })
                    }
                    MouseEventKind::Down(MouseButton::Left) => {
                        Some(gym::runs_tui::Key::Click { column, row })
                    }
                    _ => None,
                };
                if let Some(key) = key {
                    app.advance_replay(replay_at.elapsed());
                    if app.runs_key(key) == gym::runs_tui::Reply::Quit {
                        return Ok(());
                    }
                    replay_at = std::time::Instant::now();
                }
            } else {
                match mouse.kind {
                    MouseEventKind::ScrollUp => app.up(),
                    MouseEventKind::ScrollDown => app.down(),
                    _ => {}
                }
            }
            continue;
        }
        if let Event::Key(key) = read {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            let control = key.modifiers.contains(KeyModifiers::CONTROL);
            if control && matches!(key.code, KeyCode::Char('c' | 'd')) {
                return Ok(());
            }
            if app.view() == terminal_bench_tui::View::Runs {
                if let Some(key) = runs_key(key.code) {
                    app.advance_replay(replay_at.elapsed());
                    match app.runs_key(key) {
                        gym::runs_tui::Reply::Quit => return Ok(()),
                        gym::runs_tui::Reply::Open(digit) => {
                            if let Some(view) = terminal_bench_tui::View::from_digit(digit) {
                                app.open(view);
                            }
                        }
                        gym::runs_tui::Reply::Handled => {}
                    }
                    replay_at = std::time::Instant::now();
                }
                continue;
            }
            match key.code {
                KeyCode::Char('q') => return Ok(()),
                KeyCode::Esc => {
                    if !app.back_to_runs() {
                        return Ok(());
                    }
                }
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
    let _ = execute!(
        stdout(),
        DisableMouseCapture,
        LeaveAlternateScreen,
        cursor::Show
    );
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
