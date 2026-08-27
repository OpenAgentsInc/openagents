use openagents_cli::coder::tui::{CoderUi, Entry};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

#[test]
fn renders_delegate_tool_call_and_four_line_box() {
    let mut ui = CoderUi::new();
    let mut call = Entry::tool_call("delegate devin: Read src/main.rs");
    call.output = Some("Reading file...\nFound main()\nDone".to_string());
    ui.entries.push(call);

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            let area = f.area();
            ui.render(f, area);
        })
        .unwrap();

    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();

    assert!(
        text.contains("delegate devin: Read src/main.rs"),
        "{}",
        text
    );
    assert!(text.contains("Reading file..."), "{}", text);
    assert!(text.contains("Found main()"), "{}", text);
    assert!(text.contains("Done"), "{}", text);
    assert!(text.contains("│"), "box border not rendered: {}", text);
}

/// A tool header that runs long wraps instead of clipping. `checkpoint` is
/// the case that paid for this: its header is the note itself, and the note
/// a reader needs was the part past the first line.
#[test]
fn a_long_tool_header_wraps_to_a_second_line_instead_of_clipping() {
    let mut ui = CoderUi::new();
    let long_note = "a".repeat(120);
    let mut call = Entry::tool_call(format!("checkpoint {long_note}"));
    call.output = Some("Checkpoint recorded.".to_string());
    ui.entries.push(call);

    let backend = TestBackend::new(60, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let area = f.area();
            ui.render(f, area);
        })
        .unwrap();

    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();

    // The whole note survives: 120 characters at width 56 wrap to three
    // chunks — head on the marker line, two continuations under it — so the
    // assertion reads the chunks, not a slice across a boundary.
    assert!(text.contains(&long_note[..56]), "{}", text);
    assert!(text.contains(&long_note[56..112]), "{}", text);
    assert!(text.contains(&long_note[112..]), "{}", text);
}
