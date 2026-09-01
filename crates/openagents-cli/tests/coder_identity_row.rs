//! What the row under the composer says, and what `/info` says instead.
//!
//! Issue #130: the most valuable row on the screen was carrying a token count
//! that changes every turn and is read once in a while, in place of the one
//! fact a person wants standing there — who they are signed in as.
//!
//! Everything here reads the rendered frame rather than the struct behind it.
//! A field can hold the right value and still be drawn nowhere, and a row that
//! reports a stale account is a row that passes every assertion made against
//! the data structure.

use openagents_cli::coder::commands;
use openagents_cli::coder::interactive::apply;
use openagents_cli::coder::runtime::Control;
use openagents_cli::coder::tui::{CoderUi, Identity};
use openagents_cli::runtime::TurnUsage;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::sync::mpsc;

fn draw(ui: &mut CoderUi, width: u16, height: u16) -> ratatui::buffer::Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
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

/// One row of the frame, as the reader sees it.
fn row(buffer: &ratatui::buffer::Buffer, y: u16) -> String {
    (0..buffer.area.width)
        .map(|x| buffer.cell((x, y)).unwrap().symbol())
        .collect::<String>()
        .trim_end()
        .to_string()
}

/// The row under the composer: the last one on the frame.
fn status_row(ui: &mut CoderUi) -> String {
    let buffer = draw(ui, 80, 24);
    row(&buffer, buffer.area.height - 1)
}

fn signed_in() -> Identity {
    Identity::Named {
        login: "AtlantisPleb".to_string(),
        id: 14167547,
        namespaces: vec!["AtlantisPleb".to_string(), "OpenAgentsInc".to_string()],
        expires_at: "2026-09-25T00:00:00Z".to_string(),
    }
}

/// Run one `/` line against a frame and hand back what it drew.
fn command(ui: &mut CoderUi, line: &str) -> String {
    let (tx, _rx) = mpsc::channel::<Control>();
    let cwd = std::env::current_dir().unwrap();
    commands::run(ui, line, &tx, &cwd);
    // Tall enough that a whole `/info` report is on screen at once; the
    // transcript follows the bottom, so a short frame would clip the top of it.
    text_of(&draw(ui, 100, 40))
}

// ───────────────────────────────────────────────────────────── footer fields

/// The footer leaves account and endpoint details to `/info`.
#[test]
fn the_footer_omits_account_and_endpoint_details() {
    let mut ui = CoderUi::new();
    ui.identity = signed_in();
    ui.endpoint = "https://openagents.com/api/v1".to_string();
    ui.credit = priced();
    ui.lane = "Coder Flash".to_string();

    let row = status_row(&mut ui);
    assert!(!row.contains("AtlantisPleb"), "{row}");
    assert!(!row.contains("openagents.com"), "{row}");
    assert!(row.starts_with("$18.40 left"), "{row}");
    // One Coder, chosen nowhere: the row names no lane or model.
    assert!(!row.contains("Coder Flash"), "{row}");
    assert!(
        row.ends_with(&format!("v{}", openagents_cli::VERSION)),
        "{row}"
    );
}

/// The row is about what you can do next. A token count is not that.
#[test]
fn the_row_carries_no_token_count() {
    let mut ui = CoderUi::new();
    ui.identity = signed_in();
    ui.endpoint = "https://openagents.com/api/v1".to_string();
    apply(
        &mut ui,
        Control::Usage(TurnUsage {
            prompt_tokens: 4321,
            completion_tokens: 1234,
            total_tokens: 5555,
        }),
    );

    let row = status_row(&mut ui);
    assert!(!row.contains("tokens"), "{row}");
    for figure in ["4321", "1234", "5555"] {
        assert!(!row.contains(figure), "the row still counts tokens: {row}");
    }
}

// ───────────────────────────────────────────────────────────────────── /info

/// The two figures that can disagree, both named, both readable.
#[test]
fn info_reports_the_server_s_billed_figure_and_the_client_count_as_two_numbers() {
    let mut ui = CoderUi::new();
    apply(
        &mut ui,
        Control::Usage(TurnUsage {
            prompt_tokens: 900,
            completion_tokens: 334,
            total_tokens: 1234,
        }),
    );
    apply(&mut ui, Control::Billed(1500));

    let text = command(&mut ui, "/info");
    assert!(text.contains("Billed by the server"), "{text}");
    assert!(
        text.contains("1500"),
        "the server's figure is missing: {text}"
    );
    assert!(text.contains("1234"), "the client count is missing: {text}");
    // And the gap is named rather than left for the reader to subtract.
    assert!(
        text.contains("266"),
        "the difference is not reported: {text}"
    );
}

/// A figure nobody sent is not printed as a figure.
///
/// `0` here would read as "the server billed nothing", which is a measurement,
/// and no measurement was taken.
#[test]
fn info_prints_no_billed_figure_it_did_not_receive() {
    let mut ui = CoderUi::new();
    apply(
        &mut ui,
        Control::Usage(TurnUsage {
            prompt_tokens: 900,
            completion_tokens: 334,
            total_tokens: 1234,
        }),
    );

    let text = command(&mut ui, "/info");
    assert!(text.contains("not reported yet"), "{text}");
    assert!(
        !text.contains("Billed by the server — 0"),
        "a zero was reported as the billed figure: {text}"
    );
}

/// Last turn and the session total are two facts, and `/info` keeps them two.
#[test]
fn info_separates_the_last_turn_from_the_session_total() {
    let mut ui = CoderUi::new();
    apply(
        &mut ui,
        Control::Usage(TurnUsage {
            prompt_tokens: 100,
            completion_tokens: 20,
            total_tokens: 120,
        }),
    );
    apply(
        &mut ui,
        Control::Usage(TurnUsage {
            prompt_tokens: 300,
            completion_tokens: 40,
            total_tokens: 340,
        }),
    );

    let text = command(&mut ui, "/info");
    assert!(text.contains("Last turn"), "{text}");
    assert!(
        text.contains("340"),
        "the last turn is not reported: {text}"
    );
    assert!(text.contains("This session counted"), "{text}");
    assert!(
        text.contains("460"),
        "the session total is not reported: {text}"
    );
}

/// The model, the lane, and the thread, each as the session was given it.
#[test]
fn info_names_the_model_that_answered_and_the_thread_the_server_opened() {
    let mut ui = CoderUi::new();
    ui.lane = "Coder Auto".to_string();
    apply(&mut ui, Control::Model("glm-5.3-flash".to_string()));
    apply(&mut ui, Control::Thread("thr_01J9ABCD".to_string()));

    let text = command(&mut ui, "/info");
    assert!(text.contains("glm-5.3-flash"), "{text}");
    assert!(text.contains("Coder Auto"), "{text}");
    assert!(text.contains("thr_01J9ABCD"), "{text}");
}

/// Nothing has answered, so nothing is named.
#[test]
fn info_names_no_model_before_one_has_answered() {
    let mut ui = CoderUi::new();
    ui.lane = "Coder Auto".to_string();

    let text = command(&mut ui, "/info");
    assert!(text.contains("no model has answered yet"), "{text}");
    assert!(text.contains("none open"), "a thread was claimed: {text}");
    assert!(
        text.contains("nothing reported"),
        "a zero was reported as usage: {text}"
    );
}

/// `/info` carries the identity too, in the same words the row uses.
#[test]
fn info_reports_the_same_identity_the_row_does() {
    let mut ui = CoderUi::new();
    ui.identity = Identity::Unverified;
    ui.endpoint = "https://openagents.com/api/v1".to_string();

    let text = command(&mut ui, "/info");
    assert!(text.contains("unverified"), "{text}");
    assert!(!text.contains("AtlantisPleb"), "{text}");
}

// ───────────────────────────────────────────────────────────── credit footer

fn priced() -> openagents_cli::coder::credit::CreditField {
    openagents_cli::coder::credit::CreditField::Known(openagents_cli::coder::credit::Credit {
        allowance_microusd: 20_000_000,
        spent_microusd: 1_600_000,
        remaining_microusd: 18_400_000,
        unpriced_calls: 0,
        complete: true,
    })
}

fn unpriced(calls: u64) -> openagents_cli::coder::credit::CreditField {
    openagents_cli::coder::credit::CreditField::Known(openagents_cli::coder::credit::Credit {
        allowance_microusd: 20_000_000,
        spent_microusd: 0,
        remaining_microusd: 20_000_000,
        unpriced_calls: calls,
        complete: false,
    })
}

/// Credit begins at the left edge and only the build version ends at the
/// right edge. There is one Coder and no model or lane is chosen, so the row
/// never names one — even after a turn recorded which model answered.
#[test]
fn the_footer_aligns_credit_left_and_only_the_version_right() {
    let mut ui = CoderUi::new();
    ui.credit = priced();
    ui.lane = "Coder Flash".to_string();
    apply(&mut ui, Control::Model("gemini-3.7-flash".to_string()));

    let row = status_row(&mut ui);
    assert_eq!(row.find("$18.40 left"), Some(0), "{row}");
    assert!(
        row.trim_end()
            .ends_with(&format!("v{}", openagents_cli::VERSION)),
        "only the build version should end at the footer edge: {row}"
    );
    assert!(!row.contains("Coder Flash"), "the row named a lane: {row}");
    assert!(
        !row.contains("gemini-3.7-flash"),
        "the row named a model: {row}"
    );
}

/// An incomplete server price does not add an `unpriced calls` disclosure.
#[test]
fn an_unpriced_balance_shows_only_the_credit_figure() {
    for calls in [3, 12, 999] {
        let mut ui = CoderUi::new();
        ui.credit = unpriced(calls);

        let row = status_row(&mut ui);
        let expected = "$20.00 left";
        assert!(
            row.contains(&expected),
            "the row should carry {expected:?} untruncated, and held {row:?}"
        );
        assert!(row.starts_with(expected), "{row}");
        assert!(
            !row.contains("unpriced"),
            "the status row must not show unpriced calls: {row:?}"
        );
    }
}

// ─────────────────────────────────────────────────────── one Coder, no lane

// The product has one Coder, spoken to through the Coder Responses API. No
// model or lane is chosen anywhere, so the status row never names one: the
// row is spend on the left and working state on the right, with the build
// version as the only chrome. These assertions read the rendered frame.

/// Even after a turn recorded which model answered, the status row never
/// names a model or a lane, at any width.
#[test]
fn the_row_never_names_a_model_or_lane_at_any_width() {
    let mut ui = CoderUi::new();
    ui.credit = priced();
    ui.identity = signed_in();
    // Whatever the runtime recorded internally, the row must not surface it.
    ui.lane = "Coder Flash".to_string();
    apply(&mut ui, Control::Model("gemini-3.7-flash".to_string()));

    for width in [30u16, 60, 74, 80, 100, 120] {
        let row = row(&draw(&mut ui, width, 24), 23);
        for forbidden in [
            "Coder Flash",
            "Coder Pro",
            "Coder Free",
            "Coder Local",
            "gemini-3.7-flash",
            "glm-5.3-flash",
        ] {
            assert!(
                !row.contains(forbidden),
                "the row named '{forbidden}' at {width} columns: {row}"
            );
        }
    }
}

/// The right edge of the status row carries only the build version.
#[test]
fn the_row_ends_with_the_build_version() {
    let mut ui = CoderUi::new();
    ui.credit = priced();
    ui.lane = "Coder Flash".to_string();

    let buffer = draw(&mut ui, 120, 24);
    let row = row(&buffer, buffer.area.height - 1);
    assert_eq!(row.find("$18.40 left"), Some(0), "{row}");
    assert!(
        row.ends_with(&format!("v{}", openagents_cli::VERSION)),
        "the row should end with the build version and nothing else: {row}"
    );
}
