//! The transcript laid out as Claude Code's: the person's `❯` in column 0
//! on a band as wide as the screen, `●` on replies and tool calls with a
//! two-column hanging indent, `⎿` on a tool's first result row, and one
//! blank row between items.

use coder_new::{App, Mode, live::Entry, theme, ui};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
use serde_json::json;

const LONG: &str = "The composer rail named a vendor's model because a saved setting chose it, and the default named the free router; both now read auto unless the person picks a model in /models.";

fn chat() -> App {
    let mut app = App::default();
    app.set_mode(Mode::Live);
    app.live.entries.extend([
        Entry::User("Why does the rail name a vendor's model?".into()),
        Entry::Tool {
            name: "Run".into(),
            input: json!({"command": "cargo test -p coder-new"}),
            output: json!({"output": "running 415 tests\ntest result: ok. 415 passed"}),
            running: false,
        },
        Entry::Assistant {
            text: LONG.into(),
            model: Some("openai/gpt-6-luna".into()),
            elapsed_ms: Some(1800),
        },
    ]);
    app
}

fn render(app: &mut App, width: u16, height: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| ui::render(frame, app)).unwrap();
    terminal.backend().buffer().clone()
}

fn rows(buffer: &Buffer) -> Vec<String> {
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect()
}

#[test]
fn items_start_in_column_zero_hang_two_columns_and_sit_one_blank_row_apart() {
    for width in [110, 40, 24] {
        let mut app = chat();
        let buffer = render(&mut app, width, 40);
        let text = rows(&buffer);
        let user = text
            .iter()
            .position(|row| row.starts_with("❯ Why"))
            .unwrap();
        // A wrapped message hangs under its text; the band runs the whole
        // width on every row of it, under the text and past it.
        let mut end = user + 1;
        while text[end].starts_with("  ") && !text[end].starts_with("   ") {
            end += 1;
        }
        for y in user..end {
            for x in 0..width {
                assert_eq!(buffer[(x, y as u16)].bg, theme::BG_LIGHT, "{width}");
            }
        }
        assert_eq!(text[end], "", "{width}: one blank row after the message");
        assert!(text[end + 1].starts_with("● Run"), "{width}: {text:#?}");
        assert!(text[end + 2].starts_with("  ⎿  "), "{width}: {text:#?}");
        let reply = text
            .iter()
            .position(|row| row.starts_with("● The composer"))
            .unwrap();
        assert_eq!(
            text[reply - 1],
            "",
            "{width}: one blank row before the reply"
        );
        assert_ne!(text[reply - 2], "", "{width}: only one");
        // Every wrapped row hangs under the text, not the bullet.
        let mut row = reply + 1;
        while !text[row].is_empty() {
            assert!(text[row].starts_with("  "), "{width}: {}", text[row]);
            assert!(!text[row].starts_with("   "), "{width}: {}", text[row]);
            row += 1;
        }
        assert!(row > reply + 1, "{width}: the long reply wraps");
        // No row runs into the right gutter but the person's band.
        for (y, line) in text.iter().enumerate().take(row) {
            if y != user {
                assert!(line.chars().count() <= usize::from(width - 2), "{line}");
            }
        }
        // The transcript names no model and no time under the reply (the
        // composer rail below is not the transcript).
        assert!(!text[..=row].iter().any(|line| line.contains("gpt-6-luna")));
        assert!(!text[..=row].iter().any(|line| line.contains("1.8s")));
    }
}
