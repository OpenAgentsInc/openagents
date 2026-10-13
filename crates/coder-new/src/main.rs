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
    // The floors' hook a Codex run calls (#11475).
    if args
        .first()
        .is_some_and(|command| command == coder_new::floors::HOOK_COMMAND)
    {
        coder_new::floors::hook_main();
        return Ok(());
    }
    // The hook bridge a harness with only command hooks runs (#11500).
    if args
        .first()
        .is_some_and(|command| command == coder_new::floors::BRIDGE_COMMAND)
    {
        std::process::exit(coder_new::floors::bridge_main(&args[1..]));
    }
    // Coder's tools over MCP stdio, for a harness that takes MCP (#11501).
    if args
        .first()
        .is_some_and(|command| command == coder_new::claude_tools::STDIO_COMMAND)
    {
        std::process::exit(coder_new::claude_tools::stdio_main(&args[1..]));
    }
    // The squeezer a Claude Code session's Bash runs a noisy command
    // through (#11486).
    if args
        .first()
        .is_some_and(|command| command == squeeze::CODER_SUBCOMMAND)
    {
        std::process::exit(squeeze::main(&args[1..]));
    }
    if args.first().is_some_and(|command| command == "trace") {
        return trace_command(&args[1..]);
    }
    if args.first().is_some_and(|command| command == "export") {
        return export_command(&args[1..]);
    }
    if args.first().is_some_and(|command| command == "sync") {
        return sync_command(&args[1..]);
    }
    if args.first().is_some_and(|command| command == "update") {
        return update_command(&args[1..]);
    }
    if args.first().is_some_and(|command| command == "comms") {
        return coder_new::comms::command(&args[1..]);
    }
    let mut issue_run = None;
    let mut solver = None;
    if args.len() >= 2
        && args[0] == "solver"
        && (args[1] == "ledger"
            || args[1] == "queue"
            || args[1] == "hand-backs"
            || args[1] == "conform"
            || args[1] == "watch"
            || args[1] == "merge")
    {
        let runtime = tokio::runtime::Runtime::new()?;
        let text = if args[1] == "ledger" {
            runtime.block_on(coder_new::solver::ledger::command(&args[2..]))
        } else if args[1] == "merge" {
            runtime.block_on(coder_new::solver::merge::command(&args[2..]))
        } else if args[1] == "hand-backs" {
            coder_new::solver::rollup::command(&args[2..])
        } else if args[1] == "watch" {
            runtime.block_on(coder_new::solver::sense::watch(&args[2..]))
        } else if args[1] == "conform" {
            coder_new::solver::conform::command(&args[2..])
        } else {
            runtime.block_on(coder_new::solver::queue::command(&args[2..]))
        }
        .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;
        print!("{text}");
        return Ok(());
    }
    let args = if args.first().is_some_and(|command| command == "solver") {
        if args
            .iter()
            .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
        {
            println!("{}", coder_new::solver::USAGE);
            return Ok(());
        }
        let options = coder_new::solver::Options::parse(&args[1..])
            .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;
        if options.inner.plain || !io::IsTerminal::is_terminal(&io::stdout()) {
            let snapshot = options.inner.snapshot.clone();
            let json = options.json;
            return run_plain(coder_new::solver::start(options), snapshot, json);
        }
        solver = Some(options);
        Vec::new()
    } else if args.first().is_some_and(|command| command == "issue-run") {
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
    app.routing_records = true;
    // Each turn keeps the files it changed, for /rewind (#11515).
    app.rewind_records = true;
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
    if let Some(dir) = &state {
        coder_new::home::keep_records_under(dir);
    }
    if capture {
        if models {
            app.plugins.enabled = true;
            app.open_models();
        }
        return io::stdout().write_all(snapshot::svg(&mut app, 110, 36).as_bytes());
    }
    app.interactive_disclosures = true;
    // The rail says Auto until a harness is picked or routed (#11524), so
    // nothing waits on the Claude account login: it is read on a thread of
    // its own, ready for the first turn's routing. The harness pick's
    // facts load the same way (`--snapshot` renders without either).
    std::thread::spawn(|| {
        let _ = coder_new::claude::available();
    });
    coder_new::harness_pick::start(state.clone().or_else(coder_new::home::data_dir));
    let openagents_root = model_access::store::openagents_dir();
    let update = state
        .clone()
        .or_else(|| coder_new::home::data_dir())
        .and_then(|dir| start_update(&dir, &mut app));
    let update_events = update
        .as_ref()
        .and_then(|context| coder_new::update::spawn_check(context.clone()));
    if let Some(store) = state.clone().or_else(|| coder_new::home::data_dir()) {
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
    if let Some(options) = solver {
        app.set_mode(Mode::Solver);
        app.watch_issue_run(coder_new::solver::start(options));
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
        let mut status_report = String::new();
        let mut tty_checked = Instant::now();
        loop {
            app.elapsed_seconds = started.elapsed().as_secs();
            // A terminal that went away leaves `read` returning at once, so
            // the poll below would spin a core forever (#11429): end instead.
            if tty_checked.elapsed() >= Duration::from_secs(2) {
                tty_checked = Instant::now();
                if terminal_detached() {
                    app.cancel_request();
                    app.persist_session(true);
                    break;
                }
            }
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
            report_program_status(&app, &mut status_report)?;
            execute!(io::stdout(), BeginSynchronizedUpdate)?;
            terminal.draw(|frame| ui::render(frame, &mut app))?;
            execute!(io::stdout(), EndSynchronizedUpdate)?;
            // A message waiting for System One's route is sent as soon as
            // the answer lands, not at the next tick (#11460).
            let wait = next_tick.saturating_duration_since(Instant::now());
            let wait = if app.routing_pending() {
                wait.min(Duration::from_millis(10))
            } else {
                wait
            };
            if event::poll(wait)? {
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
        report_program_status_clear(&mut status_report)?;
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

/// `coder comms SUBCOMMAND --channel DIR --as NAME ...` (#11346): the
/// external-engine half of the agent channel. An engine that is not Coder
/// itself, or shares no process with it, reaches the same channel through
/// this command, which reads and writes the same `log.jsonl` and
/// `state.json`.
fn issue_run_plain(options: coder_new::issue_run::Options) -> io::Result<()> {
    let snapshot = options.snapshot.clone();
    run_plain(coder_new::issue_run::start(options), snapshot, false)
}

/// A run's feed printed as text, or as one JSON line per change when
/// `json`, until the run ends.
fn run_plain(
    feed: coder_new::issue_run::Feed,
    snapshot: Option<std::path::PathBuf>,
    json: bool,
) -> io::Result<()> {
    let width = 110;
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
        if json {
            writeln!(out, "{}", coder_new::issue_run::event_to_json(&event))?;
            out.flush()?;
            app.apply_issue_run(event);
            continue;
        }
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
    if !json {
        flush(&app, &mut printed, &mut out, true)?;
    }
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
    app.live.thinking = false;
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
    let dir =
        coder_new::home::data_dir().ok_or_else(|| io::Error::other("Set HOME to update Coder."))?;
    coder_new::update::command(rest, &dir, &mut io::stdout()).map_err(|message| {
        if message.starts_with("Unknown option") || message.starts_with("Choose") {
            io::Error::new(io::ErrorKind::InvalidInput, message)
        } else {
            io::Error::other(message)
        }
    })
}

/// `coder login [--pair CODE | --key] [--state DIR]` and `coder logout
/// [--state DIR]`.
fn account_command(command: &str, rest: &[String]) -> io::Result<()> {
    let usage = || {
        let pair = if command == "login" {
            " [--pair CODE | --key]"
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
    let mut key = false;
    let mut args = rest.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--state" => state = Some(args.next().ok_or_else(usage)?.into()),
            "--key" if command == "login" => key = true,
            // A key typed on the command line lands in shell history.
            value if command == "login" && value.starts_with("--key=") => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Don't put the key on the command line. Run coder login --key and paste it, or pipe it in: coder login --key < key.txt",
                ));
            }
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
        .or_else(|| coder_new::home::data_dir())
        .ok_or_else(|| io::Error::other("Set HOME, or pass --state DIR."))?;
    let mut out = io::stdout();
    if key && pair.is_some() {
        return Err(usage());
    }
    if command == "login" && key {
        coder_new::account::login_key_command(&dir, &mut out, &mut key_input()?)
    } else if command == "login" {
        coder_new::account::login_command(&dir, &mut out, pair.as_deref())
    } else {
        coder_new::account::logout_command(&dir, &mut out)
    }
    .map_err(io::Error::other)
}

/// Where `coder login --key` reads the key: piped standard input as is,
/// or a prompt on a terminal with typing hidden.
fn key_input() -> io::Result<io::Cursor<String>> {
    use std::io::{BufRead, IsTerminal, Write};
    let stdin = io::stdin();
    if !stdin.is_terminal() {
        let mut line = String::new();
        stdin.lock().read_line(&mut line)?;
        return Ok(io::Cursor::new(line));
    }
    eprint!("Paste your sign-in key (from Settings, Sign-in keys for agents): ");
    let _ = io::stderr().flush();
    let hidden = cfg!(unix)
        && std::process::Command::new("stty")
            .arg("-echo")
            .stdin(std::process::Stdio::inherit())
            .status()
            .is_ok_and(|status| status.success());
    let mut line = String::new();
    let read = stdin.lock().read_line(&mut line);
    if hidden {
        let _ = std::process::Command::new("stty")
            .arg("echo")
            .stdin(std::process::Stdio::inherit())
            .status();
        eprintln!();
    }
    read?;
    Ok(io::Cursor::new(line))
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
        .or_else(|| coder_new::home::data_dir())
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
/// `coder sync [--state DIR]`: send every chat here to the account now.
fn sync_command(rest: &[String]) -> io::Result<()> {
    if rest
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        println!(
            "Usage: coder sync [--state DIR]\n\nSend every chat on this computer, background agents' chats included, to your openagents.com account now. Sync must be on (/sync on in Coder)."
        );
        return Ok(());
    }
    let dir = match rest {
        [] => coder_new::home::data_dir(),
        [flag, dir] if flag == "--state" => Some(std::path::PathBuf::from(dir)),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Usage: coder sync [--state DIR]",
            ));
        }
    }
    .ok_or_else(|| io::Error::other("Set HOME, or pass --state DIR."))?;
    match coder_new::account_sync::sync_now(&dir) {
        Ok(text) => {
            println!("{text}");
            Ok(())
        }
        Err(message) => Err(io::Error::other(message)),
    }
}

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
        .or_else(|| coder_new::home::data_dir())
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
  coder login --key      Sign in with a sign-in key from Settings, pasted or piped in. Or set OPENAGENTS_TOKEN.
  coder logout           Sign this computer out.
  coder trace upload     Upload a chat to your account as a trace (coder trace --help).
  coder trace list       List the traces on your account.
  coder export --account Save everything on your account to one file (coder export --help).
  coder sync             Send every chat here to your account now (sync must be on).
  coder update           Install the newest Coder now (coder update --help).
  coder issue-run N      Play issue N from issue to pull request as a test run (coder issue-run --help).
  coder solver N         Solve issue N to a pull request in four constrained hands (coder solver --help).
  coder comms SUBCOMMAND Talk to the other subagents in a session over the agent channel (coder comms --help).

Options:
  --in DIR            Work in DIR instead of this directory.
  --models            Open the model picker first.
  --plugins           Open plugins first.
  --plugin-settings   Open the OpenRouter key settings first.
  --follow ID         Watch a chat another program is running; press any key to take it over.
  --state DIR         Keep Coder's data in DIR instead of ~/.openagents/coder.
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
  Esc stops a reply. Ctrl+N starts a new chat (the last one stays in /resume).
  Ctrl+C quits.
  Cmd+A selects everything in the input (Cmd+C copies, Cmd+X cuts) where the
  terminal passes Cmd keys on. Rex, Ghostty, iTerm2 and Terminal keep Cmd+A for
  their own Select All: use Ctrl+Shift+A there. Ctrl+A and Ctrl+E move to the
  start and end of the line.
  Drag over the conversation to copy text; double-click copies a word,
  triple-click a line.

Coder keeps itself up to date: once a day it checks for a newer version,
downloads and verifies it, and installs it when you quit. To only be told,
run coder update --mode notify; to stop checking, coder update --mode off.

Scripts and agents: openagents coder --help"
    )
}

/// Whether this terminal session has lost its terminal: the process was
/// left to launchd (its parent exited) or it has no controlling terminal
/// any more. Either way no keystroke can arrive again (#11429).
fn terminal_detached() -> bool {
    #[cfg(unix)]
    {
        detached(
            std::os::unix::process::parent_id(),
            std::fs::File::open("/dev/tty").is_ok(),
        )
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// [`terminal_detached`]'s rule, for a parent process id and whether
/// `/dev/tty` still opens.
fn detached(parent: u32, tty_opens: bool) -> bool {
    parent == 1 || !tty_opens
}

/// Writes an OSC 7501 program-status report when the composer's state
/// changed since the last report. A terminal that supports the protocol
/// (such as Rex) shows the state and picks the pane's icon from `app`;
/// one that does not ignores the sequence.
fn report_program_status(app: &App, previous: &mut String) -> io::Result<()> {
    use base64::Engine as _;
    let (state, msg) = app.program_status();
    let mut pairs = format!("state={state}:app=openagents");
    if state == "blocked" {
        pairs.push_str(":kind=permission");
        if let Some(msg) = msg {
            let msg = base64::engine::general_purpose::STANDARD
                .encode(msg.chars().take(200).collect::<String>());
            pairs.push_str(":msg=");
            pairs.push_str(&msg);
        }
    }
    if pairs != *previous {
        write!(io::stdout(), "\x1b]7501;{pairs}\x07")?;
        io::stdout().flush()?;
        *previous = pairs;
    }
    Ok(())
}

/// Drops every record this run reported; the process is leaving, so the
/// pane should keep nothing it attributed to us.
fn report_program_status_clear(previous: &mut String) -> io::Result<()> {
    if !previous.is_empty() {
        io::stdout().write_all(b"\x1b]7501;state=clear\x07")?;
        io::stdout().flush()?;
        previous.clear();
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::detached;

    #[test]
    fn a_session_left_to_launchd_or_without_a_tty_is_detached() {
        assert!(!detached(4242, true));
        assert!(detached(1, true), "reparented to launchd");
        assert!(detached(4242, false), "no controlling terminal");
    }
}
