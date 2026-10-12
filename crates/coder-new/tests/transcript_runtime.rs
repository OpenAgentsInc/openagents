use std::time::Instant;

use coder_new::{
    App, Mode, ViewMode,
    bundled_runtime::RuntimeEvent,
    credentials::Imported,
    live::{Entry, Update, Work},
    models::GenerationOptions,
    theme, tools, ui,
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use model_access::ApiKey;
use openrouter::Streamed;
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
use serde_json::{Value, json};
use unicode_width::UnicodeWidthStr;

fn key(app: &mut App, code: KeyCode) {
    assert!(app.handle(Event::Key(KeyEvent::new(code, KeyModifiers::NONE))));
}

/// Presses Ctrl+O, the compact/verbose switch.
fn ctrl_o(app: &mut App) {
    assert!(app.handle(Event::Key(KeyEvent::new(
        KeyCode::Char('o'),
        KeyModifiers::CONTROL
    ))));
}

fn scroll(app: &mut App, kind: MouseEventKind, row: u16) {
    assert!(app.handle(Event::Mouse(MouseEvent {
        kind,
        column: 0,
        row,
        modifiers: KeyModifiers::NONE,
    })));
}

fn live_app() -> App {
    let mut app = App::default();
    app.set_mode(Mode::Live);
    // Most of these tests read a tool call's own rows, so they start in the
    // verbose view; the compact default has its own tests.
    app.view = ViewMode::Verbose;
    app.plugins.bootstrap_credentials(Imported {
        openrouter_key: Some(ApiKey::new("offline-transcript-fixture")),
        ..Imported::default()
    });
    assert!(app.plugins.enabled);
    app
}

fn draw(app: &mut App, width: u16, height: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| ui::render(frame, app)).unwrap();
    terminal.backend().buffer().clone()
}

fn row(buffer: &Buffer, y: u16) -> String {
    (0..buffer.area.width)
        .map(|x| buffer[(x, y)].symbol())
        .collect()
}

fn canvas(buffer: &Buffer) -> String {
    (0..buffer.area.height)
        .map(|y| row(buffer, y))
        .collect::<Vec<_>>()
        .join("\n")
}

fn composer_caret(buffer: &Buffer) -> (u16, u16) {
    (0..buffer.area.height)
        .find(|&y| buffer[(1, y)].symbol() == "❯")
        .map(|y| (1, y))
        .expect("The composer caret is visible.")
}

fn delegation(app: &mut App, event: RuntimeEvent) {
    app.apply_update(Update::Delegation {
        id: app.request_id,
        delegation: "microcoder-task-1".into(),
        name: "microcoder".into(),
        task: "Review the fixture implementation".into(),
        event,
    });
}

fn tool(name: &str, input: Value, output: Value, running: bool) -> RuntimeEvent {
    RuntimeEvent::Tool {
        name: name.into(),
        input,
        output,
        running,
    }
}

fn begin_delegation(app: &mut App) {
    delegation(
        app,
        tool(
            "microcoder",
            json!({"task":"Review the fixture implementation"}),
            Value::Null,
            true,
        ),
    );
}

#[test]
fn live_delegation_appears_at_the_bottom_of_a_long_parent_transcript() {
    let mut app = live_app();
    app.live.busy = true;
    app.live.entries.push(Entry::Assistant {
        text: (0..60)
            .map(|line| format!("Earlier line {line}\n\n"))
            .collect(),
        model: None,
        elapsed_ms: None,
    });
    app.live.partial = "Handing the review to microcoder.".into();
    app.scroll = u16::MAX;
    draw(&mut app, 110, 24);

    begin_delegation(&mut app);
    assert!(app.live.partial.is_empty());
    assert!(
        matches!(app.live.entries[1], Entry::Assistant { ref text, .. }
        if text == "Handing the review to microcoder.")
    );
    assert!(app.handle(Event::Key(KeyEvent::new(
        KeyCode::End,
        KeyModifiers::CONTROL
    ))));
    let buffer = draw(&mut app, 110, 24);
    let text = canvas(&buffer);
    assert!(text.contains("Delegate microcoder"));
    assert!(text.contains("Review the fixture implementation · Running"));
    assert!(text.contains("— tokens"));
    delegation(&mut app, RuntimeEvent::Tokens(1_234));
    assert!(canvas(&draw(&mut app, 110, 24)).contains("1.2k tokens"));

    delegation(&mut app, RuntimeEvent::Text("Review in progress.".into()));
    assert!(canvas(&draw(&mut app, 110, 24)).contains("Delegate microcoder"));
    delegation(
        &mut app,
        tool(
            "microcoder",
            Value::Null,
            json!({"reply":"Reviewed."}),
            false,
        ),
    );
    assert!(canvas(&draw(&mut app, 110, 24)).contains("Review the fixture implementation · Done"));
    assert_eq!(app.delegations[0].chat.tokens, 1_234);
}

#[test]
fn scrolled_transcript_shows_a_bottom_arrow() {
    let mut app = live_app();
    app.live.entries.push(Entry::Assistant {
        text: (0..60)
            .map(|line| format!("Earlier line {line}\n\n"))
            .collect(),
        model: None,
        elapsed_ms: None,
    });
    app.scroll = 0;
    assert!(canvas(&draw(&mut app, 110, 24)).contains('↓'));
    assert!(app.handle(Event::Key(KeyEvent::new(
        KeyCode::End,
        KeyModifiers::CONTROL
    ))));
    assert!(!canvas(&draw(&mut app, 110, 24)).contains('↓'));
}

#[test]
fn switching_modes_leaves_the_transcript_where_the_reader_left_it() {
    let mut app = live_app();
    app.live.entries.push(Entry::Assistant {
        text: (0..60)
            .map(|line| format!("Earlier line {line}\n\n"))
            .collect(),
        model: None,
        elapsed_ms: None,
    });
    app.scroll = 0;
    assert!(row(&draw(&mut app, 110, 24), 0).contains("Earlier line 0"));
    // Scrolled back from the newest output, as the wheel or PageUp does.
    scroll(&mut app, MouseEventKind::ScrollDown, 2);
    let scrolled = app.scroll;
    assert!(scrolled > 0);
    let first = row(&draw(&mut app, 110, 24), 0);
    assert!(!first.contains("Earlier line 0"));
    // Shift+Tab's switch must leave the reader where they were.
    app.set_mode(Mode::Claude);
    assert_eq!(app.scroll, scrolled);
    assert_eq!(row(&draw(&mut app, 110, 24), 0), first);
    // Back at the newest output, the switch keeps it followed.
    app.scroll = u16::MAX;
    app.set_mode(Mode::Live);
    draw(&mut app, 110, 24);
    assert_eq!(app.scroll, u16::MAX);
}

#[test]
fn the_compact_view_keeps_each_tool_call_on_one_row_with_no_gaps() {
    // Verbose is opt-in: the app starts compact.
    assert_eq!(App::default().view, ViewMode::Compact);
    let mut app = live_app();
    app.view = ViewMode::Compact;
    app.live.entries.extend([
        Entry::Tool {
            name: "Read".into(),
            input: json!({"path":"src/read.rs"}),
            output: json!({"total_lines":120,"content":"fn read() {}"}),
            running: false,
        },
        Entry::Tool {
            name: "Grep".into(),
            input: json!({"pattern":"fn read"}),
            output: json!({"count":2,"files":1,"matches":"src/read.rs:4:fn read() {}"}),
            running: false,
        },
    ]);
    let buffer = draw(&mut app, 100, 24);
    let text = canvas(&buffer);
    // Each call is one row: its name and its parameters, and nothing else.
    assert!(row(&buffer, 0).contains("Read"), "{}", row(&buffer, 0));
    assert!(row(&buffer, 0).contains("src/read.rs"));
    assert_eq!(row(&buffer, 1).trim(), "● Grep pattern: fn read");
    // No blank row between them, and no result rows below.
    assert!(!text.contains("⎿"));
    assert!(!text.contains("120 lines"));
    assert!(!text.contains("2 matches"));
}

/// The file or folder a call works on leads its row, bare, the way the call
/// names it: `Grep crates/coder-new · pattern: ...`, not `pattern: ...`
/// first and a `path:` label in the middle.
#[test]
fn the_compact_view_leads_each_call_with_the_path_it_works_on() {
    assert_eq!(
        tools::compact_params(
            &json!({"path":"crates/coder-new","pattern":"fn rows","max_results":40}),
            80
        ),
        "crates/coder-new · pattern: fn rows · max_results: 40"
    );
    // A call with no path is unchanged, and one with only a path shows it
    // alone.
    assert_eq!(
        tools::compact_params(&json!({"pattern":"fn rows"}), 80),
        "pattern: fn rows"
    );
    assert_eq!(
        tools::compact_params(&json!({"path":"src/read.rs"}), 80),
        "src/read.rs"
    );
    // An Edit's path comes first even though its other keys sort before it.
    assert_eq!(
        tools::compact_params(
            &json!({"path":"a.rs","old_string":"one","new_string":"two"}),
            80
        ),
        "a.rs · old_string: one · new_string: two"
    );
    // The verbose parameter rows lead with the path too.
    let lines = tools::parameter_lines(&json!({"path":"a.rs","old_string":"one"}), 40);
    assert!(
        lines[0].to_string().contains("path: a.rs"),
        "{}",
        lines[0].to_string()
    );
}

#[test]
fn a_compact_tool_call_leaves_one_blank_row_above_the_reply_that_follows() {
    let mut app = live_app();
    app.view = ViewMode::Compact;
    app.live.entries.extend([
        Entry::Tool {
            name: "Read".into(),
            input: json!({"path":"src/read.rs"}),
            output: json!({"total_lines":120,"content":"fn read() {}"}),
            running: false,
        },
        Entry::Assistant {
            text: "The file reads one path.".into(),
            model: None,
            elapsed_ms: None,
        },
    ]);
    let buffer = draw(&mut app, 100, 24);
    // The call is one row, then one blank row, then the reply — the same
    // single blank row the reply has below it.
    assert!(row(&buffer, 0).contains("Read"), "{}", row(&buffer, 0));
    assert_eq!(row(&buffer, 1).trim(), "", "{}", row(&buffer, 1));
    assert_eq!(row(&buffer, 2).trim(), "● The file reads one path.");
    assert_eq!(row(&buffer, 3).trim(), "", "{}", row(&buffer, 3));
}

#[test]
fn ctrl_o_switches_between_the_compact_and_verbose_views() {
    let mut app = live_app();
    app.view = ViewMode::Compact;
    app.live.entries.push(Entry::Tool {
        name: "Read".into(),
        input: json!({"path":"src/read.rs"}),
        output: json!({"total_lines":120,"content":"fn read() {}"}),
        running: false,
    });
    // The default is compact: one row, no result row.
    assert_eq!(app.view, ViewMode::Compact);
    assert!(!canvas(&draw(&mut app, 100, 24)).contains("120 lines"));
    ctrl_o(&mut app);
    assert_eq!(app.view, ViewMode::Verbose);
    let verbose = canvas(&draw(&mut app, 100, 24));
    assert!(verbose.contains("120 lines"), "{verbose}");
    ctrl_o(&mut app);
    assert_eq!(app.view, ViewMode::Compact);
    assert!(!canvas(&draw(&mut app, 100, 24)).contains("120 lines"));
}

/// The one place the key is documented is `/help`, where Esc closes it.
#[test]
fn the_help_lists_the_view_key() {
    assert!(coder_new::slash::help().contains("Ctrl+O  Show or hide each tool call's details"));
}

#[test]
fn selected_subagent_header_shows_its_own_model_and_reasoning() {
    let mut app = live_app();
    app.live.busy = true;
    begin_delegation(&mut app);
    delegation(&mut app, RuntimeEvent::Model("openai/gpt-test:high".into()));
    key(&mut app, KeyCode::Down);
    let buffer = draw(&mut app, 110, 24);
    assert!(row(&buffer, 1).contains("microcoder · gpt-test"));
    delegation(&mut app, RuntimeEvent::Text("Reviewed.".into()));
    delegation(
        &mut app,
        tool(
            "microcoder",
            Value::Null,
            json!({"reply":"Reviewed."}),
            false,
        ),
    );
    assert!(row(&draw(&mut app, 110, 24), 1).contains("microcoder · gpt-test"));
}

#[test]
fn delegation_completion_sums_usage_without_a_total() {
    let mut app = live_app();
    app.live.busy = true;
    begin_delegation(&mut app);
    delegation(
        &mut app,
        tool(
            "acp_subagent",
            Value::Null,
            json!({"usage":{"input_tokens":1200,"output_tokens":34}}),
            false,
        ),
    );
    assert_eq!(app.delegations[0].chat.tokens, 1_234);
}

#[test]
fn nested_cli_delegations_get_their_own_selectable_rows_and_retained_chats() {
    let mut app = live_app();
    app.live.busy = true;
    begin_delegation(&mut app);
    let nested = |event| RuntimeEvent::Delegation {
        id: "cli-codex".into(),
        name: "Codex".into(),
        task: "Review the nested fixture".into(),
        event: Box::new(event),
    };
    delegation(
        &mut app,
        nested(tool(
            "acp_subagent",
            json!({"agent":"codex","task":"Review the nested fixture"}),
            Value::Null,
            true,
        )),
    );
    delegation(
        &mut app,
        nested(RuntimeEvent::Model("fixture/codex".into())),
    );
    delegation(
        &mut app,
        nested(RuntimeEvent::Text("Nested child reply.".into())),
    );
    delegation(
        &mut app,
        nested(tool(
            "acp_subagent",
            Value::Null,
            json!({"reply":"Nested child reply.","model":"fixture/codex","tokens":18}),
            false,
        )),
    );
    assert_eq!(app.delegations.len(), 2);
    assert_eq!(app.delegations[1].name, "Codex");
    assert!(!app.delegations[1].running);
    let buffer = draw(&mut app, 80, 24);
    // The delegation rail sits directly under the input's bottom rule, one
    // row per delegation, nested ones included.
    let bottom = (0..buffer.area.height)
        .rfind(|&y| row(&buffer, y).starts_with('└'))
        .unwrap();
    assert!(row(&buffer, bottom + 1).contains("microcoder"));
    assert!(row(&buffer, bottom + 2).contains("Codex"));
    app.selected_agent = Some(1);
    assert!(canvas(&draw(&mut app, 80, 24)).contains("Nested child reply."));
    let root = tempfile::tempdir().unwrap();
    let document = coder_new::trajectory::main_document(&app, root.path());
    let mut restored = App::default();
    coder_new::trajectory::restore_app(&mut restored, &document).unwrap();
    assert_eq!(restored.delegations.len(), 2);
    assert_eq!(restored.delegations[1].chat.tokens, 18);
}

#[test]
fn parameter_blocks_keep_keys_readable_and_fit_in_five_physical_rows() {
    let input = json!({
        "state": "A long state value that must not consume every row. ".repeat(30),
        "questions": {"refund":"Refund requested?","urgency":"How urgent?"},
        "model":"jev-fixture",
        "notes":"First line\nSecond line",
    });
    for width in [24, 40, 80, 120] {
        let lines = tools::parameter_lines(&input, width);
        assert_eq!(lines.len(), 5);
        for line in &lines {
            assert!(line.to_string().width() <= usize::from(width));
            assert!(!line.to_string().contains('\n'));
            assert_eq!(line.style.bg, Some(theme::BG_DARK));
        }
        let text = lines
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("model:"));
        assert!(text.contains("notes:"));
        assert_eq!(text.matches("questions.").count(), 2);
        assert!(text.contains("state:"));
        assert!(text.contains('…'));
        if width >= 40 {
            assert!(text.contains("questions.refund:"));
            assert!(text.contains("questions.urgency:"));
            assert!(text.contains("First line ↵ Second line"));
        }
    }

    let many_fields =
        json!({"a":"one", "b":"two", "c":"three", "d":"four", "e":"five", "f":"six", "g":"seven"});
    let lines = tools::parameter_lines(&many_fields, 40);
    assert_eq!(lines.len(), 5);
    assert_eq!(
        lines.last().unwrap().to_string().trim(),
        "│  … 3 more fields"
    );
    for value in [Value::Null, json!({})] {
        assert!(tools::parameter_lines(&value, 40).is_empty());
    }
    let unicode = tools::parameter_lines(&json!({"任务":"Read 界面 and é safely"}), 24);
    assert!(unicode.iter().all(|line| line.to_string().width() <= 24));
}

#[test]
fn live_plugin_calls_show_separate_bounded_input_and_result_blocks() {
    let mut app = live_app();
    app.live.entries.push(Entry::Tool {
        name: "jev".into(),
        input: json!({"model":"jev-fixture","questions":{"refund":"Was a refund requested?"},"state":"Long evidence. ".repeat(50)}),
        output: json!({"answers":{"refund":"yes"},"model":"jev-fixture","usage":{"input_tokens":120,"output_tokens":3}}),
        running: false,
    });
    let buffer = draw(&mut app, 80, 40);
    let text = canvas(&buffer);
    assert!(text.contains("Plugin jev"));
    for label in [
        "questions.refund:",
        "state:",
        "answers.refund:",
        "usage.input_tokens:",
        "usage.output_tokens:",
    ] {
        assert!(text.contains(label), "Missing visible parameter {label}.");
    }
    assert!(!text.contains("{\"model\""));
    assert!(!text.contains("\\n"));
    let block_rows: Vec<_> = (0..buffer.area.height)
        .filter(|&y| row(&buffer, y).contains('│'))
        .collect();
    assert!(block_rows.len() <= 10);
    assert!(block_rows.len() >= 5);
}

#[test]
fn microcoder_work_stays_in_its_selectable_child_chat_and_trackpad_keeps_the_selection() {
    let cwd = tempfile::tempdir().unwrap();
    let mut app = live_app();
    app.submit("Parent task", cwd.path());
    app.request.take().unwrap();
    begin_delegation(&mut app);
    delegation(&mut app, RuntimeEvent::Model("fixture/child".into()));
    delegation(
        &mut app,
        tool(
            "Run",
            json!({"command":"child-only command"}),
            Value::Null,
            true,
        ),
    );
    delegation(
        &mut app,
        tool(
            "Run",
            json!({"command":"child-only command"}),
            json!({"stdout":"child-only output"}),
            false,
        ),
    );
    delegation(&mut app, RuntimeEvent::Text("Child answer only.".into()));
    app.elapsed_seconds = 75;
    delegation(
        &mut app,
        tool(
            "microcoder",
            json!({"task":"Review the fixture implementation"}),
            json!({"reply":"Child answer only.","model":"fixture/child","tokens":1_234}),
            false,
        ),
    );
    assert_eq!(app.delegations.len(), 1);
    assert_eq!(app.delegations[0].name, "microcoder");
    assert_eq!(app.delegations[0].chat.tokens, 1_234);
    assert_eq!(app.delegations[0].elapsed_seconds, 75);
    assert!(!app.delegations[0].running);
    assert!(
        !app.live
            .entries
            .iter()
            .any(|entry| matches!(entry, Entry::Tool { .. }))
    );
    assert_eq!(
        app.live
            .entries
            .iter()
            .filter(|entry| matches!(entry, Entry::Delegation { .. }))
            .count(),
        1
    );
    assert!(
        app.delegations[0]
            .chat
            .entries
            .iter()
            .any(|entry| matches!(entry, Entry::Tool {name, running:false, ..} if name == "Run"))
    );

    app.draft.text = "unsent main draft".into();
    app.draft.cursor = app.draft.text.len();
    app.scroll = 0;
    let main = draw(&mut app, 110, 40);
    let main_text = canvas(&main);
    assert!(main_text.contains("Delegate microcoder"));
    assert!(!main_text.contains("Plugin microcoder"));
    assert!(!main_text.contains("child-only command"));
    assert!(!main_text.contains("child-only output"));
    assert!(!main_text.contains("Child answer only."));
    let main_caret = composer_caret(&main);
    assert_eq!(main[main_caret].fg, theme::TEXT_SECONDARY);

    key(&mut app, KeyCode::Down);
    assert_eq!(app.selected_agent, Some(0));
    assert!(app.draft.text.is_empty());
    app.scroll = 0;
    let child = draw(&mut app, 110, 40);
    let child_text = canvas(&child);
    assert!(child_text.contains("child-only command"));
    assert!(child_text.contains("child-only output"));
    assert!(child_text.contains("Child answer only."));
    assert!(!child_text.contains("Parent task"));
    let child_caret = composer_caret(&child);
    assert_eq!(child[child_caret].fg, theme::GRAY_DIM);
    let rail_y = (0..child.area.height)
        .find(|&y| {
            let text = row(&child, y);
            text.contains("microcoder") && text.contains("tokens")
        })
        .unwrap();
    let rail = row(&child, rail_y);
    assert!(rail.contains("1m 15s"));
    assert!(rail.contains("1.2k tokens ↓"));
    assert_eq!(child[(0, rail_y)].bg, theme::BG_DARK);
    assert_eq!(child[(child.area.width - 1, rail_y)].bg, theme::BG_DARK);

    for index in 0..40 {
        app.delegations[0].chat.entries.push(Entry::Assistant {
            elapsed_ms: None,
            text: format!("Child history {index}."),
            model: None,
        });
    }
    app.scroll = 0;
    draw(&mut app, 110, 40);
    let initial_builds = app.delegations[0].chat.layout_builds();
    scroll(&mut app, MouseEventKind::ScrollDown, rail_y);
    assert_eq!(app.scroll, 3);
    assert_eq!(app.selected_agent, Some(0));
    let scrolled = draw(&mut app, 110, 40);
    assert_eq!(app.delegations[0].chat.layout_builds(), initial_builds);
    assert_eq!(row(&scrolled, rail_y), rail);
    app.apply_update(Update::Delta {
        id: app.request_id,
        text: "Parent is still working.".into(),
    });
    app.apply_update(Update::Tool {
        id: app.request_id,
        name: "parent-plugin".into(),
        input: json!({"query":"parent"}),
        output: Value::Null,
        running: true,
    });
    assert_eq!(
        app.scroll, 3,
        "Parent updates preserve the selected child's transcript position."
    );
    draw(&mut app, 110, 40);
    assert_eq!(app.delegations[0].chat.layout_builds(), initial_builds);
    scroll(&mut app, MouseEventKind::ScrollUp, rail_y);
    assert_eq!(app.scroll, 0);
    assert_eq!(app.selected_agent, Some(0));
    app.draft.text = "child draft".into();
    app.draft.cursor = app.draft.text.len();
    key(&mut app, KeyCode::Up);
    assert_eq!(app.selected_agent, None);
    assert_eq!(app.draft.text, "unsent main draft");
    let restored = draw(&mut app, 110, 40);
    assert_eq!(
        restored[composer_caret(&restored)].fg,
        theme::TEXT_SECONDARY
    );
    key(&mut app, KeyCode::Down);
    assert_eq!(app.draft.text, "child draft");
    key(&mut app, KeyCode::Up);
    app.scroll = 0;
    scroll(&mut app, MouseEventKind::ScrollDown, rail_y);
    assert_eq!(app.selected_agent, None);
    assert_eq!(app.scroll, 3);
}

#[test]
fn served_model_options_are_captured_for_the_request_and_its_delegations() {
    let cwd = tempfile::tempdir().unwrap();
    let mut app = live_app();
    app.plugins.model = "openai/gpt-6-luna".into();
    app.plugins.options = GenerationOptions {
        reasoning: Some("low".into()),
        max_tokens: Some(2_048),
    };
    app.submit("First request", cwd.path());
    let request = app.request.take().unwrap();
    assert!(
        matches!(request.kind, Work::Chat {options,..} if options.reasoning.as_deref() == Some("low") && options.max_tokens == Some(2_048))
    );
    app.plugins.options = GenerationOptions {
        reasoning: Some("high".into()),
        max_tokens: None,
    };
    app.apply_update(Update::Model {
        id: request.id,
        model: "openai/gpt-6-luna".into(),
    });
    app.apply_update(Update::Delta {
        id: request.id,
        text: "Streaming answer.".into(),
    });
    assert_eq!(
        app.live.partial_model.as_deref(),
        Some("openai/gpt-6-luna:low:max-tokens=2048")
    );
    begin_delegation(&mut app);
    delegation(&mut app, RuntimeEvent::Model("fixture/child".into()));
    delegation(&mut app, RuntimeEvent::Text("Child answer.".into()));
    assert_eq!(
        app.delegations[0].chat.partial_model.as_deref(),
        Some("fixture/child")
    );
    delegation(
        &mut app,
        tool(
            "microcoder",
            Value::Null,
            json!({"reply":"Child answer.","model":"fixture/child","tokens":10}),
            false,
        ),
    );
    let reply = Streamed {
        text: "Completed answer.".into(),
        model: "openai/gpt-6-luna".into(),
        ..Streamed::default()
    };
    app.apply_update(Update::Finished {
        id: request.id,
        result: Ok(reply),
    });
    // The text streamed before the delegation is committed above it with the
    // served model and the request's options (kept for exports, never shown);
    // the finished turn adds nothing it already showed.
    assert!(matches!(
        &app.live.entries[1],
        Entry::Assistant {text, model:Some(model),..}
            if text == "Streaming answer." && model == "openai/gpt-6-luna:low:max-tokens=2048"
    ));
    assert!(matches!(
        app.live.entries.last().unwrap(),
        Entry::Delegation { .. }
    ));
    assert!(
        matches!(app.delegations[0].chat.entries.last().unwrap(),Entry::Assistant {model:Some(model),..} if model == "fixture/child")
    );
    app.submit("Second request", cwd.path());
    let second = app.request.take().unwrap();
    app.apply_update(Update::Finished {
        id: second.id,
        result: Ok(Streamed {
            text: "Second answer.".into(),
            model: "openai/gpt-6-luna".into(),
            ..Streamed::default()
        }),
    });
    assert!(
        matches!(app.live.entries.last().unwrap(),Entry::Assistant {model:Some(model),..} if model == "openai/gpt-6-luna:high")
    );
    assert!(app.live.entries.iter().any(|entry| matches!(entry,Entry::Assistant {model:Some(model),..} if model == "openai/gpt-6-luna:low:max-tokens=2048")));
}

#[test]
fn submitting_in_a_selected_child_routes_the_follow_up_to_that_delegation() {
    let cwd = tempfile::tempdir().unwrap();
    let mut app = live_app();
    app.submit("Parent question stays in the parent chat", cwd.path());
    let parent = app.request.take().unwrap();
    begin_delegation(&mut app);
    delegation(
        &mut app,
        RuntimeEvent::Text("Original child answer.".into()),
    );
    delegation(
        &mut app,
        tool(
            "microcoder",
            Value::Null,
            json!({"reply":"Original child answer.","tokens":10}),
            false,
        ),
    );
    app.apply_update(Update::Finished {
        id: parent.id,
        result: Ok(Streamed {
            text: "Parent summary.".into(),
            ..Streamed::default()
        }),
    });
    let parent_entries = app.live.entries.clone();
    app.plugins.model = "openai/gpt-6-luna".into();
    app.plugins.options = GenerationOptions {
        reasoning: Some("low".into()),
        max_tokens: None,
    };
    key(&mut app, KeyCode::Down);
    assert!(app.handle(Event::Paste("Child follow-up only".into())));
    key(&mut app, KeyCode::Enter);
    let follow_up = app.request.take().expect("The child follow-up was queued.");
    assert_ne!(follow_up.id, parent.id);
    match follow_up.kind {
        Work::Delegate {
            delegation,
            name,
            tool,
            arguments,
            options,
            model,
            ..
        } => {
            assert_eq!(delegation, format!("{}:microcoder-task-1", parent.id));
            assert_eq!(name, "microcoder");
            assert_eq!(tool, "microcoder");
            assert_eq!(model, "openai/gpt-6-luna");
            assert_eq!(options.reasoning.as_deref(), Some("low"));
            let task = arguments["task"].as_str().unwrap();
            assert!(task.contains("Original child answer."));
            assert!(task.contains("Child follow-up only"));
            assert!(!task.contains("Parent question stays"));
            assert!(!task.contains("Parent summary."));
        }
        _ => panic!("A child submission must queue a delegation request."),
    }
    assert!(app.live.entries == parent_entries);
    assert!(
        matches!(app.delegations[0].chat.entries.last().unwrap(), Entry::User(text) if text == "Child follow-up only")
    );
    assert!(app.delegations[0].running);
    assert!(app.delegations[0].chat.busy);
    assert_eq!(app.selected_agent, Some(0));
    assert!(app.draft.text.is_empty());
}

#[test]
fn a_thousand_message_transcript_reuses_layout_while_scrolling_and_editing_the_draft() {
    let mut app = live_app();
    app.live.entries = (0..1_000).map(|index| Entry::Assistant {
 elapsed_ms: None,
        text:format!("Message {index}: **bold text**, `inline_code`, and a [link](https://example.com).\n\n```rust\nlet value = {index};\n```\n\n| Field | Value |\n| --- | --- |\n| Count | {index} |"),
        model:Some("fixture/model:low".into()),
    }).collect();
    let mut terminal = Terminal::new(TestBackend::new(110, 40)).unwrap();
    terminal.draw(|frame| ui::render(frame, &mut app)).unwrap();
    assert_eq!(app.live.layout_builds(), 1_000);
    let started = Instant::now();
    for _ in 0..25 {
        scroll(&mut app, MouseEventKind::ScrollDown, 15);
        terminal.draw(|frame| ui::render(frame, &mut app)).unwrap();
    }
    let average = started.elapsed() / 25;
    eprintln!("Warm scrolling of 1,000 Markdown messages: {average:?} per draw.");
    assert_eq!(app.live.layout_builds(), 1_000);
    for _ in 0..5 {
        app.tick();
    }
    let phase = app.animation_frame;
    assert_ne!(phase, 0);
    key(&mut app, KeyCode::Char('x'));
    assert_eq!(app.animation_frame, phase);
    terminal.draw(|frame| ui::render(frame, &mut app)).unwrap();
    assert_eq!(app.live.layout_builds(), 1_000);

    if let Entry::Assistant { text, .. } = &mut app.live.entries[500] {
        text.push_str(" Edited message.");
    }
    terminal.draw(|frame| ui::render(frame, &mut app)).unwrap();
    assert_eq!(app.live.layout_builds(), 1_001);
    app.live.entries.push(Entry::User("A new message.".into()));
    terminal.draw(|frame| ui::render(frame, &mut app)).unwrap();
    assert_eq!(app.live.layout_builds(), 1_002);
    app.live.partial = "A streaming answer.".into();
    terminal.draw(|frame| ui::render(frame, &mut app)).unwrap();
    assert_eq!(app.live.layout_builds(), 1_003);
    app.live.partial.push_str(" More text.");
    terminal.draw(|frame| ui::render(frame, &mut app)).unwrap();
    assert_eq!(app.live.layout_builds(), 1_004);
    draw(&mut app, 110, 55);
    assert_eq!(app.live.layout_builds(), 1_004);
    draw(&mut app, 90, 40);
    assert_eq!(app.live.layout_builds(), 2_006);
    draw(&mut app, 90, 50);
    assert_eq!(app.live.layout_builds(), 2_006);
}

#[test]
fn reused_provider_call_ids_create_distinct_delegations_between_turns() {
    let root = tempfile::tempdir().unwrap();
    let mut app = live_app();
    app.submit("First task", root.path());
    let first = app.request.take().unwrap();
    begin_delegation(&mut app);
    delegation(
        &mut app,
        tool(
            "microcoder",
            Value::Null,
            json!({"reply":"First result"}),
            false,
        ),
    );
    app.apply_update(Update::Finished {
        id: first.id,
        result: Ok(Streamed {
            text: "Done".into(),
            ..Default::default()
        }),
    });
    app.submit("Second task", root.path());
    app.request.take().unwrap();
    begin_delegation(&mut app);
    assert_eq!(app.delegations.len(), 2);
    assert_ne!(app.delegations[0].id, app.delegations[1].id);
    assert!(!app.delegations[0].running);
    assert!(app.delegations[1].running);
}

#[test]
fn fallback_attribution_drops_options_the_fallback_did_not_receive() {
    let root = tempfile::tempdir().unwrap();
    let mut app = live_app();
    app.plugins.options = GenerationOptions {
        reasoning: Some("low".into()),
        max_tokens: Some(2048),
    };
    app.submit("Continue after provider failure", root.path());
    let request = app.request.take().unwrap();
    for model in ["openagents/fallback", "google/gemini-2.5-flash"] {
        app.apply_update(Update::Model {
            id: request.id,
            model: model.into(),
        });
    }
    assert_eq!(
        app.live.partial_model.as_deref(),
        Some("google/gemini-2.5-flash")
    );
}

/// The model the person chose failed or refused the turn (#11132): the
/// failure stays on screen and Try another model is one Enter away; any
/// other failure leaves the composer alone.
#[test]
fn a_chosen_model_that_misses_offers_another_model() {
    let root = tempfile::tempdir().unwrap();
    let mut app = live_app();
    app.submit("Fix the build", root.path());
    let request = app.request.take().unwrap();
    app.apply_update(Update::Model {
        id: request.id,
        model: coder_new::provider::PINNED_MISSED.into(),
    });
    // The marker names no model that answered.
    assert_eq!(app.live.partial_model, None);
    let failure = coder_new::provider::pinned_failure(
        "anthropic/claude-fable-5.1",
        "OpenRouter denied this request (HTTP 403).",
    );
    app.apply_update(Update::Finished {
        id: request.id,
        result: Err(failure.clone()),
    });
    assert!(!app.live.busy);
    assert_eq!(app.live.notice.as_deref(), Some(failure.as_str()));
    assert!(failure.contains("Try another model"));
    assert_eq!(app.draft.text, coder_new::TRY_ANOTHER_MODEL);
    assert!(!app.live.pinned_missed);
    key(&mut app, KeyCode::Enter);
    assert!(app.model_picker.is_some());

    // A failure on the free router, which picks its own model, offers nothing.
    let mut app = live_app();
    app.submit("Fix the build", root.path());
    let request = app.request.take().unwrap();
    app.apply_update(Update::Finished {
        id: request.id,
        result: Err("OpenRouter denied this request (HTTP 403).".into()),
    });
    assert!(app.draft.text.is_empty());
}

#[test]
fn a_judged_step_shows_jevs_estimate_on_the_run_row_and_never_a_budget() {
    let mut app = live_app();
    app.live.busy = true;
    begin_delegation(&mut app);
    // Jev has judged nothing yet: the row says the run is going, no share.
    let text = canvas(&draw(&mut app, 110, 24));
    assert!(text.contains("Review the fixture implementation · Running"));
    assert!(!text.contains("% done"));

    // A step Jev could not judge shows the step alone, never a made-up share.
    delegation(
        &mut app,
        RuntimeEvent::Progress {
            step: 1,
            complete: None,
        },
    );
    let text = canvas(&draw(&mut app, 110, 24));
    assert!(text.contains("Review the fixture implementation · step 1"));
    assert!(!text.contains("% done"));

    delegation(
        &mut app,
        RuntimeEvent::Progress {
            step: 3,
            complete: Some(0.4),
        },
    );
    let text = canvas(&draw(&mut app, 110, 24));
    assert!(text.contains("Review the fixture implementation · step 3 · ≈40% done"));
    assert!(!text.contains(" of "), "no step budget: {text}");

    // The ended run shows its outcome, not a stale estimate.
    delegation(
        &mut app,
        tool(
            "microcoder",
            Value::Null,
            json!({"reply":"Reviewed."}),
            false,
        ),
    );
    let text = canvas(&draw(&mut app, 110, 24));
    assert!(text.contains("Review the fixture implementation · Done"));
    assert!(!text.contains("% done"));
}

#[test]
fn new_chat_heading_shows_the_version_and_disappears_on_send() {
    let mut app = live_app();
    let title = "OpenAgents Coder";
    let version = concat!("v", env!("CARGO_PKG_VERSION"));
    let buffer = draw(&mut app, 80, 24);
    // The mark takes rows 0-7 at x=2; the name sits beside row 4 and the
    // version on row 5, each three cells past the mark at x=22.
    assert!(row(&buffer, 4).contains(title));
    assert!(row(&buffer, 5).contains(version));
    for x in 22..(22 + title.len() as u16) {
        assert_eq!(buffer[(x, 4)].fg, theme::TEXT_PRIMARY);
    }
    for x in 22..(22 + version.len() as u16) {
        assert_eq!(buffer[(x, 5)].fg, theme::GRAY_BRIGHT);
    }
    key(&mut app, KeyCode::Char('H'));
    assert!(canvas(&draw(&mut app, 80, 24)).contains(title));
    let root = tempfile::tempdir().unwrap();
    app.submit("Hello", root.path());
    let request = app.request.take().unwrap();
    assert!(!canvas(&draw(&mut app, 80, 24)).contains(title));
    app.apply_update(Update::Finished {
        id: request.id,
        result: Err("Offline fixture".into()),
    });
    assert!(!canvas(&draw(&mut app, 80, 24)).contains(title));
}

#[test]
fn existing_chat_does_not_show_the_new_chat_heading() {
    let mut app = live_app();
    app.live.entries.push(Entry::User("Earlier message".into()));
    assert!(!canvas(&draw(&mut app, 80, 24)).contains("OpenAgents Coder"));
}
