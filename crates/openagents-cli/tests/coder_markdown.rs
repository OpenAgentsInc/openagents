//! Rendering contract for Coder's transcript.
//!
//! Two things are being defended here.
//!
//! **Fidelity.** A markdown renderer that silently drops or mangles content is
//! the same class of defect as a command that fabricates data. Every construct
//! the model can send either renders or renders literally — never vanishes.
//!
//! **Streaming.** The renderer must show a chunk while the stream is still
//! open, and must not reparse the whole document to do it.

use openagents_cli::coder::tui::{CoderUi, Entry, Role};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

/// Draw `ui` to an 80x24 test terminal and flatten the buffer to text.
fn draw(ui: &mut CoderUi) -> String {
    draw_sized(ui, 80, 24)
}

fn draw_sized(ui: &mut CoderUi, w: u16, h: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    terminal
        .draw(|f| {
            let area = f.area();
            ui.render(f, area);
        })
        .unwrap();
    let buf = terminal.backend().buffer().clone();
    (0..buf.area.height)
        .map(|y| row_text(&buf, y))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Flatten one buffer row to text.
///
/// ratatui writes a double-width glyph into one cell and *resets* the cell it
/// covers, so a naive per-cell join turns `日本語` into `日 本 語`. Skipping the
/// covered cell reproduces what the terminal actually shows.
fn row_text(buf: &ratatui::buffer::Buffer, y: u16) -> String {
    let mut out = String::new();
    let mut x = 0u16;
    while x < buf.area.width {
        let symbol = buf.cell((x, y)).unwrap().symbol();
        out.push_str(symbol);
        let width = unicode_width::UnicodeWidthStr::width(symbol).max(1) as u16;
        x += width;
    }
    out
}

fn assistant(text: &str) -> CoderUi {
    let mut ui = CoderUi::new();
    ui.entries.push(Entry::new(Role::Assistant, text));
    ui
}

// ── inline formatting ────────────────────────────────────────────────

#[test]
fn renders_markdown_bold_and_italic() {
    let mut ui = assistant("**bold** and *italic*");
    let text = draw(&mut ui);

    assert!(text.contains("bold"), "{text}");
    assert!(text.contains("italic"), "{text}");
    assert!(
        !text.contains("**"),
        "expected markdown asterisks to be consumed\n{text}"
    );
}

#[test]
fn bold_is_bold_and_italic_is_italic_in_the_buffer() {
    use ratatui::style::Modifier;

    let mut ui = assistant("**bold** and *italic*");
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|f| {
            let area = f.area();
            ui.render(f, area);
        })
        .unwrap();
    let buf = terminal.backend().buffer().clone();

    let mut bold = String::new();
    let mut italic = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let cell = buf.cell((x, y)).unwrap();
            if cell.modifier.contains(Modifier::BOLD) {
                bold.push_str(cell.symbol());
            }
            if cell.modifier.contains(Modifier::ITALIC) {
                italic.push_str(cell.symbol());
            }
        }
    }
    assert_eq!(bold.trim(), "bold", "BOLD covered {bold:?}");
    assert_eq!(italic.trim(), "italic", "ITALIC covered {italic:?}");
}

// ── the palette must not move ─────────────────────────────────────────

#[test]
fn every_painted_cell_keeps_the_amber_palette() {
    use ratatui::style::Color;

    let source = "# Heading\n\nProse with **bold**, `code`, and [a link](https://example.test/).\n\n\
                  ```rust\nfn main() { println!(\"hi\"); }\n```\n\n\
                  > a quote\n\n- one\n- two\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n$E=mc^2$\n";
    let mut ui = assistant(source);
    let mut terminal = Terminal::new(TestBackend::new(80, 40)).unwrap();
    terminal
        .draw(|f| {
            let area = f.area();
            ui.render(f, area);
        })
        .unwrap();
    let buf = terminal.backend().buffer().clone();

    let amber = [Color::Rgb(255, 176, 0), Color::Rgb(131, 91, 0)];
    let ground = Color::Rgb(8, 6, 0);
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let cell = buf.cell((x, y)).unwrap();
            assert!(
                amber.contains(&cell.fg),
                "cell ({x},{y}) {:?} drifted off amber",
                cell.symbol()
            );
            assert_eq!(
                cell.bg,
                ground,
                "cell ({x},{y}) {:?} drifted off the background",
                cell.symbol()
            );
        }
    }
}

#[test]
fn syntax_highlighting_shows_up_as_weight_not_hue() {
    use ratatui::style::Color;

    // syntect emits real RGB for a Rust block; the amber pass must flatten it
    // while leaving some cells visibly distinguished.
    let mut ui = assistant("```rust\n// a comment\nfn main() { let x = 1; }\n```\n");
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|f| {
            let area = f.area();
            ui.render(f, area);
        })
        .unwrap();
    let buf = terminal.backend().buffer().clone();

    let mut hues = std::collections::BTreeSet::new();
    let mut modifiers = std::collections::BTreeSet::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let cell = buf.cell((x, y)).unwrap();
            hues.insert(format!("{:?}", cell.fg));
            modifiers.insert(cell.modifier.bits());
        }
    }
    assert!(
        hues.is_subset(&std::collections::BTreeSet::from([
            format!("{:?}", Color::Rgb(255, 176, 0)),
            format!("{:?}", Color::Rgb(131, 91, 0)),
        ])),
        "code highlighting introduced a non-amber color: {hues:?}"
    );
    assert!(
        modifiers.len() > 1,
        "highlighting produced no visible distinction at all"
    );
}

// ── fidelity: nothing the model sent may disappear ────────────────────

/// Every construct here must leave its content on screen. The check is
/// deliberately about *content*, not about how it is decorated.
#[test]
fn no_construct_swallows_its_content() {
    let cases: &[(&str, &[&str])] = &[
        ("plain paragraph", &["plain paragraph"]),
        ("# Heading one", &["Heading one"]),
        ("###### Heading six", &["Heading six"]),
        ("- alpha\n- beta\n", &["alpha", "beta"]),
        ("1. first\n2. second\n", &["first", "second"]),
        ("- [ ] todo\n- [x] done\n", &["todo", "done"]),
        ("> quoted line\n", &["quoted line"]),
        ("`inline code`", &["inline code"]),
        ("```\nfenced body\n```\n", &["fenced body"]),
        ("```rust\nfn f() {}\n```\n", &["fn f() {}"]),
        ("~~struck~~", &["struck"]),
        ("[label](https://example.test/)", &["label"]),
        ("| a | b |\n|---|---|\n| 1 | 2 |\n", &["a", "b", "1", "2"]),
        ("---\n\nafter rule\n", &["after rule"]),
        ("<div>raw html</div>\n", &["raw html"]),
        ("term \\* escaped asterisk", &["escaped asterisk"]),
        ("a & b < c", &["a & b < c"]),
        ("emoji 🎯 and 日本語", &["🎯", "日本語"]),
        // An unterminated fence is still content, not a black hole.
        ("```python\nstill streaming", &["still streaming"]),
        // A bare underscore-heavy identifier must survive intact.
        ("call some_function_name(x)", &["some_function_name"]),
    ];

    for (source, expected) in cases {
        let mut ui = assistant(source);
        let text = draw_sized(&mut ui, 100, 40);
        let flat = text.replace('\n', " ");
        for needle in *expected {
            assert!(
                flat.contains(needle),
                "rendering {source:?} lost {needle:?}\n--- screen ---\n{text}"
            );
        }
    }
}

#[test]
fn latex_becomes_unicode_and_never_vanishes() {
    let mut ui = assistant("mass energy $E=mc^2$ done\n");
    let text = draw(&mut ui).replace('\n', " ");
    assert!(
        text.contains("E=mc²"),
        "expected Unicode superscript: {text}"
    );
    assert!(text.contains("mass energy"), "{text}");
    assert!(text.contains("done"), "{text}");
}

#[test]
fn an_unknown_language_fence_still_shows_its_body() {
    let mut ui = assistant("```not-a-real-language\nliteral body line\n```\n");
    let text = draw(&mut ui).replace('\n', " ");
    assert!(
        text.contains("literal body line"),
        "an unhighlightable fence swallowed its body: {text}"
    );
}

#[test]
fn malformed_markdown_renders_literally_rather_than_disappearing() {
    // Unbalanced emphasis, a stray bracket, a dangling pipe: all of these are
    // things a model actually emits mid-stream.
    let cases = [
        ("**unclosed bold", "unclosed bold"),
        ("[unclosed link](", "unclosed link"),
        ("| ragged | table", "ragged"),
        ("**", "**"),
        ("< not html >", "< not html >"),
    ];
    for (source, needle) in cases {
        let mut ui = assistant(source);
        let text = draw_sized(&mut ui, 100, 24).replace('\n', " ");
        assert!(
            text.contains(needle.trim_end()) || text.contains(needle),
            "{source:?} lost {needle:?}: {text}"
        );
    }
}

// ── the non-assistant roles are untouched ────────────────────────────

#[test]
fn user_and_notice_entries_keep_their_markers() {
    let mut ui = CoderUi::new();
    ui.entries.push(Entry::new(Role::You, "what changed?"));
    ui.entries
        .push(Entry::new(Role::Notice, "found ACP agents: none"));
    let text = draw(&mut ui);
    assert!(text.contains("> what changed?"), "{text}");
    assert!(text.contains("⏺ found ACP agents: none"), "{text}");
    assert!(
        !text.contains("**"),
        "a notice should not be markdown-rendered: {text}"
    );
}

#[test]
fn a_user_message_containing_markdown_is_shown_verbatim() {
    // The composer echo is the user's own text. Rendering it as markdown would
    // change what they typed.
    let mut ui = CoderUi::new();
    ui.entries
        .push(Entry::new(Role::You, "run **make** and `cargo test`"));
    let text = draw(&mut ui).replace('\n', " ");
    assert!(text.contains("**make**"), "{text}");
    assert!(text.contains("`cargo test`"), "{text}");
}

#[test]
fn spinner_frames_still_animate() {
    let mut ui = CoderUi::new();
    ui.loading = true;
    let frames: Vec<char> = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏".chars().collect();
    let mut seen = Vec::new();
    for _ in 0..frames.len() {
        let text = draw(&mut ui);
        seen.push(
            frames
                .iter()
                .copied()
                .find(|f| text.contains(*f))
                .expect("a braille spinner frame should be on screen"),
        );
    }
    let unique: std::collections::BTreeSet<char> = seen.iter().copied().collect();
    assert_eq!(
        unique.len(),
        frames.len(),
        "the spinner stopped cycling: {seen:?}"
    );
}

#[test]
fn a_link_keeps_a_separator_between_its_text_and_its_url() {
    // Pretty mode prints the destination after the label. Without the
    // parentheses the two run together as `the forgehttps://…`.
    let mut ui = assistant("read [the forge](https://openagents.com/x) today\n");
    let text = draw_sized(&mut ui, 100, 24).replace('\n', " ");
    assert!(
        text.contains("the forge (https://openagents.com/x)"),
        "link text and URL ran together: {text}"
    );
}

#[test]
fn a_fenced_block_starts_its_body_on_a_fresh_line() {
    // The info string is hidden in pretty mode; hiding it must not also eat
    // the newline and glue `rust` onto the first line of code.
    let mut ui = assistant("```rust\nfn main() {}\n```\n");
    let lines: Vec<String> = draw_sized(&mut ui, 100, 24)
        .lines()
        .map(|l| l.trim_end().to_string())
        .collect();
    assert!(
        lines.iter().any(|l| l == "fn main() {}"),
        "expected the body on its own line, got {:?}",
        &lines[..6.min(lines.len())]
    );
    assert!(
        !lines.iter().any(|l| l.contains("rustfn")),
        "the language tag collided with the code body: {:?}",
        &lines[..6.min(lines.len())]
    );
}

#[test]
fn a_table_renders_as_a_box_with_every_cell_intact() {
    let mut ui = assistant("| lang | speed |\n|------|-------|\n| rust | fast |\n");
    let text = draw_sized(&mut ui, 100, 24);
    for needle in ["┌", "│", "└", "lang", "speed", "rust", "fast"] {
        assert!(text.contains(needle), "table lost {needle:?}:\n{text}");
    }
}
