//! What the frame does with what the runtime tells it.
//!
//! `apply` is the whole of it, and it is public so this can drive it without a
//! terminal. The properties here are the ones issue #105 names: a status field
//! reports a value it received or stays empty, a failure does not become an
//! answer, and nothing the session prints leaves the palette.

use openagents_cli::coder::interactive::apply;
use openagents_cli::coder::markdown::theme::{
    DIM_TEXT_COLOR, MODEL_TEXT_COLOR, TEXT_COLOR, USER_TEXT_COLOR,
};
use openagents_cli::coder::runtime::Control;
use openagents_cli::coder::tui::{CoderUi, Entry, Role};
use openagents_cli::runtime::TurnUsage;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::Color;

fn draw(ui: &mut CoderUi) -> ratatui::buffer::Buffer {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|f| {
            let area = f.area();
            ui.render(f, area);
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

fn text_of(buffer: &ratatui::buffer::Buffer) -> String {
    buffer.content.iter().map(|c| c.symbol()).collect()
}

#[test]
fn startup_facts_are_centered_outside_the_transcript() {
    let mut ui = CoderUi::new();
    ui.cwd = "/Users/example/work/openagents".to_string();
    ui.endpoint = "https://openagents.com/api/v1".to_string();
    ui.lane = "Coder Flash".to_string();

    let buffer = draw(&mut ui);
    let text = text_of(&buffer);
    assert!(text.contains(&format!("Coder v{}", openagents_cli::VERSION)));
    assert!(text.contains("/Users/example/work/openagents"));
    assert!(text.contains("https://openagents.com/api/v1"));
    assert!(text.contains("Type /help for commands and keys."));

    let title_row = (0..buffer.area.height)
        .find(|y| {
            (0..buffer.area.width)
                .map(|x| buffer.cell((x, *y)).unwrap().symbol())
                .collect::<String>()
                .contains("Coder v")
        })
        .expect("startup box title");
    assert!(
        (6..=9).contains(&title_row),
        "the startup box is not vertically centered: row {title_row}"
    );
}

#[test]
fn the_startup_box_leaves_when_the_conversation_starts() {
    let mut ui = CoderUi::new();
    ui.cwd = "/Users/example/work/openagents".to_string();
    assert!(text_of(&draw(&mut ui)).contains("Working directory"));

    ui.entries.push(Entry::new(Role::You, "hello"));
    let text = text_of(&draw(&mut ui));
    assert!(!text.contains("Working directory"));
    assert!(text.contains("> hello"));
}

/// The model field holds what answered, and nothing until something has.
#[test]
fn the_model_field_is_empty_until_a_model_answers() {
    let mut ui = CoderUi::new();
    assert_eq!(ui.model, "", "a model was named before one answered");
    apply(&mut ui, Control::Model("gemini-3.7-flash".to_string()));
    assert_eq!(ui.model, "gemini-3.7-flash");
}

#[test]
fn an_assistant_message_shows_its_model_at_the_top_right() {
    let mut ui = CoderUi::new();
    ui.entries.push(Entry::new(
        Role::Assistant,
        "A concise answer that continues onto another line because its model label owns the \
         right edge of the first row.",
    ));
    apply(&mut ui, Control::Model("glm-5.3-flash".to_string()));

    let buffer = draw(&mut ui);
    let model = "glm-5.3-flash";
    let model_x = buffer.area.width - model.len() as u16;
    let model_y = (0..buffer.area.height)
        .find(|y| {
            (0..buffer.area.width)
                .map(|x| buffer.cell((x, *y)).unwrap().symbol())
                .collect::<String>()
                .contains(model)
        })
        .expect("the model label is visible");

    assert_eq!(model_x, 67, "the label is not flush right");
    assert_eq!(model_y, 0, "the model label is not on the message's top row");
    assert!(
        (0..model.len() as u16)
            .all(|offset| buffer.cell((model_x + offset, model_y)).unwrap().fg == MODEL_TEXT_COLOR),
        "the model label does not use 25% amber"
    );
    let row = (0..buffer.area.width)
        .map(|x| buffer.cell((x, model_y)).unwrap().symbol())
        .collect::<String>();
    assert!(row.starts_with("A concise answer"), "{row}");
    let next_row = (0..buffer.area.width)
        .map(|x| buffer.cell((x, model_y + 1)).unwrap().symbol())
        .collect::<String>();
    assert!(!next_row.trim().is_empty(), "the test message did not wrap");
}

#[test]
fn each_assistant_entry_keeps_the_model_that_answered_it() {
    let mut ui = CoderUi::new();
    ui.entries.push(Entry::new(Role::Assistant, "first"));
    apply(&mut ui, Control::Model("glm-5.3-flash".to_string()));
    ui.entries.push(Entry::new(Role::Assistant, "second"));
    apply(&mut ui, Control::Model("openrouter/free".to_string()));

    assert_eq!(ui.entries[0].model.as_deref(), Some("glm-5.3-flash"));
    assert_eq!(ui.entries[1].model.as_deref(), Some("openrouter/free"));
}

/// A turn that failed does not read afterwards as one that answered.
#[test]
fn a_failure_settles_the_stream_and_is_not_written_into_it() {
    let mut ui = CoderUi::new();
    ui.entries.push(Entry::new(Role::Assistant, "half an ans"));
    ui.loading = true;

    apply(
        &mut ui,
        Control::Failed("the proxy refused the turn: 503".to_string()),
    );

    let assistant = ui
        .entries
        .iter()
        .rfind(|e| e.role == Role::Assistant)
        .expect("the streamed entry survived");
    assert_eq!(
        assistant.text, "half an ans",
        "the failure was appended to the reply"
    );
    assert!(
        ui.entries
            .iter()
            .any(|e| e.role == Role::Notice && e.text.contains("503")),
        "the failure was swallowed"
    );
    assert!(!ui.loading, "the frame kept spinning over a finished turn");
}

/// The header says a call failed, where the reader is looking.
#[test]
fn a_failed_tool_call_says_so_on_its_header() {
    let mut ui = CoderUi::new();
    apply(
        &mut ui,
        Control::Tool {
            call_id: "c1".to_string(),
            name: "shell".to_string(),
            arguments: r#"{"command":"cargo test"}"#.to_string(),
        },
    );
    let header = ui.entries.last().unwrap().text.clone();
    assert_eq!(header, "shell cargo test");

    apply(
        &mut ui,
        Control::ToolOutput {
            call_id: "c1".to_string(),
            chunk: "The command exited with code 1.".to_string(),
        },
    );
    apply(
        &mut ui,
        Control::ToolDone {
            call_id: "c1".to_string(),
            is_error: true,
        },
    );

    let entry = ui.entries.last().unwrap();
    assert_eq!(entry.text, "shell cargo test — failed");
    assert!(
        entry
            .output
            .as_deref()
            .unwrap()
            .contains("exited with code 1")
    );
    // And the ATIF record carries the failure too, not just the screen.
    assert!(entry.tool.as_ref().unwrap().error.is_some());
}

#[test]
fn an_active_tool_rail_moves_until_the_call_finishes() {
    let mut ui = CoderUi::new();
    apply(
        &mut ui,
        Control::Tool {
            call_id: "c1".to_string(),
            name: "shell".to_string(),
            arguments: r#"{"command":"cargo test"}"#.to_string(),
        },
    );
    apply(
        &mut ui,
        Control::ToolOutput {
            call_id: "c1".to_string(),
            chunk: "running\nstill running".to_string(),
        },
    );

    let first = draw(&mut ui);
    let second = draw(&mut ui);
    let first_rail = first.cell((0, 1)).unwrap();
    let second_rail = second.cell((0, 1)).unwrap();
    assert_eq!(first_rail.symbol(), "│");
    assert_eq!(
        first.cell((2, 1)).unwrap().fg,
        USER_TEXT_COLOR,
        "tool output text is not 75% amber"
    );
    assert_ne!(
        first_rail.fg, second_rail.fg,
        "the active rail did not move"
    );

    apply(
        &mut ui,
        Control::ToolDone {
            call_id: "c1".to_string(),
            is_error: false,
        },
    );
    let settled = draw(&mut ui);
    let settled_again = draw(&mut ui);
    assert_eq!(settled.cell((0, 1)).unwrap().fg, TEXT_COLOR);
    assert_eq!(
        settled.cell((0, 1)).unwrap().fg,
        settled_again.cell((0, 1)).unwrap().fg,
        "a finished tool rail kept moving"
    );
    assert!(ui.entries[0].tool.as_ref().unwrap().done);
}

#[test]
fn reduced_motion_keeps_the_active_tool_state_static() {
    let mut ui = CoderUi::new();
    ui.motion_enabled = false;
    apply(
        &mut ui,
        Control::Tool {
            call_id: "c1".to_string(),
            name: "shell".to_string(),
            arguments: r#"{"command":"cargo test"}"#.to_string(),
        },
    );
    apply(
        &mut ui,
        Control::ToolOutput {
            call_id: "c1".to_string(),
            chunk: "running".to_string(),
        },
    );

    let first = draw(&mut ui);
    let second = draw(&mut ui);
    assert_eq!(first.cell((0, 1)).unwrap().fg, TEXT_COLOR);
    assert_eq!(
        first.cell((0, 1)).unwrap().fg,
        second.cell((0, 1)).unwrap().fg
    );
    assert!(text_of(&second).starts_with("○ shell cargo test"));
}

/// Output for a call the frame never saw start goes nowhere rather than onto
/// whatever entry happens to be last.
#[test]
fn output_for_an_unknown_call_lands_on_nothing() {
    let mut ui = CoderUi::new();
    ui.entries.push(Entry::new(Role::Assistant, "an answer"));
    apply(
        &mut ui,
        Control::ToolOutput {
            call_id: "never-started".to_string(),
            chunk: "stray".to_string(),
        },
    );
    assert!(
        !ui.entries.iter().any(|e| e.text.contains("stray")),
        "stray output was attached to an unrelated entry"
    );
}

/// Usage is accumulated and never added to the transcript.
#[test]
fn an_unreported_usage_is_not_printed_as_zero() {
    let mut ui = CoderUi::new();
    apply(&mut ui, Control::Usage(TurnUsage::default()));
    assert!(ui.entries.is_empty(), "a zero was reported as a figure");
    assert!(!ui.total_usage.reported());

    apply(
        &mut ui,
        Control::Usage(TurnUsage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
        }),
    );
    assert!(
        ui.entries.is_empty(),
        "usage was pushed as a transcript entry"
    );
    assert!(ui.total_usage.reported());
    assert_eq!(ui.total_usage.total_tokens, 15);
}

/// Command output uses the Coder amber palette.
#[test]
fn command_output_uses_only_amber_palette_colors() {
    let mut ui = CoderUi::new();
    apply(
        &mut ui,
        Control::Output(
            "**Commands**\n\n- `/help` — list them\n\n```diff\n+added\n-removed\n```".to_string(),
        ),
    );
    let buffer = draw(&mut ui);

    let amber = [Color::Rgb(255, 176, 0), Color::Rgb(131, 91, 0)];
    let ground = Color::Rgb(8, 6, 0);
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            let cell = buffer.cell((x, y)).unwrap();
            assert!(
                amber.contains(&cell.fg),
                "cell ({x},{y}) {:?} drifted off the amber palette",
                cell.fg
            );
            assert_eq!(
                cell.bg, ground,
                "cell ({x},{y}) {:?} drifted off ground",
                cell.bg
            );
        }
    }
    let text = text_of(&buffer);
    assert!(text.contains("+added"), "{text}");
    assert!(text.contains("Commands"), "{text}");
}

#[test]
fn a_user_turn_uses_75_percent_amber() {
    let mut ui = CoderUi::new();
    ui.entries.push(Entry::new(Role::You, "user turn"));

    let buffer = draw(&mut ui);
    let cells = &buffer.content;
    let start = cells
        .windows(4)
        .position(|cells| {
            cells
                .iter()
                .map(|cell| cell.symbol())
                .eq(["u", "s", "e", "r"])
        })
        .expect("the user turn is visible");

    assert_eq!(cells[start].fg, USER_TEXT_COLOR);
}

#[test]
fn long_tool_output_sweeps_from_its_first_rows_to_its_last_rows() {
    let mut ui = CoderUi::new();
    apply(
        &mut ui,
        Control::Tool {
            call_id: "scrolling-output".to_string(),
            name: "shell".to_string(),
            arguments: r#"{"command":"long command"}"#.to_string(),
        },
    );
    apply(
        &mut ui,
        Control::ToolOutput {
            call_id: "scrolling-output".to_string(),
            chunk: (1..=20)
                .map(|line| format!("line {line:02}"))
                .collect::<Vec<_>>()
                .join("\n"),
        },
    );

    let first_frame = text_of(&draw(&mut ui));
    assert!(first_frame.contains("line 01"), "{first_frame}");
    assert!(first_frame.contains("line 05"), "{first_frame}");
    assert!(!first_frame.contains("line 20"), "{first_frame}");

    for _ in 0..8 {
        draw(&mut ui);
    }
    let settled_frame = text_of(&draw(&mut ui));
    assert!(!settled_frame.contains("line 01"), "{settled_frame}");
    assert!(settled_frame.contains("line 16"), "{settled_frame}");
    assert!(settled_frame.contains("line 20"), "{settled_frame}");
}

#[test]
fn reduced_motion_opens_long_tool_output_at_its_last_rows() {
    let mut ui = CoderUi::new();
    ui.motion_enabled = false;
    apply(
        &mut ui,
        Control::Tool {
            call_id: "still-output".to_string(),
            name: "shell".to_string(),
            arguments: r#"{"command":"long command"}"#.to_string(),
        },
    );
    apply(
        &mut ui,
        Control::ToolOutput {
            call_id: "still-output".to_string(),
            chunk: (1..=20)
                .map(|line| format!("line {line:02}"))
                .collect::<Vec<_>>()
                .join("\n"),
        },
    );

    let frame = text_of(&draw(&mut ui));
    assert!(!frame.contains("line 01"), "{frame}");
    assert!(frame.contains("line 16"), "{frame}");
    assert!(frame.contains("line 20"), "{frame}");
}

#[test]
fn the_status_row_uses_50_percent_amber() {
    let mut ui = CoderUi::new();
    ui.identity = openagents_cli::coder::tui::Identity::Named {
        login: "user".to_string(),
        id: 1,
        namespaces: vec![],
        expires_at: "".to_string(),
    };

    let buffer = draw(&mut ui);
    let y = buffer.area.height - 1;
    assert_eq!(buffer.cell((0, y)).unwrap().fg, DIM_TEXT_COLOR);
}

/// Typing goes into the composer and the caret follows it.
#[test]
fn what_is_typed_is_drawn_in_the_input_box() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut ui = CoderUi::new();
    let width = openagents_cli::coder::interactive::composer_width(80);
    for ch in "explain rust".chars() {
        ui.composer
            .handle_key(&KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE), width);
    }
    let buffer = draw(&mut ui);
    assert!(text_of(&buffer).contains("explain rust"));

    // Ctrl+A is one of the chords `/help` claims. It moves the caret without
    // moving the text.
    ui.composer.handle_key(
        &KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
        width,
    );
    assert_eq!(ui.composer.cursor_rowcol(width), (0, 0));
    assert_eq!(ui.composer.text(), "explain rust");
}

/// What `c06badb472` fixed, against the composer that replaced the code it
/// fixed.
///
/// That commit rewrote `wrap_input` to wrap by character because the old one
/// went through `split_whitespace`, which threw trailing spaces away, and it
/// put a `REVERSED` block on the cursor cell so a line ending in a space did
/// not look like a line that ended earlier. The wrapping is now
/// `openagents_cli::composer`, which slices byte ranges out of the text and so
/// cannot lose a space; this holds it to that.
#[test]
fn trailing_spaces_are_kept_and_the_block_cursor_sits_after_them() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::style::Modifier;

    let mut ui = CoderUi::new();
    let width = openagents_cli::coder::interactive::composer_width(80);
    for ch in "ab   ".chars() {
        ui.composer
            .handle_key(&KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE), width);
    }
    assert_eq!(
        ui.composer.text(),
        "ab   ",
        "the trailing spaces were eaten"
    );
    // Five columns in, not two: the caret is past the spaces.
    assert_eq!(ui.composer.cursor_rowcol(width), (0, 5));

    let buffer = draw(&mut ui);
    let reversed: Vec<(u16, u16)> = (0..buffer.area.height)
        .flat_map(|y| (0..buffer.area.width).map(move |x| (x, y)))
        .filter(|(x, y)| {
            buffer
                .cell((*x, *y))
                .unwrap()
                .modifier
                .contains(Modifier::REVERSED)
        })
        .collect();
    assert_eq!(reversed.len(), 1, "expected exactly one block cursor");

    // One cell past `ab   `, inside the border and the `" > "` gutter.
    let (x, _) = reversed[0];
    assert_eq!(x, 1 + 3 + 5, "the block cursor is not after the spaces");

    // And it is still the palette: REVERSED swaps the two colours for that
    // cell, it does not introduce a third.
    let cell = buffer.cell(reversed[0]).unwrap();
    assert_eq!(cell.fg, Color::Rgb(255, 176, 0));
    assert_eq!(cell.bg, Color::Rgb(8, 6, 0));
}
