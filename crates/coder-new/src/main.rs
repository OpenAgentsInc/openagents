use std::{
    io::{self, Write},
    time::{Duration, Instant},
};

use coder_new::{App, Mode, live::Background, snapshot, ui};
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

fn main() -> io::Result<()> {
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
    let mut app = App::default();
    let mut capture = false;
    let mut models = false;
    if !args.iter().any(|arg| arg == "--demo")
        && (!args.iter().any(|arg| arg == "--snapshot") || args.iter().any(|arg| arg == "--live"))
    {
        app.set_mode(Mode::Live);
    }
    for arg in args {
        match arg.as_str() {
            "--live" => app.set_mode(Mode::Live),
            "--demo" => app.set_mode(Mode::Demo),
            "--plugins" => app.open_plugins(),
            "--plugin-settings" => app.open_plugin_settings(),
            "--models" => models = true,
            "--snapshot" => capture = true,
            "--help" | "-h" => {
                println!(
                    "Coder terminal\n\nUsage: coder [--live | --demo] [--plugins | --plugin-settings | --models] [--snapshot]\n\n--live             Use enabled providers and tools (default).\n--demo             Use local example conversations.\n--plugins          Start with plugin management.\n--plugin-settings  Start with OpenRouter settings.\n--models           Open the model picker for an enabled provider.\n--snapshot         Write a 110×36 SVG to stdout; defaults to demo.\n--version          Print the release version and build commit.\n\n/demo toggles live and demo. /models chooses a model and reasoning level. /export [path] writes ATIF. Type / for commands; Up/Down selects, Tab completes, Enter runs. Cmd+P on macOS, Ctrl+P on Windows, F2, or /plugins opens plugins. Esc stops a reply. Ctrl+C quits."
                );
                return Ok(());
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Unknown argument: {arg}. Use --help."),
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
    let openagents_root = model_access::store::openagents_dir();
    if let Some(root) = &openagents_root {
        if let Err(error) = app.load_plugin_settings(coder_new::plugin_store::Store::under(
            root.join("coder-new"),
        )) {
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
        let ratatui::style::Color::Rgb(r, g, b) = coder_new::theme::TEXT_SECONDARY else {
            unreachable!("Coder uses RGB colors");
        };
        write!(io::stdout(), "\x1b]12;#{r:02x}{g:02x}{b:02x}\x07")?;
        io::stdout().flush()?;
        let interval = Duration::from_millis(125);
        let started = Instant::now();
        let mut next_tick = started + interval;
        let mut background = Background::default();
        let mut catalog = coder_new::model_catalog::Loader::default();
        loop {
            app.elapsed_seconds = started.elapsed().as_secs();
            background.sync(&mut app);
            catalog.sync(&mut app);
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
    let extra_restore = restore_extras();
    ratatui::restore();
    result.and(extra_restore)
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
