//! The jobs teach what the design says they teach: each test plays a job
//! the way a child might, wrong and then right, through the same inputs
//! the page sends, and checks the physics answers honestly.

use crate::game::{Fail, Game, Input, Status};
use crate::kinds::PieceKind;
use crate::level;
use crate::mm;

fn job(number: usize) -> Game {
    Game::new(level::job(number))
}

fn put(game: &mut Game, kind: PieceKind, x: f64, y: f64, turn: u8) {
    let before = game.build.pieces.len();
    game.step(&[Input::Place {
        kind,
        x: mm(x),
        y: mm(y),
        turn,
    }]);
    if kind.is_tile() {
        assert_eq!(game.build.pieces.len(), before + 1, "{kind:?} at {x} {y}");
    }
}

/// A square at `(x, y)` (its middle), braced if asked.
fn square(game: &mut Game, x: f64, y: f64, braced: bool) {
    put(game, PieceKind::Square, x, y, 0);
    if braced {
        put(game, PieceKind::Strut, x, y, 1);
        let last = game.build.pieces.len() - 1;
        assert!(game.build.pieces[last].strut.is_some(), "braced at {x} {y}");
    }
}

fn bolt_all(game: &mut Game) {
    for index in 0..game.build.seams.len() {
        if game.left(PieceKind::Bolt) == 0 {
            break;
        }
        let at = game.build.seams[index].middle(&game.build);
        put(game, PieceKind::Bolt, at.x, at.y, 0);
    }
}

/// Goes and runs to the verdict, then comes back to building.
fn try_it(game: &mut Game) -> Status {
    game.step(&[Input::Go]);
    while game.status == Status::Running {
        game.step(&[]);
    }
    let verdict = game.status;
    let broke = game
        .world
        .as_ref()
        .map_or(0, |w| w.joints.iter().filter(|j| j.broken).count());
    let popped = game
        .world
        .as_ref()
        .map_or(0, |w| w.popped.iter().filter(|p| **p).count());
    eprintln!(
        "job {} {}: {verdict:?} ({broke} seams let go, {popped} squares popped)",
        game.level.number, game.level.name
    );
    game.step(&[Input::Fix]);
    verdict
}

#[test]
fn job_1_three_squares_join_the_modules() {
    let mut game = job(1);
    for x in [1.5, 2.5, 3.5] {
        square(&mut game, x, 0.5, false);
    }
    assert_eq!(try_it(&mut game), Status::Won);
}

#[test]
fn job_2_a_push_through_the_middle_goes_straight_and_one_off_it_spins() {
    let mut crooked = job(2);
    put(&mut crooked, PieceKind::Panel, 0.5, 1.5, 0);
    put(&mut crooked, PieceKind::Thruster, -0.25, 1.5, 3);
    let verdict = try_it(&mut crooked);
    assert!(
        matches!(verdict, Status::Failed(Fail::Spun | Fail::Missed)),
        "{verdict:?}"
    );
    let mut straight = job(2);
    put(&mut straight, PieceKind::Thruster, -0.25, 0.5, 3);
    assert_eq!(try_it(&mut straight), Status::Won);
}

#[test]
fn job_3_the_storm_pops_bare_squares_and_pulls_magnets_but_braced_bolted_holds() {
    let mut bare = job(3);
    for x in [1.5, 2.5, 3.5] {
        square(&mut bare, x, 0.5, false);
    }
    assert_eq!(try_it(&mut bare), Status::Failed(Fail::CameApart));

    let mut braced = job(3);
    for x in [1.5, 2.5, 3.5] {
        square(&mut braced, x, 0.5, true);
    }
    assert_eq!(
        try_it(&mut braced),
        Status::Failed(Fail::CameApart),
        "braced but not bolted: the magnets let go"
    );
    bolt_all(&mut braced);
    assert_eq!(try_it(&mut braced), Status::Won);

    let mut bolted = job(3);
    for x in [1.5, 2.5, 3.5] {
        square(&mut bolted, x, 0.5, false);
    }
    bolt_all(&mut bolted);
    assert_eq!(
        try_it(&mut bolted),
        Status::Failed(Fail::CameApart),
        "bolted but not braced: the squares pop"
    );
}

/// Spokes from the hub to both pods, braced.
fn spokes(game: &mut Game) {
    for x in [1.5, 2.5, -0.5, -1.5] {
        square(game, x, 0.5, true);
    }
}

#[test]
fn job_4_spinning_makes_gravity_and_pulls_unbolted_pods_off() {
    let mut loose = job(4);
    spokes(&mut loose);
    // Thrusters under the right pod and over the left one spin it
    // anticlockwise.
    put(&mut loose, PieceKind::Thruster, 3.5, -0.25, 6);
    put(&mut loose, PieceKind::Thruster, -2.5, 1.25, 0);
    put(&mut loose, PieceKind::Thruster, 2.5, -0.25, 6);
    put(&mut loose, PieceKind::Thruster, -1.5, 1.25, 0);
    assert_eq!(try_it(&mut loose), Status::Failed(Fail::PodLost));

    let mut slow = job(4);
    spokes(&mut slow);
    bolt_all(&mut slow);
    put(&mut slow, PieceKind::Thruster, 3.5, -0.25, 6);
    assert_eq!(try_it(&mut slow), Status::Failed(Fail::TooLight));
    put(&mut slow, PieceKind::Thruster, -2.5, 1.25, 0);
    put(&mut slow, PieceKind::Thruster, 2.5, -0.25, 6);
    put(&mut slow, PieceKind::Thruster, -1.5, 1.25, 0);
    let verdict = try_it(&mut slow);
    assert_eq!(verdict, Status::Won);
}

/// An arm of `count` squares from the base along row `y`.
fn arm(game: &mut Game, y: f64, count: usize, braced: bool) {
    for i in 0..count {
        square(game, 0.5 + i as f64, y, braced);
    }
}

#[test]
fn job_5_the_ship_needs_arms_long_enough_and_strong_enough() {
    let mut short = job(5);
    arm(&mut short, 1.5, 2, true);
    arm(&mut short, -0.5, 2, true);
    assert_eq!(try_it(&mut short), Status::Failed(Fail::NoMooring));

    let mut weak = job(5);
    arm(&mut weak, 1.5, 3, false);
    arm(&mut weak, -0.5, 3, false);
    assert_eq!(try_it(&mut weak), Status::Failed(Fail::Broke));

    let mut good = job(5);
    arm(&mut good, 1.5, 3, true);
    arm(&mut good, -0.5, 3, true);
    bolt_all(&mut good);
    assert_eq!(try_it(&mut good), Status::Won);

    // A piece in the berth: the ship runs into it and can't get in.
    let mut blocked = job(5);
    arm(&mut blocked, 1.5, 3, true);
    arm(&mut blocked, -0.5, 3, true);
    bolt_all(&mut blocked);
    put(&mut blocked, PieceKind::Square, 0.5, 0.5, 0);
    assert_eq!(try_it(&mut blocked), Status::Failed(Fail::NoRoom));
}

#[test]
fn the_two_ring_route_needs_its_off_centre_push() {
    let level = level::job(6);
    let mut plan = level.solution.clone();
    for input in &mut plan {
        if let Input::Place {
            kind: PieceKind::Panel,
            y,
            ..
        } = input
        {
            *y = -500;
        }
    }
    let wrong = crate::bot::play_plan(&level, &plan);
    assert_eq!(wrong.status, Status::Failed(Fail::Missed));
    assert_eq!(
        crate::bot::play_plan(&level, &level.solution).status,
        Status::Won
    );
}

#[test]
fn cargo_at_the_middle_spins_more_easily_than_cargo_at_the_ends() {
    let level = level::job(7);
    let mut plan = level.solution.clone();
    for input in &mut plan {
        if let Input::Place { kind, x, y, .. } = input {
            if *kind == PieceKind::Cargo {
                *x = if *y > 500 { 4500 } else { -3500 };
                *y = 500;
            } else if *kind == PieceKind::Bolt && *x == 500 && (*y == 0 || *y == 1000) {
                *x = if *y == 1000 { 4000 } else { -3000 };
                *y = 500;
            }
        }
    }
    let wrong = crate::bot::play_plan(&level, &plan);
    assert_eq!(wrong.status, Status::Failed(Fail::TooLight));
    assert_eq!(
        crate::bot::play_plan(&level, &level.solution).status,
        Status::Won
    );
}

#[test]
fn a_ring_must_close_around_the_hub() {
    let level = level::job(9);
    let plan: Vec<_> = level
        .solution
        .iter()
        .copied()
        .filter(|i| {
            !matches!(
                i,
                Input::Place {
                    kind: PieceKind::Square,
                    x: 1500,
                    y: 500,
                    ..
                }
            )
        })
        .collect();
    let open = crate::bot::play_plan(&level, &plan);
    assert_eq!(open.status, Status::Failed(Fail::OpenRing));
}

#[test]
fn a_hinge_really_turns_and_soft_joins_hold_when_plain_magnets_cannot() {
    for (n, tool, why) in [
        (16, PieceKind::Hinge, Fail::DoorStuck),
        (17, PieceKind::Spring, Fail::CameApart),
        (18, PieceKind::Tether, Fail::CameApart),
    ] {
        let mut level = level::job(n);
        level.required.clear();
        let plan: Vec<_> = level
            .solution
            .iter()
            .copied()
            .filter(|i| !matches!(i,Input::Place {kind,..}if *kind==tool))
            .collect();
        let wrong = crate::bot::play_plan(&level, &plan);
        assert_eq!(wrong.status, Status::Failed(why), "job {n}");
        let right = crate::bot::play_plan(&level, &level.solution);
        assert_eq!(right.status, Status::Won);
        assert!(
            right
                .world
                .as_ref()
                .unwrap()
                .joints
                .iter()
                .all(|j| !j.broken)
        );
    }
}

#[test]
fn a_reaction_wheel_stops_the_hull_while_the_wheel_keeps_spinning() {
    let level = level::job(11);
    let right = crate::bot::play_plan(&level, &level.solution);
    let w = right.world.as_ref().unwrap();
    let hull = right.build.tagged('H').unwrap();
    let wheel = w.kinds.iter().position(|k| *k == PieceKind::Wheel).unwrap();
    assert!(w.motion(&[hull]).2.abs() < 0.05);
    assert!(w.motion(&[wheel]).2.abs() > 1.);
    let mut unpowered = level.clone();
    unpowered.burns = vec![0];
    let wrong = crate::bot::play_plan(&unpowered, &unpowered.solution);
    assert_eq!(wrong.status, Status::Failed(Fail::StillSpinning));
}

#[test]
fn each_storm_waits_for_another_go_and_only_the_last_one_awards_stars() {
    let level = level::job(13);
    let mut game = Game::new(level.clone());
    for input in &level.solution {
        game.step(&[*input]);
    }
    while game.status == Status::Running {
        game.step(&[]);
    }
    assert_eq!(game.phase, 1);
    assert_eq!(game.status, Status::Building);
    assert_eq!(game.stars, 0);
    for _ in 0..120 {
        game.step(&[]);
    }
    assert_eq!(game.phase, 1);
    let (finished, receipt) = crate::bot::record_plan(&level, &level.solution);
    assert_eq!(finished.phase, 2);
    assert_eq!(finished.stars, 3);
    receipt.verify(&level).unwrap();
}
