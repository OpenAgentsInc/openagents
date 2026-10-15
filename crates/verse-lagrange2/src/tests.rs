//! Lagrange 2 tests, in the style of Lagrange 1's: each names what it
//! checks and where its threshold comes from
//! (docs/spacecraft-design/sim-mapping.md).

use glam::{DMat3, DVec3};
use physics::Momentum;

use crate::arm::{self, ArmSpec};
use crate::assembly::{Assembly, Failure, Phase, Plan, Status};
use crate::burn::Burn;
use crate::factory::{self, Factory, Holder};
use crate::ship::{Edit, PartKind, Spacecraft};

// --- The spacecraft document ---------------------------------------------

#[test]
fn the_sample_ship_passes_every_check() {
    let ship = Spacecraft::sample();
    assert_eq!(ship.check(), Vec::new());
    let report = ship.report();
    // About 98.4 t loaded (CAD's measured dry masses plus 44 t of
    // propellant), 105 t with contingency against a 115 t budget.
    assert!(
        (98_300.0..98_500.0).contains(&report.mass.mass),
        "{}",
        report.mass.mass
    );
    assert!(report.mass_margin > 0.05, "{}", report.mass_margin);
    // Two tanks of 22 t at Isp 330 s give about 1.9 km/s.
    assert!((1_850.0..1_950.0).contains(&report.delta_v_m_s));
    assert!(report.thrust_offset_m < 0.25);
    assert!(report.latch_loads.iter().all(|l| l.margin > 0.0));
    assert_eq!(
        ship.assembly_order().map(|o| o.len()),
        Some(ship.parts.len())
    );
}

#[test]
fn the_parts_mass_properties_come_from_cad() {
    use crate::cad_feed::{self, MassFeed};
    let feed = MassFeed::tug();
    let ship = Spacecraft::sample();
    // Every part is CAD's instance: its measured mass, center of mass, and
    // tensor, in ship axes, from the version the feed names.
    for part in &ship.parts {
        let measured = feed.part(&part.id).unwrap();
        let cad = part.cad.as_ref().unwrap();
        assert_eq!(cad.digest, feed.source.digest);
        assert!(cad.source.ends_with(&format!("#{}", part.id)));
        assert_eq!(part.dry_mass_kg, measured.mass.mass_kg);
        assert_eq!(
            part.position,
            cad_feed::to_ship(measured.mass.center_of_mass_m)
        );
    }
    // The dry ship's mass properties are the feed's own total: the
    // document's parallel-axis sum agrees with CAD's.
    let mut dry = ship.clone();
    for p in &mut dry.parts {
        if let PartKind::Tank { propellant_kg } = &mut p.kind {
            *propellant_kg = 0.0;
        }
    }
    let mp = dry.mass_properties();
    assert!((mp.mass - feed.total.mass_kg).abs() < 1e-9 * mp.mass);
    assert!((mp.com - cad_feed::to_ship(feed.total.center_of_mass_m)).length() < 1e-9);
    let total = cad_feed::tensor_to_ship(feed.total.inertia_kg_m2);
    let norm = total.x_axis.length();
    for (a, b) in mp.inertia.to_cols_array().iter().zip(total.to_cols_array()) {
        assert!((a - b).abs() < 1e-9 * norm, "{a} vs {b}");
    }
    // The CAD walls were sized for the old typed list's dry masses; the
    // measured ones land within 1%.
    for (id, kg) in [
        ("keel-aft", 7_000.0),
        ("engine", 9_000.0),
        ("habitat", 18_000.0),
    ] {
        let m = ship.part(id).unwrap().dry_mass_kg;
        assert!((m - kg).abs() < 0.01 * kg, "{id}: {m}");
    }
    // Every latch is a CAD joint's origin; the keels meet at the origin.
    let fwd = ship.part("keel-fwd").unwrap();
    assert_eq!(fwd.attach.parent.as_deref(), Some("keel-aft"));
    assert!(fwd.attach.at.length() < 1e-12);
}

#[test]
fn mass_properties_are_the_sum_of_parts() {
    // Two parts by hand: a 2 t keel box at the origin and a 1 t engine
    // cylinder 3 m forward and 1 m up, with their shapes' inertia.
    let mut ship = Spacecraft::sample();
    for p in &mut ship.parts {
        p.cad = None;
    }
    ship.parts.truncate(1);
    let mut second = ship.parts[0].clone();
    second.id = "engine".into();
    second.shape = Spacecraft::sample().parts[2].shape;
    second.dry_mass_kg = 1_000.0;
    second.position = DVec3::new(3.0, 1.0, 0.0);
    second.attach.parent = Some(ship.parts[0].id.clone());
    ship.parts[0].dry_mass_kg = 2_000.0;
    ship.parts[0].position = DVec3::ZERO;
    ship.parts.push(second);
    let mp = ship.mass_properties();
    assert!((mp.mass - 3_000.0).abs() < 1e-9);
    let com = DVec3::new(1.0, 1.0 / 3.0, 0.0);
    assert!((mp.com - com).length() < 1e-12);
    // Box 16 x 2.4 x 2.4 and cylinder r 2.2, L 6, by the textbook formulas.
    let box_i = DVec3::new(
        2.4 * 2.4 * 2.0,
        16.0 * 16.0 + 2.4 * 2.4,
        16.0 * 16.0 + 2.4 * 2.4,
    ) * (2_000.0 / 12.0);
    let across = 1_000.0 * (3.0 * 2.2 * 2.2 + 36.0) / 12.0;
    let cyl_i = DVec3::new(1_000.0 * 2.2 * 2.2 / 2.0, across, across);
    let shift = |m: f64, d: DVec3| {
        (DMat3::IDENTITY * d.length_squared() - DMat3::from_cols(d * d.x, d * d.y, d * d.z)) * m
    };
    let expected = DMat3::from_diagonal(box_i)
        + shift(2_000.0, -com)
        + DMat3::from_diagonal(cyl_i)
        + shift(1_000.0, DVec3::new(3.0, 1.0, 0.0) - com);
    for (a, b) in mp
        .inertia
        .to_cols_array()
        .iter()
        .zip(expected.to_cols_array())
    {
        assert!((a - b).abs() <= 1e-9 * b.abs().max(1.0), "{a} vs {b}");
    }
}

#[test]
fn a_negative_margin_is_refused() {
    let ship = Spacecraft::sample();
    let refused = ship
        .edit(&Edit::SetDryMass {
            part: "habitat".into(),
            kg: 30_000.0,
        })
        .unwrap_err();
    assert_eq!(refused.requirement, "mass budget");
    // A small change passes and bumps the revision and the version.
    let next = ship
        .edit(&Edit::SetDryMass {
            part: "habitat".into(),
            kg: 18_500.0,
        })
        .unwrap();
    assert_eq!(next.revision, ship.revision + 1);
    assert_ne!(next.version(), ship.version());
    assert_eq!(ship.version(), Spacecraft::sample().version());
}

#[test]
fn an_off_axis_engine_is_refused() {
    let mut ship = Spacecraft::sample();
    let engine = ship.parts.iter_mut().find(|p| p.id == "engine").unwrap();
    engine.position.z = 0.8;
    let findings = ship.check();
    assert!(
        findings.iter().any(|f| f.requirement == "thrust line"),
        "{findings:?}"
    );
    // The same through an edit, which names the requirement and leaves the
    // document as it was.
    let refused = Spacecraft::sample()
        .edit(&Edit::MovePart {
            part: "engine".into(),
            position: DVec3::new(-19.0, 0.0, 0.8),
        })
        .unwrap_err();
    assert_eq!(refused.requirement, "thrust line");
}

#[test]
fn latch_loads_follow_the_burn_acceleration() {
    let ship = Spacecraft::sample();
    let report = ship.report();
    let a = report.acceleration_m_s2;
    // A tank's latch pushes the loaded tank (about 24.5 t) at the
    // acceleration, with the tank's center about 3.2 m outboard of its
    // latch.
    let part = ship.part("tank-port").unwrap();
    let tank = report
        .latch_loads
        .iter()
        .find(|l| l.part == "tank-port")
        .unwrap();
    assert!((tank.force_n - part.mass() * a).abs() < 1e-6);
    let arm = part.position - part.attach.at;
    let lever = arm.y.hypot(arm.z);
    assert!((3.1..3.3).contains(&lever), "{lever}");
    assert!((tank.moment_nm - part.mass() * a * lever).abs() < 1e-6);
    // The engine's latch pushes everything but the engine.
    let engine = report
        .latch_loads
        .iter()
        .find(|l| l.part == "engine")
        .unwrap();
    let rest = report.mass.mass - ship.part("engine").unwrap().mass();
    assert!((engine.force_n - rest * a).abs() < 1e-6);
    // Doubling a tank's propellant overloads its latch at ignition.
    let mut heavy = ship.clone();
    heavy.requirements.max_mass_kg = 1e6;
    heavy.requirements.max_thrust_offset_m = 10.0;
    let refused = heavy
        .edit(&Edit::SetPropellant {
            part: "tank-port".into(),
            kg: 150_000.0,
        })
        .unwrap_err();
    assert_eq!(refused.requirement, "latch load", "{refused}");
}

#[test]
fn a_looping_or_broken_tree_is_refused() {
    let mut ship = Spacecraft::sample();
    ship.parts[0].attach.parent = Some("habitat".into());
    assert!(
        ship.check()
            .iter()
            .any(|f| f.requirement == "assembly tree")
    );
    let refused = Spacecraft::sample()
        .edit(&Edit::RemovePart {
            part: "keel-fwd".into(),
        })
        .unwrap_err();
    assert_eq!(refused.requirement, "assembly tree");
}

// --- The arm ---------------------------------------------------------------

#[test]
fn the_arm_is_the_so101_scaled_up() {
    let urdf = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../robot-description/assets/so101/so101_new_calib.urdf");
    let robot = robot_description::load(&urdf).unwrap();
    let movable: Vec<_> = robot.movable().collect();
    for (i, name) in arm::JOINTS.iter().enumerate() {
        let joint = movable.iter().find(|j| j.name == *name).unwrap();
        let limit = joint.limit.unwrap();
        assert!((limit.lower - arm::LIMITS[i].0).abs() < 1e-5, "{name}");
        assert!((limit.upper - arm::LIMITS[i].1).abs() < 1e-5, "{name}");
    }
    // Upper arm and forearm lengths from the URDF's joint origins.
    let origin = |name: &str| {
        let j = movable.iter().find(|j| j.name == name).unwrap();
        j.origin.w_axis.truncate()
    };
    let upper = origin("elbow_flex").length();
    let forearm = origin("wrist_flex").length();
    let spec = ArmSpec::so101_scaled(DVec3::ZERO, 0.0);
    assert!((spec.upper - upper * arm::SCALE).abs() < 0.05, "{upper}");
    assert!(
        (spec.forearm - forearm * arm::SCALE).abs() < 0.05,
        "{forearm}"
    );
}

#[test]
fn arms_match_mujoco_for_a_reference_move() {
    use crate::oracle::{Reference, ReferenceMove, TOLERANCE};
    use physics::{Body, BodyKind, Joint, NoField};
    let reference = Reference::checked_in().unwrap();
    let r = ReferenceMove::sample();
    assert_eq!(
        reference.model_xml,
        r.mjcf(),
        "the arm changed: regenerate oracle/arm-reference.json (robot-sim's lagrange2_arm_oracle)"
    );
    assert_eq!(reference.mujoco, "3.3.7");
    assert_eq!(reference.timestep, factory::DT);
    assert_eq!(reference.angles.len(), r.steps / r.record_every);
    // The factory's own arm and servos on a fixed pedestal, holding the
    // loaded tank in its grapple.
    let mut world = physics::World::new(factory::DT);
    world.sleep.enabled = false;
    let base = world.add(Body::new(1.0, DVec3::ONE, DVec3::ZERO).with_kind(BodyKind::Static));
    let arm = factory::build_arm(&mut world, base, r.spec, r.motion.from).unwrap();
    let tool = arm.links[4];
    let tip = world[tool].to_world(DVec3::Y * factory::TOOL / 2.0);
    let held = world[tool].orientation * arm::nominal_grip();
    let center = world[tool].to_world(DVec3::Y * (factory::TOOL / 2.0 + r.payload_drop));
    let mut payload = Body::new(r.payload_kg, r.payload_inertia, center);
    payload.orientation = held;
    payload.prev_orientation = held;
    let payload = world.add(payload);
    world.add_joint(Joint::weld_here(&world, tool, payload, tip));
    let mut worst = [0.0_f64; 5];
    let mut moved = 0.0_f64;
    for k in 0..r.steps {
        let target = r.target(k);
        for (i, &q) in target.iter().enumerate() {
            world.set_joint_target(arm.joints[i], q).unwrap();
        }
        world.step(&NoField);
        if (k + 1) % r.record_every == 0 {
            let want = reference.angles[(k + 1) / r.record_every - 1];
            for i in 0..5 {
                let got = world
                    .joint(arm.joints[i])
                    .unwrap()
                    .coordinate(&world)
                    .unwrap();
                worst[i] = worst[i].max((got - want[i]).abs());
                moved = moved.max((want[i] - r.motion.from[i]).abs());
            }
        }
    }
    eprintln!(
        "largest joint differences from MuJoCo 3.3.7, rad: {worst:?} over a move of up to {moved:.2} rad"
    );
    assert!(moved > 0.3, "the reference actually moves");
    for (i, w) in worst.iter().enumerate() {
        assert!(*w < TOLERANCE, "{}: {w} rad", arm::JOINTS[i]);
    }
}

#[test]
fn inverse_kinematics_puts_the_tool_where_asked() {
    let spec = ArmSpec::so101_scaled(DVec3::new(-16.0, 8.0, -15.0), -std::f64::consts::FRAC_PI_2);
    for (target, yaw) in [
        (DVec3::new(-8.0, 9.2, 0.0), 0.0),
        (DVec3::new(-30.0, 7.0, -9.0), 1.0),
        (DVec3::new(-12.0, 20.0, -4.0), -0.5),
    ] {
        let pose = spec.solve(target, yaw).unwrap();
        let frames = spec.frames(&pose);
        assert!(frames.points[4].distance(target) < 1e-9);
        // The tool points down and the held part sits at the asked yaw.
        let down = frames.orientations[4] * DVec3::Y;
        assert!((down + DVec3::Y).length() < 1e-9, "{down}");
        let held = spec.held_orientation(&pose);
        let want = glam::DQuat::from_rotation_y(yaw);
        assert!(held.angle_between(want) < 1e-6);
    }
    assert!(spec.solve(DVec3::new(30.0, 8.0, 30.0), 0.0).is_err());
}

// --- The factory: momentum and reactions -----------------------------------

/// Momentum residual relative to the momentum in play.
fn residual(start: Momentum, now: Momentum, scale: (f64, f64)) -> (f64, f64) {
    (
        (now.linear - start.linear).length() / scale.0.max(1e-12),
        (now.angular - start.angular).length() / scale.1.max(1e-12),
    )
}

/// The largest sum of body momentum magnitudes seen: the momentum in play.
fn in_play(f: &Factory, scale: &mut (f64, f64)) {
    let mut lin = 0.0;
    let mut ang = 0.0;
    for b in f.world.bodies() {
        let m = Momentum::of(b, DVec3::ZERO);
        lin += m.linear.length();
        ang += m.angular.length();
    }
    scale.0 = scale.0.max(lin);
    scale.1 = scale.1.max(ang + f.cmg.h.length());
}

/// Grasp `part` with its arm and carry it toward its transit pose for
/// `seconds`.
fn carry(f: &mut Factory, part: usize, seconds: f64, scale: &mut (f64, f64)) -> Momentum {
    let arm = f.parts[part].arm;
    let w = crate::assembly::waypoints(f, part).unwrap();
    let start = f.momentum();
    for (target, pace) in [(w.above_rack, 1.0), (w.at_rack, 0.3)] {
        f.move_arm(arm, target, pace);
        let end = f.time() + f.arms[arm].motion.unwrap().duration + 2.0;
        while f.time() < end {
            f.step();
            in_play(f, scale);
        }
    }
    f.grasp(arm, part).unwrap();
    f.move_arm(arm, w.lift, 0.6);
    let end = f.time() + seconds;
    while f.time() < end {
        f.step();
        in_play(f, scale);
    }
    start
}

#[test]
fn arm_motion_conserves_the_factorys_momentum() {
    // Attitude control off: factory, arms, and part float free, and the
    // only forces are the joints' internal pairs.
    let mut f = Factory::sample();
    f.attitude.enabled = false;
    let mut scale = (0.0, 0.0);
    let start = carry(&mut f, 3, 60.0, &mut scale);
    let (lin, ang) = residual(start, f.momentum(), scale);
    assert!(lin < 1e-10 && ang < 1e-10, "{lin:e} {ang:e}");
    assert!(scale.1 > 1e4, "the move carried real momentum: {scale:?}");
    // With the CMGs holding attitude, their stored momentum closes the books.
    let mut f = Factory::sample();
    let mut scale = (0.0, 0.0);
    let start = carry(&mut f, 3, 60.0, &mut scale);
    let (lin, ang) = residual(start, f.momentum(), scale);
    assert!(lin < 1e-10 && ang < 1e-10, "{lin:e} {ang:e}");
    assert!(f.cmg.h.length() > 10.0, "the CMGs worked: {}", f.cmg.h);
}

#[test]
fn a_swing_turns_the_free_floating_factory_as_predicted() {
    let mut f = Factory::sample();
    f.attitude.enabled = false;
    let mut scale = (0.0, 0.0);
    // Pick up the 24.5 t tank and lift it, then swing the pan 0.8 rad with
    // the factory drifting free.
    carry(&mut f, 3, 200.0, &mut scale);
    let arm = f.parts[3].arm;
    let before = f.world[f.body].orientation;
    let pan0 = f.measured(arm)[0];
    let mut to = f.measured(arm);
    to[0] += 0.8;
    f.move_arm(arm, to, 1.0);
    let end = f.time() + f.arms[arm].motion.unwrap().duration + 5.0;
    let mut swinging: Vec<physics::BodyId> = f.arms[arm].links[1..].to_vec();
    swinging.push(f.parts[3].body);
    // Angular momentum about the vertical: the swinging bodies' momentum
    // relative to the deck, about the system's center of mass c, is
    // sum m (p - c)_h . (p - a)_h w + I_yy w for a pan rate w about the
    // pan axis through a; the deck and everything on it turn at
    // -L / I_total so the total stays zero
    // (in-space-assembly.md, "The momentum relation").
    let mut predicted = 0.0;
    let mut pan = pan0;
    while f.time() < end {
        let c = physics::Composite::of(f.world.bodies().iter()).com;
        let a = f.to_world(f.arms[arm].spec.base);
        let mut l = 0.0;
        for &id in &swinging {
            let b = &f.world[id];
            let (u, w) = (b.pos - c, b.pos - a);
            l += b.mass * (u.x * w.x + u.z * w.z) + b.inertia_world().y_axis.y;
        }
        let i_total: f64 = f
            .world
            .bodies()
            .iter()
            .map(|b| {
                let d = b.pos - c;
                b.inertia_world().y_axis.y + b.mass * (d.x * d.x + d.z * d.z)
            })
            .sum();
        f.step();
        let now = f.measured(arm)[0];
        predicted -= l / i_total * (now - pan);
        pan = now;
    }
    let turned = (f.world[f.body].orientation * before.inverse()).to_scaled_axis();
    let swing = f.measured(arm)[0] - pan0;
    eprintln!(
        "factory turned {:.6} rad about y for a {swing:.3} rad swing; predicted {predicted:.6}",
        turned.y
    );
    assert!(
        turned.y * swing < 0.0,
        "the factory turns against the swing"
    );
    // The deck's own small translation and the lift joints' settling are
    // left out of the estimate: within 2%.
    assert!(
        ((turned.y - predicted) / predicted).abs() < 0.02,
        "{} vs {predicted}",
        turned.y
    );
}

#[test]
fn a_closed_joint_loop_leaves_the_base_turned() {
    // Out along pan, up along lift, back along pan, down along lift: the
    // joints return to where they started and the free-floating factory
    // does not (angular momentum is not integrable).
    let mut f = Factory::sample();
    f.attitude.enabled = false;
    let arm = 0;
    let start = f.measured(arm);
    let before = f.world[f.body].orientation;
    let mut corner = start;
    for (joint, delta) in [(0, 0.6), (1, 0.3), (0, -0.6), (1, -0.3)] {
        corner[joint] += delta;
        f.move_arm(arm, corner, 1.0);
        let end = f.time() + f.arms[arm].motion.unwrap().duration + 5.0;
        while f.time() < end {
            f.step();
        }
    }
    let back = f.measured(arm);
    for i in 0..5 {
        assert!((back[i] - start[i]).abs() < 2e-3, "joint {i}");
    }
    let turned = f.world[f.body].orientation.angle_between(before);
    eprintln!("the loop left the deck turned {turned:e} rad");
    assert!(
        turned > 1e-6,
        "the loop left the base turned {turned:e} rad"
    );
}

#[test]
fn opposed_arms_cancel_their_reactions() {
    // All four arms make mirror-image moves: each one's reaction is
    // cancelled by its mirror images', so the free-floating deck does not
    // turn.
    let mut f = Factory::sample();
    f.attitude.enabled = false;
    // Empty racks, so the deck and its arms are symmetric.
    for i in 0..f.parts.len() {
        let body = f.parts[i].body;
        f.world[body].mass = 1e-3;
        f.world[body].inertia = DVec3::splat(1e-3);
    }
    let before = f.world[f.body].orientation;
    let mut alone = f.clone();
    let moves = |f: &mut Factory, arms: &[usize]| {
        for &a in arms {
            let mut to = f.measured(a);
            let mirror = if a == 0 || a == 3 { 1.0 } else { -1.0 };
            to[0] += 0.5 * mirror;
            to[1] += 0.3;
            to[4] -= 0.4 * mirror;
            f.move_arm(a, to, 1.0);
        }
        let end = f.time() + f.arms[arms[0]].motion.unwrap().duration + 5.0;
        while f.time() < end {
            f.step();
        }
    };
    moves(&mut f, &[0, 1, 2, 3]);
    moves(&mut alone, &[0]);
    let together = f.world[f.body].orientation.angle_between(before);
    let single = alone.world[alone.body].orientation.angle_between(before);
    eprintln!("deck turned {together:e} rad with four mirrored arms, {single:e} with one");
    assert!(single > 1e-4);
    assert!(together < 1e-6 * single, "{together:e}");
}

// --- Assembly ----------------------------------------------------------------

/// The sample assembly, approved, run until `stages` stages pass or it
/// finishes.
fn run_stages(mut a: Assembly, stages: usize) -> Assembly {
    a.approve();
    let end = a.factory.time() + 4.0 * 3600.0;
    while !a.finished() && a.passed < stages && a.factory.time() < end {
        a.step();
        assert!(a.factory.all_restrained(), "nothing floats free");
    }
    a
}

#[test]
fn four_arms_assemble_the_sample_ship() {
    let mut a = Assembly::sample();
    assert!(a.plan.respects_precedence());
    assert_eq!(a.plan.ship_version, a.factory.ship.version());
    let arms: std::collections::BTreeSet<usize> = a.plan.stages.iter().map(|s| s.arm).collect();
    assert_eq!(arms.len(), 4, "every arm has work");
    a.approve();
    let start = a.factory.momentum();
    let mut scale = (0.0, 0.0);
    let end = 4.0 * 3600.0;
    while !a.finished() && a.factory.time() < end {
        a.step();
        in_play(&a.factory, &mut scale);
        assert!(a.factory.all_restrained(), "nothing floats free");
        assert!(a.factory.attitude_error() < crate::assembly::ATTITUDE_BOX);
    }
    for e in &a.events {
        eprintln!("{:7.1} s  {}", e.time, e.text);
    }
    assert_eq!(a.status, Status::Complete, "{}", a.caption());
    assert_eq!(a.passed, a.plan.stages.len());
    for i in 0..a.factory.parts.len() {
        assert!(matches!(a.factory.parts[i].holder, Holder::Latched(_)));
        let (position, angle) = a.factory.placement_error(i);
        assert!(
            position < 0.005 && angle < 0.0035,
            "{i}: {position} {angle}"
        );
    }
    // The assembled ship's mass properties are its document's.
    let want = a.factory.ship.mass_properties();
    let got = a.factory.assembled();
    assert!((got.mass - want.mass).abs() < 1e-9 * want.mass);
    assert!(
        (got.com - want.com).length() < 1e-4,
        "{} {}",
        got.com,
        want.com
    );
    let norm = want.inertia.x_axis.length();
    for (g, w) in got
        .inertia
        .to_cols_array()
        .iter()
        .zip(want.inertia.to_cols_array())
    {
        assert!((g - w).abs() < 1e-5 * norm, "{g} vs {w}");
    }
    // Nothing outside acted: the books balance with the CMGs counted.
    let (lin, ang) = residual(start, a.factory.momentum(), scale);
    assert!(lin < 1e-10 && ang < 1e-10, "{lin:e} {ang:e}");
    eprintln!(
        "assembled in {:.0} s; peak CMG {:.0} N m s",
        a.factory.time(),
        a.cmg_peak
    );
}

#[test]
fn every_stage_is_simulated_before_it_moves() {
    let a = run_stages(Assembly::sample(), 2);
    let texts: Vec<&str> = a.events.iter().map(|e| e.text.as_str()).collect();
    let simulated = texts
        .iter()
        .position(|t| t.starts_with("simulated keel-aft"));
    let grasped = texts.iter().position(|t| t.contains("has keel-aft"));
    assert!(simulated.unwrap() < grasped.unwrap());
    // The prediction matched the real stage (the check compares them).
    assert_eq!(a.passed, 2);
}

#[test]
fn stuck_latch_steps_back() {
    let mut a = Assembly::sample();
    a.faults.stuck_latch = Some(("keel-aft".into(), 1));
    let a = run_stages(a, 1);
    assert_eq!(a.passed, 1, "{}", a.caption());
    assert!(a.events.iter().any(|e| e.text.contains("stepping back")));
    assert!(a.events.iter().any(|e| e.text.contains("keel-aft latched")));
}

#[test]
fn a_latch_that_never_seats_makes_safe() {
    let mut a = Assembly::sample();
    a.faults.stuck_latch = Some(("keel-aft".into(), 99));
    let a = run_stages(a, 1);
    assert_eq!(
        a.status,
        Status::Safe(Failure::LatchDidNotSeat {
            part: "keel-aft".into(),
            attempts: crate::assembly::MAX_RETRIES + 1,
        })
    );
    // Made safe: the part is still in the arm, and every arm has stopped.
    assert!(matches!(a.factory.parts[0].holder, Holder::Arm { .. }));
    assert!(a.factory.arms.iter().all(|arm| arm.motion.is_none()));
    let mut f = a.factory.clone();
    for _ in 0..300 {
        f.step();
    }
    for arm in 0..4 {
        let q0 = a.factory.measured(arm);
        let q1 = f.measured(arm);
        for i in 0..5 {
            assert!((q0[i] - q1[i]).abs() < 1e-3);
        }
    }
}

#[test]
fn assembly_survives_link_loss() {
    let mut a = Assembly::sample();
    // The link drops a minute into the first stage and returns ten
    // minutes later.
    a.faults.link_loss = vec![(110.0, 710.0)];
    let a = run_stages(a, 2);
    assert_eq!(a.passed, 2, "{}", a.caption());
    let first_done = a
        .events
        .iter()
        .find(|e| e.text == "keel-aft checked")
        .unwrap()
        .time;
    assert!(first_done < 710.0, "the stage under way finished");
    assert!(a.events.iter().any(|e| e.text.contains("holding")));
    let second = a
        .events
        .iter()
        .find(|e| e.text.starts_with("simulated keel-fwd"))
        .unwrap()
        .time;
    assert!(second >= 710.0, "no new stage while the link was down");
}

#[test]
fn reset_resumes_from_last_check() {
    // Find when arm 1 is carrying the keel, then reset its controller then.
    let mut probe = Assembly::sample();
    probe.approve();
    while probe
        .run
        .as_ref()
        .is_none_or(|r| r.phase != Phase::Transfer)
    {
        probe.step();
    }
    let at = probe.factory.time() + 20.0;
    let mut a = Assembly::sample();
    a.faults.resets = vec![(0, at)];
    let a = run_stages(a, 1);
    assert_eq!(a.passed, 1, "{}", a.caption());
    assert!(
        a.events
            .iter()
            .any(|e| e.text.contains("controller reset while carrying"))
    );
}

/// The factory time `ahead` seconds after the sample's stage `stage`
/// reaches `phase`.
fn when(stage: usize, phase: Phase, ahead: f64) -> f64 {
    let mut probe = Assembly::sample();
    probe.approve();
    while probe
        .run
        .as_ref()
        .is_none_or(|r| r.stage != stage || r.phase != phase)
    {
        probe.step();
        assert!(!probe.finished(), "{}", probe.caption());
    }
    probe.factory.time() + ahead
}

/// Run a solar event from `warning` to `clear`; returns the assembly at
/// the all clear and when the factory was quiet.
fn quiet_through(warning: f64, clear: f64) -> (Assembly, f64) {
    use crate::assembly::QUIET_WITHIN;
    let mut a = Assembly::sample();
    a.faults.solar_events = vec![(warning, clear)];
    a.approve();
    let mut quiet_at = None;
    while a.factory.time() < clear - 1.0 {
        a.step();
        assert!(a.factory.all_restrained(), "nothing floats free");
        let f = &a.factory;
        let parked = a.arms_idle()
            && f.arms.iter().enumerate().all(|(i, arm)| {
                let q = f.measured(i);
                (0..5).all(|j| (q[j] - arm.home[j]).abs() < 0.01)
            });
        let secured = f
            .parts
            .iter()
            .all(|p| matches!(p.holder, Holder::Rack(_) | Holder::Latched(_)));
        let holding = a.status == Status::Holding("solar particle event".into());
        let quiet = parked && secured && holding;
        if quiet && quiet_at.is_none() {
            quiet_at = Some(f.time());
        }
        // Once quiet, it stays quiet until the all clear.
        if quiet_at.is_some() {
            assert!(quiet, "{} at {:.0} s", a.caption(), f.time());
        }
    }
    let quiet_at = quiet_at.expect("the factory went quiet");
    eprintln!(
        "warning at {warning:.0} s; quiet {:.0} s later",
        quiet_at - warning
    );
    assert!(quiet_at - warning <= QUIET_WITHIN, "{}", quiet_at - warning);
    // No stage was simulated or started during the event.
    assert!(
        !a.events
            .iter()
            .any(|e| e.time > warning && e.text.starts_with("simulated"))
    );
    (a, quiet_at)
}

#[test]
fn solar_event_quiet_hold() {
    // A warning while arm 1 carries the aft keel: it latches the keel
    // first (a part in a grapple is not yet secured), then parks.
    let carrying = when(0, Phase::Transfer, 10.0);
    let (a, _) = quiet_through(carrying, carrying + 1_800.0);
    assert!(
        a.events
            .iter()
            .any(|e| e.text.contains("latching keel-aft first"))
    );
    assert!(matches!(a.factory.parts[0].holder, Holder::Latched(_)));
    // After the all clear the next stage is simulated and the work goes
    // on, every check passing.
    let a = run_stages(a, 2);
    assert_eq!(a.passed, 2, "{}", a.caption());
    assert!(a.events.iter().any(|e| e.text.contains("all clear")));

    // A warning while an arm reaches for a part: it stops before the
    // grasp, backs off, and parks; the part stays on its rack.
    let reaching = when(1, Phase::Descend, 5.0);
    let (a, _) = quiet_through(reaching, reaching + 1_200.0);
    assert!(
        a.events
            .iter()
            .any(|e| e.text.contains("stopping before the grasp"))
    );
    assert!(matches!(a.factory.parts[1].holder, Holder::Rack(_)));
    assert_eq!(a.passed, 1);
    let a = run_stages(a, 2);
    assert_eq!(a.passed, 2, "{}", a.caption());
}

#[test]
fn debris_avoidance_hold() {
    use crate::assembly::POSE_TOLERANCE;
    // The ground asks for a 5 cm/s avoidance burn while arm 2 carries the
    // forward keel.
    let request = when(1, Phase::Transfer, 10.0);
    let dv = DVec3::new(0.0, 0.03, 0.04);
    let mut a = Assembly::sample();
    a.faults.avoidance = vec![(request, dv)];
    a.approve();
    let end = 4.0 * 3600.0;
    while !a.finished() && a.passed < 3 && a.factory.time() < end {
        a.step();
        assert!(a.factory.all_restrained(), "nothing floats free");
        if a.avoidance.is_some() {
            // No stage moves during the burn, and the latched work does
            // not shift on its latches.
            assert!(a.run.is_none());
            for i in 0..a.factory.parts.len() {
                if matches!(a.factory.parts[i].holder, Holder::Latched(_)) {
                    let (position, angle) = a.factory.placement_error(i);
                    assert!(
                        position < POSE_TOLERANCE.0 && angle < POSE_TOLERANCE.1,
                        "{i}: {position} {angle}"
                    );
                    assert!(!a.factory.latch_joint(i).unwrap().saturated);
                }
            }
        }
    }
    assert_eq!(a.passed, 3, "{}", a.caption());
    assert_eq!(a.avoidances.len(), 1);
    let flown = &a.avoidances[0];
    // It waited for the stage under way to finish: the next hold point.
    let checked = a
        .events
        .iter()
        .find(|e| e.text == "keel-fwd checked")
        .unwrap()
        .time;
    assert!(flown.started >= checked && flown.started > request);
    eprintln!(
        "requested at {request:.0} s, flown at {:.0} s: {:.4} m/s",
        flown.started,
        flown.achieved.length()
    );
    // The whole factory's center of mass changed velocity as asked.
    assert!(
        (flown.achieved - dv).length() < 0.01 * dv.length(),
        "{} vs {dv}",
        flown.achieved
    );
}

#[test]
fn cmg_saturation_is_a_named_failure() {
    let mut a = Assembly::sample();
    a.factory.cmg.capacity = 50.0;
    let a = run_stages(a, 1);
    assert_eq!(a.status, Status::Refused(Failure::CmgSaturated));
    // Refused in simulation: nothing moved.
    assert!(matches!(a.factory.parts[0].holder, Holder::Rack(_)));
}

#[test]
fn a_torque_limit_is_a_named_failure() {
    let mut a = Assembly::sample();
    // A part thirty times heavier than the document says, with CMGs big
    // enough that the motors give out first.
    let body = a.factory.parts[0].body;
    a.factory.world[body].mass *= 30.0;
    a.factory.world[body].inertia *= 30.0;
    a.factory.cmg.capacity = 1e9;
    a.factory.cmg.max_torque = 1e9;
    let a = run_stages(a, 1);
    match &a.status {
        Status::Refused(Failure::MotorLimit { arm, .. }) => assert_eq!(*arm, 0),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_collision_is_a_named_failure() {
    let mut a = Assembly::sample();
    // Park arm 2 with its tool where arm 1 must put the aft keel.
    let f = &mut a.factory;
    let target = f.parts[0].design + DVec3::Y * 4.0;
    let pose = f.arms[1].spec.solve(target, 0.0).unwrap();
    f.move_arm(1, pose, 1.0);
    let end = f.time() + f.arms[1].motion.unwrap().duration + 3.0;
    while f.time() < end {
        f.step();
    }
    let a = run_stages(a, 1);
    match &a.status {
        Status::Refused(Failure::Collision { a, b }) => {
            assert!(a.contains("arm") || b.contains("arm"), "{a} {b}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn assembly_replays_bit_for_bit() {
    let mut a = Assembly::sample();
    a.approve();
    a.run(300.0);
    let saved = serde_json::to_string(&a).unwrap();
    let mut restored: Assembly = serde_json::from_str(&saved).unwrap();
    a.run(300.0);
    restored.run(300.0);
    assert_eq!(a, restored);
    // And two runs from the start agree exactly.
    let mut b = Assembly::sample();
    b.approve();
    b.run(600.0);
    assert_eq!(a.factory.world, b.factory.world);
}

#[test]
fn assembly_is_frame_rate_independent() {
    // The zone's pacing: frame time at 8x through the frame clock, at 30,
    // 60, and 144 frames a second, for ten real seconds and more (80
    // factory seconds, into the first stage's act after its simulation).
    let paced = |fps: f64| {
        let mut a = Assembly::sample();
        a.approve();
        let mut clock = crate::assembly::frame_clock();
        let mut steps = 0u64;
        while steps < 2_400 {
            for _ in 0..clock.advance(8.0 / fps) {
                a.step();
                steps += 1;
            }
        }
        (a, steps)
    };
    let exact = |steps: u64| {
        let mut a = Assembly::sample();
        a.approve();
        for _ in 0..steps {
            a.step();
        }
        a
    };
    for fps in [30.0, 60.0, 144.0] {
        let (a, steps) = paced(fps);
        // 144 fps frames are 1.67 steps long: the run ends on or just past
        // 2,400 steps; every rate matches stepping that count exactly.
        assert!((2_400..2_402).contains(&steps), "{fps}: {steps}");
        assert_eq!(a, exact(steps), "{fps} fps");
        assert!(
            a.events
                .iter()
                .any(|e| e.text.starts_with("simulated keel-aft"))
        );
    }
    let (at30, _) = paced(30.0);
    let (at60, _) = paced(60.0);
    assert_eq!(at30, at60);
    // A long frame (a stalled tab) runs at most its cap and counts the
    // rest as dropped, rather than freezing the page to catch up.
    let mut clock = crate::assembly::frame_clock();
    let steps = clock.advance(10.0 * 64.0);
    assert_eq!(steps, crate::assembly::MAX_FRAME_STEPS);
    assert!((clock.dropped - (640.0 - f64::from(steps) * factory::DT)).abs() < 1e-6);
}

#[test]
fn the_plan_is_a_typed_document() {
    let f = Factory::sample();
    let plan = Plan::new(&f).unwrap();
    assert_eq!(plan.schema, crate::assembly::PLAN_SCHEMA);
    let json = serde_json::to_string(&plan).unwrap();
    assert_eq!(serde_json::from_str::<Plan>(&json).unwrap(), plan);
    assert_eq!(plan.stages[0].part, "keel-aft");
    assert_eq!(plan.stages[0].parent, None);
}

// --- The test burn -----------------------------------------------------------

#[test]
fn the_burn_obeys_the_rocket_equation() {
    let ship = Spacecraft::sample();
    let mut burn = Burn::new(ship, 150.0);
    let p0 = burn.system_momentum();
    burn.run(30.0);
    assert!(!burn.firing);
    let rocket = burn.rocket_equation();
    assert!(((burn.flown - 150.0) / 150.0).abs() < 1e-9);
    assert!(
        ((rocket - burn.flown) / rocket).abs() < 1e-6,
        "{rocket} {}",
        burn.flown
    );
    // The ship's speed is the scalar sum, to the small gimbal angles.
    assert!(((burn.vel.length() - burn.flown) / burn.flown).abs() < 1e-4);
    // Exhaust momentum balances the ship's.
    let p1 = burn.system_momentum();
    assert!((p1 - p0).length() / (burn.vel.length() * burn.mass()) < 1e-9);
    // About 4.5 t of propellant for 150 m/s at Isp 330 s.
    let used = burn.initial_mass - burn.mass();
    assert!((4_000.0..5_000.0).contains(&used), "{used}");
}

#[test]
fn the_gimbal_tracks_the_center_of_mass() {
    let ship = Spacecraft::sample();
    // A long burn drains most of the tanks and moves the center of mass.
    let mut burn = Burn::new(ship.clone(), 1_500.0);
    burn.run(10.0);
    let com0 = ship.mass_properties().com;
    let com1 = burn.mass_properties().com;
    assert!(com0.distance(com1) > 0.3, "the center of mass moved");
    // The gimbal kept the ship from tumbling: under 0.05 °/s.
    eprintln!("peak body rate {:.4} °/s", burn.peak_rate.to_degrees());
    assert!(burn.peak_rate < 0.05_f64.to_radians(), "{}", burn.peak_rate);
    // With the gimbal frozen at ignition the same burn turns the ship
    // faster.
    let mut frozen = Burn::new(ship, 1_500.0);
    let start = frozen.com_estimate;
    while frozen.firing {
        frozen.com_estimate = start;
        frozen.step();
    }
    assert!(frozen.peak_rate > 2.0 * burn.peak_rate);
}

#[test]
fn the_burn_rings_the_truss() {
    use crate::burn::KEEL_DAMPING;
    let ship = Spacecraft::sample();
    let report = ship.report();
    let mut burn = Burn::new(ship.clone(), 150.0);
    let keel = burn.keel.clone().expect("the tug has a keel joint");
    assert_eq!(keel.joint, "keel-fwd");
    // The static load is the keel joint's latch load at ignition: the
    // forward half pushed at the burn's acceleration.
    let latch = report
        .latch_loads
        .iter()
        .find(|l| l.part == "keel-fwd")
        .unwrap();
    let f = keel.mode.frequency;
    eprintln!(
        "keel axial mode {f:.1} Hz, static load {:.1} kN",
        latch.force_n / 1e3
    );
    assert!((5.0..60.0).contains(&f), "{f}");
    // A step thrust overshoots to 1 + e^(-zeta pi / sqrt(1 - zeta^2)) of
    // the static load in the first half period.
    for _ in 0..30 {
        burn.step();
    }
    let zeta = KEEL_DAMPING;
    let overshoot = 1.0 + (-zeta * std::f64::consts::PI / (1.0 - zeta * zeta).sqrt()).exp();
    let peak = burn.keel.as_ref().unwrap().peak_load;
    eprintln!(
        "ignition peak {:.1} kN, {:.3} x static",
        peak / 1e3,
        peak / latch.force_n
    );
    assert!(
        ((peak / latch.force_n) - overshoot).abs() < 0.005 * overshoot,
        "{} vs {overshoot}",
        peak / latch.force_n
    );
    // At cutoff the keel springs back and rings down at its damping:
    // the amplitude falls as e^(-zeta omega t) (within 1%: the energy
    // measure wobbles by about zeta within a period).
    while burn.firing {
        burn.step();
    }
    burn.step();
    let a0 = burn.keel.as_ref().unwrap().amplitude(0.0);
    assert!(a0 > 0.0, "the cutoff rings the keel");
    let w = burn.keel.as_ref().unwrap().mode.omega();
    for _ in 0..60 {
        burn.step();
    }
    let a1 = burn.keel.as_ref().unwrap().amplitude(0.0);
    let expected = (-zeta * w * 60.0 * crate::burn::DT).exp();
    assert!(
        ((a1 / a0) - expected).abs() < 0.01 * expected,
        "{} vs {expected}",
        a1 / a0
    );
    // The check: the dynamic peak stays inside the keel joint's latch
    // rating, which the static check alone would not show.
    let peak = burn.keel.as_ref().unwrap().peak_load;
    let rating = ship.part("keel-fwd").unwrap().attach.rated_force_n;
    assert!(peak < rating, "{peak} N against {rating} N");
    assert!(peak > 1.9 * latch.force_n);
}

#[test]
fn the_ship_departs_l2() {
    use verse_lagrange::earth_moon::HaloOrbit;
    let mut burn = Burn::new(Spacecraft::sample(), 150.0);
    burn.run(0.0);
    // The cradle's +X points along the rotating frame's +x (away from the
    // Earth): the burn's velocity hands over in those axes.
    let mut orbit = HaloOrbit::new();
    orbit.apply_burn(burn.vel);
    orbit.advance(3.0 * 86_400.0);
    let snap = orbit.snapshot();
    eprintln!(
        "three days after a {:.0} m/s burn: {:.0} km off the halo",
        burn.flown, snap.deviation_km
    );
    assert!(snap.deviation_km > 20_000.0);
}

#[test]
fn factory_constants_are_consistent() {
    // The racks and the pedestals stay on the deck.
    for &[x, z] in &factory::ARM_BASES {
        assert!(x.abs() < factory::DECK_SIZE.x / 2.0 && z.abs() < factory::DECK_SIZE.z / 2.0);
    }
    assert!(factory::RACK_X + 8.0 < factory::DECK_SIZE.x / 2.0);
    // The tanks are the heaviest parts the arms carry.
    let ship = Spacecraft::sample();
    let heaviest = ship
        .parts
        .iter()
        .max_by(|a, b| a.mass().total_cmp(&b.mass()))
        .unwrap();
    assert!(matches!(heaviest.kind, PartKind::Tank { .. }));
}
