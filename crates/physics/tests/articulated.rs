use glam::{DQuat, DVec3};
use physics::{Body, BodyKind, Joint, JointKind, JointLimits, NoField, Servo, World};

fn hinge(
    servo: Option<Servo>,
    limits: Option<JointLimits>,
) -> (World, physics::JointId, physics::BodyId) {
    let mut world = World::new(1.0 / 240.0);
    let a = world.add(Body::new(1.0, DVec3::ONE, DVec3::ZERO).with_kind(BodyKind::Static));
    let b = world.add(Body::new(1.0, DVec3::splat(0.01), DVec3::ZERO));
    let id = world
        .try_add_joint(Joint::new(
            a,
            DVec3::ZERO,
            b,
            DVec3::ZERO,
            JointKind::Hinge {
                axis: DVec3::Z,
                relative: DQuat::IDENTITY,
                limits,
                servo,
            },
        ))
        .unwrap();
    (world, id, b)
}

fn servo(target: f64) -> Servo {
    Servo {
        target,
        kp: 4.0,
        kd: 0.4,
        max_effort: 2.4,
        max_speed: 2.0,
    }
}

#[test]
fn hinge_step_response_has_bounded_rise_overshoot_speed_and_effort() {
    let (mut world, id, body) = hinge(Some(servo(0.5)), None);
    let mut rise = None;
    let mut peak = 0.0_f64;
    for _ in 0..480 {
        world.step(&NoField);
        let joint = world.joint(id).unwrap();
        let angle = joint.coordinate(&world).unwrap();
        if angle >= 0.45 && rise.is_none() {
            rise = Some(world.time());
        }
        peak = peak.max(angle);
        assert!(joint.motor_effort.abs() <= 2.4 + 1e-9);
        assert!(world[body].omega.z.abs() <= 2.0 + 1e-9);
        assert!(joint.angle_error(&world).length() < 1e-9);
    }
    assert!(rise.unwrap() <= 0.5, "rise time {rise:?}");
    assert!(peak <= 0.525, "overshoot {peak}");
    assert!((world.joint(id).unwrap().coordinate(&world).unwrap() - 0.5).abs() < 1e-5);
}

#[test]
fn hinge_limits_hold_and_report_reaction_torque() {
    let (mut world, id, body) = hinge(
        None,
        Some(JointLimits {
            lower: -0.3,
            upper: 0.3,
        }),
    );
    let mut peak = 0.0_f64;
    for _ in 0..240 {
        world[body].apply_torque(DVec3::Z * 1.2);
        world.step(&NoField);
        peak = peak.max(world.joint(id).unwrap().coordinate(&world).unwrap());
    }
    let joint = world.joint(id).unwrap();
    assert!(peak <= 0.300001, "position {peak}");
    assert!(
        (joint.limit_effort - 1.2).abs() < 1e-6,
        "effort {}",
        joint.limit_effort
    );
    for _ in 0..480 {
        world[body].apply_torque(-DVec3::Z * 1.2);
        world.step(&NoField);
    }
    let joint = world.joint(id).unwrap();
    assert!((joint.coordinate(&world).unwrap() + 0.3).abs() < 1e-6);
    assert!((joint.limit_effort - 1.2).abs() < 1e-6);
}

#[test]
fn hinge_removes_swing_but_preserves_rotation_about_its_axis() {
    let (mut world, id, body) = hinge(None, None);
    world[body].orientation = DQuat::from_rotation_x(0.2) * DQuat::from_rotation_z(0.4);
    for _ in 0..240 {
        world.step(&NoField);
    }
    let joint = world.joint(id).unwrap();
    assert!(joint.angle_error(&world).length() < 1e-7);
    assert!((joint.coordinate(&world).unwrap() - 0.4).abs() < 0.005);
}

#[test]
fn slider_moves_on_axis_and_holds_attitude_under_side_load() {
    let mut world = World::new(1.0 / 240.0);
    let a = world.add(Body::new(1.0, DVec3::ONE, DVec3::ZERO).with_kind(BodyKind::Static));
    let b = world.add(Body::new(1.0, DVec3::splat(0.1), DVec3::ZERO));
    let id = world
        .try_add_joint(Joint::new(
            a,
            DVec3::ZERO,
            b,
            DVec3::ZERO,
            JointKind::Slider {
                axis: DVec3::X,
                relative: DQuat::IDENTITY,
                limits: Some(JointLimits {
                    lower: -0.4,
                    upper: 0.4,
                }),
                servo: Some(Servo {
                    target: 0.3,
                    kp: 100.0,
                    kd: 20.0,
                    max_effort: 50.0,
                    max_speed: 0.5,
                }),
            },
        ))
        .unwrap();
    for _ in 0..720 {
        world[b].apply_force(DVec3::Y * 2.0);
        world[b].apply_torque(DVec3::Z);
        world.step(&NoField);
        assert!(world[b].pos.y.abs() < 1e-8);
        assert!(world[b].vel.x.abs() <= 0.5 + 1e-8);
    }
    let joint = world.joint(id).unwrap();
    assert!((joint.coordinate(&world).unwrap() - 0.3).abs() < 1e-5);
    assert!(joint.angle_error(&world).length() < 1e-8);
}

#[test]
fn invalid_frames_limits_servos_and_inertia_are_refused() {
    let (mut world, id, body) = hinge(Some(servo(0.5)), None);
    let original = *world.joint(id).unwrap();
    for kind in [
        JointKind::Hinge {
            axis: DVec3::ZERO,
            relative: DQuat::IDENTITY,
            limits: None,
            servo: None,
        },
        JointKind::Slider {
            axis: DVec3::Z,
            relative: DQuat::from_xyzw(0.0, 0.0, 0.0, 2.0),
            limits: None,
            servo: None,
        },
        JointKind::Hinge {
            axis: DVec3::Z,
            relative: DQuat::IDENTITY,
            limits: Some(JointLimits {
                lower: 1.0,
                upper: 0.0,
            }),
            servo: None,
        },
        JointKind::Hinge {
            axis: DVec3::Z,
            relative: DQuat::IDENTITY,
            limits: None,
            servo: Some(Servo {
                kd: 0.0,
                ..servo(0.0)
            }),
        },
    ] {
        let mut joint = original;
        joint.kind = kind;
        assert!(world.try_add_joint(joint).is_err());
    }
    world[body].inertia = DVec3::new(1.0, 1.0, 3.0);
    assert!(world.try_add_joint(original).is_err());
    assert!(world.set_joint_target(id, f64::NAN).is_err());
}

#[test]
fn target_change_wakes_sleepers_and_saved_world_continues_exactly() {
    let (mut world, id, body) = hinge(Some(servo(0.0)), None);
    for _ in 0..240 {
        world.step(&NoField);
    }
    assert!(world[body].sleeping);
    world.set_joint_target(id, 0.4).unwrap();
    assert!(!world[body].sleeping);
    for _ in 0..50 {
        world.step(&NoField);
    }
    let bytes = serde_json::to_vec(&world).unwrap();
    let mut restored: World = serde_json::from_slice(&bytes).unwrap();
    for _ in 0..200 {
        world.step(&NoField);
        restored.step(&NoField);
    }
    assert_eq!(
        serde_json::to_value(&world).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    assert!((world.joint(id).unwrap().coordinate(&world).unwrap() - 0.4).abs() < 0.0001);
}

#[test]
fn neighboring_chain_links_do_not_collide_but_other_links_and_environment_do() {
    use physics::collision::Filter;
    let links: Vec<_> = (0..6).map(|i| Filter::chain_link(i, 6).unwrap()).collect();
    for i in 0..6 {
        for j in 0..6 {
            assert_eq!(links[i].allows(links[j]), i.abs_diff(j) > 1);
        }
        assert!(links[i].allows(Filter {
            group: 1 << 31,
            mask: u32::MAX
        }));
    }
    assert!(Filter::chain_link(31, 32).is_none());
}

#[test]
fn hinge_internal_motor_preserves_angular_momentum() {
    let (mut world, id, body) = hinge(Some(servo(0.5)), None);
    let a = world.joint(id).unwrap().a;
    world[a].kind = BodyKind::Dynamic;
    let initial = world[a].angular_momentum() + world[body].angular_momentum();
    for _ in 0..240 {
        world.step(&NoField);
    }
    let result = world[a].angular_momentum() + world[body].angular_momentum();
    assert!((result - initial).length() < 1e-10);
}

#[test]
fn hinge_coordinate_uses_parent_and_zero_frames() {
    let (mut world, id, body) = hinge(None, None);
    let a = world.joint(id).unwrap().a;
    let parent = DQuat::from_rotation_z(0.7);
    let zero = DQuat::from_rotation_y(0.3);
    world[a].orientation = parent;
    world[body].orientation = parent * DQuat::from_rotation_x(0.4) * zero;
    world.joint_mut(id).unwrap().kind = JointKind::Hinge {
        axis: DVec3::X,
        relative: zero,
        limits: None,
        servo: None,
    };
    let joint = world.joint(id).unwrap();
    assert!((joint.coordinate(&world).unwrap() - 0.4).abs() < 1e-12);
    assert!(joint.angle_error(&world).length() < 1e-12);
}

#[test]
fn saturated_motor_cannot_exceed_its_torque_budget() {
    let (mut world, id, body) = hinge(
        Some(Servo {
            max_effort: 0.01,
            ..servo(0.5)
        }),
        None,
    );
    world.step(&NoField);
    let joint = world.joint(id).unwrap();
    assert!(joint.saturated);
    assert!((joint.motor_effort - 0.01).abs() < 1e-10);
    assert!(world[body].omega.z > 0.0 && world[body].omega.z < 0.01);
}

#[test]
fn slider_limit_reports_force_and_stops_outward_velocity() {
    let mut world = World::new(1.0 / 240.0);
    let a = world.add(Body::new(1.0, DVec3::ONE, DVec3::ZERO).with_kind(BodyKind::Static));
    let b = world.add(Body::new(1.0, DVec3::ONE, DVec3::ZERO));
    let id = world
        .try_add_joint(Joint::new(
            a,
            DVec3::ZERO,
            b,
            DVec3::ZERO,
            JointKind::Slider {
                axis: DVec3::X,
                relative: DQuat::IDENTITY,
                limits: Some(JointLimits {
                    lower: -0.1,
                    upper: 0.1,
                }),
                servo: None,
            },
        ))
        .unwrap();
    for _ in 0..240 {
        world[b].apply_force(DVec3::X * 3.0);
        world.step(&NoField);
    }
    let joint = world.joint(id).unwrap();
    assert!((joint.coordinate(&world).unwrap() - 0.1).abs() < 1e-9);
    assert!((joint.limit_effort - 3.0).abs() < 1e-9);
    assert!(world[b].vel.x.abs() < 1e-9);
}

#[derive(serde::Deserialize)]
struct ReferenceData {
    mujoco: String,
    timestep: f64,
    cases: Vec<ReferenceCase>,
}

#[derive(serde::Deserialize)]
struct ReferenceCase {
    name: String,
    model_xml: String,
    model_sha256: String,
    count: usize,
    kind: String,
    mass: f64,
    size: [f64; 3],
    kp: f64,
    kd: f64,
    effort: f64,
    speed: f64,
    gravity: [f64; 3],
    steps: Vec<ReferenceFrame>,
}

#[derive(serde::Deserialize)]
struct ReferenceFrame {
    actions: Vec<f64>,
    state: Vec<f64>,
}

#[test]
fn pinned_mujoco_episodes_qualify_hinge_slider_and_six_link_tracking() {
    use sha2::{Digest, Sha256};
    let reference: ReferenceData =
        serde_json::from_str(include_str!("../oracle/articulated-reference.json")).unwrap();
    assert_eq!(reference.mujoco, "3.3.7");
    for case in reference.cases {
        assert_eq!(
            format!("{:x}", Sha256::digest(case.model_xml.as_bytes())),
            case.model_sha256
        );
        let mut world = World::new(reference.timestep);
        world.sleep.enabled = false;
        world.solver.iterations = 80;
        let base = world.add(Body::new(1.0, DVec3::ONE, DVec3::ZERO).with_kind(BodyKind::Static));
        let size = DVec3::from_array(case.size);
        let mut parent = base;
        let mut joints = Vec::new();
        for i in 0..case.count {
            let chained = case.count > 1;
            let position = if chained {
                DVec3::X * size.x * (i as f64 + 0.5)
            } else {
                DVec3::ZERO
            };
            let body = world.add(Body::new(
                case.mass,
                Body::box_inertia(case.mass, size),
                position,
            ));
            let servo = Some(Servo {
                target: 0.0,
                kp: case.kp,
                kd: case.kd,
                max_effort: case.effort,
                max_speed: case.speed,
            });
            let limits = Some(JointLimits {
                lower: -1.0,
                upper: 1.0,
            });
            let kind = if case.kind == "hinge" {
                JointKind::Hinge {
                    axis: DVec3::Z,
                    relative: DQuat::IDENTITY,
                    limits,
                    servo,
                }
            } else {
                JointKind::Slider {
                    axis: DVec3::Z,
                    relative: DQuat::IDENTITY,
                    limits,
                    servo,
                }
            };
            let anchor_a = if chained && i > 0 {
                DVec3::X * size.x * 0.5
            } else {
                DVec3::ZERO
            };
            let anchor_b = if chained {
                -DVec3::X * size.x * 0.5
            } else {
                DVec3::ZERO
            };
            joints.push(
                world
                    .try_add_joint(Joint::new(parent, anchor_a, body, anchor_b, kind))
                    .unwrap(),
            );
            parent = body;
        }
        let mut worst_error = 0.0_f64;
        let mut worst_gap = 0.0_f64;
        for frame in &case.steps {
            assert_eq!(frame.actions.len(), joints.len());
            assert_eq!(frame.state.len(), joints.len());
            for (&joint, &target) in joints.iter().zip(&frame.actions) {
                world.set_joint_target(joint, target).unwrap();
            }
            world.step(&physics::Uniform(DVec3::from_array(case.gravity)));
            for (&id, &recorded) in joints.iter().zip(&frame.state) {
                let joint = world.joint(id).unwrap();
                worst_error = worst_error.max((joint.coordinate(&world).unwrap() - recorded).abs());
                if case.kind == "hinge" {
                    let (a, b) = joint.anchors(&world);
                    worst_gap = worst_gap.max(a.distance(b));
                }
                assert!(joint.motor_effort.abs() <= case.effort + 1e-9);
            }
        }
        let tolerance = if case.count > 1 { 0.025 } else { 0.02 };
        println!(
            "{}: maximum coordinate error {worst_error:.8}, anchor gap {worst_gap:.8} m",
            case.name
        );
        assert!(
            worst_error < tolerance,
            "{}: {worst_error} exceeds {tolerance}",
            case.name
        );
        assert!(worst_gap < 0.001, "{}: anchor gap {worst_gap}", case.name);
    }
}
