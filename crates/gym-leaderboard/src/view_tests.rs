//! The presentation rules, tested on the committed publication.

use std::path::PathBuf;

use super::*;
use crate::contract::{Leaderboard, TraceBundle};

fn published() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench/published")
}

fn leaderboard() -> Leaderboard {
    let bytes = std::fs::read(published().join(crate::LEADERBOARD_FILE)).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn bundle(path: &str) -> TraceBundle {
    let bytes = std::fs::read(published().join(path)).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

const DELEGATE: &str = "tb4-fable-delegate-repro-9776";
const TB21: &str = "tb21-oos-microcoder-9683";

fn board_page_of(lb: &Leaderboard, id: &str, filter: Filter, open: bool) -> BoardPage {
    let mut nav = Nav::default();
    nav.select_board(lb, id).unwrap();
    nav.set_filter(filter).unwrap();
    nav.set_caveats_open(open).unwrap();
    match render(&nav, lb, None, None).unwrap() {
        Page::Board(page) => *page,
        other => panic!("not a board page: {other:?}"),
    }
}

fn attempt_page_of(lb: &Leaderboard, board: &str, attempt: &str) -> AttemptPage {
    let mut nav = Nav::default();
    nav.select_board(lb, board).unwrap();
    nav.select_attempt(lb, attempt).unwrap();
    match render(&nav, lb, None, None).unwrap() {
        Page::Attempt(page) => *page,
        other => panic!("not an attempt page: {other:?}"),
    }
}

fn cells(page: &BoardPage) -> impl Iterator<Item = &AttemptCell> {
    page.tasks.iter().flat_map(|t| &t.attempts)
}

fn trace_nav(lb: &Leaderboard, attempt: &str) -> (Nav, TraceBundle) {
    let mut nav = Nav::default();
    nav.select_board(lb, DELEGATE).unwrap();
    nav.select_attempt(lb, attempt).unwrap();
    let trace = nav.open_trace(lb).unwrap();
    (nav, bundle(&trace.path))
}

fn trace_page_of(nav: &Nav, lb: &Leaderboard, bundle: &TraceBundle) -> TracePage {
    match render(nav, lb, None, Some(bundle)).unwrap() {
        Page::Trace(page) => *page,
        other => panic!("not a trace page: {other:?}"),
    }
}

/// Rule 1: an unknown cost reads "unknown", its bound is labeled a bound,
/// it never shows as zero, and it never beats.
#[test]
fn rule_1_unknown_cost_is_never_zero() {
    let lb = leaderboard();
    let page = board_page_of(&lb, DELEGATE, Filter::All, false);
    let board = &lb.boards[0];
    let mut unknown = 0;
    for cell in cells(&page) {
        let attempt = board.attempts.iter().find(|a| a.id == cell.id).unwrap();
        if matches!(attempt.cost, Cost::Unknown { .. }) {
            unknown += 1;
            assert!(cell.text.contains("unknown"), "{}", cell.text);
            assert!(cell.text.contains("bound"), "{}", cell.text);
            assert!(!cell.beat);
            assert!(!cell.text.contains("$0.0000"));
            assert!(cell.labels.iter().any(|c| c.code == Label::CostBound));
        }
    }
    assert_eq!(unknown, 20);
    // The deadline-stopped attempt's own screen says so too.
    let attempt = attempt_page_of(&lb, DELEGATE, "batched-eval-parity.p1");
    assert!(attempt.header.cost.starts_with("unknown"));
    assert!(attempt.misses.contains("cost unknown"));
    assert!(
        attempt
            .numbers
            .iter()
            .any(|n| n.contains("comparing the bound"))
    );
    let mut money = Money::new();
    assert_eq!(
        money.cost(&Cost::Unknown {
            lower_bound_usd: None,
            upper_bound_usd: None
        }),
        "unknown"
    );
}

/// Rule 2: every rate keeps its whole denominator, faults apart.
#[test]
fn rule_2_denominators_stay_whole() {
    let lb = leaderboard();
    let page = board_page_of(&lb, DELEGATE, Filter::All, false);
    let all = &page.tallies[0];
    assert_eq!(all.passed, "13 of 28");
    assert_eq!(all.beat, "4 of 28");
    assert_eq!(all.cost_unknown, "20 of 28");
    assert!(all.faults.contains("not counted as attempts"));
    for row in &page.tallies {
        for rate in [&row.passed, &row.beat, &row.cost_unknown] {
            assert!(rate.contains(" of "), "{rate}");
        }
    }
    let tb21 = board_page_of(&lb, TB21, Filter::All, false);
    assert_eq!(tb21.tallies[0].passed, "83 of 127");
}

/// Rule 3: the board's labels and the attempt's own are on every attempt
/// row, in the table, on the attempt screen, and on the trace header.
#[test]
fn rule_3_labels_travel_with_every_attempt_row() {
    let lb = leaderboard();
    for board in &lb.boards {
        let page = board_page_of(&lb, &board.id, Filter::All, false);
        assert_eq!(cells(&page).count(), board.attempts.len());
        for cell in cells(&page) {
            let attempt = board.attempts.iter().find(|a| a.id == cell.id).unwrap();
            for label in board.labels.iter().chain(&attempt.labels) {
                assert!(
                    cell.labels.iter().any(|c| c.code == *label),
                    "{} lacks {label:?}",
                    cell.id
                );
                assert!(cell.accessibility.contains(label.text()));
            }
        }
    }
    let attempt = attempt_page_of(&lb, TB21, &lb.boards[1].attempts[0].id);
    for label in &lb.boards[1].labels {
        assert!(attempt.header.labels.iter().any(|c| c.code == *label));
    }
    let (nav, bundle) = trace_nav(&lb, "coq-block-bound.p2");
    let trace = trace_page_of(&nav, &lb, &bundle);
    for label in &lb.boards[0].labels {
        assert!(trace.header.labels.iter().any(|c| c.code == *label));
    }
}

/// Rule 4 and rule 7: a beat under a 5% margin says thin margin, and a
/// beat's row carries the in-sample and thin-margin caveats.
#[test]
fn rule_4_and_7_thin_margins_and_in_sample_travel_with_beats() {
    let lb = leaderboard();
    let page = board_page_of(&lb, DELEGATE, Filter::Beats, false);
    let beats: Vec<&AttemptCell> = cells(&page).filter(|c| c.beat).collect();
    assert_eq!(beats.len(), 4);
    for cell in &beats {
        let thin = ["gsea-proteomics.p2", "sound-change-cascade.p2"].contains(&cell.id.as_str());
        assert_eq!(
            cell.labels.iter().any(|c| c.code == Label::ThinMargin),
            thin,
            "{}",
            cell.id
        );
        assert!(cell.caveats.iter().any(|c| c.code == "in_sample"));
        assert_eq!(cell.caveats.iter().any(|c| c.code == "thin_margin"), thin);
    }
    // A fabricated thin beat without the generator's label still gets it.
    let mut board = lb.boards[0].clone();
    let attempt = board.attempts.iter_mut().find(|a| a.beat).unwrap();
    attempt.labels.retain(|l| *l != Label::ThinMargin);
    attempt.cost_ratio = Some(0.97);
    let attempt = attempt.clone();
    assert!(
        attempt_labels(&board, &attempt)
            .iter()
            .any(|c| c.code == Label::ThinMargin)
    );
    // No thin label on a row that isn't a beat.
    for cell in cells(&board_page_of(&lb, DELEGATE, Filter::All, false)).filter(|c| !c.beat) {
        assert!(cell.caveats.is_empty());
        assert!(!cell.labels.iter().any(|c| c.code == Label::ThinMargin));
    }
    let attempt = attempt_page_of(&lb, DELEGATE, "gsea-proteomics.p2");
    assert!(attempt.caveats.iter().any(|c| c.code == "thin_margin"));
}

/// Rule 5: each pass is its own tally, never pooled into another.
#[test]
fn rule_5_passes_are_separate_splits() {
    let lb = leaderboard();
    let page = board_page_of(&lb, DELEGATE, Filter::All, false);
    let pass = |name: &str| page.tallies.iter().find(|t| t.name == name).unwrap();
    assert_eq!(pass("pass 1").beat, "0 of 14");
    assert_eq!(pass("pass 2").beat, "4 of 14");
    assert_eq!(page.tallies.len(), 1 + lb.boards[0].splits.len());
}

/// Rule 6: the headline is the board's, verbatim, on the list and board.
#[test]
fn rule_6_the_headline_is_verbatim() {
    let lb = leaderboard();
    let Page::Boards(list) = render(&Nav::default(), &lb, None, None).unwrap() else {
        panic!()
    };
    for (row, board) in list.rows.iter().zip(&lb.boards) {
        assert_eq!(row.headline, board.headline);
        assert_eq!(
            board_page_of(&lb, &board.id, Filter::All, false).headline,
            board.headline
        );
    }
}

/// Rule 7: the caveat count and the first caveat show without opening.
#[test]
fn rule_7_the_first_caveat_is_always_shown() {
    let lb = leaderboard();
    for board in &lb.boards {
        let closed = board_page_of(&lb, &board.id, Filter::All, false);
        assert_eq!(closed.caveat_count, board.caveats.len());
        assert_eq!(closed.caveats.len(), 1);
        assert_eq!(closed.caveats[0].text, board.caveats[0].text);
        let open = board_page_of(&lb, &board.id, Filter::All, true);
        assert_eq!(open.caveats.len(), board.caveats.len());
    }
}

/// Rule 8: the list keeps the publication's order and sums nothing.
#[test]
fn rule_8_no_cross_board_ranking() {
    let mut lb = leaderboard();
    // Reversing the publication reverses the list: order is the file's,
    // not a score's.
    lb.boards.reverse();
    let Page::Boards(list) = render(&Nav::default(), &lb, None, None).unwrap() else {
        panic!()
    };
    let ids: Vec<&str> = list.rows.iter().map(|r| r.id.as_str()).collect();
    let want: Vec<&str> = lb.boards.iter().map(|b| b.id.as_str()).collect();
    assert_eq!(ids, want);
    let value = serde_json::to_value(&list).unwrap();
    assert_eq!(
        value.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["rows", "footer"]
    );
}

/// Rule 9: a bar always comes with the reference's name, and its
/// conditions are on the same screen.
#[test]
fn rule_9_reference_conditions_beside_the_bar() {
    let lb = leaderboard();
    for board in &lb.boards {
        let page = board_page_of(&lb, &board.id, Filter::All, false);
        assert_eq!(page.reference.conditions, board.reference.conditions);
        for task in &page.tasks {
            assert!(task.bar.starts_with(&board.reference.name), "{}", task.bar);
        }
        let attempt = attempt_page_of(&lb, &board.id, &board.attempts[0].id);
        assert_eq!(attempt.reference.conditions, board.reference.conditions);
        assert!(
            attempt
                .numbers
                .iter()
                .any(|n| n.contains(&board.reference.name))
        );
    }
}

/// Rule 10: the first dollar figure on each screen says list price.
#[test]
fn rule_10_first_dollar_figure_is_list_price() {
    fn first_dollar(strings: &[String]) -> &String {
        strings.iter().find(|s| s.contains('$')).unwrap()
    }
    let lb = leaderboard();
    for board in &lb.boards {
        let page = board_page_of(&lb, &board.id, Filter::All, false);
        match page.headline_note {
            // The headline carries a dollar figure; the note right under
            // it labels it.
            Some(note) => {
                assert!(page.headline.contains('$'));
                assert!(note.contains("list price"));
            }
            None => assert!(first_dollar(&page.spend).contains("list price")),
        }
        let dollars: Vec<String> = page
            .spend
            .iter()
            .cloned()
            .chain(page.tasks.iter().map(|t| t.bar.clone()))
            .filter(|s| s.contains('$'))
            .collect();
        assert_eq!(
            dollars.iter().filter(|s| s.contains("list price")).count(),
            usize::from(page.headline_note.is_none())
        );
        let attempt = attempt_page_of(&lb, &board.id, &board.attempts[0].id);
        let figures = [attempt.header.cost.clone()]
            .into_iter()
            .chain(attempt.numbers.clone())
            .collect::<Vec<_>>();
        assert!(first_dollar(&figures).contains("list price"));
        assert_eq!(
            figures.iter().filter(|s| s.contains("list price")).count(),
            1
        );
    }
    let Page::Boards(list) = render(&Nav::default(), &lb, None, None).unwrap() else {
        panic!()
    };
    let first = list.rows.iter().find(|r| r.headline.contains('$')).unwrap();
    assert!(first.headline_note.is_some());
}

/// Every screen of every board and bundle stays under the page bound,
/// far below the 1 MiB native packet cap.
#[test]
fn every_page_fits_its_slice_bound() {
    let lb = leaderboard();
    let size = |page: &Page| serde_json::to_vec(page).unwrap().len();
    let mut largest = 0;
    assert!(size(&render(&Nav::default(), &lb, None, None).unwrap()) < MAX_PAGE_BYTES);
    for board in &lb.boards {
        for filter in Filter::ALL {
            for open in [false, true] {
                let mut nav = Nav::default();
                nav.select_board(&lb, &board.id).unwrap();
                nav.set_filter(filter).unwrap();
                nav.set_caveats_open(open).unwrap();
                let bytes = size(&render(&nav, &lb, None, None).unwrap());
                largest = largest.max(bytes);
                assert!(bytes < MAX_PAGE_BYTES, "{} {filter:?}: {bytes}", board.id);
            }
        }
        for attempt in &board.attempts {
            let mut nav = Nav::default();
            nav.select_board(&lb, &board.id).unwrap();
            nav.select_attempt(&lb, &attempt.id).unwrap();
            assert!(size(&render(&nav, &lb, None, None).unwrap()) < MAX_PAGE_BYTES);
            let Some(trace) = &attempt.trace else {
                continue;
            };
            let bundle = bundle(&trace.path);
            nav.open_trace(&lb).unwrap();
            let steps = rows(&bundle).len();
            for tab in Tab::ALL {
                nav.set_tab(tab).unwrap();
                for page in 0..pages(steps) {
                    nav.set_page(&bundle, page).unwrap();
                    for expanded in [None, Some(steps.saturating_sub(1))] {
                        nav.expand(&bundle, expanded).unwrap();
                        let bytes = size(&render(&nav, &lb, None, Some(&bundle)).unwrap());
                        largest = largest.max(bytes);
                        assert!(bytes < MAX_PAGE_BYTES, "{}: {bytes}", attempt.id);
                    }
                }
            }
        }
    }
    // The whole leaderboard is several times one page.
    let whole = std::fs::metadata(published().join(crate::LEADERBOARD_FILE))
        .unwrap()
        .len() as usize;
    assert!(largest < whole, "{largest} vs {whole}");
}

/// Screens follow the Gym panel's convention: select by ID, back one
/// screen at a time, and refuse IDs that aren't there.
#[test]
fn navigation_selects_by_id_and_backs_out_one_screen_at_a_time() {
    let lb = leaderboard();
    let mut nav = Nav::default();
    assert!(nav.select_attempt(&lb, "coq-block-bound.p2").is_err());
    assert!(nav.select_board(&lb, "no-such-board").is_err());
    nav.select_board(&lb, DELEGATE).unwrap();
    assert!(nav.select_attempt(&lb, "no-such-attempt").is_err());
    nav.select_attempt(&lb, "coq-block-bound.p2").unwrap();
    assert!(nav.set_filter(Filter::Beats).is_err());
    let trace = nav.open_trace(&lb).unwrap();
    assert!(trace.path.ends_with("coq-block-bound.p2.json"));
    // Without its bundle the trace screen isn't ready.
    assert!(render(&nav, &lb, None, None).is_err());
    // A bundle for another attempt never renders as this one.
    let other = bundle("traces/tb4-fable-delegate-repro-9776/shadow-relay.p1.json");
    assert!(render(&nav, &lb, None, Some(&other)).is_err());
    nav.back();
    assert_eq!(nav.attempt(), Some((DELEGATE, "coq-block-bound.p2")));
    assert!(nav.trace().is_none());
    nav.back();
    assert_eq!(nav.board(), Some(DELEGATE));
    nav.back();
    assert!(nav.board().is_none());
    // A TB2.1 attempt has no bundle yet, so no trace opens.
    nav.select_board(&lb, TB21).unwrap();
    nav.select_attempt(&lb, &lb.boards[1].attempts[0].id)
        .unwrap();
    assert!(nav.open_trace(&lb).is_err());
    assert!(
        attempt_page_of(&lb, TB21, &lb.boards[1].attempts[0].id)
            .trace
            .is_none()
    );
}

#[test]
fn filters_count_and_select_tasks() {
    let lb = leaderboard();
    let all = board_page_of(&lb, DELEGATE, Filter::All, false);
    assert_eq!(all.tasks.len(), 14);
    let count = |f: Filter| all.filters.iter().find(|c| c.filter == f).unwrap().count;
    assert_eq!(count(Filter::Beats), 4);
    assert_eq!(
        count(Filter::OwnKnowledge) + count(Filter::NoOwnKnowledge),
        14
    );
    for filter in Filter::ALL {
        let page = board_page_of(&lb, DELEGATE, filter, false);
        assert_eq!(page.tasks.len(), count(filter));
        assert!(
            page.filters
                .iter()
                .any(|c| c.selected && c.filter == filter)
        );
    }
    let never = board_page_of(&lb, DELEGATE, Filter::NeverPassed, false);
    assert!(never.tasks.iter().all(|t| t.status == "never passed"));
}

/// The trace viewer's clock: seek, step, play, and pages agree on one
/// row order, usage only moves the token counter, and outputs expand one
/// at a time.
#[test]
fn the_trace_viewer_steps_pages_and_plays_on_the_bundles_clock() {
    let lb = leaderboard();
    let (mut nav, bundle) = trace_nav(&lb, "coq-block-bound.p2");
    let page = trace_page_of(&nav, &lb, &bundle);
    assert_eq!(page.tab, Tab::Jev);
    assert_eq!(page.header.result, "passed");
    assert_eq!(page.header.beat, "beat the bar");
    assert!(page.header.time.contains("Fable 5.1 low"));
    let jev = page.jev.unwrap();
    assert!(jev.candidates.iter().any(|c| c.kept && c.own));
    assert!(
        jev.candidates
            .iter()
            .all(|c| c.kept == (c.p >= jev.keep_threshold))
    );
    assert!(
        jev.requirements
            .iter()
            .all(|r| r.flagged == (r.p >= jev.flag_threshold))
    );
    let steps = page.clock.steps;
    assert_eq!(steps, rows(&bundle).len());
    assert!(steps < bundle.steps.len(), "usage takes no rows");

    nav.set_tab(Tab::Agent).unwrap();
    let agent = trace_page_of(&nav, &lb, &bundle).agent.unwrap();
    assert_eq!(agent.tokens, "No tokens yet");
    assert!(agent.rows.iter().all(|r| r.output.is_none()));
    // Stepping forward walks rows in clock order.
    let mut last = 0;
    for _ in 0..steps {
        nav.step(&bundle, true).unwrap();
        let clock = trace_page_of(&nav, &lb, &bundle).clock;
        assert!(clock.playhead_ms >= last);
        last = clock.playhead_ms;
    }
    let end = trace_page_of(&nav, &lb, &bundle);
    assert_eq!(end.clock.step, steps - 1);
    assert_ne!(end.agent.unwrap().tokens, "No tokens yet");
    nav.step(&bundle, false).unwrap();
    assert_eq!(trace_page_of(&nav, &lb, &bundle).clock.step, steps - 2);
    // Seeking to the start and the end.
    nav.seek(&bundle, 0.0).unwrap();
    assert_eq!(trace_page_of(&nav, &lb, &bundle).clock.step, 0);
    nav.seek(&bundle, 1.0).unwrap();
    assert_eq!(trace_page_of(&nav, &lb, &bundle).clock.step, steps - 1);
    assert!(nav.seek(&bundle, f64::NAN).is_err());
    // Playing from the end starts over, and runs to the end.
    nav.set_playing(&bundle, true).unwrap();
    assert_eq!(trace_page_of(&nav, &lb, &bundle).clock.step, 0);
    let mut changes = 0;
    while nav.playing() {
        if nav.tick(&bundle, 1.0) {
            changes += 1;
        }
        assert!(changes <= steps + 1);
    }
    assert!(changes > 0);
    assert_eq!(trace_page_of(&nav, &lb, &bundle).clock.step, steps - 1);
    assert!(!nav.tick(&bundle, 1.0), "a paused trace doesn't change");
    // Outputs expand one at a time.
    let result = agent_index(&trace_page_of(&nav, &lb, &bundle), "command_result");
    nav.seek(&bundle, 0.0).unwrap();
    nav.set_page(&bundle, result / STEPS_PER_PAGE).unwrap();
    nav.expand(&bundle, Some(result)).unwrap();
    let agent = trace_page_of(&nav, &lb, &bundle).agent.unwrap();
    assert_eq!(agent.rows.iter().filter(|r| r.output.is_some()).count(), 1);
    assert!(nav.expand(&bundle, Some(steps)).is_err());
    assert!(nav.set_page(&bundle, pages(steps)).is_err());
    // Verifier and briefing.
    nav.set_tab(Tab::Verifier).unwrap();
    let verifier = trace_page_of(&nav, &lb, &bundle).verifier.unwrap();
    assert!(verifier.tests.iter().all(|t| t.passed));
    nav.set_tab(Tab::Briefing).unwrap();
    assert!(
        !trace_page_of(&nav, &lb, &bundle)
            .briefing
            .unwrap()
            .text
            .is_empty()
    );
}

fn agent_index(page: &TracePage, kind: &str) -> usize {
    page.agent
        .as_ref()
        .and_then(|a| a.rows.iter().find(|r| r.kind == kind))
        .map_or_else(|| panic!("no {kind} row on the page"), |r| r.index)
}

/// A deadline-stopped attempt: unknown cost in the header, and a cut
/// output says how large it was.
#[test]
fn a_deadline_stopped_trace_says_unknown_and_cut_sizes() {
    let lb = leaderboard();
    let (mut nav, bundle) = trace_nav(&lb, "batched-eval-parity.p1");
    let page = trace_page_of(&nav, &lb, &bundle);
    assert!(page.header.cost.starts_with("unknown"));
    assert_eq!(page.header.beat, "no beat");
    nav.set_tab(Tab::Agent).unwrap();
    let rows = rows(&bundle).len();
    let mut cut = false;
    for p in 0..pages(rows) {
        nav.set_page(&bundle, p).unwrap();
        let agent = trace_page_of(&nav, &lb, &bundle).agent.unwrap();
        cut |= agent
            .rows
            .iter()
            .any(|r| r.cut.as_deref().is_some_and(|c| c.starts_with("cut from ")));
    }
    let truncated = bundle.scrub.truncated_fields > 0;
    assert!(
        !truncated
            || cut
            || bundle
                .briefing
                .as_ref()
                .is_some_and(|b| b.original_bytes.is_some())
    );
}

#[test]
fn the_footer_names_the_digest_commit_and_freshness() {
    let source = Source {
        digest: "50711c103afa4da8".into(),
        commit: Some("6a7a057bae35".into()),
        freshness: Freshness::Offline,
        age_seconds: Some(7200),
    };
    assert_eq!(
        source.footer(),
        "Publication 50711c10 · commit 6a7a057 · offline, cached 2 h ago"
    );
    let lb = leaderboard();
    let Page::Boards(list) = render(&Nav::default(), &lb, Some(&source), None).unwrap() else {
        panic!()
    };
    assert!(list.footer.unwrap().contains("offline"));
}
