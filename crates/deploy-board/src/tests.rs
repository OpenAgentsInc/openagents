use nostr::deploy::{self, Env, Phase, Status, Tier};
use nostr::domain::{RelaySigner, Tag};

use super::*;
use crate::layout::{Cell, Ink, Piece};
use crate::state::{Flash, Outcome, Step};

const SECRET: &str = "3333333333333333333333333333333333333333333333333333333333333333";
const STRANGER: &str = "4444444444444444444444444444444444444444444444444444444444444444";

fn announcer() -> RelaySigner {
    RelaySigner::from_secret_hex(SECRET).unwrap()
}

fn status(env: Env, run: &str, phase: Phase, started: u64) -> Status {
    Status {
        v: deploy::V.into(),
        env,
        tier: Tier::Image,
        phase,
        run: run.into(),
        commit: Some("2ac6f0c4ce".into()),
        subject: Some("List the Grid Flint pin check".into()),
        by: "agent".into(),
        build: None,
        started,
        test: false,
    }
}

fn ingest() -> Ingest {
    Ingest::new(announcer().pubkey(), false)
}

/// Signs and feeds one phase; returns whether the board changed.
fn feed(ingest: &mut Ingest, status: &Status, at: u64, wall: f64) -> bool {
    let [phase, head] = deploy::sign(&announcer(), status, at).unwrap();
    let changed = ingest.event(&phase, wall);
    // The head carries the same phase: never a second change.
    assert!(!ingest.event(&head, wall));
    changed
}

fn run_through(ingest: &mut Ingest, env: Env, run: &str, phases: &[Phase], from: u64) {
    for (i, phase) in phases.iter().enumerate() {
        let at = from + i as u64 * 60;
        feed(ingest, &status(env, run, *phase, from), at, at as f64);
    }
}

#[test]
fn an_empty_board_says_so() {
    let ingest = ingest();
    let lane = ingest.board.lane(Env::Staging, 1_000.0);
    assert!(lane.current.is_none());
    let pieces = layout::pieces(&ingest.board, 1_000.0, 1024);
    assert!(!pieces.is_empty());
    assert!(
        pieces
            .iter()
            .all(|p| matches!(p.piece, Piece::Letter(_, Ink::Quiet)))
    );
}

#[test]
fn a_staging_deploy_walks_the_strip_to_live() {
    let mut ingest = ingest();
    let s = |phase| status(Env::Staging, "r1", phase, 1_000);
    feed(&mut ingest, &s(Phase::BuildStarted), 1_000, 1_000.0);
    let lane = ingest.board.lane(Env::Staging, 1_030.0);
    let current = lane.current.unwrap();
    assert_eq!(current.word, "BUILDING");
    assert_eq!(current.elapsed, 30);
    assert!(current.running);
    assert_eq!(current.steps[0], Step::Active);
    assert_eq!(current.steps[1], Step::Idle);

    feed(&mut ingest, &s(Phase::BuildDone), 1_400, 1_400.0);
    feed(&mut ingest, &s(Phase::Deploying), 1_410, 1_410.0);
    let current = ingest.board.lane(Env::Staging, 1_420.0).current.unwrap();
    assert_eq!(current.steps[..2], [Step::Done, Step::Active]);
    feed(&mut ingest, &s(Phase::SmokeRunning), 1_500, 1_500.0);
    feed(&mut ingest, &s(Phase::SmokePassed), 1_600, 1_600.0);
    let current = ingest.board.lane(Env::Staging, 1_600.0).current.unwrap();
    assert_eq!(current.steps[..3], [Step::Done; 3]);
    assert_eq!(current.word, "SMOKE PASSED");
    assert!(feed(&mut ingest, &s(Phase::Live), 1_610, 1_610.0));
    let lane = ingest.board.lane(Env::Staging, 1_611.0);
    let current = lane.current.unwrap();
    assert_eq!(current.steps, [Step::Live; 5]);
    assert_eq!(current.word, "LIVE");
    assert_eq!(current.elapsed, 610);
    assert!(!current.running);
    assert_eq!(lane.last, Some((Outcome::Live, 1)));
    assert!(matches!(lane.flash, Some((Flash::Live, _))));
    // The flash ends, and production never moved.
    assert!(ingest.board.lane(Env::Staging, 1_700.0).flash.is_none());
    assert!(
        ingest
            .board
            .lane(Env::Production, 1_700.0)
            .current
            .is_none()
    );
}

#[test]
fn a_failed_smoke_turns_its_cell_red_and_flashes_red() {
    let mut ingest = ingest();
    run_through(
        &mut ingest,
        Env::Staging,
        "r1",
        &[
            Phase::BuildStarted,
            Phase::BuildDone,
            Phase::Deploying,
            Phase::SmokeRunning,
            Phase::SmokeFailed,
        ],
        1_000,
    );
    let lane = ingest.board.lane(Env::Staging, 1_241.0);
    let current = lane.current.unwrap();
    assert_eq!(current.steps[..3], [Step::Done, Step::Done, Step::Failed]);
    assert_eq!(current.outcome, Some(Outcome::Failed));
    assert_eq!(current.word, "SMOKE FAILED");
    assert!(matches!(lane.flash, Some((Flash::Failed, _))));
    let pieces = layout::pieces(&ingest.board, 1_241.0, 1024);
    assert!(pieces.iter().any(|p| p.piece == Piece::Cell(Cell::Failed)));
    let words: Vec<&str> = pieces
        .iter()
        .filter_map(|p| match p.piece {
            Piece::Word(i) => Some(layout::WORDS[i].0),
            _ => None,
        })
        .collect();
    assert!(
        words.contains(&"SMOKE FAILED") && words.contains(&"FAILED"),
        "{words:?}"
    );
}

#[test]
fn production_smoke_after_the_shift_checks_the_live_step() {
    let mut ingest = ingest();
    run_through(
        &mut ingest,
        Env::Production,
        "p1",
        &[
            Phase::Deploying,
            Phase::Promoted,
            Phase::SmokeRunning,
            Phase::SmokePassed,
            Phase::TrafficShifted,
            Phase::SmokeRunning,
        ],
        5_000,
    );
    let current = ingest.board.lane(Env::Production, 5_300.0).current.unwrap();
    assert_eq!(
        current.steps,
        [Step::Done, Step::Done, Step::Done, Step::Done, Step::Active]
    );
    feed(
        &mut ingest,
        &status(Env::Production, "p1", Phase::SmokeFailed, 5_000),
        5_400,
        5_400.0,
    );
    let current = ingest.board.lane(Env::Production, 5_400.0).current.unwrap();
    assert_eq!(current.steps[4], Step::Failed);
}

#[test]
fn records_arriving_out_of_order_give_the_same_board() {
    let phases = [
        Phase::BuildStarted,
        Phase::BuildDone,
        Phase::Deploying,
        Phase::SmokeRunning,
        Phase::SmokePassed,
        Phase::Live,
    ];
    let mut forward = ingest();
    run_through(&mut forward, Env::Staging, "r1", &phases, 1_000);
    let mut backward = ingest();
    for (i, phase) in phases.iter().enumerate().rev() {
        let at = 1_000 + i as u64 * 60;
        feed(
            &mut backward,
            &status(Env::Staging, "r1", *phase, 1_000),
            at,
            9_999.0,
        );
    }
    let a = forward.board.lane(Env::Staging, 9_999.0);
    let b = backward.board.lane(Env::Staging, 9_999.0);
    assert_eq!(a.current, b.current);
    assert_eq!(a.last, b.last);
    // Old news never flashes.
    assert!(b.flash.is_none());
}

#[test]
fn earlier_runs_become_history_newest_first() {
    let mut ingest = ingest();
    let done = [Phase::BuildStarted, Phase::Live];
    run_through(&mut ingest, Env::Staging, "r1", &done, 1_000);
    run_through(
        &mut ingest,
        Env::Staging,
        "r2",
        &[Phase::BuildStarted, Phase::BuildFailed],
        2_000,
    );
    run_through(&mut ingest, Env::Staging, "r3", &done, 3_000);
    run_through(
        &mut ingest,
        Env::Staging,
        "r4",
        &[Phase::BuildStarted],
        4_000,
    );
    let lane = ingest.board.lane(Env::Staging, 4_100.0);
    assert_eq!(lane.current.as_ref().unwrap().word, "BUILDING");
    let outcomes: Vec<_> = lane.history.iter().map(|p| p.outcome).collect();
    assert_eq!(outcomes, [Outcome::Live, Outcome::Failed, Outcome::Live]);
    assert_eq!(lane.history[0].age, 4_100 - 3_060);
    assert_eq!(lane.last, Some((Outcome::Live, 4_100 - 3_060)));
    // A newer run that never ended leaves the older one stopped.
    run_through(
        &mut ingest,
        Env::Staging,
        "r5",
        &[Phase::BuildStarted],
        5_000,
    );
    let lane = ingest.board.lane(Env::Staging, 5_100.0);
    assert_eq!(lane.history[0].outcome, Outcome::Stopped);
}

#[test]
fn a_run_with_no_word_for_hours_is_stalled() {
    let mut ingest = ingest();
    run_through(
        &mut ingest,
        Env::Staging,
        "r1",
        &[Phase::BuildStarted],
        1_000,
    );
    let current = ingest
        .board
        .lane(Env::Staging, (1_000 + state::STALL_SECS + 1) as f64)
        .current
        .unwrap();
    assert_eq!(current.word, "STALLED");
    assert!(!current.running);
    assert_eq!(current.elapsed, 0);
}

#[test]
fn unsigned_and_stranger_events_never_reach_the_board() {
    let mut ingest = ingest();
    let stranger = RelaySigner::from_secret_hex(STRANGER).unwrap();
    let s = status(Env::Staging, "r1", Phase::Live, 1_000);
    let [forged, _] = deploy::sign(&stranger, &s, 1_000).unwrap();
    assert!(!ingest.event(&forged, 1_000.0));
    let [mut unsigned, _] = deploy::sign(&announcer(), &s, 1_000).unwrap();
    unsigned.sig = "0".repeat(128);
    assert!(!ingest.event(&unsigned, 1_000.0));
    let [mut claimed, _] = deploy::sign(&stranger, &s, 1_000).unwrap();
    claimed.pubkey = announcer().pubkey().to_owned();
    assert!(!ingest.event(&claimed, 1_000.0));
    assert_eq!(ingest.refused, 3);
    assert!(ingest.board.lane(Env::Staging, 1_000.0).current.is_none());
}

#[test]
fn test_events_show_only_on_a_test_board() {
    let mut s = status(Env::Staging, "t1", Phase::BuildStarted, 1_000);
    s.test = true;
    let [event, _] = deploy::sign(&announcer(), &s, 1_000).unwrap();
    let mut real = ingest();
    assert!(!real.event(&event, 1_000.0));
    let mut test = Ingest::new(announcer().pubkey(), true);
    assert!(test.event(&event, 1_000.0));
    let [real_event, _] = deploy::sign(
        &announcer(),
        &status(Env::Staging, "r1", Phase::Live, 1_000),
        1_000,
    )
    .unwrap();
    assert!(!test.event(&real_event, 1_000.0));
}

fn livehost_pointer(secret: &str, d: &str, at: u64) -> nostr::domain::Event {
    let signer = RelaySigner::from_secret_hex(secret).unwrap();
    let tag = |n: &str, v: &str| Tag::new(vec![n.into(), v.into()]);
    signer.sign(
        at,
        pointer::KIND,
        vec![
            tag("d", d),
            tag("tenant", "openagents"),
            tag("channel", "staging"),
            tag("m", &"ab".repeat(32)),
        ],
        String::new(),
    )
}

#[test]
fn pointers_count_only_from_their_trusted_key() {
    // The trusted staging key is not ours, so its signature can't be made
    // here: a pointer under another key is refused, and one claiming the
    // trusted key without its signature is too.
    let mut ingest = ingest();
    let wrong = livehost_pointer(SECRET, "openagents/staging", 1_000);
    assert!(!ingest.event(&wrong, 1_000.0));
    let mut claimed = wrong.clone();
    claimed.pubkey = pointer::TRUSTED[0].1.to_owned();
    assert!(!ingest.event(&claimed, 1_000.0));
    assert_eq!(ingest.refused, 2);
}

#[test]
fn a_pointer_is_a_content_deploy_unless_one_was_announced() {
    let mut board = Board::new(false);
    let pointer = state::Pointer {
        env: Env::Staging,
        manifest: "bf2443bd4a9d".repeat(6)[..64].to_owned(),
        at: 1_000,
    };
    assert!(board.apply_pointer(&pointer, 1_010.0));
    let lane = board.lane(Env::Staging, 1_010.0);
    let current = lane.current.unwrap();
    assert_eq!(current.tier, Tier::Content);
    assert_eq!(current.word, "LIVE");
    assert!(matches!(lane.flash, Some((Flash::Live, _))));
    assert!(!board.apply_pointer(&pointer, 1_010.0));

    let mut announced = Board::new(false);
    let mut s = status(Env::Staging, "c1", Phase::Live, 900);
    s.tier = Tier::Content;
    let [event, _] = deploy::sign(&announcer(), &s, 990).unwrap();
    let record = deploy::read(&event, announcer().pubkey(), 990).unwrap();
    announced.apply(&record, 990.0);
    assert!(!announced.apply_pointer(&pointer, 1_010.0));
}

#[test]
fn the_board_holds_a_bounded_number_of_runs() {
    let mut ingest = ingest();
    for i in 0..(state::MAX_RUNS as u64 + 10) {
        run_through(
            &mut ingest,
            Env::Staging,
            &format!("r{i}"),
            &[Phase::Live],
            1_000 + i * 100,
        );
    }
    let lane = ingest.board.lane(Env::Staging, 99_999.0);
    assert_eq!(lane.history.len(), state::HISTORY);
}

#[test]
fn an_active_step_pulses() {
    let mut ingest = ingest();
    run_through(
        &mut ingest,
        Env::Staging,
        "r1",
        &[Phase::BuildStarted],
        1_000,
    );
    let cells = |wall: f64| -> Vec<Cell> {
        layout::pieces(&ingest.board, wall, 1024)
            .into_iter()
            .filter_map(|p| match p.piece {
                Piece::Cell(c) => Some(c),
                _ => None,
            })
            .collect()
    };
    assert_eq!(cells(1_000.1), [Cell::Active]);
    assert_eq!(cells(1_000.9), [Cell::Dim]);
}

#[test]
fn pieces_stay_within_the_budget_and_on_the_face() {
    let mut ingest = ingest();
    for env in Env::ALL {
        for i in 0..5u64 {
            let mut s = status(env, &format!("r{i}"), Phase::Live, 1_000 + i * 100);
            s.subject = Some("W".repeat(deploy::MAX_SUBJECT));
            s.by = "a-very-long-label-here".into();
            feed(&mut ingest, &s, 1_000 + i * 100, 1_500.0);
        }
    }
    let all = layout::pieces(&ingest.board, 1_500.0, 1024);
    assert!(all.len() < 400, "{}", all.len());
    for p in &all {
        let [x, y, _] = p.at;
        assert!((0.0..layout::WIDTH).contains(&x), "{p:?}");
        assert!(
            (layout::LIFT..layout::LIFT + layout::HEIGHT).contains(&y),
            "{p:?}"
        );
        if let Piece::Letter(_, _) = p.piece {
            // A letter ends inside its lane.
            let right = x + 5.0 / 7.0 * p.size;
            let lane = usize::from(x >= layout::LANE_X[1]);
            assert!(right <= layout::LANE_X[lane] + layout::LANE_WIDTH, "{p:?}");
        }
    }
    let few = layout::pieces(&ingest.board, 1_500.0, 100);
    assert!(few.len() <= 100);
}

#[test]
fn every_letter_and_cell_is_drawn_in_a_known_ink() {
    for &c in font::CHARSET {
        for ink in Ink::LETTERS {
            let art = layout::letter(c, ink);
            assert!(!art.lit.is_empty());
            assert!(art.lit.iter().all(|(_, i)| *i == ink));
        }
    }
    for (text, ink) in layout::WORDS {
        assert!(layout::word(text, ink).lit.iter().all(|(_, i)| *i == ink));
    }
    // The Grid, its terminal, and the Bay use 61 of a pack's 256 models.
    assert!(layout::MODELS <= 140, "{}", layout::MODELS);
    for look in Cell::ALL {
        assert_eq!(layout::cell(look).lit.len(), 1);
    }
    let frame = layout::frame();
    assert!(!frame.lines.is_empty() && !frame.faces.is_empty() && !frame.lit.is_empty());
}

#[test]
fn the_board_stands_clear_of_the_grids_landmarks() {
    // World [x, z] boxes, m: the Gym at its Grid site, the Pylon Bay's pad,
    // the terminal, the spawn, and each arch with its approach.
    let landmarks: [(&str, [f64; 4]); 9] = [
        ("gym", [-9.0, 26.0, 9.0, 50.0]),
        ("pylon bay", [14.8, -3.5, 41.3, 26.5]),
        ("terminal", [-2.0, -7.0, 2.0, -3.0]),
        ("spawn", [-6.0, -16.0, 6.0, -4.0]),
        ("lagrange 1", [-13.0, -3.0, -5.0, 5.0]),
        ("everglade", [5.0, -3.0, 13.0, 5.0]),
        ("lagrange 2", [-24.0, 5.0, -16.0, 13.0]),
        ("ritual", [-4.0, 12.0, 4.0, 20.0]),
        ("ice cream man", [16.0, 5.0, 24.0, 13.0]),
    ];
    let [x0, z0, x1, z1] = layout::footprint();
    for (name, [a0, b0, a1, b1]) in landmarks {
        let apart = x1 + 2.0 < a0 || a1 + 2.0 < x0 || z1 + 2.0 < b0 || b1 + 2.0 < z0;
        assert!(
            apart,
            "the board overlaps the {name}: {:?}",
            layout::footprint()
        );
    }
    // It faces into the Pylon Bay.
    let front = layout::world([layout::WIDTH / 2.0, 0.0, 1.0]);
    let back = layout::world([layout::WIDTH / 2.0, 0.0, 0.0]);
    let (fx, fz) = (front[0] - back[0], front[2] - back[2]);
    let (bx, bz) = (30.0 - back[0], 10.0 - back[2]);
    assert!(fx * bx + fz * bz > 0.9 * (bx * bx + bz * bz).sqrt());
}

#[test]
fn from_the_spawn_only_the_boards_back_shows() {
    // The spawn disc (`verse_world::social::arrival::SpawnArea::GRID`:
    // center (0, -14), radius 6, facing (0, 0)), and the third-person
    // camera 4 m behind each spot. Every one of them stands behind the
    // board's face, so no lane, status, or Pylon count reads from there
    // (#12482).
    let front = layout::world([layout::WIDTH / 2.0, 0.0, 1.0]);
    let back = layout::world([layout::WIDTH / 2.0, 0.0, 0.0]);
    let normal = [front[0] - back[0], front[2] - back[2]];
    let mut checked = 0;
    for ring in 0..=6 {
        for step in 0..24 {
            let r = f64::from(ring);
            let a = f64::from(step) * std::f64::consts::TAU / 24.0;
            let spot = [r * a.cos(), -14.0 + r * a.sin()];
            let toward = [-spot[0], -spot[1]];
            let length = toward[0].hypot(toward[1]);
            let eye = [
                spot[0] - 4.0 * toward[0] / length,
                spot[1] - 4.0 * toward[1] / length,
            ];
            for p in [spot, eye] {
                let (dx, dz) = (p[0] - back[0], p[1] - back[2]);
                assert!(dx * normal[0] + dz * normal[1] < 0.0, "{p:?}");
                checked += 1;
            }
        }
    }
    assert!(checked > 300);
}

#[test]
fn the_filters_ask_for_heads_history_and_pointers() {
    let filters = filters();
    assert_eq!(filters.len(), 3);
    assert_eq!(filters[0]["kinds"][0], deploy::HEAD_KIND);
    assert_eq!(filters[1]["limit"], HISTORY_EVENTS);
    assert_eq!(filters[2]["kinds"][0], pointer::KIND);
}
