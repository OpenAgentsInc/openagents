//! What each frame draws, from the game: the build (or the running build,
//! leaning and coming apart as the simulation says), struts and bolts,
//! flames, the beacon, the cargo ship, the crew member who gave the job,
//! the solar storm's streaks, sparks, the piece in the player's hand, and
//! where it would snap. Native code, so the tests can check it without a
//! browser.

use glam::{Vec2, Vec3};
use snap_rules::game::{Game, Status};
use snap_rules::level::Job;
use snap_rules::{PieceKind, V};

use crate::kit::{self, Crew, Mood, Style};
use crate::mesh::{Mesh, rgb};

/// A piece held in the player's hand.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Held {
    pub kind: PieceKind,
    /// Where the pointer (or the keyboard cursor) holds it, in metres.
    pub at: V,
    pub turn: u8,
}

/// A short-lived spark: where, which way, its colour, and its age from 0
/// to 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spark {
    pub at: Vec3,
    pub velocity: Vec3,
    pub colour: u32,
    pub age: f32,
}

/// Everything a frame needs.
pub struct Frame<'a> {
    pub game: &'a Game,
    pub time: f32,
    pub held: Option<Held>,
    pub contrast: bool,
    pub marks: bool,
    pub sparks: &'a [Spark],
}

/// A frame's meshes: lit, glowing (unlit), and see-through (held pieces,
/// snap previews and the ship's outline while building).
#[derive(Default)]
pub struct Drawn {
    pub lit: Mesh,
    pub glow: Mesh,
    pub ghost: Mesh,
}

fn v2(p: V) -> Vec2 {
    Vec2::new(p.x as f32, p.y as f32)
}

/// Each piece's corners now: from the running world if there is one.
#[must_use]
pub fn corners(game: &Game) -> Vec<Vec<Vec2>> {
    match &game.world {
        Some(world) => (0..game.build.pieces.len())
            .map(|piece| world.nodes(piece).map(|i| v2(world.pos[i])).collect())
            .collect(),
        None => game
            .build
            .pieces
            .iter()
            .map(|p| p.at.iter().copied().map(v2).collect())
            .collect(),
    }
}

/// The crew member who gave the job.
#[must_use]
pub fn crew(game: &Game) -> Crew {
    Crew::from_word(game.level.crew)
}

/// Where the crew member floats: above and left of the job's own pieces.
#[must_use]
pub fn crew_spot(game: &Game) -> Vec2 {
    let mut low = Vec2::splat(f32::MAX);
    let mut high = Vec2::splat(f32::MIN);
    for piece in game.build.pieces.iter().filter(|p| p.given) {
        for p in &piece.at {
            low = low.min(v2(*p));
            high = high.max(v2(*p));
        }
    }
    if low.x > high.x {
        return Vec2::ZERO;
    }
    Vec2::new(low.x - 0.9, high.y + 1.3)
}

fn glow_rods(mesh: &mut Mesh, outline: &[Vec2], colour: [f32; 3], z: f32) {
    let n = outline.len();
    for i in 0..n {
        mesh.rod(outline[i], outline[(i + 1) % n], 0.07, z, z + 0.05, colour);
    }
}

/// Draws a frame.
#[must_use]
pub fn draw(frame: &Frame) -> Drawn {
    let game = frame.game;
    let mut out = Drawn::default();
    let all = corners(game);
    let world = game.world.as_ref();
    let base = Style {
        contrast: frame.contrast,
        marks: frame.marks,
        ..Style::default()
    };
    for (index, piece) in game.build.pieces.iter().enumerate() {
        let at = &all[index];
        let broken = world.is_some_and(|w| w.popped[index]);
        let style = Style { broken, ..base };
        kit::tile(&mut out.lit, piece.kind, at, style);
        if let Some(diagonal) = piece.strut {
            let (a, b) = if diagonal == 0 { (0, 2) } else { (1, 3) };
            kit::strut(&mut out.lit, at[a], at[b], style);
        }
    }
    for (index, seam) in game.build.seams.iter().enumerate() {
        let broken = world.is_some_and(|w| w.joints[index].broken);
        if !broken && seam.coupling != snap_rules::mechanism::Coupling::Magnet {
            use snap_rules::mechanism::Coupling;
            let a = &all[seam.a];
            let b = &all[seam.b];
            let midpoint = a[seam.a_side].lerp(a[(seam.a_side + 1) % a.len()], 0.5);
            if matches!(seam.coupling, Coupling::Wheel | Coupling::CargoArm) {
                let at = if seam.coupling == Coupling::Wheel {
                    let p = if game.build.pieces[seam.a].kind == PieceKind::Wheel {
                        seam.a
                    } else {
                        seam.b
                    };
                    kit::middle(&all[p])
                } else {
                    midpoint
                };
                out.lit.rod(midpoint, at, 0.09, 0.2, 0.3, rgb(0xC8CDD6));
                out.lit
                    .annulus(at, 0.07, 0.14, 12, 0.25, 0.35, rgb(0xC8CDD6));
            } else {
                let kind = match seam.coupling {
                    Coupling::Hinge => PieceKind::Hinge,
                    Coupling::Spring => PieceKind::Spring,
                    Coupling::Tether => PieceKind::Tether,
                    _ => unreachable!(),
                };
                kit::connector(
                    &mut out.lit,
                    kind,
                    midpoint,
                    b[seam.b_side].lerp(b[(seam.b_side + 1) % b.len()], 0.5),
                    base,
                );
            }
        }
        if !seam.bolted || broken {
            continue;
        }
        let piece = &all[seam.a];
        let a = piece[seam.a_side];
        let b = piece[(seam.a_side + 1) % piece.len()];
        kit::bolt(&mut out.lit, a.lerp(b, 0.5), base);
    }
    // Flames while thrusters burn.
    if let Some(world) = world {
        let throttle = world.throttle(&game.level) as f32;
        if throttle > 0.0 {
            for (index, piece) in game.build.pieces.iter().enumerate() {
                if piece.kind == PieceKind::Thruster {
                    let flicker = (frame.time * 31.0 + index as f32 * 1.7).sin() * 0.5 + 0.5;
                    kit::flame(&mut out.glow, &all[index], throttle, flicker);
                }
            }
        }
        // The storm's streaks, blowing the way the wind blows.
        let wind = world.wind(&game.level) as f32;
        if wind != 0.0 {
            storm(&mut out.glow, game, wind, frame.time);
        }
    }
    if matches!(game.level.job, Job::Push(_) | Job::Tow(_)) {
        for (i, (beacon, r)) in game.level.beacons.iter().enumerate() {
            let mesh = if i == game.waypoint {
                &mut out.glow
            } else {
                &mut out.ghost
            };
            kit::beacon(mesh, v2(*beacon), *r as f32, frame.time);
        }
    }
    if let Some(ship) = game.level.ship {
        match world.and_then(|w| w.ship_box(&game.level)) {
            Some((low, high)) => {
                let moving = world.is_some_and(|w| w.ship.is_some_and(|s| !s.parked && !s.blocked));
                kit::ship(
                    &mut out.lit,
                    &mut out.glow,
                    v2(low),
                    v2(high),
                    if moving { 1.0 } else { 0.0 },
                );
            }
            None => {
                // While building, the ship's outline waits in its berth, so
                // the berth's size is plain to see.
                let low = Vec2::new(ship.berth as f32, (ship.y - ship.height / 2.0) as f32);
                let high = low + Vec2::new(ship.length as f32, ship.height as f32);
                let mut unused = Mesh::new();
                kit::ship(&mut out.ghost, &mut unused, low, high, 0.0);
            }
        }
    }
    let mood = match game.status {
        Status::Won => Mood::Cheering,
        Status::Failed(_) => Mood::Thinking,
        _ => Mood::Waiting,
    };
    kit::robot(&mut out.lit, crew(game), crew_spot(game), frame.time, mood);
    for spark in frame.sparks {
        let size = 0.08 * (1.0 - spark.age).max(0.1);
        out.glow.disc(
            Vec2::new(spark.at.x, spark.at.y),
            size,
            4,
            spark.age * 3.0,
            spark.at.z,
            spark.at.z + 0.02,
            rgb(spark.colour),
        );
    }
    if let Some(held) = frame.held
        && game.status == Status::Building
    {
        hand(&mut out, game, held, frame.time, base);
    }
    out
}

/// The held piece at the pointer and, where it would go, a glowing
/// outline.
fn hand(out: &mut Drawn, game: &Game, held: Held, time: f32, base: Style) {
    let pulse = (time * 6.0).sin() * 0.2 + 0.8;
    let glow = [0.55 * pulse + 0.45, 1.0 * pulse, 0.85 * pulse];
    let lifted = Style { lift: 0.6, ..base };
    match held.kind {
        PieceKind::Strut => {
            let dir = Vec2::new(
                snap_rules::COS[usize::from(held.turn % 12)] as f32,
                snap_rules::SIN[usize::from(held.turn % 12)] as f32,
            ) * 0.7;
            let at = v2(held.at);
            kit::strut(&mut out.ghost, at - dir, at + dir, lifted);
            if let Some((square, diagonal)) = game.build.strut_target(held.at, held.turn) {
                let c = &game.build.pieces[square].at;
                let (a, b) = if diagonal == 0 {
                    (c[0], c[2])
                } else {
                    (c[1], c[3])
                };
                out.glow.rod(v2(a), v2(b), 0.1, 0.3, 0.36, glow);
            }
        }
        kind @ (PieceKind::Hinge | PieceKind::Spring | PieceKind::Tether) => {
            kit::connector(
                &mut out.ghost,
                kind,
                v2(held.at) - Vec2::X * 0.2,
                v2(held.at) + Vec2::X * 0.2,
                lifted,
            );
            if let Some(seam) = game.build.link_target(held.at) {
                let at = v2(game.build.seams[seam].middle(&game.build));
                out.glow.annulus(at, 0.16, 0.25, 16, 0.3, 0.34, glow);
            }
        }
        PieceKind::Bolt => {
            kit::bolt(&mut out.ghost, v2(held.at), lifted);
            if let Some(seam) = game.build.bolt_target(held.at) {
                let at = v2(game.build.seams[seam].middle(&game.build));
                out.glow.annulus(at, 0.15, 0.22, 16, 0.3, 0.34, glow);
            }
        }
        kind => {
            let corners: Vec<Vec2> = snap_rules::build::Build::held(kind, held.at, held.turn)
                .into_iter()
                .map(v2)
                .collect();
            kit::tile(&mut out.ghost, kind, &corners, lifted);
            if let Some(snapped) = game.build.candidate(kind, held.at, held.turn) {
                let snapped: Vec<Vec2> = snapped.into_iter().map(v2).collect();
                glow_rods(&mut out.glow, &snapped, glow, 0.25);
            }
        }
    }
}

/// Streaks of the solar wind over the job area.
fn storm(mesh: &mut Mesh, game: &Game, wind: f32, time: f32) {
    let (low, high) = game.level.area;
    let (w, h) = ((high.x - low.x) as f32, (high.y - low.y) as f32);
    let colour = rgb(0xFFE3A3);
    for i in 0..40 {
        let fx = ((i as f32 * 0.618_034).fract() * w) + low.x as f32;
        let speed = 6.0 + (i % 5) as f32;
        let phase = ((time * speed * wind.signum() / h) + i as f32 * 0.37).rem_euclid(1.0);
        let fy = low.y as f32 + phase * h;
        let len = 0.3 + wind.abs() * 0.05;
        mesh.rod(
            Vec2::new(fx, fy),
            Vec2::new(fx, fy + len * wind.signum()),
            0.03,
            -0.3,
            -0.28,
            colour,
        );
    }
}

/// The view the camera frames: the build, the job's places, and some room
/// around them, as lowest and highest corners.
#[must_use]
pub fn bounds(game: &Game) -> (Vec2, Vec2) {
    let mut low = Vec2::splat(f32::MAX);
    let mut high = Vec2::splat(f32::MIN);
    let mut take = |p: Vec2| {
        low = low.min(p);
        high = high.max(p);
    };
    for piece in corners(game) {
        for p in piece {
            take(p);
        }
    }
    for (beacon, r) in game
        .level
        .beacons
        .iter()
        .copied()
        .filter(|_| matches!(game.level.job, Job::Push(_) | Job::Tow(_)))
    {
        let r = r as f32 + 0.5;
        take(v2(beacon) - Vec2::splat(r));
        take(v2(beacon) + Vec2::splat(r));
    }
    if let Some(ship) = game.level.ship {
        take(Vec2::new(ship.berth as f32, (ship.y - ship.height) as f32));
        take(Vec2::new(
            (ship.berth + ship.length) as f32 + 0.5,
            (ship.y + ship.height) as f32,
        ));
    }
    if matches!(
        game.level.job,
        Job::Connect(..) | Job::Spin(_) | Job::Dock(_) | Job::Stop(_) | Job::Swing(_) | Job::Tow(_)
    ) && game.status == Status::Building
    {
        // Room to build in, while building.
        let (a, b) = game.level.area;
        let (a, b) = (v2(a), v2(b));
        let mid = (a + b) / 2.0;
        take(mid + (a - mid) * 0.55);
        take(mid + (b - mid) * 0.55);
    }
    take(crew_spot(game) + Vec2::new(-0.6, 0.6));
    let middle = (low + high) * 0.5;
    let size = (high - low + Vec2::splat(1.2)).max(Vec2::new(4.2, 3.2));
    (middle - size * 0.5, middle + size * 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;
    use snap_rules::game::Input;
    use snap_rules::level;

    #[test]
    fn every_job_draws_within_budget_while_building_and_running() {
        for number in 1..=level::COUNT {
            let mut game = Game::new(level::job(number));
            let plan = snap_rules::bot::solve(&game.level).expect("a plan");
            for input in &plan[..plan.len() - 1] {
                game.step(&[*input]);
            }
            let held = Some(Held {
                kind: PieceKind::Square,
                at: V::new(0.0, 0.0),
                turn: 0,
            });
            let building = draw(&Frame {
                game: &game,
                time: 1.0,
                held,
                contrast: false,
                marks: false,
                sparks: &[],
            });
            game.step(&[Input::Go]);
            for _ in 0..30 {
                game.step(&[]);
            }
            let running = draw(&Frame {
                game: &game,
                time: 1.5,
                held: None,
                contrast: true,
                marks: true,
                sparks: &[],
            });
            for drawn in [&building, &running] {
                let total =
                    drawn.lit.triangles() + drawn.glow.triangles() + drawn.ghost.triangles();
                assert!(total < 40_000, "job {number}: {total} triangles");
                assert!(drawn.lit.triangles() > 0);
            }
            let (low, high) = bounds(&game);
            assert!(high.x - low.x > 4.0 && high.y - low.y > 3.0, "job {number}");
        }
    }

    #[test]
    fn a_held_piece_shows_where_it_would_snap() {
        let game = Game::new(level::job(1));
        let near = draw(&Frame {
            game: &game,
            time: 0.0,
            held: Some(Held {
                kind: PieceKind::Square,
                at: V::new(1.6, 0.5),
                turn: 0,
            }),
            contrast: false,
            marks: false,
            sparks: &[],
        });
        let far = draw(&Frame {
            game: &game,
            time: 0.0,
            held: Some(Held {
                kind: PieceKind::Square,
                at: V::new(2.5, 3.0),
                turn: 0,
            }),
            contrast: false,
            marks: false,
            sparks: &[],
        });
        assert!(
            near.glow.triangles() > far.glow.triangles(),
            "an outline where it snaps"
        );
        assert!(near.ghost.triangles() > 0 && far.ghost.triangles() > 0);
    }
}
