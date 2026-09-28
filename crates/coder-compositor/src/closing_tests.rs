//! Tests for the close chord.
//!
//! What counts as work in flight is not here. It lives in
//! `crates/coder/src/activity.rs`, the verb this chord asks, and that
//! file's tests hold it. What is tested here is the decision the
//! chord makes from that answer: an idle window closes on the first press,
//! a working one takes a notice and the press after it, and the client is
//! asked about the window's process and everything under it.

use super::*;

/// One window's identifier.
fn window(id: u64) -> WinId {
    WinId(id)
}

#[test]
fn an_idle_window_closes_on_the_first_press() {
    let mut closing = Closing::default();
    let now = Instant::now();
    assert_eq!(closing.decide(window(1), None, now), Decision::Close);
}

#[test]
fn a_window_with_work_in_flight_asks_and_does_not_close() {
    let mut closing = Closing::default();
    let now = Instant::now();
    let said = Some("A turn is streaming.".to_string());
    assert_eq!(
        closing.decide(window(1), said, now),
        Decision::Ask("A turn is streaming.".to_string())
    );
}

#[test]
fn the_press_after_the_notice_closes_it() {
    let mut closing = Closing::default();
    let now = Instant::now();
    let said = || Some("A turn is streaming.".to_string());
    closing.decide(window(1), said(), now);
    let second = now + Duration::from_secs(1);
    assert_eq!(closing.decide(window(1), said(), second), Decision::Close);
}

#[test]
fn a_press_after_the_notice_ran_out_asks_again() {
    let mut closing = Closing::default();
    let now = Instant::now();
    let said = || Some("A turn is streaming.".to_string());
    closing.decide(window(1), said(), now);
    let late = now + WINDOW + Duration::from_secs(1);
    assert!(matches!(
        closing.decide(window(1), said(), late),
        Decision::Ask(_)
    ));
}

#[test]
fn a_notice_arms_the_window_it_was_raised_for_and_no_other() {
    let mut closing = Closing::default();
    let now = Instant::now();
    let said = || Some("1 delegation has not reported.".to_string());
    closing.decide(window(1), said(), now);
    let second = now + Duration::from_secs(1);
    assert!(matches!(
        closing.decide(window(2), said(), second),
        Decision::Ask(_)
    ));
}

#[test]
fn a_window_that_answered_idle_after_a_notice_closes_at_once() {
    let mut closing = Closing::default();
    let now = Instant::now();
    closing.decide(window(1), Some("A turn is streaming.".to_string()), now);
    let second = now + Duration::from_secs(1);
    assert_eq!(closing.decide(window(1), None, second), Decision::Close);
}

#[test]
fn the_client_is_asked_about_the_window_and_every_process_under_it() {
    // The window is the terminal emulator, 100; the session it runs is
    // 101, and the command that session started is 102. The session beside
    // it, 200, is never asked about.
    let table = [(100, 1), (101, 100), (102, 101), (200, 1), (201, 200)];
    let mut asked = descendants(&table, 100);
    asked.sort_unstable();
    assert_eq!(asked, vec![100, 101, 102]);
}

#[test]
fn a_window_with_no_process_under_it_is_asked_about_itself() {
    assert_eq!(descendants(&[(200, 1)], 100), vec![100]);
}

#[test]
fn a_process_table_reads_the_parent_of_a_command_with_a_parenthesis_in_its_name() {
    let stat = "4242 (foot (the terminal)) S 1701 4242 4242 0 -1 4194304 0";
    assert_eq!(parent_of(stat), Some(1701));
    assert_eq!(parent_of("4242 (foot) S 7 1 2"), Some(7));
    assert_eq!(parent_of("nothing like a stat line"), None);
}

#[test]
fn this_machine_answers_a_process_table_holding_this_process() {
    let table = process_table();
    let own = std::process::id() as i64;
    assert!(
        table.iter().any(|(pid, _)| *pid == own),
        "the table holds {} rows and not this process",
        table.len()
    );
}

#[test]
fn a_notice_shows_until_it_runs_out() {
    let mut notices = Notices::default();
    let now = Instant::now();
    assert_eq!(notices.showing(now), None);
    notices.raise("A turn is streaming.", now);
    assert_eq!(notices.showing(now), Some("A turn is streaming."));
    assert_eq!(notices.showing(now + NOTICE + Duration::from_secs(1)), None);
}

#[test]
fn the_question_is_not_asked_about_no_processes() {
    let client = Client {
        program: "this-client-is-not-installed".to_string(),
    };
    assert_eq!(client.busy(&[]), None);
    // A client that is not installed answers the way an idle session
    // does, and the window closes.
    assert_eq!(client.busy(&[1]), None);
}
