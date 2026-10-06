use std::{
    io::{self, Write},
    time::{Duration, Instant},
};

use coder_new::{App, Screen, snapshot, ui};
use crossterm::{
    cursor::{SetCursorStyle, Show},
    event::{self, DisableBracketedPaste, EnableBracketedPaste},
    execute,
    terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate},
};

fn main() -> io::Result<()> {
    let mut app = App::default();
    let mut capture = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--welcome" => app.screen = Screen::Welcome,
            "--snapshot" => capture = true,
            "--help" | "-h" => {
                println!(
                    "Coder terminal UI preview\n\nUsage: coder-new [--welcome] [--snapshot]\n\n--welcome   Start with the welcome screen.\n--snapshot  Write a 110×36 SVG preview to stdout.\n\nUp/Down selects agent conversations. Esc returns to main. Tab switches views. Ctrl+C closes the preview."
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
        return io::stdout().write_all(snapshot::svg(&mut app, 110, 36).as_bytes());
    }

    let mut terminal = match ratatui::try_init() {
        Ok(terminal) => terminal,
        Err(error) => {
            let _ = ratatui::try_restore();
            return Err(error);
        }
    };
    // Ratatui restores terminal modes on panic; bracketed paste needs the same cleanup.
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore_extras();
        previous_hook(info);
    }));
    let result: io::Result<()> = (|| {
        execute!(
            io::stdout(),
            EnableBracketedPaste,
            SetCursorStyle::BlinkingBlock
        )?;
        let ratatui::style::Color::Rgb(r, g, b) = coder_new::theme::TEXT_SECONDARY else {
            unreachable!("Grok Night uses RGB colors");
        };
        write!(io::stdout(), "\x1b]12;#{r:02x}{g:02x}{b:02x}\x07")?;
        io::stdout().flush()?;
        let interval = Duration::from_millis(125);
        let mut next_tick = Instant::now() + interval;
        loop {
            execute!(io::stdout(), BeginSynchronizedUpdate)?;
            terminal.draw(|frame| ui::render(frame, &mut app))?;
            // Keep the block blinking while progress updates move the terminal cursor.
            if app.animation_frame >= 4 {
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
        SetCursorStyle::DefaultUserShape,
        Show
    )?;
    write!(io::stdout(), "\x1b]112\x07")?;
    io::stdout().flush()
}
