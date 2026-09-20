//! A shell that shows the composer working: scrollback above, the framed
//! input box at the foot, `Enter` to submit, `Alt-Enter` for a newline,
//! `Up`/`Down` for history, `PageUp`/`PageDown` for scrollback, `Ctrl-C` or
//! an empty `Ctrl-D` to quit.
//!
//! Run it with `cargo run -p coder-terminal --example shell`.

use std::io::{self, stdout};

use coder_terminal::{Composer, ComposerAction, Editor, Intensity, Ladder, handle_key};
use coder_terminal::{Guard, Step, guard::Stdout};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::style::Style;

/// One line in the scrollback: `>` leads a submitted draft, a dim line is a
/// reply. The stub replies stand in for the agent the plan in `docs/coder/`
/// describes.
struct Line {
    dim: bool,
    text: String,
}

fn main() -> io::Result<()> {
    // The guard takes raw mode and the alternate screen and hands both back
    // when it drops — after a quit, an error, or a panic.
    let guard = Guard::enter(Stdout, &[Step::RawMode, Step::AlternateScreen])?;
    guard.arm_panic_hook();
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let result = run(&mut terminal);

    result.and(guard.restore())
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let ladder = Ladder::from_environment();
    let mut editor = Editor::new();
    let mut lines: Vec<Line> = Vec::new();
    let mut scroll = 0usize;

    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            let mut composer = Composer::new(&mut editor, ladder)
                .status("ready")
                .location("openagents");
            let box_height = composer.height(area.width).min(area.height);
            let log_area = Rect::new(0, 0, area.width, area.height - box_height);
            let box_area = Rect::new(0, log_area.height, area.width, box_height);

            let amber = ladder.style(Intensity::Full);
            let dim = ladder.style(Intensity::Half);
            let buf = frame.buffer_mut();
            for y in log_area.top()..log_area.bottom() {
                for x in log_area.left()..log_area.right() {
                    buf[(x, y)].set_style(Style::new().bg(ladder.background()));
                }
            }

            // The scrollback draws newest-at-bottom; `scroll` walks the view
            // toward older lines.
            let shown = log_area.height as usize;
            let end = lines.len().saturating_sub(scroll);
            let start = end.saturating_sub(shown);
            for (offset, line) in lines[start..end].iter().enumerate() {
                let style = if line.dim { dim } else { amber };
                buf.set_string(
                    log_area.left() + 1,
                    log_area.top() + offset as u16,
                    &line.text,
                    style,
                );
            }

            let caret = composer.render(box_area, buf);
            frame.set_cursor_position(caret);
        })?;

        let Event::Key(key) = event::read()? else {
            continue;
        };
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(()),
            KeyCode::Char('d')
                if key.modifiers.contains(KeyModifiers::CONTROL) && editor.is_empty() =>
            {
                return Ok(());
            }
            KeyCode::PageUp => {
                scroll = (scroll + 10).min(lines.len());
            }
            KeyCode::PageDown => {
                scroll = scroll.saturating_sub(10);
            }
            _ => {
                if let ComposerAction::Submitted(draft) =
                    handle_key(&mut editor, terminal.size()?.width as usize, &key)
                {
                    scroll = 0;
                    for (i, part) in draft.lines().enumerate() {
                        lines.push(Line {
                            dim: false,
                            text: format!("{} {}", if i == 0 { '>' } else { ' ' }, part),
                        });
                    }
                    if !draft.is_empty() {
                        lines.push(Line {
                            dim: true,
                            text: "  no agent yet — the plan lives in docs/coder/".to_owned(),
                        });
                    }
                }
            }
        }
    }
}
