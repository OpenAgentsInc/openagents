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

use coder_lite::commands;
use coder_lite::interactive::apply;
use coder_lite::runtime::Control;
use coder_lite::tui::{CoderUi, Identity};
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

// ─────────────────────────────────────────────────────────── the identity row

/// The account, where the token count used to be.
#[test]
fn the_row_names_the_account_the_server_confirmed() {
    let mut ui = CoderUi::new();
    ui.identity = signed_in();
    ui.endpoint = "https://openagents.com/api/v1".to_string();

    let row = status_row(&mut ui);
    assert!(row.contains("AtlantisPleb"), "{row}");
    assert!(row.contains("openagents.com"), "{row}");
}

/// No credential is said out loud, not left blank.
#[test]
fn the_row_says_not_signed_in_when_there_is_no_credential() {
    let mut ui = CoderUi::new();
    ui.endpoint = "https://openagents.com/api/v1".to_string();

    let row = status_row(&mut ui);
    assert!(row.contains("not signed in"), "{row}");
    assert!(row.contains("openagents.com"), "{row}");
}

/// The one that bites: a token in the keychain that nothing confirmed.
///
/// A stored credential is a claim, not a fact. Drawn as an account it makes a
/// dead session look live, and the reader finds out three failed turns later.
#[test]
fn an_unverified_credential_is_never_drawn_as_an_account() {
    let mut ui = CoderUi::new();
    ui.identity = Identity::Unverified;
    ui.endpoint = "https://openagents.com/api/v1".to_string();

    let row = status_row(&mut ui);
    assert!(row.contains("unverified"), "{row}");
    // Not any account name, and not the wording that means "there is no
    // credential" either — this state is neither of those.
    assert!(!row.contains("AtlantisPleb"), "{row}");
    assert!(!row.contains("not signed in"), "{row}");
}

/// The three states have to be told apart on screen, not just in the enum.
///
/// A test that only asserts "the row shows the account" passes against a row
/// that shows a stale one. This is the assertion that does not.
#[test]
fn the_three_identity_states_read_differently() {
    let render = |identity: Identity| {
        let mut ui = CoderUi::new();
        ui.identity = identity;
        ui.endpoint = "https://openagents.com/api/v1".to_string();
        status_row(&mut ui)
    };

    let named = render(signed_in());
    let unverified = render(Identity::Unverified);
    let anonymous = render(Identity::Anonymous);

    assert_ne!(named, unverified, "a dead credential reads as a live account");
    assert_ne!(named, anonymous);
    assert_ne!(
        unverified, anonymous,
        "a refused credential reads as no credential at all"
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

/// The row is shared. The identity takes the left and stops.
///
/// The credit balance draws on the right of this same row, and a full-width
/// paragraph would paint over it.
#[test]
fn the_identity_leaves_the_right_of_the_row_clear() {
    let mut ui = CoderUi::new();
    ui.identity = signed_in();
    // Long enough to run the whole width if nothing stopped it.
    ui.endpoint = "https://a-very-long-deployment-name-that-runs-on-and-on.openagents.com/api/v1"
        .to_string();

    let buffer = draw(&mut ui, 80, 24);
    let y = buffer.area.height - 1;
    let written = (0..buffer.area.width)
        .filter(|x| buffer.cell((*x, y)).unwrap().symbol().trim() != "")
        .count() as u16;
    assert!(
        written <= 80 - 24,
        "the identity claimed {written} of 80 columns, leaving nothing for the balance"
    );
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
    assert!(text.contains("1500"), "the server's figure is missing: {text}");
    assert!(text.contains("1234"), "the client count is missing: {text}");
    // And the gap is named rather than left for the reader to subtract.
    assert!(text.contains("266"), "the difference is not reported: {text}");
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
    assert!(text.contains("340"), "the last turn is not reported: {text}");
    assert!(text.contains("This session counted"), "{text}");
    assert!(text.contains("460"), "the session total is not reported: {text}");
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

// ───────────────────────────────────── the row the identity shares with money

/// Columns the renderer keeps clear on the right of the row for the balance,
/// mirroring `tui::BALANCE_COLUMNS`.
const BALANCE_COLUMNS: usize = 26;

fn priced() -> coder_lite::credit::CreditField {
    coder_lite::credit::CreditField::Known(coder_lite::credit::Credit {
        allowance_microusd: 20_000_000,
        spent_microusd: 1_600_000,
        remaining_microusd: 18_400_000,
        unpriced_calls: 0,
        complete: true,
    })
}

fn unpriced(calls: u64) -> coder_lite::credit::CreditField {
    coder_lite::credit::CreditField::Known(coder_lite::credit::Credit {
        allowance_microusd: 20_000_000,
        spent_microusd: 0,
        remaining_microusd: 20_000_000,
        unpriced_calls: calls,
        complete: false,
    })
}

/// Both fields are on the row, and neither has painted over the other.
///
/// This is the seam where #130 and `7cbe78af4f` met. #130 moved the token
/// counts to `/info` and put the identity on the left; the balance had already
/// taken the right. Each was correct against the tree it was written on, and
/// the failure that only exists once both are on one tree is a renderer that
/// draws either as a full-width paragraph and erases the other.
///
/// All three identity states are checked against a balance, because the states
/// are different lengths and a bug that only truncates the longest one would
/// pass a single-case test.
#[test]
fn the_row_carries_the_identity_and_the_balance_without_either_erasing_the_other() {
    for (identity, who) in [
        (Identity::Anonymous, "not signed in"),
        (Identity::Unverified, "credential unverified"),
        (signed_in(), "AtlantisPleb"),
    ] {
        let mut ui = CoderUi::new();
        ui.identity = identity;
        ui.endpoint = "https://openagents.com/api/v1".to_string();
        ui.credit = priced();

        let row = status_row(&mut ui);
        let left = row.find(who).unwrap_or_else(|| {
            panic!("the row should name the account state {who:?}, and held {row:?}")
        });
        let right = row
            .find("$18.40 left")
            .unwrap_or_else(|| panic!("the row should carry the balance, and held {row:?}"));

        assert!(
            left < right,
            "the identity takes the left of the row and the balance the right, \
             but {who:?} and the balance came back at {left} and {right}: {row:?}"
        );
        assert!(
            row.contains("openagents.com"),
            "the endpoint host belongs beside the account, and the row held {row:?}"
        );
        assert!(
            right >= row.len() - BALANCE_COLUMNS,
            "the balance should be drawn inside the {BALANCE_COLUMNS} columns \
             reserved on the right of a {} column row, and started at {right}: {row:?}",
            row.len()
        );
    }
}

/// The widest thing the balance field ever says still fits its columns.
///
/// `BALANCE_COLUMNS` is 26 because these strings are 24 and 25 columns wide,
/// not because 26 is round. A benchmark run on this lane came back with 12
/// unpriced calls, so the two-digit case is the one the field exists to report
/// and the one a narrower reservation would truncate.
#[test]
fn the_widest_balance_string_fits_the_columns_reserved_for_it() {
    for calls in [3, 12, 999] {
        let mut ui = CoderUi::new();
        ui.identity = signed_in();
        ui.endpoint = "https://openagents.com/api/v1".to_string();
        ui.credit = unpriced(calls);

        let row = status_row(&mut ui);
        let expected = format!("credit: {calls} unpriced calls");
        assert!(
            row.contains(&expected),
            "the row should carry {expected:?} untruncated, and held {row:?}"
        );
        assert!(
            !row.contains('$'),
            "an unpriced balance prints no dollar figure at all, and the row held {row:?}"
        );
        assert!(
            row.contains("AtlantisPleb"),
            "the balance must not have erased the account, and the row held {row:?}"
        );
    }
}
