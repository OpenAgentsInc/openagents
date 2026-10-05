//! The Physics Lab inside the world runtime: entering through its portal,
//! the controls it scopes, and the intents that walk its knobs.

use super::lab::*;
use crate::{
    controller::InputState,
    runtime::WorldRuntime,
    zones::{Intent, ZoneId, atmosphere},
};

fn at_lab_portal() -> WorldRuntime {
    let mut runtime = WorldRuntime::new();
    runtime
        .set_spawn(glam::Vec3::new(0.0, 0.0, -25.0), 0.0)
        .unwrap();
    runtime
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
