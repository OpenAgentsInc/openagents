use super::scenes::Rig;
use super::*;
use crate::{
    controller::InputState,
    runtime::WorldRuntime,
    zones::{Intent, ZoneId, atmosphere},
};
use physics::trace::{Tolerance, Trace};

fn at_lab_portal() -> WorldRuntime {
    let mut runtime = WorldRuntime::new();
    runtime
        .set_spawn(glam::Vec3::new(0.0, 0.0, -25.0), 0.0)
        .unwrap();
    runtime
}

/// Seconds of simulated time at the fixed step.
fn run(lab: &mut Lab, seconds: f64) {
    for _ in 0..(seconds / DT).round() as usize {
        lab.step_once();
        assert!(lab.scene.finite(), "{:?} diverged", lab.kind());
    }
}

#[test]
fn the_lab_portal_enters_immediately_and_returns_to_the_plaza_pose() {
    let mut runtime = at_lab_portal();
    let pose = runtime.player;
    let snapshot = runtime.zone_snapshot(1.0);
    assert!(snapshot.portal.near);
    let enter = snapshot.controls.iter().find(|c| c.action == Intent::Enter);
    assert_eq!(enter.map(|c| c.label.as_str()), Some("Enter Lab"));
    runtime.zone_intent(Intent::Enter).unwrap();
    assert_eq!(runtime.zone, ZoneId::PhysicsLab);
    assert_eq!(runtime.zone_revision, 1);
    assert!(!runtime.zone_loading(), "no download for a generated zone");
    assert!(atmosphere(ZoneId::PhysicsLab).validate().is_ok());
    let inside = runtime.zone_snapshot(1.0);
    let lab = inside.lab.clone().expect("lab snapshot");
    assert_eq!(lab.number, 1);
    assert_eq!(lab.count, 9);
    assert_eq!(inside.controls.len(), 8);
    assert!(inside.caption.contains("Box-on-box manifold"));
    // The shared HUD fits the lab's two rows and four caption lines.
    let hud = crate::zones::hud::Hud::default().snapshot([800.0, 900.0], &inside, true);
    assert!(hud.visible);
    assert_eq!(hud.lines, 4);
    let rows: std::collections::BTreeSet<u32> =
        hud.buttons.iter().map(|b| b.frame[1] as u32).collect();
    assert_eq!(rows.len(), 2);
    // Walking and the lab clock both run.
    let start = runtime.player.pos;
    for _ in 0..20 {
        runtime.tick(
            &InputState {
                strafe_left: true,
                ..Default::default()
            },
            0.05,
        );
    }
    assert!(runtime.player.pos.distance(start) > 1.0);
    assert!(runtime.zone_snapshot(1.0).lab.unwrap().time > 0.9);
    assert!(!runtime.zone_dynamic_mesh().faces.is_empty());
    runtime.zone_intent(Intent::Return).unwrap();
    assert_eq!(runtime.zone, ZoneId::Plaza);
    assert_eq!(runtime.player.pos, pose.pos);
    assert_eq!(runtime.player.yaw, pose.yaw);
    assert!(runtime.zone_state.lab.is_none());
}

#[test]
fn lab_controls_are_scoped_to_the_lab() {
    let mut runtime = at_lab_portal();
    for intent in [Intent::Increase, Intent::Reset, Intent::Step] {
        assert!(runtime.zone_intent(intent).is_err());
    }
    runtime.zone_intent(Intent::Enter).unwrap();
    assert!(runtime.zone_intent(Intent::Grab).is_err());
    assert!(runtime.zone_intent(Intent::Fireball).is_err());
    assert!(runtime.zone_intent(Intent::Enter).is_err());
}

#[test]
fn every_scenario_builds_steps_and_draws_without_nans() {
    let mut lab = Lab::new();
    for kind in Kind::ALL {
        lab.select(kind);
        for gravity in [true, false] {
            lab.gravity = gravity;
            lab.reset();
            run(&mut lab, 4.0);
            let snapshot = lab.snapshot();
            assert_eq!(snapshot.scenario, kind);
            assert!(!snapshot.readout.is_empty(), "{kind:?}");
            let mesh = draw::scene(&lab.scene, 0.5, true);
            assert!(!mesh.faces.is_empty() && !mesh.lines.is_empty());
            assert!(
                mesh.faces.iter().chain(&mesh.lines).all(|v| v
                    .pos
                    .iter()
                    .chain(&v.color)
                    .all(|x| x.is_finite())),
                "{kind:?} drew a nonfinite vertex"
            );
        }
    }
}

#[test]
fn every_knob_option_builds_and_steps() {
    let mut lab = Lab::new();
    for kind in Kind::ALL {
        lab.select(kind);
        for (index, def) in kind.knobs().iter().enumerate() {
            for option in 0..def.options.len() {
                lab.set_option(index, option);
                lab.reset();
                run(&mut lab, 0.5);
            }
            lab.set_option(index, def.default);
        }
    }
}

/// The breakaway limit: a box holds under half the Coulomb limit and slips
/// past it.
fn friction_drift(load: f64) -> (f64, bool) {
    let mut lab = Lab::new();
    lab.select(Kind::Friction);
    assert!(lab.set("load", load));
    run(&mut lab, 2.5);
    let Rig::Friction {
        block,
        start: Some(start),
        ..
    } = &lab.scene.rig
    else {
        panic!("the load started");
    };
    let body = &lab.scene.world[*block];
    (
        body.pos.distance(*start),
        lab.scene.highlight(*block) == Some(true),
    )
}

#[test]
fn friction_holds_at_half_the_limit_and_slips_past_it() {
    let (drift, holding) = friction_drift(0.5);
    assert!(drift < 5e-3 && holding, "0.5 load drifted {drift} m");
    let (drift, holding) = friction_drift(1.2);
    assert!(drift > 0.2 && !holding, "1.2 load drifted only {drift} m");
}

#[test]
fn torsional_friction_holds_below_its_limit_and_slips_past_it() {
    let turned = |torque: f64| {
        let mut lab = Lab::new();
        lab.select(Kind::Torsion);
        lab.set("torque", torque);
        run(&mut lab, 3.0);
        let Rig::Torsion {
            ball,
            start: Some(start),
            ..
        } = &lab.scene.rig
        else {
            panic!("the torque started");
        };
        start.angle_between(lab.scene.world[*ball].orientation)
    };
    // The limit is 0.05 m × 1 kg × g ≈ 0.49 N·m.
    assert!(turned(0.45) < 5e-3, "held: {}", turned(0.45));
    assert!(turned(0.55) > 5e-2, "slips: {}", turned(0.55));
}

#[test]
fn projectiles_stop_at_the_panel_at_every_speed() {
    let mut lab = Lab::new();
    lab.select(Kind::Tunneling);
    for speed in [2.0, 40.0, 200.0] {
        lab.set("speed", speed);
        for _ in 0..(1.4 / DT) as usize {
            lab.step_once();
            let Rig::Tunneling { shots, .. } = &lab.scene.rig else {
                unreachable!()
            };
            for id in shots {
                assert!(lab.scene.world[*id].pos.x < 0.0, "{speed} m/s tunneled");
            }
        }
    }
}

#[test]
fn zero_g_collisions_keep_the_ledger_balanced_with_and_without_gravity() {
    let mut lab = Lab::new();
    lab.select(Kind::Momentum);
    assert!(!lab.gravity, "the scenario starts in zero g");
    for gravity in [false, true] {
        lab.gravity = gravity;
        lab.reset();
        let mut touched = false;
        for _ in 0..(3.0 / DT) as usize {
            lab.step_once();
            touched |= !lab.scene.world.contacts.is_empty();
            let Rig::Momentum { error, .. } = &lab.scene.rig else {
                unreachable!()
            };
            assert!(error.linear < 1e-9 && error.angular < 1e-9, "{error:?}");
        }
        assert!(touched, "the bodies collided");
    }
}

#[test]
fn solver_iterations_and_count_change_the_stack() {
    let mut lab = Lab::new();
    lab.select(Kind::Stack);
    lab.set("count", 5.0);
    run(&mut lab, 4.0);
    let resting = lab.snapshot().readout[0].clone();
    assert!(resting.starts_with("0 awake · 5 asleep"), "{resting}");
    // Turning sleep off wakes the stack.
    lab.set("sleep", 0.0);
    run(&mut lab, 0.1);
    assert!(lab.scene.world.bodies().iter().all(|b| !b.sleeping));
    lab.set("count", 8.0);
    assert_eq!(
        lab.scene.world.bodies().len(),
        1 + 8,
        "a rebuild with eight"
    );
    lab.set("iterations", 4.0);
    run(&mut lab, 0.1);
    assert_eq!(lab.scene.world.solver.iterations, 4, "a live knob");
}

#[test]
fn a_soft_grip_settles_and_slips_past_its_force_limit() {
    let settle = |limit: f64| {
        let mut lab = Lab::new();
        lab.select(Kind::SoftGrip);
        lab.set("limit", limit);
        run(&mut lab, 1.9);
        let Rig::SoftGrip { joint, .. } = &lab.scene.rig else {
            unreachable!()
        };
        let joint = *lab.scene.world.joint(*joint).unwrap();
        let (a, b) = joint.anchors(&lab.scene.world);
        (a.distance(b), joint.saturated)
    };
    // A soft weld sags by its static deflection g / ω² at 2 Hz.
    let (gap, saturated) = settle(300.0);
    let sag = scenes::G / (std::f64::consts::TAU * 2.0).powi(2);
    assert!((gap - sag).abs() < 5e-3 && !saturated, "settled to {gap} m");
    // The part sleeps while the hand is still, then follows it when it
    // sways.
    let mut lab = Lab::new();
    lab.select(Kind::SoftGrip);
    run(&mut lab, 1.9);
    let Rig::SoftGrip { part, joint, .. } = &lab.scene.rig else {
        unreachable!()
    };
    let (part, joint) = (*part, *joint);
    assert!(lab.scene.world[part].sleeping);
    for _ in 0..(2.0 / DT) as usize {
        lab.step_once();
        let w = &lab.scene.world;
        let (a, b) = w.joint(joint).unwrap().anchors(w);
        assert!(a.distance(b) < 0.2, "the part followed the hand");
    }
    assert!(!lab.scene.world[part].sleeping);
    // 20 N cannot hold a 5 kg part against gravity.
    let (gap, saturated) = settle(20.0);
    assert!(gap > 0.3 && saturated, "slipped only {gap} m");
}

#[test]
fn the_tether_catches_and_the_weld_holds_under_impact() {
    let mut lab = Lab::new();
    lab.select(Kind::Tether);
    run(&mut lab, 4.0);
    let Rig::Tether {
        anchor,
        tether,
        length,
        weld,
        pair,
        ball,
        ..
    } = &lab.scene.rig
    else {
        unreachable!()
    };
    let w = &lab.scene.world;
    let end = w.joint(*tether).unwrap().anchors(w).1;
    assert!(end.distance(*anchor) <= length + 0.02, "the tether caught");
    let weld = w.joint(*weld).unwrap();
    let (a, b) = weld.anchors(w);
    assert!(a.distance(b) < 0.01 && weld.angle_error(w).length() < 0.02);
    assert!(w[*ball].vel.x > -5.0, "the ball struck");
    assert!(
        w[pair[0]].pos.x != 1.9 || w[pair[0]].vel.length() > 0.0,
        "the welded pair moved"
    );
}

#[test]
fn thrusters_recover_from_a_tumble_and_follow_a_command() {
    let mut lab = Lab::new();
    lab.select(Kind::Thrusters);
    lab.set("command", 0.0);
    run(&mut lab, 12.0);
    let error = |lab: &Lab| {
        let Rig::Thrusters { craft, target, .. } = &lab.scene.rig else {
            unreachable!()
        };
        lab.scene.world[*craft].pos.distance(target.0)
    };
    let Rig::Thrusters {
        craft, throttles, ..
    } = &lab.scene.rig
    else {
        unreachable!()
    };
    assert!(
        lab.scene.world[*craft].omega_world().length() < 0.05,
        "attitude held"
    );
    assert!(error(&lab) < 0.1, "position held: {}", error(&lab));
    let attitude = lab.scene.world[*craft]
        .orientation
        .angle_between(glam::DQuat::IDENTITY);
    assert!(
        attitude < 0.3f64.to_radians(),
        "attitude held: {attitude} rad"
    );
    assert!(throttles.iter().all(|u| (0.0..=1.0).contains(u)));
    lab.set("command", 1.0);
    run(&mut lab, 0.2);
    let firing = lab.snapshot().readout[0].clone();
    assert!(!firing.starts_with("0 of 24"), "{firing}");
    run(&mut lab, 8.0);
    assert!(error(&lab) < 0.1, "moved to the command: {}", error(&lab));
}

#[test]
fn reset_replays_bit_for_bit() {
    let mut lab = Lab::new();
    for kind in Kind::ALL {
        lab.select(kind);
        let mut first = Trace::default();
        for _ in 0..240 {
            lab.step_once();
            first.record(&lab.scene.world);
        }
        lab.reset();
        let mut second = Trace::default();
        for _ in 0..240 {
            lab.step_once();
            second.record(&lab.scene.world);
        }
        first
            .compare(&second, Tolerance::EXACT)
            .unwrap_or_else(|d| panic!("{kind:?} diverged: {d:?}"));
    }
}

#[test]
fn frame_rate_does_not_change_the_simulation() {
    let run_at = |fps: u32| {
        let mut lab = Lab::new();
        lab.select(Kind::Stack);
        for _ in 0..fps * 2 {
            lab.tick(1.0 / fps as f32);
        }
        (lab.scene.world.tick, lab.scene.world.bodies()[3].pos)
    };
    let (ticks_60, pos_60) = run_at(60);
    let (ticks_30, pos_30) = run_at(30);
    assert_eq!(ticks_60, ticks_30);
    assert_eq!(pos_60, pos_30);
}

#[test]
fn intents_walk_the_knobs_pause_and_step() {
    let mut runtime = at_lab_portal();
    runtime.zone_intent(Intent::Enter).unwrap();
    // The scenario knob comes first; + moves to the next scenario.
    runtime.zone_intent(Intent::Increase).unwrap();
    let lab = runtime.zone_snapshot(1.0).lab.unwrap();
    assert_eq!(lab.scenario, Kind::Friction);
    assert!(lab.gravity);
    runtime.zone_intent(Intent::Decrease).unwrap();
    runtime.zone_intent(Intent::Decrease).unwrap();
    assert_eq!(
        runtime.zone_snapshot(1.0).lab.unwrap().scenario,
        Kind::Thrusters,
        "the scenario list wraps"
    );
    // Gravity is the third knob.
    runtime.zone_intent(Intent::KnobNext).unwrap();
    runtime.zone_intent(Intent::KnobNext).unwrap();
    runtime.zone_intent(Intent::Increase).unwrap();
    let lab = runtime.zone_snapshot(1.0).lab.unwrap();
    assert!(lab.gravity);
    assert!(lab.knobs[2].selected);
    runtime.zone_intent(Intent::Pause).unwrap();
    let paused = runtime.zone_snapshot(1.0);
    assert!(paused.lab.as_ref().unwrap().paused);
    assert!(paused.controls.iter().any(|c| c.label == "Run"));
    let time = paused.lab.unwrap().time;
    runtime.tick(&InputState::default(), 0.1);
    assert_eq!(runtime.zone_snapshot(1.0).lab.unwrap().time, time);
    runtime.zone_intent(Intent::Step).unwrap();
    let stepped = runtime.zone_snapshot(1.0).lab.unwrap().time;
    assert!((stepped - time - DT).abs() < 1e-12);
    runtime.zone_intent(Intent::KnobPrev).unwrap();
    runtime.zone_intent(Intent::KnobPrev).unwrap();
    runtime.zone_intent(Intent::KnobPrev).unwrap();
    assert!(
        runtime
            .zone_snapshot(1.0)
            .lab
            .unwrap()
            .knobs
            .last()
            .unwrap()
            .selected,
        "knob selection wraps"
    );
    runtime.zone_intent(Intent::Reset).unwrap();
    assert_eq!(runtime.zone_snapshot(1.0).lab.unwrap().time, 0.0);
}

#[test]
fn intents_serialize_as_plain_strings_for_native_hosts() {
    for (intent, name) in [
        (Intent::KnobPrev, "knob_prev"),
        (Intent::KnobNext, "knob_next"),
        (Intent::Decrease, "decrease"),
        (Intent::Increase, "increase"),
        (Intent::Reset, "reset"),
        (Intent::Pause, "pause"),
        (Intent::Step, "step"),
    ] {
        assert_eq!(serde_json::to_value(intent).unwrap(), name);
    }
    assert_eq!(
        serde_json::to_value(ZoneId::PhysicsLab).unwrap(),
        "physics_lab"
    );
}
