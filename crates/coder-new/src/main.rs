use std::{
    io::{self, Write},
    time::{Duration, Instant},
};

use coder_new::{App, DEMO_AVAILABLE, Mode, live::Background, snapshot, ui};
#[cfg(unix)]
use crossterm::event::{
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::{
    cursor::{SetCursorStyle, Show},
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    },
    execute,
    terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate},
};

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            // `main` returning the error would print Rust's debug form
            // (`Error: Custom { kind: .. }`); people get the sentence.
            eprintln!("coder: {}", plain_error(&error));
            std::process::ExitCode::from(if error.kind() == io::ErrorKind::InvalidInput {
                2
            } else {
                1
            })
        }
    }
}

/// The error's sentence without the operating system's code suffix, such as
/// ` (os error 2)`.
fn plain_error(error: &io::Error) -> String {
    let text = error.to_string();
    match text.rfind(" (os error ") {
        Some(at) if text.ends_with(')') => text[..at].to_owned(),
        _ => text,
    }
}

fn run() -> io::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--version" | "-V"))
    {
        println!(
            "coder {} ({} {})",
            env!("CARGO_PKG_VERSION"),
            env!("CODER_GIT_COMMIT"),
            env!("CODER_GIT_TREE"),
        );
        return Ok(());
    }
    if let Some(command) = args
        .first()
        .filter(|command| matches!(command.as_str(), "login" | "logout"))
    {
        return account_command(command, &args[1..]);
    }
    if args.first().is_some_and(|command| command == "trace") {
        return trace_command(&args[1..]);
    }
    if args.first().is_some_and(|command| command == "export") {
        return export_command(&args[1..]);
    }
    if args.first().is_some_and(|command| command == "update") {
        return update_command(&args[1..]);
    }
    let mut issue_run = None;
    let args = if args.first().is_some_and(|command| command == "issue-run") {
        if args
            .iter()
            .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
        {
            println!("{}", coder_new::issue_run::USAGE);
            return Ok(());
        }
        let options = coder_new::issue_run::Options::parse(&args[1..])
            .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;
        if options.plain || !io::IsTerminal::is_terminal(&io::stdout()) {
            return issue_run_plain(options);
        }
        issue_run = Some(options);
        Vec::new()
    } else {
        args
    };
    let mut app = App::default();
    let mut capture = false;
    let mut models = false;
    let mut follow: Option<String> = None;
    let mut state: Option<std::path::PathBuf> = None;
    if !DEMO_AVAILABLE
        || !args.iter().any(|arg| arg == "--demo")
            && (!args.iter().any(|arg| arg == "--snapshot")
                || args.iter().any(|arg| arg == "--live"))
    {
        app.set_mode(Mode::Live);
    }
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let mut value = |name: &str| {
            args.next().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("{name} needs a value. Run coder --help to see the options."),
                )
            })
        };
        match arg.as_str() {
            "--follow" => follow = Some(value("--follow")?),
            "--state" => state = Some(value("--state")?.into()),
            "--in" => {
                let dir = value("--in")?;
                std::env::set_current_dir(&dir).map_err(|error| {
                    io::Error::new(
                        error.kind(),
                        format!("Cannot work in {dir}: {}.", plain_error(&error)),
                    )
                })?;
            }
            "--live" => app.set_mode(Mode::Live),
            "--demo" if DEMO_AVAILABLE => app.set_mode(Mode::Demo),
            "--demo" => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Demo mode is available only in local development builds.",
                ));
            }
            "--plugins" => app.open_plugins(),
            "--plugin-settings" => app.open_plugin_settings(),
            "--models" => models = true,
            "--snapshot" => capture = true,
            "--help" | "-h" => {
                println!("{}", help());
                return Ok(());
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Unknown option {arg}. Run coder --help to see the options."),
                ));
            }
        }
    }
    if capture {
        if models {
            app.plugins.enabled = true;
            app.open_models();
        }
        return io::stdout().write_all(snapshot::svg(&mut app, 110, 36).as_bytes());
    }
    app.interactive_disclosures = true;
    let openagents_root = model_access::store::openagents_dir();
    let update = state
        .clone()
        .or_else(|| openagents_root.as_ref().map(|root| root.join("coder-new")))
        .and_then(|dir| start_update(&dir, &mut app));
    let update_events = update
        .as_ref()
        .and_then(|context| coder_new::update::spawn_check(context.clone()));
    if let Some(store) = state
        .clone()
        .or_else(|| openagents_root.as_ref().map(|root| root.join("coder-new")))
    {
        app.account = coder_new::account::signed_in(&store);
        app.account_dir = Some(store.clone());
        app.attach_session_store(coder_new::sessions::Store::under(&store));
        app.start_sync();
        if let Err(error) = app.load_plugin_settings(coder_new::plugin_store::Store::under(&store))
        {
            app.notice = Some(error);
        }
    }
    // The desktop app's Agents panel lists this process's background
    // agents (#11180).
    if let Some(root) = &openagents_root {
        app.publish_agents(root);
    }
    match std::env::current_dir() {
        Ok(cwd) => match coder_new::credentials::load(&cwd, openagents_root.as_deref(), |name| {
            std::env::var(name).ok()
        }) {
            Ok(imported) => app.plugins.bootstrap_credentials(imported),
            Err(error) => {
                if app.notice.is_none() {
                    app.notice = Some(error);
                }
            }
        },
        Err(_) => {
            if app.notice.is_none() {
                app.notice =
                    Some("Cannot determine the working directory for plugin credentials.".into());
            }
        }
    }
    app.plugins.bundled.discover_acp(&|name| {
        if name == "CODER_ACP_CWD" {
            std::env::current_dir()
                .ok()
                .map(std::path::PathBuf::into_os_string)
        } else {
            std::env::var_os(name)
        }
    });
    if let Ok(cwd) = std::env::current_dir() {
        app.branch = std::process::Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&cwd)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|branch| branch.trim().to_owned())
            .filter(|branch| !branch.is_empty());
        app.cwd = Some(cwd);
    }
    if models {
        app.open_models();
    }
    if let Some(id) = &follow
        && !app.follow(id)
    {
        return Err(io::Error::other(
            app.notice
                .clone()
                .unwrap_or_else(|| "Cannot follow that conversation.".into()),
        ));
    }
    if app.mode == Mode::Live && app.plugins.enabled && app.plugins.key_configured {
        app.check_key();
    }
    let issue_snapshot = issue_run
        .as_ref()
        .and_then(|options| options.snapshot.clone());
    if let Some(options) = issue_run {
        app.watch_issue_run(coder_new::issue_run::start(options));
    }

    let mut terminal = match ratatui::try_init() {
        Ok(terminal) => terminal,
        Err(error) => {
            let _ = ratatui::try_restore();
            return Err(error);
        }
    };
    // Restore mouse capture and bracketed paste with Ratatui's terminal modes on panic.
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore_extras();
        previous_hook(info);
    }));
    let result: io::Result<()> = (|| {
        #[cfg(unix)]
        execute!(
            io::stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )?;
        execute!(
            io::stdout(),
            EnableBracketedPaste,
            EnableMouseCapture,
            SetCursorStyle::BlinkingBlock
        )?;
        let ratatui::style::Color::Rgb(r, g, b) = coder_new::theme::CURSOR else {
            unreachable!("Coder uses RGB colors");
        };
        write!(io::stdout(), "\x1b]12;#{r:02x}{g:02x}{b:02x}\x07")?;
        io::stdout().flush()?;
        let interval = Duration::from_millis(125);
        let started = Instant::now();
        let mut next_tick = started + interval;
        let mut background = Background::default();
        let mut catalog = coder_new::model_catalog::Loader::default();
        #[cfg(unix)]
        let mut plugins = background::Layout::from_env()
            .ok()
            .map(coder_new::plugin_catalog::Loader::new);
        loop {
            app.elapsed_seconds = started.elapsed().as_secs();
            if let Some(Ok(coder_new::update::Event::Notice(line))) = update_events
                .as_ref()
                .map(std::sync::mpsc::Receiver::try_recv)
            {
                app.update_line = Some(line);
            }
            app.follow_tick();
            app.poll_issue_run();
            background.sync(&mut app);
            app.persist_session(false);
            catalog.sync(&mut app);
            #[cfg(unix)]
            if let Some(loader) = &mut plugins {
                loader.sync(&mut app);
            }
            execute!(io::stdout(), BeginSynchronizedUpdate)?;
            terminal.draw(|frame| ui::render(frame, &mut app))?;
            // Keep the block blinking while progress updates move the terminal cursor.
            if app.cursor_blink_frame >= 4 {
                terminal.hide_cursor()?;
            }
            execute!(io::stdout(), EndSynchronizedUpdate)?;
            if event::poll(next_tick.saturating_duration_since(Instant::now()))? {
                let mut keep_running = app.handle(event::read()?);
                // Trackpads emit bursts; update the viewport once per drained batch.
                for _ in 0..255 {
                    if !keep_running || !event::poll(Duration::ZERO)? {
                        break;
                    }
                    keep_running = app.handle(event::read()?);
                }
                app.copy_export_path(coder_terminal::clipboard::to_clipboard);
                app.copy_pending(coder_terminal::clipboard::to_clipboard);
                if !keep_running {
                    app.cancel_request();
                    app.persist_session(true);
                    break;
                }
            }
            if Instant::now() >= next_tick {
                app.tick();
                next_tick = Instant::now() + interval;
            }
        }
        Ok(())
    })();
    app.cancel_request();
    app.persist_session(true);
    let extra_restore = restore_extras();
    ratatui::restore();
    if let Some(path) = issue_snapshot {
        write_issue_snapshot(&mut app, &path)?;
    }
    // Background agents end with the terminal; their worktrees and
    // transcripts stay (#11163).
    if app.fleet.running() > 0 {
        eprintln!(
            "Stopping {} background agent(s); their worktrees and transcripts are kept.",
            app.fleet.running()
        );
        app.stop_agents(Duration::from_secs(10));
    }
    // A version downloaded and verified this session installs now (#11128).
    if let Some(context) = &update
        && context.config.mode == coder_new::update::Mode::Auto
    {
        match coder_new::update::install_staged(context) {
            Ok(Some(version)) => {
                eprintln!("Updated Coder to {version}. It runs next time you start coder.");
            }
            Ok(None) => {}
            Err(message) => eprintln!("coder: {message}"),
        }
    }
    result.and(extra_restore)
}

/// `coder issue-run --plain`: the run printed as text, each item once it
/// has finished, as the terminal screen would draw it.
fn issue_run_plain(options: coder_new::issue_run::Options) -> io::Result<()> {
    let width = 110;
    let snapshot = options.snapshot.clone();
    let feed = coder_new::issue_run::start(options);
    let mut app = App::default();
    app.set_mode(Mode::Live);
    let mut printed = 0;
    let mut out = io::stdout();
    let flush =
        |app: &App, printed: &mut usize, out: &mut io::Stdout, all: bool| -> io::Result<()> {
            while let Some(entry) = app.live.entries.get(*printed) {
                let running = matches!(entry, coder_new::live::Entry::Tool { running: true, .. });
                if running && !all {
                    break;
                }
                for line in ui::transcript_text(std::slice::from_ref(entry), width) {
                    writeln!(out, "{line}")?;
                }
                *printed += 1;
            }
            out.flush()
        };
    while let Some(event) = feed.next() {
        let notice = match &event {
            coder_new::issue_run::Event::Notice(text) => Some(text.clone()),
            _ => None,
        };
        app.apply_issue_run(event);
        flush(&app, &mut printed, &mut out, false)?;
        if let Some(text) = notice {
            writeln!(out, "{text}")?;
        }
    }
    flush(&app, &mut printed, &mut out, true)?;
    if let Some(path) = snapshot {
        write_issue_snapshot(&mut app, &path)?;
    }
    Ok(())
}

/// Saves the whole conversation as one tall SVG picture of the screen.
fn write_issue_snapshot(app: &mut App, path: &std::path::Path) -> io::Result<()> {
    let width = 120;
    let rows = ui::transcript_text(&app.live.entries, width).len();
    let height = u16::try_from(rows + 12).unwrap_or(u16::MAX);
    app.live.busy = false;
    app.scroll = 0;
    std::fs::write(path, snapshot::svg(app, width, height))
}

/// Prepares automatic updates for this TUI session: none under CI, with
/// `CODER_UPDATE=off`, or in a debug build. A version an earlier session
/// downloaded but did not install is installed first, and on Unix the new
/// `coder` then runs in this one's place.
fn start_update(dir: &std::path::Path, app: &mut App) -> Option<coder_new::update::Context> {
    use coder_new::update::{self, InstallKind, Mode};
    let context = update::Context::from_env(dir).ok()?;
    let env = |name: &str| std::env::var(name).ok();
    if !context.config.automatic(&env, cfg!(debug_assertions))
        || context.kind == InstallKind::Source
    {
        return None;
    }
    if context.config.mode == Mode::Auto
        && let InstallKind::Standalone(bin) = &context.kind
    {
        match update::install_staged(&context) {
            Ok(Some(version)) => {
                eprintln!("Updated Coder to {version}.");
                #[cfg(unix)]
                {
                    use std::os::unix::process::CommandExt;
                    let coder = bin.join(update::commands(&context.platform).swap_remove(0));
                    let error = std::process::Command::new(&coder)
                        .args(std::env::args_os().skip(1))
                        .exec();
                    eprintln!("coder: cannot start Coder {version}: {error}");
                }
                #[cfg(not(unix))]
                let _ = bin;
                app.update_line = Some(format!(
                    "Coder {version} is installed. Restart Coder to use it."
                ));
                return None;
            }
            Ok(None) => {}
            Err(message) => app.notice = Some(message),
        }
    }
    app.update_line = context.cached_notice();
    Some(context)
}

/// `coder update [--check | --rollback | --mode MODE | --channel NAME]` (#11128).
fn update_command(rest: &[String]) -> io::Result<()> {
    let dir = model_access::store::openagents_dir()
        .map(|root| root.join("coder-new"))
        .ok_or_else(|| io::Error::other("Set HOME to update Coder."))?;
    coder_new::update::command(rest, &dir, &mut io::stdout()).map_err(|message| {
        if message.starts_with("Unknown option") || message.starts_with("Choose") {
            io::Error::new(io::ErrorKind::InvalidInput, message)
        } else {
            io::Error::other(message)
        }
    })
}

/// `coder login [--pair CODE] [--state DIR]` and `coder logout [--state DIR]`.
fn account_command(command: &str, rest: &[String]) -> io::Result<()> {
    let usage = || {
        let pair = if command == "login" {
            " [--pair CODE]"
        } else {
            ""
        };
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Usage: coder {command}{pair} [--state DIR]"),
        )
    };
    let mut state: Option<std::path::PathBuf> = None;
    let mut pair: Option<String> = None;
    let mut args = rest.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--state" => state = Some(args.next().ok_or_else(usage)?.into()),
            "--pair" if command == "login" => {
                let code = args.next().ok_or_else(usage)?;
                if !openagents_login::valid_pair(code) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "That pair code isn't right. Copy the command from the Connect your terminal page again.",
                    ));
                }
                pair = Some(code.clone());
            }
            _ => return Err(usage()),
        }
    }
    let dir = state
        .or_else(|| model_access::store::openagents_dir().map(|root| root.join("coder-new")))
        .ok_or_else(|| io::Error::other("Set HOME, or pass --state DIR."))?;
    let mut out = io::stdout();
    if command == "login" {
        coder_new::account::login_command(&dir, &mut out, pair.as_deref())
    } else {
        coder_new::account::logout_command(&dir, &mut out)
    }
    .map_err(io::Error::other)
}

/// `coder trace upload …` and `coder trace list [--state DIR]` (#11109).
fn trace_command(rest: &[String]) -> io::Result<()> {
    let mut args = rest.to_vec();
    let mut dir = None;
    if let Some(at) = args.iter().position(|arg| arg == "--state") {
        if at + 1 >= args.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "--state needs a folder.",
            ));
        }
        dir = Some(std::path::PathBuf::from(args.remove(at + 1)));
        args.remove(at);
    }
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
        || args.is_empty()
    {
        println!("{}", coder_new::trace_upload::USAGE);
        return Ok(());
    }
    let dir = dir
        .or_else(|| model_access::store::openagents_dir().map(|root| root.join("coder-new")))
        .ok_or_else(|| io::Error::other("Set HOME, or pass --state DIR."))?;
    let cwd = std::env::current_dir()?;
    match coder_new::trace_upload::run(&args, &dir, &cwd) {
        Ok(outcome) => {
            println!("{}", outcome.text());
            Ok(())
        }
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    }
}

/// `coder export --account [--output FILE] [--state DIR]` (#11134).
fn export_command(rest: &[String]) -> io::Result<()> {
    let mut args = rest.to_vec();
    let mut dir = None;
    if let Some(at) = args.iter().position(|arg| arg == "--state") {
        if at + 1 >= args.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "--state needs a folder.",
            ));
        }
        dir = Some(std::path::PathBuf::from(args.remove(at + 1)));
        args.remove(at);
    }
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
        || args.is_empty()
    {
        println!("{}", coder_new::account_export::USAGE);
        return Ok(());
    }
    let dir = dir
        .or_else(|| model_access::store::openagents_dir().map(|root| root.join("coder-new")))
        .ok_or_else(|| io::Error::other("Set HOME, or pass --state DIR."))?;
    let cwd = std::env::current_dir()?;
    match coder_new::account_export::run(&args, &dir, &cwd) {
        Ok(outcome) => {
            println!("{}", outcome.text());
            Ok(())
        }
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    }
}

fn help() -> String {
    let (demo_option, snapshot_mode) = if DEMO_AVAILABLE {
        (
            "  --demo              Use local example conversations.\n",
            "demo",
        )
    } else {
        ("", "live")
    };
    format!(
        "Coder, an AI coding agent in your terminal.

Usage:
  coder [OPTIONS]        Open Coder in this directory.
  coder login            Sign in to openagents.com: approve the code at https://openagents.com/device.
  coder logout           Sign this computer out.
  coder trace upload     Upload a chat to your account as a trace (coder trace --help).
  coder trace list       List the traces on your account.
  coder export --account Save everything on your account to one file (coder export --help).
  coder update           Install the newest Coder now (coder update --help).
  coder issue-run N      Play issue N from issue to pull request as a test run (coder issue-run --help).

Options:
  --in DIR            Work in DIR instead of this directory.
  --models            Open the model picker first.
  --plugins           Open plugins first.
  --plugin-settings   Open the OpenRouter key settings first.
  --follow ID         Watch a chat another program is running; press any key to take it over.
  --state DIR         Keep Coder's data in DIR instead of ~/.openagents/coder-new.
  --live              Use your providers and tools (the default).
{demo_option}  --snapshot          Print a 110x36 SVG picture of the {snapshot_mode} screen and exit.
  -V, --version       Print the version.
  -h, --help          Print this help.

Inside Coder, type / to see every command:
  /login, then /sync on    Save your chats to your account.
  /models                  Choose a model and reasoning level.
  /resume                  Reopen a saved chat.
  /export [path]           Save this chat to a file.
  /plugins                 Turn plugins on and add keys (also Ctrl+P, or Cmd+P on macOS).
  Esc stops a reply. Ctrl+C quits.
  Cmd+A selects everything in the input (Cmd+C copies, Cmd+X cuts); it needs a
  terminal with the kitty keyboard protocol. Elsewhere, Ctrl+Shift+A. Ctrl+A and
  Ctrl+E move to the start and end of the line.

Coder keeps itself up to date: once a day it checks for a newer version,
downloads and verifies it, and installs it when you quit. To only be told,
run coder update --mode notify; to stop checking, coder update --mode off.

Scripts and agents: openagents coder --help"
    )
}

fn restore_extras() -> io::Result<()> {
    #[cfg(unix)]
    execute!(io::stdout(), PopKeyboardEnhancementFlags)?;
    execute!(
        io::stdout(),
        EndSynchronizedUpdate,
        DisableBracketedPaste,
        DisableMouseCapture,
        SetCursorStyle::DefaultUserShape,
        Show
    )?;
    write!(io::stdout(), "\x1b]112\x07")?;
    io::stdout().flush()
}
