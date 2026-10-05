//! Deterministic delayed-delivery checks against the actual chamber authority.
use super::Local;
use crate::{
    Command, Controller, Intent,
    movement::{Baseline, frames::Frame},
    play::{Ability, Game},
};
use physics::queries::SceneSnapshot;
use std::collections::VecDeque;
use verse_engine::director::Scene;

fn game_at(spawn: Option<glam::Vec3>) -> Game {
    let mut scene = Scene::from_json(include_bytes!(
        "../../../../assets/verse/original/ritual.json"
    ))
    .unwrap();
    scene
        .actors
        .retain(|a| a.id == 1 || a.model == "adventurer");
    if let Some(spawn) = spawn {
        scene
            .actors
            .iter_mut()
            .find(|a| a.model == "adventurer")
            .unwrap()
            .position = spawn;
    }
    scene.cues.clear();
    scene.cut_at = 0.;
    scene.duration = 600.;
    let mut game = Game::new_in(scene, 700).unwrap();
    game.handoff_player(game.player_life(), Controller(9))
        .unwrap();
    game.tick(1. / 30., [0.; 2]).unwrap();
    game
}
struct Observation {
    due: u64,
    serial: u64,
    tick: u64,
    baseline: Baseline,
    geometry: SceneSnapshot,
}
enum Packet {
    Arrival(u64, Command<Ability>),
    Frames(Frame),
}
#[derive(Clone, Copy, Debug)]
enum Route {
    Flat,
    DiagonalJump,
    Wall,
    Stairs,
    MovingSupport,
}
fn measure(up: u64, down: u64, framed: bool) -> serde_json::Value {
    profile(up, down, framed, 4, false, Route::Flat)
}
fn profile(
    up: u64,
    down: u64,
    framed: bool,
    width: u32,
    jitter: bool,
    route: Route,
) -> serde_json::Value {
    let spawn = match route {
        Route::Wall => Some(glam::Vec3::new(20., 0., -7.)),
        Route::Stairs => Some(glam::Vec3::new(18., 0., -32.)),
        _ => None,
    };
    timed_profile(up, down, framed, width, jitter, route, spawn, false)
}
fn timed_profile(
    up: u64,
    down: u64,
    framed: bool,
    width: u32,
    jitter: bool,
    route: Route,
    spawn: Option<glam::Vec3>,
    bootstrap: bool,
) -> serde_json::Value {
    let mut game = game_at(spawn);
    let life = game.player_life();
    if framed {
        game.begin_movement_frames(Controller(9), life).unwrap();
    }
    let initial = game.movement_baseline(life).unwrap().unwrap();
    let initial_geometry = game.query_scene.snapshot(700).unwrap();
    let initial_tick = game.authority_tick;
    if bootstrap {
        for _ in 0..down / 4 {
            game.tick(1. / 30., [0.; 2]).unwrap();
        }
    }
    let support = game
        .movement_baseline(life)
        .unwrap()
        .unwrap()
        .character
        .support
        .unwrap();
    let mut local = Local::new(700);
    local
        .observe(initial, &initial_geometry, initial_tick, 1)
        .unwrap();
    let mut inputs: VecDeque<(u64, Packet)> = VecDeque::new();
    let mut acknowledgments: VecDeque<(u64, u64)> = VecDeque::new();
    let mut next_frame = local.physics_step();
    let mut frame_sequence = 0;
    let mut observations: VecDeque<Observation> = VecDeque::new();
    let mut corrections = vec![];
    let mut serial = 1;
    let mut token = 0;
    let mut control_tick = initial_tick;
    let mut refused = 0;
    let mut withheld_baselines = 0;
    let mut airborne = 0;
    let mut max_height = 0f32;
    let initial_position = local.pose().unwrap().position;
    let mut maximum_distance = 0f32;
    for wall in 0..=720 + down + 4 {
        if wall > 0 {
            local.advance(1. / 120.).unwrap();
        }
        if wall > 0 && wall % 4 == 0 {
            if matches!(route, Route::MovingSupport) {
                game.query_scene
                    .set_pose(
                        support,
                        physics::queries::Pose {
                            position: glam::DVec3::new(0.4 * (wall as f64 / 120.).sin(), 0., 0.),
                            rotation: glam::DQuat::IDENTITY,
                        },
                    )
                    .unwrap();
            }
            game.tick(1. / 30., [0.; 2]).unwrap();
        }
        if wall <= 720 && wall % 6 == 0 {
            if let Some(baseline) = game.movement_baseline(life).unwrap() {
                serial += 1;
                observations.push_back(Observation {
                    due: (wall + down + if jitter { (serial % 3) * 2 } else { 0 })
                        .max(observations.back().map_or(0, |o| o.due)),
                    serial,
                    tick: game.authority_tick,
                    baseline,
                    geometry: game.query_scene.snapshot(700).unwrap(),
                });
            } else {
                withheld_baselines += 1;
            }
        }
        while observations.front().is_some_and(|o| o.due <= wall) {
            let observation = observations.pop_front().unwrap();
            let before = local.pose().unwrap().position;
            control_tick = control_tick.max(observation.tick);
            local
                .observe(
                    observation.baseline,
                    &observation.geometry,
                    observation.tick,
                    observation.serial,
                )
                .unwrap();
            local.advance(0.).unwrap();
            corrections.push(f64::from(before.distance(local.pose().unwrap().position)));
        }
        while acknowledgments.front().is_some_and(|ack| ack.0 <= wall) {
            control_tick = control_tick.max(acknowledgments.pop_front().unwrap().1);
        }
        if framed && local.physics_step() >= next_frame + u64::from(width) {
            let mut frame = local.movement_frame(next_frame, width).unwrap();
            frame_sequence += 1;
            frame.sequence = frame_sequence;
            frame.tick = control_tick;
            local.bind_movement_frame(&frame).unwrap();
            next_frame = frame.end().unwrap();
            inputs.push_back((
                (wall + up + if jitter { frame_sequence % 3 * 2 } else { 0 })
                    .max(inputs.back().map_or(0, |i| i.0)),
                Packet::Frames(frame),
            ));
        }
        if wall % 4 == 0 && wall < 600 {
            token += 1;
            let flat_axes = match (wall / 80) % 4 {
                0 => [1., 0.],
                1 => [0., 0.],
                2 => [-1., 0.],
                _ => [0., 0.],
            };
            let axes = match route {
                Route::Flat | Route::MovingSupport => flat_axes,
                Route::DiagonalJump => [flat_axes[0], flat_axes[0]],
                Route::Wall => {
                    if wall < 160 {
                        [1., 0.]
                    } else {
                        [0.; 2]
                    }
                }
                Route::Stairs => {
                    if wall < 132 {
                        [0., 1.]
                    } else {
                        [0.; 2]
                    }
                }
            };
            let intent = Intent::Move {
                axes,
                yaw: if matches!(route, Route::Stairs) {
                    std::f32::consts::PI
                } else {
                    0.
                },
            };
            let command = Command {
                actor: life,
                epoch: game.player_admission(life.actor).unwrap().epoch(),
                sequence: token,
                tick: control_tick,
                intent: intent.clone(),
            };
            local.queue(token, intent).unwrap();
            if framed && matches!(route, Route::DiagonalJump) && wall == 40 {
                token += 1;
                local.queue(token, Intent::Jump).unwrap();
            }
            if !framed {
                local.bind(token, &command).unwrap();
                inputs.push_back((wall + up, Packet::Arrival(token, command)));
            }
        }
        while inputs.front().is_some_and(|i| i.0 <= wall) {
            let (_, packet) = inputs.pop_front().unwrap();
            let result = match packet {
                Packet::Arrival(token, command) => {
                    let result = game.submit(Controller(9), command);
                    if result.is_err() {
                        local.reject(token);
                    }
                    result
                }
                Packet::Frames(frame) => game.submit_movement_frame(Controller(9), frame),
            };
            if let Err(error) = result {
                refused += 1;
                if framed {
                    panic!("framed packet at {wall}: {error}");
                }
            }
            acknowledgments.push_back((wall + down, game.authority_tick));
        }
        let pose = local.pose().unwrap();
        airborne += u64::from(pose.airborne);
        max_height = max_height.max(pose.position.y);
        maximum_distance = maximum_distance.max(pose.position.distance(initial_position));
    }
    if matches!(route, Route::Wall) {
        assert!(game.actor_position(life.actor).unwrap().x <= 21.151);
    }
    if matches!(route, Route::Stairs) {
        assert!(max_height > 1.4, "height {max_height}");
    }
    if matches!(route, Route::DiagonalJump) {
        assert!(airborne > 0 && max_height > 0.5);
    }
    corrections.sort_by(f64::total_cmp);
    let percentile = |p: f64| corrections[(corrections.len() as f64 * p).ceil() as usize - 1];
    serde_json::json!({"route":format!("{route:?}"),"delayed_bootstrap":bootstrap,"frame_steps":width,"jitter_steps":if jitter {4} else {0},"airborne_steps":airborne,"max_height":max_height,"maximum_distance":maximum_distance,"profile":if framed {"frames"} else {"arrival"},"up_ms":up as f64/120.*1000.,"down_ms":down as f64/120.*1000.,"observations":corrections.len(),"withheld_baselines":withheld_baselines,"p50_m":percentile(0.5),"p95_m":percentile(0.95),"p99_m":percentile(0.99),"maximum_m":corrections.last().unwrap(),"refused":refused})
}
#[test]
fn delayed_input_profile_measures_authority_corrections() {
    let runs: Vec<_> = [(0, 0), (4, 4), (8, 8), (8, 12)]
        .into_iter()
        .map(|(up, down)| measure(up, down, false))
        .collect();
    eprintln!(
        "VERSE_V04_EVIDENCE {}",
        serde_json::to_string(&runs).unwrap()
    );
    assert_eq!(runs[0]["refused"], 0);
    assert!(runs[0]["p95_m"].as_f64().unwrap() < 0.00001);
}

#[test]
fn confirmed_intervals_reconcile_delayed_delivery() {
    let runs: Vec<_> = [(0, 0), (4, 4), (8, 8), (8, 12)]
        .into_iter()
        .map(|(up, down)| measure(up, down, true))
        .collect();
    eprintln!(
        "VERSE_V04_EVIDENCE {}",
        serde_json::to_string(&runs).unwrap()
    );
    for run in runs {
        assert_eq!(run["refused"], 0);
        assert!(run["p95_m"].as_f64().unwrap() < 0.1, "{run}");
    }
}

#[test]
fn intervals_cover_jitter_coalescing_jump_wall_and_stairs() {
    let runs: Vec<_> = [
        profile(4, 4, true, 4, true, Route::Flat),
        profile(8, 8, true, 12, false, Route::Flat),
        profile(8, 8, true, 4, false, Route::DiagonalJump),
        profile(8, 8, true, 4, false, Route::Wall),
        profile(8, 8, true, 4, false, Route::Stairs),
    ]
    .into();
    eprintln!(
        "VERSE_V04_EVIDENCE {}",
        serde_json::to_string(&runs).unwrap()
    );
    for run in runs {
        assert_eq!(run["refused"], 0);
        assert!(run["p95_m"].as_f64().unwrap() < 0.1, "{run}");
    }
}

#[test]
fn current_moving_support_geometry_reports_a_separate_distribution() {
    let run = profile(8, 12, true, 4, false, Route::MovingSupport);
    eprintln!(
        "VERSE_V04_EVIDENCE {}",
        serde_json::to_string(&vec![run.clone()]).unwrap()
    );
    assert_eq!(run["refused"], 0);
    assert!(run["maximum_m"].as_f64().unwrap().is_finite());
    assert!(run["p95_m"].as_f64().unwrap() < 0.1);
}

#[test]
fn initial_entry_snapshot_can_arrive_after_a_full_downlink_delay() {
    let runs: Vec<_> = [4, 12]
        .into_iter()
        .map(|width| timed_profile(8, 12, true, width, false, Route::Flat, None, true))
        .collect();
    eprintln!(
        "VERSE_V04_EVIDENCE {}",
        serde_json::to_string(&runs).unwrap()
    );
    for run in runs {
        assert_eq!(run["refused"], 0);
        assert!(run["p95_m"].as_f64().unwrap() < 0.1);
    }
}
