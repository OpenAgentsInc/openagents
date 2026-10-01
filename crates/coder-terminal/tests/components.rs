//! Text snapshots of the transcript components and the list overlay.
//!
//! Each case renders into a ratatui [`Buffer`] and compares the cells'
//! symbols, one row per line with trailing spaces trimmed, against a
//! golden file in `tests/snapshots/`. Run with `UPDATE_SNAPSHOTS=1` to
//! rewrite the golden files after an intended change, then read the diff.

use std::path::PathBuf;

use coder_terminal::components::{
    Card, FileRow, Item, ListOverlay, RunRow, Who, run,
    turn::{note, streaming, turn},
};
use coder_terminal::{Colors, Intensity, Ladder};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use ratatui::text::Line;
use unicode_width::UnicodeWidthStr;

const TRUE: Ladder = Ladder::new(Colors::True);

/// The rows of `buf` as text: a wide character's trailing cell is skipped,
/// trailing spaces trimmed.
fn text(buf: &Buffer) -> String {
    let area = buf.area;
    let mut out = String::new();
    for y in area.top()..area.bottom() {
        let mut row = String::new();
        let mut x = area.left();
        while x < area.right() {
            let symbol = buf[(x, y)].symbol();
            row.push_str(symbol);
            x += symbol.width().max(1) as u16;
        }
        out.push_str(row.trim_end());
        out.push('\n');
    }
    out
}

/// Draws transcript lines into a buffer exactly `width` wide, failing if a
/// line is wider than that.
fn draw(lines: &[Line<'_>], width: u16) -> Buffer {
    let mut buf = Buffer::empty(Rect::new(0, 0, width, lines.len() as u16));
    for (y, line) in lines.iter().enumerate() {
        assert!(
            line.width() <= usize::from(width),
            "a line is {} cells at width {width}: {line}",
            line.width()
        );
        buf.set_line(0, y as u16, line, width);
    }
    buf
}

/// Draws an overlay over a dotted screen, so the cleared box shows.
fn overlay(overlay: &ListOverlay<'_>, width: u16, height: u16) -> Buffer {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    for y in 0..height {
        for x in 0..width {
            buf[(x, y)].set_char('.');
        }
    }
    overlay.render(area, &mut buf);
    buf
}

fn check(name: &str, buf: &Buffer) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(format!("{name}.txt"));
    let actual = text(buf);
    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        std::fs::create_dir_all(path.parent().expect("snapshot dir")).expect("create dir");
        std::fs::write(&path, &actual).expect("write snapshot");
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "no snapshot at {}; run with UPDATE_SNAPSHOTS=1",
            path.display()
        )
    });
    assert_eq!(
        actual, expected,
        "snapshot {name} differs; run with UPDATE_SNAPSHOTS=1 to accept"
    );
}

// --- The cases. ---

const YOU: &str = "Can you look at the parser? It fails on 日本語 input\nand on empty lines too.";

const REPLY: &str = "## What I found\n\nThe parser drops **wide** characters when a row \
     breaks mid-word. See `wrap_rows` and the [notes](https://example.com/notes).\n\n\
     - check the grapheme width\n- keep the caret on the later row\n- add a test with a long \
     list item that wraps onto a second line";

fn welcome() -> Card {
    Card {
        title: "OpenAgents".into(),
        rows: vec![
            ("model".into(), "Space Bunny Alpha".into()),
            ("folder".into(), "~/work/openagents".into()),
            (
                "account".into(),
                "signed in as a person with a long display name that wraps".into(),
            ),
        ],
        body: vec![
            "Ask anything about this folder, or hand Coder a change to make.".into(),
            "Your threads sync with the web and mobile apps.".into(),
        ],
        art: Vec::new(),
        keys: vec![
            ("Enter".into(), "send".into()),
            ("Ctrl+T".into(), "threads".into()),
            ("Ctrl+C".into(), "quit".into()),
        ],
    }
}

fn pairing() -> Card {
    let art = (0..13)
        .map(|y| {
            (0..25)
                .map(|x| if (x * 7 + y * 3) % 5 < 2 { '█' } else { ' ' })
                .collect::<String>()
        })
        .collect();
    Card {
        title: "Pair your phone".into(),
        rows: Vec::new(),
        body: vec!["Scan this code with OpenAgents on your phone.".into()],
        art,
        keys: vec![("Esc".into(), "close".into())],
    }
}

fn run_rows() -> Vec<RunRow> {
    vec![
        RunRow::Start {
            who: "you".into(),
            place: "this Mac".into(),
            why: "fix the wrapping bug the parser test found".into(),
        },
        RunRow::Step {
            mark: '·',
            text: "reading crates/coder-terminal/src/wrap.rs and the tests that cover it".into(),
        },
        RunRow::Command {
            command: "cargo test -p coder-terminal".into(),
            exit: Some(0),
            timed_out: false,
            tail: vec![
                "running 138 tests".into(),
                "test result: ok. 138 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s".into(),
                "Doc-tests coder_terminal".into(),
            ],
        },
        RunRow::Command {
            command: "cargo clippy -p coder-terminal -- -D warnings".into(),
            exit: Some(1),
            timed_out: false,
            tail: vec!["error: unused variable: `row`".into()],
        },
        RunRow::Progress {
            step: 4,
            percent: Some(60),
            seconds: 65,
        },
        RunRow::Progress {
            step: 5,
            percent: None,
            seconds: 3_725,
        },
        RunRow::Switched {
            text: "switched to Gemini 3.8 Flash after OpenRouter timed out".into(),
        },
        RunRow::Question {
            text: "keep the old test name or rename it?".into(),
            hint: Some("answer in the composer".into()),
        },
        RunRow::Result {
            summary: "Fixed the wrap so a wide character never splits across rows, and added a regression test.".into(),
            files: vec![
                FileRow {
                    status: "M".into(),
                    path: "crates/coder-terminal/src/wrap.rs".into(),
                    added: Some(12),
                    removed: Some(3),
                },
                FileRow {
                    status: "A".into(),
                    path: "crates/coder-terminal/tests/wrap.rs".into(),
                    added: None,
                    removed: None,
                },
            ],
            insertions: 40,
            deletions: 3,
            worktree: "/Users/me/work/openagents-coder-1".into(),
        },
        RunRow::Failed {
            text: "the provider refused the request".into(),
        },
        RunRow::Stopped {
            text: "stopped at your request".into(),
        },
    ]
}

fn run_lines(width: u16, ladder: Ladder) -> Vec<Line<'static>> {
    run_rows()
        .iter()
        .flat_map(|row| run::lines(row, width, ladder))
        .collect()
}

fn three_items() -> Vec<Item> {
    vec![
        Item {
            label: "Fix the parser".into(),
            detail: "2m ago".into(),
        },
        Item {
            label: "Pair the phone".into(),
            detail: "yesterday".into(),
        },
        Item {
            label: "A thread whose title is long enough to crowd its detail column".into(),
            detail: "last week".into(),
        },
    ]
}

fn many_items() -> Vec<Item> {
    (0..30)
        .map(|i| Item {
            label: format!("thread {i}"),
            detail: format!("{i}d ago"),
        })
        .collect()
}

fn list<'a>(items: &'a [Item], selected: usize, ladder: Ladder) -> ListOverlay<'a> {
    ListOverlay {
        title: "Threads",
        items,
        selected,
        hint: "Enter open · n new · a archive · Esc close",
        empty: "No threads yet. Send a message to start one.",
        ladder,
    }
}

#[test]
fn turns() {
    for width in [80u16, 40] {
        check(
            &format!("turn_you_{width}"),
            &draw(&turn(Who::You, YOU, width, TRUE), width),
        );
        check(
            &format!("turn_openagents_{width}"),
            &draw(&turn(Who::OpenAgents, REPLY, width, TRUE), width),
        );
        check(
            &format!("streaming_{width}"),
            &draw(
                &streaming("Looking at `wrap_rows` now; the bug is", '⠋', width, TRUE),
                width,
            ),
        );
        let mut notes = note(
            "Coder can make this change in a worktree. Say \"go\" to start.",
            Intensity::ThreeQuarters,
            width,
            TRUE,
        );
        notes.extend(note(
            "[1] Show me the failing test first",
            Intensity::Half,
            width,
            TRUE,
        ));
        notes.extend(note(
            "[2] Explain why the caret joins the later row at a wrap point",
            Intensity::Half,
            width,
            TRUE,
        ));
        check(&format!("note_{width}"), &draw(&notes, width));
    }
}

#[test]
fn a_wide_character_wraps_whole_at_a_narrow_width() {
    check(
        "turn_you_20",
        &draw(&turn(Who::You, "日本語の入力が壊れる", 20, TRUE), 20),
    );
}

#[test]
fn cards() {
    for width in [80u16, 40] {
        check(
            &format!("card_welcome_{width}"),
            &draw(&welcome().lines(width, TRUE), width),
        );
        check(
            &format!("card_art_{width}"),
            &draw(&pairing().lines(width, TRUE), width),
        );
    }
    check("card_welcome_16", &draw(&welcome().lines(16, TRUE), 16));
}

#[test]
fn runs() {
    for width in [80u16, 40] {
        check(
            &format!("run_{width}"),
            &draw(&run_lines(width, TRUE), width),
        );
    }
}

#[test]
fn overlays() {
    let three = three_items();
    let many = many_items();
    check("overlay_three_80", &overlay(&list(&three, 1, TRUE), 80, 12));
    check("overlay_three_40", &overlay(&list(&three, 1, TRUE), 40, 12));
    check("overlay_empty_80", &overlay(&list(&[], 0, TRUE), 80, 10));
    check(
        "overlay_scrolled_80",
        &overlay(&list(&many, 20, TRUE), 80, 16),
    );
    check(
        "overlay_scrolled_40",
        &overlay(&list(&many, 29, TRUE), 40, 10),
    );
}

/// The text never depends on the color depth.
#[test]
fn every_color_depth_draws_the_same_text() {
    let three = three_items();
    for colors in [Colors::Indexed, Colors::None] {
        let ladder = Ladder::new(colors);
        assert_eq!(
            text(&draw(&turn(Who::OpenAgents, REPLY, 80, ladder), 80)),
            text(&draw(&turn(Who::OpenAgents, REPLY, 80, TRUE), 80))
        );
        assert_eq!(
            text(&draw(&welcome().lines(80, ladder), 80)),
            text(&draw(&welcome().lines(80, TRUE), 80))
        );
        assert_eq!(
            text(&draw(&run_lines(80, ladder), 80)),
            text(&draw(&run_lines(80, TRUE), 80))
        );
        assert_eq!(
            text(&overlay(&list(&three, 1, ladder), 80, 12)),
            text(&overlay(&list(&three, 1, TRUE), 80, 12))
        );
    }
}

fn assert_colorless(buf: &Buffer, what: &str) {
    for cell in buf.content() {
        assert_eq!(cell.fg, Color::Reset, "{what}: a cell has a foreground");
        assert_eq!(cell.bg, Color::Reset, "{what}: a cell has a background");
    }
}

#[test]
fn no_color_draws_no_color() {
    let ladder = Ladder::new(Colors::None);
    assert_colorless(&draw(&turn(Who::You, YOU, 80, ladder), 80), "turn you");
    assert_colorless(
        &draw(&turn(Who::OpenAgents, REPLY, 80, ladder), 80),
        "turn openagents",
    );
    assert_colorless(&draw(&welcome().lines(80, ladder), 80), "card");
    assert_colorless(&draw(&pairing().lines(40, ladder), 40), "art card");
    assert_colorless(&draw(&run_lines(80, ladder), 80), "run");
    let three = three_items();
    assert_colorless(&overlay(&list(&three, 1, ladder), 80, 12), "overlay");
}

#[test]
fn no_color_reverses_the_selected_row_alone() {
    let three = three_items();
    let buf = overlay(&list(&three, 1, Ladder::new(Colors::None)), 80, 12);
    let reversed: Vec<u16> = (0..12)
        .filter(|&y| (0..80).any(|x| buf[(x, y)].modifier.contains(Modifier::REVERSED)))
        .collect();
    assert_eq!(reversed.len(), 1, "one reversed row: {reversed:?}");
    let y = reversed[0];
    let row: String = (0..80).map(|x| buf[(x, y)].symbol()).collect();
    assert!(row.contains("› Pair the phone"), "{row}");

    // With color the selection is a tint, not a reversal.
    let buf = overlay(&list(&three, 1, TRUE), 80, 12);
    assert!(
        buf.content()
            .iter()
            .all(|cell| !cell.modifier.contains(Modifier::REVERSED))
    );
    assert!(buf.content().iter().any(|cell| cell.bg == TRUE.selection()));
}

#[test]
fn indexed_colors_stay_on_the_palette() {
    let ladder = Ladder::new(Colors::Indexed);
    let three = three_items();
    for buf in [
        draw(&run_lines(80, ladder), 80),
        draw(&welcome().lines(80, ladder), 80),
        overlay(&list(&three, 1, ladder), 80, 12),
    ] {
        for cell in buf.content() {
            assert!(
                matches!(cell.fg, Color::Reset | Color::Indexed(_)),
                "{:?}",
                cell.fg
            );
            assert!(
                matches!(cell.bg, Color::Reset | Color::Indexed(_)),
                "{:?}",
                cell.bg
            );
        }
    }
}

#[test]
fn a_reply_styles_its_marks() {
    let lines = turn(Who::OpenAgents, REPLY, 80, TRUE);
    let spans: Vec<_> = lines.iter().flat_map(|line| line.spans.iter()).collect();
    let code = spans
        .iter()
        .find(|span| span.content == "wrap_rows")
        .expect("the code span");
    assert_eq!(code.style.fg, TRUE.style(Intensity::Full).fg);
    let link = spans
        .iter()
        .find(|span| span.content == "notes")
        .expect("the link span");
    assert!(link.style.add_modifier.contains(Modifier::UNDERLINED));
    let bold = spans
        .iter()
        .find(|span| span.content == "wide")
        .expect("the bold span");
    assert!(bold.style.add_modifier.contains(Modifier::BOLD));
    let label = &lines[0].spans[0];
    assert_eq!(label.content, "openagents");
    assert_eq!(label.style.fg, TRUE.style(Intensity::Half).fg);
}
