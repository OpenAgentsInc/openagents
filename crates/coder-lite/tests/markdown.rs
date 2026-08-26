use coder_lite::tui::{CoderUi, Entry, Role};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

#[test]
fn renders_markdown_bold_and_italic() {
    let mut ui = CoderUi::new();
    ui.entries.push(Entry {
        role: Role::Assistant,
        text: "**bold** and *italic*".to_string(),
        output: None,
        tool: None,
        at: 0,
    });

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
        text.contains("bold"),
        "expected rendered text to contain 'bold'\n{}",
        text
    );
    assert!(
        text.contains("italic"),
        "expected rendered text to contain 'italic'\n{}",
        text
    );
    assert!(
        !text.contains("**"),
        "expected markdown asterisks to be consumed\n{}",
        text
    );
}
