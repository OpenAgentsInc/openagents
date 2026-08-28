//! What the frame does with what the runtime tells it.
//!
//! `apply` is the whole of it, and it is public so this can drive it without a
//! terminal. The properties here are the ones issue #105 names: a status field
//! reports a value it received or stays empty, a failure does not become an
//! answer, and nothing the session prints leaves the palette.

use openagents_cli::coder::interactive::{apply, apply_drained};
use openagents_cli::coder::markdown::theme::{
    DIM_TEXT_COLOR, MODEL_TEXT_COLOR, TEXT_COLOR, USER_TEXT_COLOR,
};
use openagents_cli::coder::runtime::Control;
use openagents_cli::coder::tui::{CoderUi, Entry, MAX_SUBAGENT_LINES, Role, now_ms};
use openagents_cli::coder::turn::{TurnAction, TurnEffect, TurnState};
use openagents_cli::runtime::TurnUsage;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::{Color, Modifier};

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
fn device_sign_in_url_stays_visible_and_clickable() {
    let mut ui = CoderUi::new();
    let url = "https://openagents.com/device?user_code=ABCD-EFGH";
    apply(
        &mut ui,
        Control::Output(format!(
            "Opened the OpenAgents sign-in page in your browser.\n\nURL: {url}\nCode: ABCD-EFGH"
        )),
    );

    let buffer = draw(&mut ui);
    let link = ui
        .links
        .iter()
        .find(|link| link.url == url)
        .expect("device sign-in URL hyperlink");

    for x in link.x..link.x + link.width {
        let cell = buffer.cell((x, link.y)).unwrap();
        assert_eq!(cell.fg, TEXT_COLOR, "URL cell at column {x} is too dark");
        assert!(
            !cell.modifier.contains(Modifier::HIDDEN),
            "URL cell at column {x} is hidden"
        );
    }
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
    assert!(text.contains("New in v0.1.1"), "{text}");
    assert!(text.contains("Improved subagent delegation"), "{text}");
    assert!(text.contains("Added streaming to thinking"), "{text}");
    assert!(text.contains("Grok is a first-class delegate"), "{text}");
    assert!(text.contains("Timing on each message"), "{text}");
    assert!(
        text.contains("ATIF export keeps subagent streams"),
        "{text}"
    );

    let row_at = |y: u16| {
        (0..buffer.area.width)
            .map(|x| buffer.cell((x, y)).unwrap().symbol())
            .collect::<String>()
    };
    let box_span = |y: u16| {
        let mut first = None;
        let mut last = None;
        for x in 0..buffer.area.width {
            let symbol = buffer.cell((x, y)).unwrap().symbol();
            if !symbol.chars().all(|c| c.is_whitespace()) {
                if first.is_none() {
                    first = Some(x);
                }
                last = Some(x);
            }
        }
        let left = first.expect("box left edge");
        let right = last.expect("box right edge");
        (left, right)
    };
    let title_row = (0..buffer.area.height)
        .find(|y| row_at(*y).contains("Coder v"))
        .expect("startup box title");
    let news_row = (0..buffer.area.height)
        .find(|y| row_at(*y).contains("New in v0.1.1"))
        .expect("changelog box title");
    assert!(
        news_row > title_row,
        "the changelog box must sit under the startup box: Coder v on {title_row}, New in on {news_row}"
    );
    let facts_row = (0..buffer.area.height)
        .find(|y| row_at(*y).contains("Working directory"))
        .expect("working directory row");
    let facts = row_at(facts_row);
    assert!(
        facts.contains("│ Working directory") || facts.contains(" Working directory"),
        "the startup box needs one column of inner padding: {facts:?}"
    );

    let news_title = row_at(news_row);
    let (news_left, news_right) = box_span(news_row);
    let news_box_width = (news_right - news_left + 1) as usize;
    let facts_title = row_at(title_row);
    let (_facts_left, _facts_right) = box_span(title_row);
    let facts_box_width = (_facts_right - _facts_left + 1) as usize;
    assert!(
        news_box_width < facts_box_width,
        "changelog box should wrap its lines, not match the facts box: news={news_box_width} facts={facts_box_width}\n{news_title:?}\n{facts_title:?}"
    );

    let longest = "ATIF export keeps subagent streams";
    assert_eq!(
        news_box_width,
        longest.len() + 4,
        "changelog box should be the longest line plus borders and one pad column each side: {news_title:?}"
    );
    let left_margin = news_left as usize;
    let right_margin = (buffer.area.width - news_right - 1) as usize;
    assert!(
        left_margin.abs_diff(right_margin) <= 1,
        "changelog box should be centered: left={left_margin} right={right_margin} row={news_title:?}"
    );

    let content_row = (0..buffer.area.height)
        .find(|y| row_at(*y).contains(longest))
        .expect("longest changelog line");
    let content = row_at(content_row);
    let start = content.find(longest).expect("longest line");
    let after = &content[start + longest.len()..];
    assert!(
        after.starts_with(" │") || after.starts_with(" |"),
        "longest changelog line should have one pad column then the right border: {content:?}"
    );
    let remainder = after.chars().skip(2).collect::<String>();
    assert!(
        remainder.chars().all(|c| c.is_whitespace()),
        "changelog box should not keep extra inner columns after the longest line: {content:?}"
    );
}

#[test]
fn typing_a_slash_opens_an_amber_command_helper() {
    let mut ui = CoderUi::new();
    ui.composer.insert_str("/");

    let buffer = draw(&mut ui);
    let text = text_of(&buffer);
    assert!(text.contains("Commands · 13 matches"), "{text}");
    assert!(text.contains("/clear"), "{text}");
    assert!(text.contains("clear the transcript"), "{text}");

    let command_cell = buffer
        .content
        .iter()
        .find(|cell| cell.symbol() == "/")
        .expect("a command label");
    assert_eq!(command_cell.fg, TEXT_COLOR);
}

#[test]
fn the_command_helper_filters_and_does_not_open_for_paths() {
    let mut ui = CoderUi::new();
    ui.composer.insert_str("/go");
    let filtered = text_of(&draw(&mut ui));
    assert!(filtered.contains("/goal"), "{filtered}");
    assert!(
        !filtered.contains("/help"),
        "filtering /go must not list /help: {filtered}"
    );

    ui.composer.set_text("/Users/name/work/openagents");
    let path = text_of(&draw(&mut ui));
    assert!(!path.contains(" Commands "), "{path}");
}

#[test]
fn a_dropped_image_becomes_a_path_free_attachment_marker() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("screen.png");
    std::fs::write(&path, b"\x89PNG\r\n\x1a\nimage").unwrap();
    let mut ui = CoderUi::new();

    assert!(
        ui.attach_dropped_images(path.to_str().unwrap()).unwrap(),
        "the image paste remained plain text"
    );
    assert_eq!(ui.composer.text(), "[Image #1] ");
    assert_eq!(ui.images.len(), 1);
    assert!(!ui.composer.text().contains(path.to_str().unwrap()));

    let prompt = ui.composer.text().to_string();
    let images = ui.take_referenced_images(&prompt);
    assert_eq!(images.len(), 1);
    assert!(images[0].data_url.starts_with("data:image/png;base64,"));
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
    assert_eq!(
        model_y, 0,
        "the model label is not on the message's top row"
    );
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
            duration_ms: 0,
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
fn a_shell_tool_uses_a_prompt_marker_on_screen() {
    let mut ui = CoderUi::new();
    apply(
        &mut ui,
        Control::Tool {
            call_id: "c1".to_string(),
            name: "shell".to_string(),
            arguments: r#"{"command":"cargo test"}"#.to_string(),
        },
    );

    let screen = text_of(&draw(&mut ui));
    assert!(
        screen.starts_with(&format!("{}○ > cargo test", " ".repeat(80))),
        "{screen}"
    );
    assert!(!screen.contains("shell cargo test"), "{screen}");
    assert_eq!(ui.entries[0].tool.as_ref().unwrap().function_name, "shell");
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
    let first_rail = first.cell((0, 2)).unwrap();
    let second_rail = second.cell((0, 2)).unwrap();
    assert_eq!(first_rail.symbol(), "│");
    assert_eq!(
        first.cell((2, 2)).unwrap().fg,
        DIM_TEXT_COLOR,
        "tool output text is not 50% amber"
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
            duration_ms: 0,
        },
    );
    let settled = draw(&mut ui);
    let settled_again = draw(&mut ui);
    assert_ne!(
        settled.cell((0, 2)).unwrap().fg,
        settled_again.cell((0, 2)).unwrap().fg,
        "the completion settle animation did not advance"
    );
    for _ in 0..10 {
        let _ = draw(&mut ui);
    }
    let resting = draw(&mut ui);
    assert_eq!(resting.cell((0, 2)).unwrap().fg, DIM_TEXT_COLOR);
    assert!(ui.entries[0].tool.as_ref().unwrap().done);
}

#[test]
fn a_tool_that_finishes_in_one_drain_still_paints_an_active_rail() {
    let mut ui = CoderUi::new();
    let mut pending = Vec::new();
    apply_drained(
        &mut ui,
        &mut pending,
        [
            Control::Tool {
                call_id: "c1".to_string(),
                name: "read".to_string(),
                arguments: r#"{"path":"a.rs"}"#.to_string(),
            },
            Control::ToolOutput {
                call_id: "c1".to_string(),
                chunk: "fn main() {}\n".to_string(),
            },
            Control::ToolDone {
                call_id: "c1".to_string(),
                is_error: false,
                duration_ms: 4,
            },
        ],
    );
    assert!(
        !ui.entries[0].tool.as_ref().unwrap().done,
        "ToolDone must not settle the box in the same drain as the start"
    );

    let first = draw(&mut ui);
    let second = draw(&mut ui);
    let first_rail = first.cell((0, 2)).unwrap();
    assert_eq!(first_rail.symbol(), "│");
    assert_ne!(
        first_rail.fg,
        second.cell((0, 2)).unwrap().fg,
        "the active rail did not move"
    );

    apply_drained(&mut ui, &mut pending, []);
    assert!(ui.entries[0].tool.as_ref().unwrap().done);
    let settled = draw(&mut ui);
    let settled_again = draw(&mut ui);
    assert_ne!(
        settled.cell((0, 2)).unwrap().fg,
        settled_again.cell((0, 2)).unwrap().fg,
        "the completion settle animation did not advance"
    );
}

#[test]
fn an_in_flight_tool_with_no_output_still_has_a_moving_rail() {
    let mut ui = CoderUi::new();
    apply(
        &mut ui,
        Control::Tool {
            call_id: "c1".to_string(),
            name: "read".to_string(),
            arguments: r#"{"path":"a.rs"}"#.to_string(),
        },
    );

    let first = draw(&mut ui);
    let second = draw(&mut ui);
    let first_rail = first.cell((0, 2)).unwrap();
    assert_eq!(
        first_rail.symbol(),
        "│",
        "an in-flight tool with empty output has no rail: {}",
        text_of(&first)
    );
    assert_ne!(
        first_rail.fg,
        second.cell((0, 2)).unwrap().fg,
        "the empty-output rail did not move"
    );
    assert!(!ui.entries[0].tool.as_ref().unwrap().done);
}

#[test]
fn markdown_tool_output_uses_the_shared_renderer() {
    let mut ui = CoderUi::new();
    apply(
        &mut ui,
        Control::Tool {
            call_id: "c1".to_string(),
            name: "read".to_string(),
            arguments: r#"{"path":"README.md"}"#.to_string(),
        },
    );
    apply(
        &mut ui,
        Control::ToolOutput {
            call_id: "c1".to_string(),
            chunk: "## Result\n\n**passed**".to_string(),
        },
    );

    let frame = draw(&mut ui);
    let screen = text_of(&frame);
    assert!(screen.contains("Result"), "{screen}");
    assert!(screen.contains("passed"), "{screen}");
    assert!(!screen.contains("##"), "{screen}");
    assert!(!screen.contains("**"), "{screen}");
    let passed = frame
        .content
        .iter()
        .find(|cell| cell.symbol() == "p")
        .expect("rendered markdown body");
    assert!(passed.modifier.contains(ratatui::style::Modifier::BOLD));
    assert_eq!(passed.fg, DIM_TEXT_COLOR);
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
    assert_eq!(first.cell((0, 2)).unwrap().fg, TEXT_COLOR);
    assert_eq!(
        first.cell((0, 2)).unwrap().fg,
        second.cell((0, 2)).unwrap().fg
    );
    assert!(text_of(&second).starts_with(&format!("{}○ > cargo test", " ".repeat(80))));
}

/// The loading row names what is happening and times it: `⠹ working (9s)`
/// while the turn runs, with the timer at the 50% dim amber so it can be
/// found on a glance. A specific waiting message outranks the generic word.
#[test]
fn the_loading_row_says_working_and_times_it_at_50_percent_amber() {
    let mut ui = CoderUi::new();
    ui.loading = true;
    ui.turn_started();
    ui.turn_started_at = Some(now_ms() - 9_000);

    let buffer = draw(&mut ui);
    let row = (0..buffer.area.width)
        .map(|x| buffer.cell((x, 0)).unwrap().symbol())
        .collect::<String>();
    assert!(
        row.contains("working (9s)"),
        "the loading row does not name the work and time it: {row:?}"
    );
    let timer_x = row.find("(9s)").expect("the timer is on the loading row") as u16;
    assert_eq!(
        buffer.cell((timer_x, 0)).unwrap().fg,
        DIM_TEXT_COLOR,
        "the timer is not at 50% amber"
    );

    // A waiting message is more specific than "working" and outranks it. The
    // frame's `waiting` field is set by the runtime, so set it the same way
    // the field's writer does — directly, as `Control::Waiting` would.
    ui.waiting = Some("Waiting for first token".to_string());
    let buffer = draw(&mut ui);
    let row = (0..buffer.area.width)
        .map(|x| buffer.cell((x, 0)).unwrap().symbol())
        .collect::<String>();
    assert!(
        row.contains("Waiting for first token (9s)") && !row.contains("working"),
        "the waiting message did not outrank the generic word: {row:?}"
    );
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
    assert!(first_frame.contains("line 04"), "{first_frame}");
    assert!(!first_frame.contains("line 05"), "{first_frame}");
    assert!(!first_frame.contains("line 20"), "{first_frame}");

    for _ in 0..8 {
        draw(&mut ui);
    }
    let slower_frame = text_of(&draw(&mut ui));
    assert!(!slower_frame.contains("line 20"), "{slower_frame}");
    assert!(
        slower_frame.contains("line 14") || slower_frame.contains("line 15"),
        "the preview reached its final five rows too quickly: {slower_frame}"
    );

    for _ in 0..16 {
        draw(&mut ui);
    }
    let settled_frame = text_of(&draw(&mut ui));
    assert!(!settled_frame.contains("line 01"), "{settled_frame}");
    assert!(settled_frame.contains("line 17"), "{settled_frame}");
    assert!(settled_frame.contains("line 20"), "{settled_frame}");
}

#[test]
fn cli_help_with_inline_code_stays_literal() {
    let mut ui = CoderUi::new();
    apply(
        &mut ui,
        Control::Tool {
            call_id: "help".to_string(),
            name: "shell".to_string(),
            arguments: r#"{"command":"openagents issue --help"}"#.to_string(),
        },
    );
    apply(
        &mut ui,
        Control::ToolOutput {
            call_id: "help".to_string(),
            chunk: [
                "Options:",
                "  --no-color          Disable ANSI output",
                "  --api-url <API_URL> API origin to talk to, such as https://openagents.com",
                "  --profile <PROFILE> Named API endpoint: `production`, staging, or local",
            ]
            .join("\n"),
        },
    );

    let frame = draw(&mut ui);
    let screen = text_of(&frame);
    assert!(
        screen.contains("such as https://openagents.com"),
        "{screen}"
    );
    assert!(screen.contains("`production`"), "{screen}");
    for cell in frame.content.iter().filter(|cell| cell.symbol() != " ") {
        assert_eq!(cell.bg, Color::Rgb(8, 6, 0));
    }
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
    assert!(frame.contains("line 17"), "{frame}");
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

#[test]
fn subagent_output_for_an_unknown_call_lands_on_nothing() {
    let mut ui = CoderUi::new();
    ui.entries.push(Entry::new(Role::Assistant, "an answer"));
    apply(
        &mut ui,
        Control::SubagentOutput {
            call_id: "never-started".to_string(),
            line: "· stray".to_string(),
        },
    );
    assert!(
        ui.entries
            .iter()
            .all(|entry| entry.subagent_lines.is_empty()),
        "stray subagent output was attached to an unrelated entry"
    );
}

#[test]
fn subagent_output_for_a_cancelled_turn_is_dropped() {
    let mut turns = TurnState::default();
    let TurnEffect::Started(id) = turns.apply(TurnAction::Start) else {
        panic!("the turn did not start");
    };
    turns.apply(TurnAction::RequestCancel);
    assert!(!turns.accepts(id));

    let mut ui = CoderUi::new();
    apply(
        &mut ui,
        Control::Tool {
            call_id: "d1".to_string(),
            name: "delegate".to_string(),
            arguments: r#"{"prompt":"read it"}"#.to_string(),
        },
    );
    let control = Control::Turn {
        id,
        event: Box::new(Control::SubagentOutput {
            call_id: "d1".to_string(),
            line: "· should not land".to_string(),
        }),
    };
    if let Control::Turn { id, event } = control {
        if turns.accepts(id) {
            apply(&mut ui, *event);
        }
    }
    assert!(
        ui.entries[0].subagent_lines.is_empty(),
        "a fenced turn still painted into the box"
    );
}

#[test]
fn the_delegate_box_clips_subagent_lines_to_the_last_n() {
    let mut ui = CoderUi::new();
    apply(
        &mut ui,
        Control::Tool {
            call_id: "d1".to_string(),
            name: "delegate".to_string(),
            arguments: r#"{"prompt":"read it"}"#.to_string(),
        },
    );
    let extra = 2;
    let total = MAX_SUBAGENT_LINES + extra;
    for i in 1..=total {
        apply(
            &mut ui,
            Control::SubagentOutput {
                call_id: "d1".to_string(),
                line: format!("· line {i}"),
            },
        );
    }

    assert_eq!(ui.entries.len(), 1);
    assert_eq!(ui.entries[0].subagent_lines.len(), total);

    let screen = text_of(&draw(&mut ui));
    assert!(
        screen.contains(&format!("+{extra} earlier")),
        "missing clip counter: {screen}"
    );
    assert!(
        !screen.contains("line 1"),
        "clipped first line still visible: {screen}"
    );
    assert!(
        screen.contains(&format!("line {}", extra + 1)),
        "oldest kept line missing: {screen}"
    );
    assert!(
        screen.contains(&format!("line {total}")),
        "newest line missing: {screen}"
    );
}

#[test]
fn a_nested_child_tool_stays_inside_the_delegate_box() {
    let mut ui = CoderUi::new();
    apply(
        &mut ui,
        Control::Tool {
            call_id: "d1".to_string(),
            name: "delegate".to_string(),
            arguments: r#"{"prompt":"read Cargo.toml","description":"Read Cargo"}"#.to_string(),
        },
    );
    apply(
        &mut ui,
        Control::Tool {
            call_id: "d1".to_string(),
            name: "read".to_string(),
            arguments: r#"{"path":"Cargo.toml"}"#.to_string(),
        },
    );

    assert_eq!(ui.entries.len(), 1, "child tool opened a sibling box");
    assert_eq!(
        ui.entries[0].tool.as_ref().unwrap().function_name,
        "delegate"
    );
    assert!(
        ui.entries[0]
            .subagent_lines
            .iter()
            .any(|line| line.contains("read Cargo.toml")),
        "{:?}",
        ui.entries[0].subagent_lines
    );
    let screen = text_of(&draw(&mut ui));
    assert!(screen.contains("Read Cargo"), "{screen}");
    assert!(screen.contains("read Cargo.toml"), "{screen}");
}
