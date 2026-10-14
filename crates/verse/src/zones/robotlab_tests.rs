use super::*;
use crate::{controller::InputState, runtime::WorldRuntime};
#[test]
fn robot_lab_portal_and_controls_return_to_the_previous_plaza_pose() {
    let mut runtime = WorldRuntime::new();
    runtime
        .set_spawn(glam::Vec3::new(12., 0., -25.), 0.)
        .unwrap();
    let before = runtime.player;
    let enter = runtime
        .zone_snapshot(1.)
        .controls
        .into_iter()
        .find(|c| c.action == Intent::Enter)
        .unwrap();
    assert_eq!(enter.label, "Enter Robot Lab");
    runtime.zone_intent(Intent::Enter).unwrap();
    assert_eq!(runtime.zone, ZoneId::RobotLab);
    assert!(runtime.zone_snapshot(1.).robot_lab.is_some());
    assert!(!runtime.dynamic_mesh().lit.is_empty());
    for _ in 0..3 {
        runtime.zone_intent(Intent::KnobNext).unwrap();
    }
    let angle = runtime.zone_snapshot(1.).robot_lab.unwrap().joints[0].position;
    runtime.zone_intent(Intent::Increase).unwrap();
    assert_eq!(
        runtime.zone_snapshot(1.).robot_lab.unwrap().joints[0].position,
        angle + 0.05
    );
    runtime.tick(&InputState::default(), 0.01);
    runtime.zone_intent(Intent::Return).unwrap();
    assert_eq!(runtime.zone, ZoneId::Plaza);
    assert_eq!(runtime.player.pos, before.pos);
    assert!(runtime.zone_state.robot_lab.is_none());
    assert!(runtime.zone_intent(Intent::Increase).is_err());
    assert!(atmosphere(ZoneId::RobotLab).validate().is_ok());
    assert_eq!(ZoneId::from_name("robot-lab"), Some(ZoneId::RobotLab));
}
