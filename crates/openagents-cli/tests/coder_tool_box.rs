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
