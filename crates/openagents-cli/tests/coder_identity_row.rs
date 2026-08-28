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
    assert!(row.ends_with("Coder Flash"), "{row}");
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

/// Credit begins at the left edge and the lane/model ends at the right edge.
#[test]
fn the_footer_aligns_credit_left_and_the_effective_model_right() {
    let mut ui = CoderUi::new();
    ui.credit = priced();
    ui.lane = "Coder Flash".to_string();
    apply(&mut ui, Control::Model("gemini-3.7-flash".to_string()));

    let row = status_row(&mut ui);
    assert_eq!(row.find("$18.40 left"), Some(0), "{row}");
    assert!(
        row.ends_with("Coder Flash · gemini-3.7-flash"),
        "the model should end at the footer edge: {row}"
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

// ───────────────────────────────────────────────────────────── the lane field

// Issue #131: the lane used to be announced once at session open, at the top
// of the screen, and then scrolled away. It is a live fact — it changes what
// the next turn costs and which model answers it — so it belongs in the row
// that says what you can do next.
//
// Every assertion below reads the rendered frame. The rule the row carries is
// that nothing in it may report a value it did not receive, and for a lane
// that means the *effective* model: a label still reading "Coder Flash" while
// a fallback answers is the defect this field exists to prevent.

/// The two lanes read differently from each other on the frame.
///
/// Asserting that the field "renders something" would pass against a row that
/// renders the wrong lane, so both frames are captured and compared.
#[test]
fn the_two_lanes_render_differently_from_each_other() {
    let mut flash = CoderUi::new();
    flash.identity = signed_in();
    flash.lane = "Coder Flash".to_string();

    let mut free = CoderUi::new();
    free.identity = signed_in();
    free.lane = "Coder Free".to_string();

    let on_flash = status_row(&mut flash);
    let on_free = status_row(&mut free);

    assert!(on_flash.contains("Coder Flash"), "{on_flash}");
    assert!(on_free.contains("Coder Free"), "{on_free}");
    assert!(
        !on_flash.contains("Coder Free"),
        "the Flash frame names the other lane: {on_flash}"
    );
    assert!(
        !on_free.contains("Coder Flash"),
        "the Free frame names the other lane: {on_free}"
    );
    assert_ne!(
        on_flash, on_free,
        "the two lanes render the same row, so the field reports nothing"
    );
}

/// A fallback reads differently from the lane running its own model.
///
/// This is the assertion the whole field exists for. The lane is Flash in both
/// frames; what differs is the model that answered. A row that named only the
/// lane would render these two identically and tell the reader nothing about
/// which model is spending their money.
#[test]
fn a_fallback_reads_differently_from_the_lane_on_its_own_model() {
    let mut primary = CoderUi::new();
    primary.identity = signed_in();
    primary.lane = "Coder Flash".to_string();
    apply(&mut primary, Control::Model("glm-5.3-flash".to_string()));

    let mut fell_back = CoderUi::new();
    fell_back.identity = signed_in();
    fell_back.lane = "Coder Flash".to_string();
    apply(
        &mut fell_back,
        Control::Model("gemini-3.7-flash".to_string()),
    );

    let on_primary = status_row(&mut primary);
    let on_fallback = status_row(&mut fell_back);

    assert_ne!(
        on_primary, on_fallback,
        "the row reads the same whether the lane's own model answered or a \
         fallback did, which is the defect this field exists to prevent"
    );
    assert!(
        on_primary.contains("glm-5.3-flash"),
        "the row does not name the model that answered: {on_primary}"
    );
    assert!(
        on_fallback.contains("gemini-3.7-flash"),
        "the row does not name the fallback that answered: {on_fallback}"
    );
    assert!(
        !on_fallback.contains("glm-5.3-flash"),
        "the row names the model the lane asked for rather than the one that \
         answered: {on_fallback}"
    );
}

/// Before anything answers, the row claims no model at all.
#[test]
fn the_row_names_no_model_until_one_has_answered() {
    let mut ui = CoderUi::new();
    ui.identity = signed_in();
    ui.lane = "Coder Flash".to_string();

    let row = status_row(&mut ui);
    assert!(row.contains("Coder Flash"), "{row}");
    assert!(
        !row.contains("·  ") && !row.trim_end().ends_with('·'),
        "the row left a separator with nothing after it: {row}"
    );
    // No id is invented for a lane nothing has answered on yet.
    for id in ["glm-5.3-flash", "gemini-3.7-flash", "ox-alpha"] {
        assert!(
            !row.contains(id),
            "the row named '{id}' before any model answered: {row}"
        );
    }
}

/// A fresh frame does not open on `ox-alpha`.
#[test]
fn a_fresh_session_does_not_open_on_ox_alpha() {
    let mut ui = CoderUi::new();
    ui.identity = signed_in();
    ui.lane = openagents_cli::runtime::Lane::default().label();

    let row = status_row(&mut ui);
    assert!(
        !row.contains("ox-alpha"),
        "a fresh session opened on ox-alpha: {row}"
    );
    assert!(
        !row.contains("Coder Auto"),
        "the retired Auto lane is still on the row: {row}"
    );
    assert!(row.contains("Coder Flash"), "{row}");
}

/// Shift+tab changes the lane, and the row changes with it.
///
/// The cycle is asserted through `Lane::cycle`, which is what the key handler
/// calls, and the frame is redrawn from what it produced — so a cycle that
/// moved the lane without the row following would fail here.
#[test]
fn cycling_the_lane_changes_both_the_lane_and_the_row() {
    use openagents_cli::runtime::Lane;

    let mut ui = CoderUi::new();
    ui.identity = signed_in();

    let first = Lane::default();
    ui.lane = first.label();
    let before = status_row(&mut ui);

    let second = first.cycle();
    assert_ne!(second, first, "shift+tab did not move the lane");
    ui.lane = second.label();
    // A new lane has answered nothing yet, which is what the key handler does.
    ui.model.clear();
    let after = status_row(&mut ui);

    assert_ne!(
        before, after,
        "the lane changed but the row under the input bar did not"
    );
    assert!(before.contains("Coder Flash"), "{before}");
    assert!(after.contains("Coder Free"), "{after}");

    // And it closes, back to where it started. Three table members now
    // (#291, #292), so the full walk is flash, free, local (resolved to the
    // probed tag), flash. This frame has no probe, so the walk carries the
    // tag a probe would have found.
    let tag = Some("qwen3.8:27b-mtp-q8_0".to_string());
    let back = second.cycle_gated(tag.clone());
    assert_eq!(
        back,
        Lane::Local("qwen3.8:27b-mtp-q8_0".to_string()),
        "the walk reaches the local lane, resolved to the probed tag"
    );
    assert_eq!(
        back.cycle_gated(tag),
        first,
        "the cycle does not return to the first lane"
    );
}

/// The active lane/model occupies the final footer columns without a gutter.
#[test]
fn the_lane_is_flush_against_the_footer_right_edge() {
    let mut ui = CoderUi::new();
    ui.credit = priced();
    ui.lane = "Coder Flash".to_string();
    apply(&mut ui, Control::Model("gemini-3.7-flash".to_string()));

    let buffer = draw(&mut ui, 120, 24);
    let y = buffer.area.height - 1;
    let row = row(&buffer, y);
    assert_eq!(row.find("$18.40 left"), Some(0), "{row}");
    assert!(row.ends_with("Coder Flash · gemini-3.7-flash"), "{row}");
}

/// A narrow row drops the lane name before the model, and never truncates.
///
/// Three fields and a fixed width do not always fit. What must not happen is
/// a partial catalog id, or a bare `Coder Flash` standing there while a
/// fallback answers — the second is the forbidden state written out. The
/// field gives up the lane name first, then renders nothing at all.
#[test]
fn a_narrow_row_gives_up_the_lane_name_before_the_model() {
    let mut ui = CoderUi::new();
    ui.credit = priced();
    ui.lane = "Coder Flash".to_string();
    apply(&mut ui, Control::Model("gemini-3.7-flash".to_string()));

    // Wide: both halves.
    let wide = row(&draw(&mut ui, 100, 24), 23);
    assert!(wide.contains("Coder Flash · gemini-3.7-flash"), "{wide}");

    // Narrower: the model survives, the lane name goes.
    let narrow = row(&draw(&mut ui, 30, 24), 23);
    assert!(
        narrow.contains("gemini-3.7-flash"),
        "the model that answered was dropped before the lane name: {narrow}"
    );
    assert!(
        !narrow.contains("Coder Flash"),
        "both halves were kept on a row too narrow for them: {narrow}"
    );

    // At every width, no model id is cut in half.
    for width in [30u16, 70, 74, 80, 90, 100, 120] {
        let row_at = row(&draw(&mut ui, width, 24), 23);
        let cut = ["gemini-3.7-flas", "gemini-3.7-", "emini-3.7-flash"]
            .iter()
            .any(|piece| row_at.contains(piece) && !row_at.contains("gemini-3.7-flash"));
        assert!(
            !cut,
            "a model id was cut in half at {width} columns: {row_at}"
        );
        // And a bare lane name never stands beside a model that answered.
        if row_at.contains("Coder Flash") {
            assert!(
                row_at.contains("gemini-3.7-flash"),
                "the row named the lane while a fallback answered, without \
                 naming the fallback, at {width} columns: {row_at}"
            );
        }
    }
}
