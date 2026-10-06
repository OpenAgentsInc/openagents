use super::*;
use crate::{Intent, movement::frames::Segment};
use std::process::Command as Process;

fn execution() -> Execution {
    Execution::native(
        "linux-x86_64; isolated replay fixture; strict build equality".into(),
        hash(b"verse replay fixture v1; host 30Hz cap3"),
        hash(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        )),
    )
    .unwrap()
}
fn game() -> Game {
    let scene = verse_engine::director::Scene::from_json(include_bytes!(
        "../../../../assets/verse/original/ritual.json"
    ))
    .unwrap();
    let mut game = Game::combat_in(scene, false, 700).unwrap();
    game.time = game.scene.cut_at + 1.;
    game.handoff_player(game.player_life(), Controller(7))
        .unwrap();
    game
}
fn applied(outcome: Outcome) -> Value {
    match outcome {
        Outcome::Applied { value } => value,
        other => panic!("Expected applied operation: {other:?}"),
    }
}
fn command(recorder: &Recorder, actor: u64, intent: Intent<Ability>) -> Command<Ability> {
    recorder
        .game()
        .player_admission(actor)
        .unwrap()
        .command(recorder.game().authority_tick, intent)
        .unwrap()
}
fn reseal(trace: &mut Trace) {
    let mut previous = hash(&encode(&trace.header).unwrap());
    for entry in &mut trace.entries {
        entry.chain = chain(previous, entry).unwrap();
        previous = entry.chain;
    }
    trace.count = trace.entries.len();
    trace.seal = seal(previous, trace.count).unwrap();
}
fn fixture(profile: Execution) -> Trace {
    let mut recorder = Recorder::new(game(), profile).unwrap();
    let primary = recorder.game().player_actor();
    let spawn = recorder.game().actor_position(primary).unwrap() + glam::Vec3::X * 2.;
    let second: LifeId = serde_json::from_value(applied(
        recorder
            .apply(Operation::Join {
                controller: Controller(8),
                spawn: spawn.to_array(),
            })
            .unwrap(),
    ))
    .unwrap();
    let move_second = command(
        &recorder,
        second.actor,
        Intent::Move {
            axes: [0.4, 0.],
            yaw: 0.,
        },
    );
    applied(
        recorder
            .apply(Operation::Command {
                controller: Controller(8),
                command: move_second.clone(),
            })
            .unwrap(),
    );
    assert!(matches!(
        recorder
            .apply(Operation::Command {
                controller: Controller(8),
                command: move_second
            })
            .unwrap(),
        Outcome::Refused { .. }
    ));
    let target = recorder.game().actor_life(2).unwrap();
    for (actor, controller, ability) in [
        (primary, Controller(7), Ability::Fireball),
        (second.actor, Controller(8), Ability::MagicMissile),
    ] {
        let aim = (recorder.game().actor_position(target.actor).unwrap()
            - recorder.game().actor_position(actor).unwrap())
            * glam::Vec3::new(1., 0., 1.);
        let c = command(
            &recorder,
            actor,
            Intent::Cast {
                ability,
                target: Some(target),
                aim: aim.normalize().to_array(),
            },
        );
        applied(
            recorder
                .apply(Operation::Command {
                    controller,
                    command: c,
                })
                .unwrap(),
        );
    }
    let c = command(
        &recorder,
        primary,
        Intent::Cast {
            ability: Ability::Fireball,
            target: Some(target),
            aim: [0., 0., -1.],
        },
    );
    assert!(matches!(
        recorder
            .apply(Operation::Command {
                controller: Controller(7),
                command: c.clone()
            })
            .unwrap(),
        Outcome::Refused { .. }
    ));
    assert_eq!(
        recorder
            .game()
            .player_admission(primary)
            .unwrap()
            .accepted_sequence(),
        c.sequence
    );
    assert_eq!(
        recorder.trace.entries.last().unwrap().admission_consumed,
        Some(true)
    );
    applied(recorder.apply(Operation::Commit {}).unwrap());
    for _ in 0..5 {
        applied(
            recorder
                .apply(Operation::Elapsed { seconds: 1. / 30. })
                .unwrap(),
        );
    }
    applied(recorder.apply(Operation::Restore {}).unwrap());
    // Pending projectiles and held input return to the committed state.
    applied(recorder.apply(Operation::Elapsed { seconds: 1. }).unwrap());
    let stale = command(&recorder, second.actor, Intent::Jump);
    applied(
        recorder
            .apply(Operation::Handoff {
                life: second,
                controller: Controller(9),
            })
            .unwrap(),
    );
    assert!(matches!(
        recorder
            .apply(Operation::Command {
                controller: Controller(8),
                command: stale
            })
            .unwrap(),
        Outcome::Refused { .. }
    ));
    let jump = command(&recorder, second.actor, Intent::Jump);
    applied(
        recorder
            .apply(Operation::Command {
                controller: Controller(9),
                command: jump,
            })
            .unwrap(),
    );
    for _ in 0..60 {
        applied(
            recorder
                .apply(Operation::Elapsed { seconds: 1. / 30. })
                .unwrap(),
        );
    }
    assert!(recorder.game().snapshot().counters.casts > 0);
    applied(recorder.apply(Operation::Commit {}).unwrap());
    applied(recorder.apply(Operation::Shutdown {}).unwrap());
    let before = recorder.game().checkpoint().unwrap();
    assert!(
        recorder
            .apply(Operation::Elapsed { seconds: 1. / 30. })
            .is_err()
    );
    assert_eq!(recorder.game().checkpoint().unwrap(), before);
    recorder.finish().unwrap()
}

#[test]
fn replay_child() {
    let Ok(path) = std::env::var("VERSE_INPUT_REPLAY_CHILD") else {
        return;
    };
    let trace = Trace::read(Path::new(&path)).unwrap();
    let report = trace.replay(&execution()).unwrap();
    assert!(report.divergence.is_none(), "{:?}", report.divergence);
    assert_eq!(report.ticks, 68);
    assert_eq!(report.commit_revision, 3);
    assert!((report.dropped_seconds - 0.9).abs() < 1e-12);
    assert_eq!(
        trace.entries.last().unwrap().observations[0].world["world"]["admission"]["controller"],
        0
    );
    if let Ok(path) = std::env::var("VERSE_INPUT_REPLAY_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
}

#[test]
fn multiplayer_inputs_replay_across_commits_restore_and_process_shutdown() {
    let trace = fixture(execution());
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("input-replay.json");
    trace.write_new(&path).unwrap();
    assert!(trace.write_new(&path).is_err());
    let persisted = Trace::read(&path).unwrap();
    assert_eq!(trace.digest().unwrap(), persisted.digest().unwrap());
    let mut child = Process::new(std::env::current_exe().unwrap());
    child
        .args(["--exact", "replay::tests::replay_child", "--nocapture"])
        .env("VERSE_INPUT_REPLAY_CHILD", &path);
    if let Ok(output) = std::env::var("VERSE_INPUT_REPLAY_RECEIPT") {
        let output = Path::new(&output);
        std::fs::create_dir_all(output).unwrap();
        trace.write_new(&output.join("input-replay.json")).unwrap();
        std::fs::write(
            output.join("execution.json"),
            serde_json::to_vec_pretty(trace.execution()).unwrap(),
        )
        .unwrap();
        child.env(
            "VERSE_INPUT_REPLAY_REPORT",
            output.join("replay-report.json"),
        );
    }
    let output = child.output().unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn first_divergent_tick_and_field_include_each_catch_up_step() {
    let mut trace = fixture(execution());
    let entry = trace
        .entries
        .iter_mut()
        .find(|e| matches!(e.operation,Operation::Elapsed { seconds } if seconds == 1.))
        .unwrap();
    assert_eq!(entry.observations.len(), 3);
    let tick = entry.observations[1].tick;
    entry.observations[1].world["world"]["time"] = Value::from(-1.);
    let sequence = entry.sequence;
    reseal(&mut trace);
    let report = trace.replay(trace.execution()).unwrap();
    let d = report.divergence.unwrap();
    assert_eq!(d.sequence, sequence);
    assert_eq!(d.tick, tick);
    assert_eq!(d.field, "/observation/world/world/time");
}

#[test]
fn profiles_chains_partial_files_and_unclosed_segments_are_refused() {
    let trace = fixture(execution());
    let mut foreign = trace.execution().clone();
    foreign.executable[0] ^= 1;
    assert!(trace.replay(&foreign).is_err());
    for change in 0..8 {
        let mut foreign = trace.execution().clone();
        match change {
            0 => foreign.compiler.push('x'),
            1 => foreign.target.push('x'),
            2 => foreign.rng.push('x'),
            3 => foreign.content[0] ^= 1,
            4 => foreign.configuration[0] ^= 1,
            5 => foreign.build.push('x'),
            6 => foreign.rules.push('x'),
            _ => foreign.runtime.push('x'),
        }
        assert!(trace.replay(&foreign).is_err());
    }
    let mut truncated = trace.clone();
    truncated.entries.pop();
    assert!(Trace::from_json(&truncated.to_json().unwrap()).is_err());
    let bytes = trace.to_json().unwrap();
    assert!(Trace::from_json(&bytes[..bytes.len() / 2]).is_err());
    let mut altered = trace.clone();
    altered.entries[1].sequence += 1;
    assert!(altered.replay(trace.execution()).is_err());
    let mut altered = trace.clone();
    altered.header.initial_rng["primary"]["state"] = Value::from(42);
    assert!(altered.replay(trace.execution()).is_err());
    let mut open = Recorder::new(game(), execution()).unwrap();
    assert!(
        open.apply(Operation::Elapsed { seconds: f64::NAN })
            .is_err()
    );
    assert!(open.finish().is_err());
    let mut unknown: Value = serde_json::from_slice(&bytes).unwrap();
    unknown["execute"] = Value::from("private host command");
    assert!(Trace::from_json(&serde_json::to_vec(&unknown).unwrap()).is_err());
}

#[test]
fn interval_admission_refusals_rng_streams_and_recording_budgets_preserve_semantics() {
    let mut g = crate::playground::Run::new(crate::spells::scenarios::area(6))
        .unwrap()
        .game;
    g.spells.dice = crate::spells::Dice::new(g.spells.dice.seed);
    g.spells.dice.force_save(101, 2).unwrap();
    let mut r = Recorder::new(g, execution()).unwrap();
    let primary = r.game().player_life();
    applied(
        r.apply(Operation::Handoff {
            life: primary,
            controller: Controller(7),
        })
        .unwrap(),
    );
    applied(r.apply(Operation::Elapsed { seconds: 1. / 30. }).unwrap());
    let c = command(
        &r,
        primary.actor,
        Intent::Cast {
            ability: Ability::SpellCommand(crate::spells::command::Command::Point {
                slot: 6,
                point: [0, 0, 2000],
            }),
            target: None,
            aim: [0., 0., -1.],
        },
    );
    applied(
        r.apply(Operation::Command {
            controller: Controller(7),
            command: c,
        })
        .unwrap(),
    );
    assert!(r.game().spells.dice.rolled > 0);
    let spawn = r.game().actor_position(primary.actor).unwrap() + glam::Vec3::X * 2.;
    let second: LifeId = serde_json::from_value(applied(
        r.apply(Operation::Join {
            controller: Controller(8),
            spawn: spawn.to_array(),
        })
        .unwrap(),
    ))
    .unwrap();
    let c = command(
        &r,
        second.actor,
        Intent::Cast {
            ability: Ability::SpellCommand(crate::spells::command::Command::Point {
                slot: 6,
                point: [0, 0, 2000],
            }),
            target: None,
            aim: [0., 0., -1.],
        },
    );
    applied(
        r.apply(Operation::Command {
            controller: Controller(8),
            command: c,
        })
        .unwrap(),
    );
    assert!(r.game().spells.caster_dice[&second.actor].rolled > 0);
    applied(r.apply(Operation::Elapsed { seconds: 1. / 30. }).unwrap());
    let before = r.game().checkpoint().unwrap();
    let bytes = r.bytes;
    r.bytes = MAX_BYTES - SHUTDOWN_RESERVE - 1;
    assert!(
        r.apply(Operation::Handoff {
            life: primary,
            controller: Controller(9)
        })
        .is_err()
    );
    assert_eq!(r.game().checkpoint().unwrap(), before);
    r.bytes = bytes;
    assert!(matches!(
        r.apply(Operation::Elapsed { seconds: -1. }).unwrap(),
        Outcome::Refused { .. }
    ));
    applied(r.apply(Operation::Shutdown {}).unwrap());
    let trace = r.finish().unwrap();
    assert!(
        trace
            .replay(trace.execution())
            .unwrap()
            .divergence
            .is_none()
    );

    let mut r = Recorder::new(game(), execution()).unwrap();
    applied(r.apply(Operation::Elapsed { seconds: 1. / 30. }).unwrap());
    let life = r.game().player_life();
    applied(
        r.apply(Operation::BeginFrames {
            controller: Controller(7),
            life,
        })
        .unwrap(),
    );
    let a = r.game().player_admission(life.actor).unwrap();
    let start = r
        .game()
        .movement_baseline(life)
        .unwrap()
        .unwrap()
        .physics_step;
    let frame = Frame {
        life,
        epoch: a.epoch(),
        sequence: 1,
        tick: r.game().authority_tick,
        start,
        steps: 4,
        segments: vec![Segment {
            offset: 0,
            until: start + 12,
            axes: [0.2, 0.],
            yaw: 0.,
            jump: false,
        }],
    };
    applied(
        r.apply(Operation::Frame {
            controller: Controller(7),
            frame: frame.clone(),
        })
        .unwrap(),
    );
    assert!(matches!(
        r.apply(Operation::Frame {
            controller: Controller(7),
            frame
        })
        .unwrap(),
        Outcome::Refused { .. }
    ));
    applied(r.apply(Operation::Elapsed { seconds: 1. / 30. }).unwrap());
    applied(r.apply(Operation::Commit {}).unwrap());
    applied(r.apply(Operation::Restore {}).unwrap());
    applied(r.apply(Operation::Shutdown {}).unwrap());
    let trace = r.finish().unwrap();
    assert!(
        trace
            .replay(trace.execution())
            .unwrap()
            .divergence
            .is_none()
    );
}

#[test]
fn a_full_segment_reserves_its_shutdown_and_refuses_extra_effects() {
    let mut r = Recorder::new(game(), execution()).unwrap();
    for _ in 0..MAX_ENTRIES - 1 {
        applied(r.apply(Operation::Commit {}).unwrap());
    }
    let before = r.game().checkpoint().unwrap();
    assert!(r.apply(Operation::Elapsed { seconds: 1. / 30. }).is_err());
    assert_eq!(before, r.game().checkpoint().unwrap());
    applied(r.apply(Operation::Shutdown {}).unwrap());
    let trace = r.finish().unwrap();
    assert_eq!(trace.entries.len(), MAX_ENTRIES);
    assert!(
        trace
            .replay(trace.execution())
            .unwrap()
            .divergence
            .is_none()
    );
}
