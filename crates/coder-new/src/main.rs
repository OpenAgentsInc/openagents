use std::io::{self, Write};

use coder_new::{App, Screen, snapshot, ui};
use crossterm::{
    cursor::SetCursorStyle,
    event::{self, DisableBracketedPaste, EnableBracketedPaste},
    execute,
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
        loop {
            terminal.draw(|frame| ui::render(frame, &mut app))?;
            if !app.handle(event::read()?) {
                break;
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
        DisableBracketedPaste,
        SetCursorStyle::DefaultUserShape
    )?;
    write!(io::stdout(), "\x1b]112\x07")?;
    io::stdout().flush()
}
