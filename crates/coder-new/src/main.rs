use std::{
    io::{self, Write},
    time::{Duration, Instant},
};

use coder_new::{App, Mode, Screen, live::Background, snapshot, ui};
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
            "--welcome" => app.screen = Screen::Welcome,
            "--plugins" => app.open_plugins(),
            "--plugin-settings" => app.open_plugin_settings(),
            "--models" => models = true,
            "--snapshot" => capture = true,
            "--help" | "-h" => {
                println!(
                    "Coder terminal\n\nUsage: coder [--live | --demo] [--welcome | --plugins | --plugin-settings | --models] [--snapshot]\n\n--live             Use enabled providers and tools (default).\n--demo             Use local example conversations.\n--welcome          Start with the welcome screen.\n--plugins          Start with plugin management.\n--plugin-settings  Start with OpenRouter settings.\n--models           Open the model picker for an enabled provider.\n--snapshot         Write a 110×36 SVG to stdout; defaults to demo.\n--version          Print the release version and build commit.\n\n/demo toggles live and demo. /models chooses a model and reasoning level. Type / for commands; Up/Down selects, Tab completes, Enter runs. F2 or /plugins opens plugins. Esc stops a reply. Ctrl+C quits."
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
    if models {
        app.open_models();
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
        execute!(
            io::stdout(),
            EnableBracketedPaste,
            EnableMouseCapture,
            SetCursorStyle::BlinkingBlock
        )?;
        let ratatui::style::Color::Rgb(r, g, b) = coder_new::theme::TEXT_SECONDARY else {
            unreachable!("Grok Night uses RGB colors");
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
            if event::poll(next_tick.saturating_duration_since(Instant::now()))?
                && !app.handle(event::read()?)
            {
                break;
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
