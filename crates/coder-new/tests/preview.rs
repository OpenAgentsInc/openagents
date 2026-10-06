use coder_new::agents::{DEMOS, DemoMessage, MAIN_PLUGINS, MAIN_TOOLS};
use coder_new::tools::{PluginCall, ToolCall, ToolKind, ToolState, tool_lines};
use coder_new::{App, Screen, snapshot, theme, ui};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    Terminal,
    backend::TestBackend,
    style::Style,
    text::Text,
    widgets::{Block, Paragraph},
};

fn key(app: &mut App, code: KeyCode) {
    assert!(app.handle(Event::Key(KeyEvent::new(code, KeyModifiers::NONE))));
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
    assert!(conversation.contains("Review the terminal with four agents."));
    assert!(!conversation.contains("Conversation first"));
    assert_eq!(composer_rules(&conversation).len(), 2);
    assert!(!conversation.contains("Sample data"));
    let body = transcript_text(&conversation);
    assert_tool_calls(&body, MAIN_TOOLS.iter());
    assert_plugin_calls(&body, MAIN_PLUGINS.iter());
    let lines: Vec<_> = body.lines().collect();
    for (index, demo) in DEMOS.iter().enumerate() {
        let row = lines
            .iter()
            .position(|line| line.contains(&format!("Delegate {}", demo.name)))
            .unwrap();
        if index == 0 {
            assert!(lines[row - 1].trim().is_empty());
            assert!(lines[row - 2].contains(MAIN_PLUGINS.last().unwrap().output));
        }
        assert!(lines[row + 1].contains(demo.task));
        assert!(lines[row + 1].contains("Running"));
        assert!(lines[row + 1].contains(&format!("{} tokens", demo.tokens)));
        if let Some(next) = DEMOS.get(index + 1) {
            assert!(lines[row + 2].contains(&format!("Delegate {}", next.name)));
        }
    }
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
    let tall = screen(&mut app, 110, 70);
    let lines: Vec<_> = tall.lines().collect();
    let delegation = lines
        .iter()
        .position(|line| line.contains("Delegate grok-build"))
        .unwrap();
    assert!(lines[delegation + 1].contains("Running"));
    assert!(lines[delegation + 2].trim().is_empty());
    assert!(lines[delegation + 3].contains("❯ first"));
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
    assert!(svg.contains("fill=\"#0a0a0a\""));
    assert!(svg.contains("&lt;"));
    assert!(svg.contains("&amp;"));
    assert!(svg.contains("&gt;"));
    assert!(!svg.contains("<build"));
}

fn composer_rules(rendered: &str) -> Vec<(usize, &str)> {
    rendered
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty() && line.trim().chars().all(|ch| ch == '─'))
        .collect()
}

fn composer_text(rendered: &str) -> String {
    let rules = composer_rules(rendered);
    rendered
        .lines()
        .skip(rules[0].0)
        .take(rules[1].0 - rules[0].0 + 1)
        .collect::<Vec<_>>()
        .join("\n")
}

fn assert_agent_rail(rendered: &str) {
    let rules = composer_rules(rendered);
    assert_eq!(rules.len(), 2);
    let (composer_bottom, rule) = rules[1];
    let right_edge = rule.trim_end().chars().count() - 2;
    let mut previous_row = composer_bottom;
    let mut task_column = None;
    for demo in &DEMOS {
        let (row, line) = rendered
            .lines()
            .enumerate()
            .skip(composer_bottom + 1)
            .find(|(_, line)| {
                line.contains(&format!("○ {}", demo.name))
                    || line.contains(&format!("❯ {}", demo.name))
            })
            .unwrap_or_else(|| panic!("missing agent row: {}", demo.name));
        assert!(row > previous_row);
        assert!(line.contains(demo.task));
        assert!(!line.contains(&format!("{}:", demo.name)));
        let task_offset = line.find(demo.task).unwrap();
        let column = line[..task_offset].chars().count();
        if let Some(expected) = task_column {
            assert_eq!(column, expected);
        } else {
            task_column = Some(column);
        }
        assert!(
            line.trim_end()
                .ends_with(&format!("↓ {} tokens", demo.tokens))
        );
        assert_eq!(line.trim_end().chars().count(), right_edge);
        previous_row = row;
    }
}

#[test]
fn composer_uses_two_rules_and_expands_for_a_multiline_draft() {
    let mut app = App::default();
    let empty = screen(&mut app, 110, 36);
    let rules = composer_rules(&empty);
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[1].0 - rules[0].0, 2);
    for (_, rule) in &rules {
        assert_eq!(*rule, "─".repeat(110));
    }
    assert!(empty.lines().nth(rules[0].0 + 1).unwrap().starts_with("❯ "));
    let mut terminal = Terminal::new(TestBackend::new(110, 36)).unwrap();
    terminal.draw(|frame| ui::render(frame, &mut app)).unwrap();
    let cursor = terminal.get_cursor_position().unwrap();
    assert_eq!(cursor.x, 2);
    assert_eq!(usize::from(cursor.y), rules[0].0 + 1);
    assert!(!empty.contains(" Message "));
    assert!(!composer_text(&empty).chars().any(|ch| "│╭╮╰╯".contains(ch)));

    app.handle(Event::Paste("first\nsecond\nthird".into()));
    key(&mut app, KeyCode::Left);
    key(&mut app, KeyCode::Char('x'));
    assert_eq!(app.draft.text, "first\nsecond\nthirxd");
    let expanded = screen(&mut app, 110, 36);
    let rules = composer_rules(&expanded);
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[1].0 - rules[0].0, 4);
    assert!(
        !composer_text(&expanded)
            .chars()
            .any(|ch| "│╭╮╰╯".contains(ch))
    );
    terminal.draw(|frame| ui::render(frame, &mut app)).unwrap();
    let cursor = terminal.get_cursor_position().unwrap();
    assert_eq!(usize::from(cursor.y), rules[1].0 - 1);
    assert!(cursor.x < 110);
    assert!(app.messages.is_empty());

    app.draft = Default::default();
    app.handle(Event::Paste("a".repeat(109)));
    let wrapped = screen(&mut app, 110, 36);
    let rules = composer_rules(&wrapped);
    assert_eq!(rules[1].0 - rules[0].0, 3);
    assert_eq!(
        wrapped.lines().nth(rules[0].0 + 1).unwrap(),
        format!("❯ {}", "a".repeat(108))
    );
    assert!(
        wrapped
            .lines()
            .nth(rules[0].0 + 2)
            .unwrap()
            .starts_with("  a")
    );
    terminal.draw(|frame| ui::render(frame, &mut app)).unwrap();
    let cursor = terminal.get_cursor_position().unwrap();
    assert_eq!(cursor.x, 3);
    assert_eq!(usize::from(cursor.y), rules[1].0 - 1);
}

#[test]
fn four_agent_rows_show_the_current_task_and_aligned_token_counts_below_the_composer() {
    assert_eq!(
        DEMOS.map(|demo| demo.name),
        ["claude-code", "codex", "devin-cli", "grok-build"]
    );
    let mut app = App::default();
    assert_agent_rail(&screen(&mut app, 110, 36));
}

#[test]
fn resizing_keeps_the_draft_cursor_visible_and_agent_rows_aligned() {
    let mut app = App::default();
    app.handle(Event::Paste("draft with 👩‍💻 and 界\ncontinued draft".into()));
    let draft_cursor = app.draft.cursor;
    for (width, height) in [(80, 24), (80, 16), (24, 12), (23, 9), (80, 24)] {
        assert!(app.handle(Event::Resize(width, height)));
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| ui::render(frame, &mut app)).unwrap();
        let cursor = terminal.get_cursor_position().unwrap();
        assert!(cursor.x < width && cursor.y < height);
        assert_eq!(app.draft.text, "draft with 👩‍💻 and 界\ncontinued draft");
        assert_eq!(app.draft.cursor, draft_cursor);
        assert!(app.messages.is_empty());
        if width == 80 {
            assert_agent_rail(&screen(&mut app, width, height));
        } else if width == 24 {
            let rendered = screen(&mut app, width, height);
            for demo in &DEMOS {
                let row = rendered
                    .lines()
                    .find(|line| line.contains(demo.name))
                    .unwrap();
                assert!(row.trim_end().ends_with(&format!("↓ {}", demo.tokens)));
            }
        }
    }
}

fn compact_text(text: &str) -> String {
    text.chars().filter(|ch| !ch.is_whitespace()).collect()
}

fn transcript_text(rendered: &str) -> String {
    let composer_top = composer_rules(rendered)[0].0;
    rendered
        .lines()
        .take(composer_top)
        .collect::<Vec<_>>()
        .join("\n")
}

fn assert_tool_calls<'a>(rendered: &str, calls: impl Iterator<Item = &'a ToolCall>) {
    let text = compact_text(rendered);
    for call in calls {
        let kind = match call.kind {
            ToolKind::Read => "Read",
            ToolKind::Search => "Search",
            ToolKind::Edit => "Edit",
            ToolKind::Run => "Run",
        };
        assert!(text.contains(&format!("{kind}{}", compact_text(call.input))));
        if matches!(
            (call.kind, call.state),
            (ToolKind::Edit, ToolState::Complete)
        ) {
            assert!(!text.contains("@@"));
            for patch_line in call.output.lines().filter(|line| !line.starts_with("@@")) {
                let code = patch_line
                    .strip_prefix(['+', '-', ' '])
                    .unwrap_or(patch_line);
                assert!(
                    text.contains(&compact_text(code)),
                    "missing edited code: {code}"
                );
            }
        } else {
            assert!(text.contains(&compact_text(call.output)));
        }
        match call.state {
            ToolState::Complete => {
                assert!(text.contains(&format!("◆{kind}{}", compact_text(call.input))));
            }
            ToolState::Running => assert!(text.contains("╰Running")),
            ToolState::Failed => assert!(text.contains("╰Failed")),
        }
    }
}

fn assert_plugin_calls<'a>(rendered: &str, calls: impl Iterator<Item = &'a PluginCall>) {
    let text = compact_text(rendered);
    for call in calls {
        let identity = format!("Plugin{}.{}", call.plugin, call.operation);
        assert!(text.contains(&identity));
        assert!(text.contains(&compact_text(call.input)));
        assert!(text.contains(&compact_text(call.output)));
        match call.state {
            ToolState::Complete => assert!(text.contains(&format!("◆{identity}"))),
            ToolState::Running => assert!(text.contains("╰Running")),
            ToolState::Failed => assert!(text.contains("╰Failed")),
        }
    }
}

#[test]
fn selecting_each_agent_loads_its_own_demo_conversation() {
    let mut app = App::default();
    for (index, demo) in DEMOS.iter().enumerate() {
        key(&mut app, KeyCode::Down);
        assert_eq!(app.selected_agent, Some(index));
        let rendered = screen(&mut app, 110, 70);
        let body = transcript_text(&rendered);
        assert_tool_calls(
            &body,
            demo.conversation
                .iter()
                .filter_map(|message| match message {
                    DemoMessage::Tool(call) => Some(call),
                    _ => None,
                }),
        );
        assert_plugin_calls(
            &body,
            demo.conversation
                .iter()
                .filter_map(|message| match message {
                    DemoMessage::Plugin(call) => Some(call),
                    _ => None,
                }),
        );
        let prompt = demo
            .conversation
            .iter()
            .find_map(|message| match message {
                DemoMessage::User(text) => Some(*text),
                _ => None,
            })
            .unwrap();
        let response = demo
            .conversation
            .iter()
            .find_map(|message| match message {
                DemoMessage::Assistant(text) => Some(*text),
                _ => None,
            })
            .unwrap();
        let body = compact_text(&body);
        assert!(
            body.contains(&compact_text(prompt)),
            "missing demo prompt for {}",
            demo.name
        );
        assert!(
            body.contains(&compact_text(response)),
            "missing demo response for {}",
            demo.name
        );
        assert!(!body.contains("Conversationfirst"));
        assert!(rendered.contains(&format!("❯ {}", demo.name)));
        assert_agent_rail(&rendered);
    }
}

#[test]
fn agent_navigation_clamps_at_the_ends_and_escape_restores_main() {
    let mut app = App::default();
    assert_eq!(app.selected_agent, None);
    key(&mut app, KeyCode::Up);
    assert_eq!(app.selected_agent, None);
    for _ in 0..8 {
        key(&mut app, KeyCode::Down);
    }
    assert_eq!(app.selected_agent, Some(3));
    for selected in [Some(2), Some(1), Some(0), None] {
        key(&mut app, KeyCode::Up);
        assert_eq!(app.selected_agent, selected);
    }
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Down);
    assert_eq!(app.selected_agent, Some(1));
    key(&mut app, KeyCode::Esc);
    assert_eq!(app.selected_agent, None);
    let main = screen(&mut app, 110, 36);
    assert!(main.contains("Review the terminal with four agents."));
    assert!(!main.contains("Conversation first"));
    assert!(app.draft.text.is_empty());
    assert!(app.messages.is_empty());
}

fn conversation_state(app: &App) -> (String, usize, Vec<String>, u16) {
    (
        app.draft.text.clone(),
        app.draft.cursor,
        app.messages.clone(),
        app.scroll,
    )
}

#[test]
fn switching_restores_each_conversations_draft_cursor_messages_and_scroll() {
    let mut app = App::default();
    app.handle(Event::Paste("main message".into()));
    key(&mut app, KeyCode::Enter);
    app.handle(Event::Paste("main draft 界".into()));
    key(&mut app, KeyCode::Left);
    app.scroll = 5;
    let main = conversation_state(&app);
    let mut agents = Vec::new();
    for (index, demo) in DEMOS.iter().enumerate() {
        key(&mut app, KeyCode::Down);
        assert_eq!(app.selected_agent, Some(index));
        assert!(app.messages.is_empty());
        assert!(app.draft.text.is_empty());
        assert_eq!(app.scroll, 0);
        app.handle(Event::Paste(format!("message for {}", demo.name)));
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.selected_agent, Some(index));
        assert_eq!(app.messages, [format!("message for {}", demo.name)]);
        app.handle(Event::Paste(format!("draft for {} 界", demo.name)));
        key(&mut app, KeyCode::Left);
        app.scroll = (index as u16 + 1) * 7;
        agents.push(conversation_state(&app));
    }
    for index in (0..DEMOS.len()).rev() {
        assert_eq!(app.selected_agent, Some(index));
        assert_eq!(conversation_state(&app), agents[index]);
        key(&mut app, KeyCode::Up);
    }
    assert_eq!(app.selected_agent, None);
    assert_eq!(conversation_state(&app), main);
    for (index, expected) in agents.iter().enumerate() {
        key(&mut app, KeyCode::Down);
        assert_eq!(app.selected_agent, Some(index));
        assert_eq!(&conversation_state(&app), expected);
    }
    key(&mut app, KeyCode::Esc);
    assert_eq!(app.selected_agent, None);
    assert_eq!(conversation_state(&app), main);
}

#[test]
fn selected_agent_keeps_the_rail_visible_and_tokens_aligned_after_resize() {
    let mut app = App::default();
    for selected in 0..DEMOS.len() {
        key(&mut app, KeyCode::Down);
        for (width, height) in [(80, 24), (24, 12)] {
            assert!(app.handle(Event::Resize(width, height)));
            let rendered = screen(&mut app, width, height);
            let rules = composer_rules(&rendered);
            let bottom = rules[1].0;
            let right_edge = rules[1].1.trim_end().chars().count() - 2;
            for (index, demo) in DEMOS.iter().enumerate() {
                let (_, line) = rendered
                    .lines()
                    .enumerate()
                    .skip(bottom + 1)
                    .find(|(_, line)| line.contains(demo.name))
                    .unwrap();
                if index == selected {
                    assert!(line.contains(&format!("❯ {}", demo.name)));
                } else if width == 80 {
                    assert!(line.contains(&format!("○ {}", demo.name)));
                }
                let suffix = if width == 80 {
                    format!("↓ {} tokens", demo.tokens)
                } else {
                    format!("↓ {}", demo.tokens)
                };
                assert!(line.trim_end().ends_with(&suffix));
                assert_eq!(line.trim_end().chars().count(), right_edge);
            }
            assert_eq!(app.selected_agent, Some(selected));
        }
    }
}

#[test]
fn header_and_rail_keep_compact_spacing_above_the_bottom_margin() {
    let mut app = App::default();
    let rendered = screen(&mut app, 110, 36);
    let lines: Vec<_> = rendered.lines().collect();
    let header = lines.iter().find(|line| line.contains("◆ Coder")).unwrap();
    assert!(header.contains("openagents / main"));
    assert!(!rendered.contains("UI preview"));
    assert!(!rendered.contains("Ctrl+C"));
    assert!(!rendered.contains("Enter preview"));
    let bottom = composer_rules(&rendered)[1].0;
    for (index, demo) in DEMOS.iter().enumerate() {
        assert!(lines[bottom + 1 + index].starts_with(&format!("  ○ {}", demo.name)));
    }
    assert_eq!(bottom + DEMOS.len(), lines.len() - 2);
    assert!(lines.last().unwrap().trim().is_empty());
    assert!(!rendered.contains("6 plugins"));
    assert!(!rendered.contains("24,000 sats"));
}

#[test]
fn animation_ticks_change_running_indicators_without_changing_conversation_state() {
    let mut app = App::default();
    app.handle(Event::Paste("retained draft 界".into()));
    app.messages.push("retained message".into());
    let before = screen(&mut app, 110, 70);
    let state = conversation_state(&app);
    let selected = app.selected_agent;
    let phase = app.animation_frame;
    let fixed_snapshot = snapshot::svg(&mut app, 110, 70);
    assert_eq!(snapshot::svg(&mut app, 110, 70), fixed_snapshot);
    let is_spinner = |ch: &char| ('\u{2800}'..='\u{28ff}').contains(ch);
    assert_eq!(before.chars().filter(is_spinner).count(), 1);

    app.tick();
    let after = screen(&mut app, 110, 70);
    assert_ne!(app.animation_frame, phase);
    assert_ne!(before, after);
    assert_eq!(after.chars().filter(is_spinner).count(), 1);
    assert_eq!(
        before
            .chars()
            .filter(|ch| !is_spinner(ch))
            .collect::<String>(),
        after
            .chars()
            .filter(|ch| !is_spinner(ch))
            .collect::<String>()
    );
    let pulsed_snapshot = snapshot::svg(&mut app, 110, 70);
    assert_ne!(fixed_snapshot, pulsed_snapshot);
    let changed: Vec<_> = fixed_snapshot
        .lines()
        .zip(pulsed_snapshot.lines())
        .filter(|(before_line, after_line)| before_line != after_line)
        .collect();
    assert_eq!(changed.len(), DEMOS.len() + 1);
    for (before_line, after_line) in changed {
        if before_line.ends_with(">◆</text>") {
            assert!(after_line.ends_with(">◆</text>"));
        } else {
            assert!(before_line.chars().any(|ch| is_spinner(&ch)));
            assert!(after_line.chars().any(|ch| is_spinner(&ch)));
        }
    }
    assert_eq!(conversation_state(&app), state);
    assert_eq!(app.selected_agent, selected);
    for _ in 0..7 {
        app.tick();
    }
    assert_eq!(app.animation_frame, phase);
    assert_eq!(snapshot::svg(&mut app, 110, 70), fixed_snapshot);

    key(&mut app, KeyCode::Down);
    app.handle(Event::Paste("agent draft 界".into()));
    app.messages.push("agent message".into());
    let before = screen(&mut app, 110, 70);
    let state = conversation_state(&app);
    let selected = app.selected_agent;
    let fixed_snapshot = snapshot::svg(&mut app, 110, 70);
    assert_eq!(snapshot::svg(&mut app, 110, 70), fixed_snapshot);
    assert_eq!(before.chars().filter(is_spinner).count(), 1);
    app.tick();
    let after = screen(&mut app, 110, 70);
    assert_ne!(before, after);
    assert_eq!(after.chars().filter(is_spinner).count(), 1);
    for (before_row, after_row) in before.lines().zip(after.lines()) {
        if before_row != after_row {
            assert!(before_row.chars().any(|ch| is_spinner(&ch)));
            assert!(after_row.chars().any(|ch| is_spinner(&ch)));
            assert_eq!(
                before_row
                    .chars()
                    .filter(|ch| !is_spinner(ch))
                    .collect::<String>(),
                after_row
                    .chars()
                    .filter(|ch| !is_spinner(ch))
                    .collect::<String>()
            );
        }
    }
    assert_eq!(conversation_state(&app), state);
    assert_eq!(app.selected_agent, selected);
    for _ in 0..7 {
        app.tick();
    }
    assert_eq!(app.animation_frame, phase);
    assert_eq!(snapshot::svg(&mut app, 110, 70), fixed_snapshot);
    assert_eq!(conversation_state(&app), state);
}

#[test]
fn edit_calls_highlight_old_and_new_code_on_bands_with_an_unpainted_gutter() {
    let main_edit = MAIN_TOOLS
        .iter()
        .find(|call| call.kind == ToolKind::Edit)
        .unwrap();
    let grok_edit = DEMOS
        .iter()
        .find(|demo| demo.name == "grok-build")
        .unwrap()
        .conversation
        .iter()
        .find_map(|message| match message {
            DemoMessage::Tool(call) if call.kind == ToolKind::Edit => Some(call),
            _ => None,
        })
        .unwrap();
    for call in [main_edit, grok_edit] {
        for width in [24, 80, 110] {
            let lines = tool_lines(call, 0, width);
            for line in lines.iter().skip(1) {
                assert!(line.width() <= usize::from(width));
            }
            let height = lines.len() as u16;
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| {
                    let area = frame.area();
                    frame.render_widget(
                        Block::default().style(Style::default().bg(theme::BG_BASE)),
                        area,
                    );
                    frame.render_widget(Paragraph::new(Text::from(lines)), area);
                })
                .unwrap();
            let buffer = terminal.backend().buffer();
            for (band, gutter_color) in [
                (theme::DIFF_DELETE_BG, theme::DIFF_DELETE_FG),
                (theme::DIFF_INSERT_BG, theme::DIFF_INSERT_FG),
            ] {
                let mut foregrounds = Vec::new();
                let mut changed_rows = 0;
                let mut numbered_rows = 0;
                for y in 1..height {
                    let Some(start) = (0..width).find(|x| buffer[(*x, y)].bg == band) else {
                        continue;
                    };
                    changed_rows += 1;
                    assert!(start > 0);
                    for x in 0..start {
                        let cell = &buffer[(x, y)];
                        assert_eq!(cell.bg, theme::BG_BASE);
                        if cell.symbol().chars().any(|ch| ch.is_ascii_digit()) {
                            numbered_rows += 1;
                            assert_eq!(cell.fg, gutter_color);
                        }
                    }
                    for x in start..width {
                        let cell = &buffer[(x, y)];
                        assert_eq!(cell.bg, band);
                        if !cell.symbol().trim().is_empty() && !foregrounds.contains(&cell.fg) {
                            foregrounds.push(cell.fg);
                        }
                    }
                }
                assert!(changed_rows > 0);
                assert!(numbered_rows > 0);
                assert!(
                    foregrounds.len() >= 2,
                    "code has no syntax color variation at width {width}"
                );
            }
        }
    }
}
