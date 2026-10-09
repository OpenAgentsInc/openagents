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
            app.follow_tick();
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
    result.and(extra_restore)
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

Scripts and agents: openagents coder --help
Update Coder:       curl -fsSL https://openagents.com/cli/install.sh | bash
Update on Windows:  irm https://openagents.com/cli/install.ps1 | iex"
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
