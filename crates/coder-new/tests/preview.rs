use coder_new::agents::AgentView;
use coder_new::{App, Screen, snapshot, ui};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};

fn key(app: &mut App, code: KeyCode) {
    assert!(app.handle(Event::Key(KeyEvent::new(code, KeyModifiers::NONE))));
}

fn ctrl_key(app: &mut App, code: KeyCode) {
    assert!(app.handle(Event::Key(KeyEvent::new(code, KeyModifiers::CONTROL))));
}

fn open_agents(app: &mut App) {
    key(app, KeyCode::Down);
    assert!(app.agents.view == AgentView::Footer);
    key(app, KeyCode::Enter);
}

fn screen(app: &mut App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| ui::render(frame, app)).unwrap();
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn both_views_and_tiny_terminals_render() {
    let mut app = App::default();
    let conversation = screen(&mut app, 110, 36);
    assert!(conversation.contains("Conversation first"));
    assert!(conversation.contains("Message"));
    assert!(conversation.contains("Sample data"));
    key(&mut app, KeyCode::Tab);
    assert!(screen(&mut app, 110, 36).contains("What do you want to build?"));
    for (width, height) in [(80, 24), (40, 12), (24, 10), (23, 9), (1, 1)] {
        screen(&mut app, width, height);
        app.screen = Screen::Conversation;
        app.handle(Event::Paste(
            "A long draft with 界 and multiple\nlines to wrap across a small terminal.".into(),
        ));
        screen(&mut app, width, height);
    }
}

#[test]
fn paste_and_enter_remain_local_and_key_releases_do_not_send_twice() {
    let mut app = App::default();
    app.handle(Event::Paste("first\r\nsecond\tline\u{1b}".into()));
    assert_eq!(app.draft.text, "first\nsecond    line");
    assert!(app.messages.is_empty());
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.messages, ["first\nsecond    line"]);
    let mut release = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    release.kind = KeyEventKind::Release;
    app.handle(Event::Key(release));
    assert_eq!(app.messages.len(), 1);
    assert!(app.draft.text.is_empty());
    let rendered = screen(&mut app, 80, 24);
    assert!(rendered.contains("No agent is connected."));
    assert!(rendered.contains("❯ first"));
    assert!(rendered.contains("second    line"));
    assert!(!rendered.contains("firstsecond"));
    assert!(!app.handle(Event::Key(KeyEvent::new(
        KeyCode::Char('c'),
        KeyModifiers::CONTROL
    ))));
}

#[test]
fn smallest_supported_viewport_keeps_the_draft_and_cursor_visible() {
    let mut app = App::default();
    app.handle(Event::Paste("hello".into()));
    let mut terminal = Terminal::new(TestBackend::new(24, 12)).unwrap();
    terminal.draw(|frame| ui::render(frame, &mut app)).unwrap();
    let cursor = terminal.get_cursor_position().unwrap();
    assert!(cursor.x < 24 && cursor.y < 12);
    assert!(screen(&mut app, 24, 12).contains("hello"));
    assert!(screen(&mut app, 24, 11).contains("Resize to continue."));
}

#[test]
fn deleting_a_separator_keeps_the_cursor_on_the_merged_grapheme_boundary() {
    let mut app = App::default();
    app.handle(Event::Paste("a\n\u{301}".into()));
    key(&mut app, KeyCode::Home);
    key(&mut app, KeyCode::Right);
    key(&mut app, KeyCode::Delete);
    assert_eq!(app.draft.text, "a\u{301}");
    assert_eq!(app.draft.wrapped(10).1, (1, 0));
    key(&mut app, KeyCode::Backspace);
    assert!(app.draft.text.is_empty());
}

#[test]
fn editing_and_wrapping_preserve_graphemes_and_terminal_cells() {
    let mut app = App::default();
    app.handle(Event::Paste("a👩‍💻e\u{301}界".into()));
    key(&mut app, KeyCode::Left);
    key(&mut app, KeyCode::Backspace);
    assert_eq!(app.draft.text, "a👩‍💻界");
    key(&mut app, KeyCode::Delete);
    assert_eq!(app.draft.text, "a👩‍💻");
    let (lines, cursor) = app.draft.wrapped(3);
    assert_eq!(lines, ["a👩‍💻", ""]);
    assert_eq!(cursor, (0, 1));
    key(&mut app, KeyCode::Home);
    key(&mut app, KeyCode::Delete);
    assert_eq!(app.draft.text, "👩‍💻");
    key(&mut app, KeyCode::End);
    key(&mut app, KeyCode::Backspace);
    assert!(app.draft.text.is_empty());
}

#[test]
fn exported_preview_escapes_drafts_and_uses_the_rendered_canvas() {
    let mut app = App::default();
    app.handle(Event::Paste("<build & test>".into()));
    let svg = snapshot::svg(&mut app, 110, 36);
    assert!(svg.starts_with("<svg "));
    assert!(svg.contains("fill=\"#141414\""));
    assert!(svg.contains("&lt;"));
    assert!(svg.contains("&amp;"));
    assert!(svg.contains("&gt;"));
    assert!(!svg.contains("<build"));
}

#[test]
fn agent_footer_stays_below_the_composer_and_shows_its_focus_hint() {
    let mut app = App::default();
    app.handle(Event::Paste("draft above agents".into()));
    let rendered = screen(&mut app, 110, 36);
    let lines: Vec<_> = rendered.lines().collect();
    let composer_bottom = lines.iter().position(|line| line.contains('╰')).unwrap();
    let footer = lines
        .iter()
        .position(|line| line.contains("4 local agents"))
        .unwrap();
    assert!(footer > composer_bottom);
    assert!(lines[footer].contains("↓ to manage"));

    key(&mut app, KeyCode::Down);
    assert!(app.agents.view == AgentView::Footer);
    let rendered = screen(&mut app, 110, 36);
    assert!(rendered.contains("Enter to view tasks"));
    key(&mut app, KeyCode::Up);
    assert!(app.agents.view == AgentView::Composer);
    assert_eq!(app.draft.text, "draft above agents");
    assert!(app.messages.is_empty());
}

#[test]
fn background_task_list_shows_all_four_local_demos_below_the_transcript() {
    let mut app = App::default();
    open_agents(&mut app);
    assert!(app.agents.view == AgentView::List);
    let rendered = screen(&mut app, 110, 36);
    assert!(rendered.contains("Background tasks"));
    assert!(rendered.contains("4 active agents"));
    assert!(rendered.contains("Local agents (4)"));
    for name in ["claude-code", "codex", "devin-cli", "grok-build"] {
        assert!(rendered.contains(name), "missing task row: {name}");
    }
    assert_eq!(app.agents.demos.len(), 4);
    assert!(app.agents.demos.iter().all(|agent| agent.running));
    let transcript = rendered
        .lines()
        .position(|line| line.contains("Conversation first"))
        .unwrap();
    let tasks = rendered
        .lines()
        .position(|line| line.contains("Background tasks"))
        .unwrap();
    assert!(transcript < tasks);
    assert!(!rendered.contains(" Message "));
    assert!(!rendered.contains("4 local agents"));
}

#[test]
fn task_navigation_clamps_selection_and_preserves_the_conversation() {
    let mut app = App::default();
    app.handle(Event::Paste("retained message".into()));
    key(&mut app, KeyCode::Enter);
    app.handle(Event::Paste("unfinished draft 界".into()));
    key(&mut app, KeyCode::Left);
    let draft_cursor = app.draft.cursor;

    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Down);
    assert!(app.agents.view == AgentView::List);
    key(&mut app, KeyCode::Up);
    assert_eq!(app.agents.selected, 0);
    for _ in 0..8 {
        key(&mut app, KeyCode::Down);
    }
    assert_eq!(app.agents.selected, 3);
    key(&mut app, KeyCode::Enter);
    assert!(app.agents.view == AgentView::Detail);
    assert!(screen(&mut app, 110, 36).contains("grok-build"));
    key(&mut app, KeyCode::Left);
    assert!(app.agents.view == AgentView::List);
    assert_eq!(app.agents.selected, 3);
    for _ in 0..8 {
        key(&mut app, KeyCode::Up);
    }
    assert_eq!(app.agents.selected, 0);
    key(&mut app, KeyCode::Esc);
    assert!(app.agents.view == AgentView::Composer);
    open_agents(&mut app);
    key(&mut app, KeyCode::Left);
    assert!(app.agents.view == AgentView::Composer);

    assert_eq!(app.draft.text, "unfinished draft 界");
    assert_eq!(app.draft.cursor, draft_cursor);
    assert_eq!(app.messages, ["retained message"]);
}

#[test]
fn task_detail_exit_keys_return_to_the_composer_without_sending() {
    for exit in [KeyCode::Esc, KeyCode::Enter, KeyCode::Char(' ')] {
        let mut app = App::default();
        app.handle(Event::Paste("keep this draft".into()));
        open_agents(&mut app);
        key(&mut app, KeyCode::Enter);
        assert!(app.agents.view == AgentView::Detail);
        key(&mut app, exit);
        assert!(app.agents.view == AgentView::Composer);
        assert_eq!(app.draft.text, "keep this draft");
        assert!(app.messages.is_empty());
    }
}

#[test]
fn stopping_tasks_clamps_selection_and_hides_an_empty_footer() {
    let mut app = App::default();
    app.handle(Event::Paste("keep this draft".into()));
    open_agents(&mut app);
    for _ in 0..3 {
        key(&mut app, KeyCode::Down);
    }
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Char('x'));
    assert!(app.agents.view == AgentView::List);
    assert_eq!(app.agents.selected, 2);
    assert_eq!(app.agents.demos.len(), 3);
    assert!(
        app.agents
            .demos
            .iter()
            .all(|agent| agent.name != "grok-build")
    );
    assert!(screen(&mut app, 110, 36).contains("3 active agents"));

    key(&mut app, KeyCode::Char('x'));
    key(&mut app, KeyCode::Char('x'));
    assert_eq!(app.agents.demos.len(), 1);
    assert_eq!(app.agents.selected, 0);
    key(&mut app, KeyCode::Esc);
    open_agents(&mut app);
    assert!(app.agents.view == AgentView::Detail);
    assert!(screen(&mut app, 110, 36).contains("claude-code"));
    key(&mut app, KeyCode::Char('x'));
    assert!(app.agents.view == AgentView::Composer);
    assert!(app.agents.demos.is_empty());
    assert!(!screen(&mut app, 110, 36).contains("local agents"));
    assert_eq!(app.draft.text, "keep this draft");
    assert!(app.messages.is_empty());
}

#[test]
fn stop_all_requires_the_complete_chord_in_both_task_views() {
    for detail in [false, true] {
        let mut app = App::default();
        app.handle(Event::Paste("keep this draft".into()));
        open_agents(&mut app);
        if detail {
            key(&mut app, KeyCode::Enter);
        }
        ctrl_key(&mut app, KeyCode::Char('x'));
        assert_eq!(app.agents.demos.len(), 4);
        assert!(
            app.agents.view
                == if detail {
                    AgentView::Detail
                } else {
                    AgentView::List
                }
        );
        ctrl_key(&mut app, KeyCode::Char('k'));
        assert!(app.agents.demos.is_empty());
        assert!(app.agents.view == AgentView::Composer);
        assert_eq!(app.draft.text, "keep this draft");
        assert!(app.messages.is_empty());
    }
}

#[test]
fn exported_task_views_escape_demo_names() {
    let mut app = App::default();
    app.agents.demos[0].name = "<task & work>";
    open_agents(&mut app);
    for detail in [false, true] {
        if detail {
            key(&mut app, KeyCode::Enter);
        }
        let svg = snapshot::svg(&mut app, 110, 36);
        assert!(svg.contains("&lt;"));
        assert!(svg.contains("&amp;"));
        assert!(svg.contains("&gt;"));
        assert!(!svg.contains("><</text>"));
        assert!(!svg.contains(">&</text>"));
    }
}

#[test]
fn resizing_task_views_preserves_selection_and_the_draft() {
    for detail in [false, true] {
        let mut app = App::default();
        app.handle(Event::Paste("draft with 👩‍💻 and 界".into()));
        open_agents(&mut app);
        for _ in 0..3 {
            key(&mut app, KeyCode::Down);
        }
        if detail {
            key(&mut app, KeyCode::Enter);
        }
        let view = app.agents.view;
        for (width, height) in [(80, 24), (40, 12), (24, 12), (23, 9), (1, 1), (110, 36)] {
            assert!(app.handle(Event::Resize(width, height)));
            let rendered = screen(&mut app, width, height);
            assert!(app.agents.view == view);
            assert_eq!(app.agents.selected, 3);
            assert_eq!(app.draft.text, "draft with 👩‍💻 and 界");
            assert!(app.messages.is_empty());
            if width >= 24 && height >= 12 {
                assert!(rendered.contains(if detail {
                    "grok-build"
                } else {
                    "Background tasks"
                }));
            }
        }
        key(&mut app, KeyCode::Esc);
        let mut terminal = Terminal::new(TestBackend::new(24, 12)).unwrap();
        terminal.draw(|frame| ui::render(frame, &mut app)).unwrap();
        let cursor = terminal.get_cursor_position().unwrap();
        assert!(cursor.x < 24 && cursor.y < 12);
    }
}

#[test]
fn selected_task_stays_visible_at_transition_heights_in_a_narrow_terminal() {
    let mut app = App::default();
    open_agents(&mut app);
    for _ in 0..3 {
        key(&mut app, KeyCode::Down);
    }
    for height in [12, 14, 15, 16, 17, 18] {
        assert!(app.handle(Event::Resize(24, height)));
        let rendered = screen(&mut app, 24, height);
        assert!(rendered.contains("Background tasks"));
        assert!(
            rendered.contains("❯ grok-build"),
            "selected task is hidden at height {height}:\n{rendered}"
        );
        assert!(app.agents.view == AgentView::List);
        assert_eq!(app.agents.selected, 3);
    }
}

#[test]
fn paste_preserves_the_hidden_draft_and_returns_footer_focus_to_the_composer() {
    for detail in [false, true] {
        let mut app = App::default();
        app.handle(Event::Paste("retained message".into()));
        key(&mut app, KeyCode::Enter);
        app.handle(Event::Paste("retained draft 界".into()));
        key(&mut app, KeyCode::Left);
        let draft_cursor = app.draft.cursor;
        open_agents(&mut app);
        if detail {
            key(&mut app, KeyCode::Enter);
        }
        let view = app.agents.view;
        assert!(app.handle(Event::Paste("hidden\r\npaste\u{1b}".into())));
        assert!(app.agents.view == view);
        assert_eq!(app.draft.text, "retained draft 界");
        assert_eq!(app.draft.cursor, draft_cursor);
        assert_eq!(app.messages, ["retained message"]);

        key(&mut app, KeyCode::Esc);
        key(&mut app, KeyCode::Down);
        assert!(app.agents.view == AgentView::Footer);
        let mut expected = app.draft.text.clone();
        expected.insert_str(draft_cursor, "visible paste");
        assert!(app.handle(Event::Paste("visible paste".into())));
        assert!(app.agents.view == AgentView::Composer);
        assert_eq!(app.draft.text, expected);
        assert_eq!(app.draft.cursor, draft_cursor + "visible paste".len());
        assert_eq!(app.messages, ["retained message"]);
    }
}

#[test]
fn narrow_detail_can_scroll_to_the_end_of_the_prompt_and_back() {
    let mut app = App::default();
    app.handle(Event::Paste("keep this draft".into()));
    open_agents(&mut app);
    key(&mut app, KeyCode::Enter);
    let ending: String = app.agents.demos[0]
        .prompt
        .split_whitespace()
        .rev()
        .take(5)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let initial = screen(&mut app, 24, 36);
    let initial_text: String = initial.chars().filter(|ch| !ch.is_whitespace()).collect();
    assert!(!initial_text.contains(&ending));
    assert_eq!(app.agents.detail_scroll, 0);

    for _ in 0..20 {
        key(&mut app, KeyCode::PageDown);
    }
    let bottom = screen(&mut app, 24, 36);
    let bottom_text: String = bottom.chars().filter(|ch| !ch.is_whitespace()).collect();
    assert!(
        bottom_text.contains(&ending),
        "prompt ending is hidden:\n{bottom}"
    );
    assert!(app.agents.detail_scroll > 0);
    for _ in 0..20 {
        key(&mut app, KeyCode::PageUp);
    }
    assert_eq!(app.agents.detail_scroll, 0);
    assert_eq!(screen(&mut app, 24, 36), initial);
    assert!(app.agents.view == AgentView::Detail);
    assert_eq!(app.draft.text, "keep this draft");
    assert!(app.messages.is_empty());
}
