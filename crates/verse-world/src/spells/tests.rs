use super::*;
use physics::kinematic::Aabb;

fn floor() -> SpellWorld {
    SpellWorld::new(
        &[Aabb {
            min: DVec3::new(-30., -0.6, -30.),
            max: DVec3::new(30., 0., 30.),
        }],
        1,
    )
}

fn life(n: u64) -> physics::queries::Life {
    physics::queries::Life {
        instance: 0,
        entity: PROP_ENTITY_BASE + n,
        generation: 0,
    }
}

fn settle(world: &mut SpellWorld, seconds: f32) {
    for _ in 0..(seconds * 30.) as u32 {
        world.begin_tick();
        world.step(4, 0.).unwrap();
    }
}

#[test]
fn an_unobstructed_push_slides_ten_feet_whatever_the_mass() {
    for kind in [PropKind::Crate, PropKind::TrainingDummy, PropKind::Anvil] {
        let mut world = floor();
        let spec = PropSpec::reference(kind);
        let half = spec.dimensions.y * 0.5;
        let index = world
            .add_prop(life(0), "prop", spec, DVec3::Y * half, 0., None)
            .unwrap();
        settle(&mut world, 1.);
        let start = world.prop_center(index);
        assert!(
            world
                .push_prop(index, DVec3::X, 10. * FEET, "test")
                .unwrap()
        );
        settle(&mut world, 3.);
        let moved = world.prop_center(index) - start;
        let distance = DVec3::new(moved.x, 0., moved.z).length();
        assert!(
            (distance - 10. * FEET).abs() < 0.05 * 10. * FEET,
            "{kind:?} slid {distance} m"
        );
        let error = world.ledger_error();
        assert!(
            error.linear < LEDGER_TOLERANCE && error.angular < LEDGER_TOLERANCE,
            "{kind:?} {error:?}"
        );
    }
}

#[test]
fn a_secured_prop_does_not_move() {
    let mut world = floor();
    let index = world
        .add_prop(
            life(0),
            "secured",
            PropSpec::reference(PropKind::Crate).secured(),
            DVec3::Y * 0.3,
            0.,
            None,
        )
        .unwrap();
    assert!(
        !world
            .push_prop(index, DVec3::X, 10. * FEET, "test")
            .unwrap()
    );
    settle(&mut world, 1.);
    assert!(world.prop_center(index).distance(DVec3::Y * 0.3) < 1e-12);
}

#[test]
fn a_wall_stops_a_pushed_prop() {
    let mut world = SpellWorld::new(
        &[
            Aabb {
                min: DVec3::new(-30., -0.6, -30.),
                max: DVec3::new(30., 0., 30.),
            },
            Aabb {
                min: DVec3::new(1.5, 0., -3.),
                max: DVec3::new(2., 3., 3.),
            },
        ],
        1,
    );
    let index = world
        .add_prop(
            life(0),
            "crate",
            PropSpec::reference(PropKind::Crate),
            DVec3::Y * 0.3,
            0.,
            None,
        )
        .unwrap();
    settle(&mut world, 0.5);
    world
        .push_prop(index, DVec3::X, 10. * FEET, "test")
        .unwrap();
    settle(&mut world, 3.);
    let center = world.prop_center(index);
    assert!(center.x < 1.21 && center.x > 1.0, "{center}");
    let error = world.ledger_error();
    assert!(error.linear < LEDGER_TOLERANCE, "{error:?}");
}

#[test]
fn a_knocked_character_shoves_a_crate_and_slows() {
    let mut world = floor();
    let index = world
        .add_prop(
            life(0),
            "crate",
            PropSpec::reference(PropKind::Crate),
            DVec3::new(0.66, 0.3, 0.),
            0.,
            None,
        )
        .unwrap();
    settle(&mut world, 0.5);
    let mut character = physics::character::Character::new(DVec3::ZERO);
    character.add_velocity(DVec3::X * 5.);
    let before = character.external.x * 75.;
    world
        .couple(&mut [Mover {
            actor: 1,
            mass: 75.,
            character: &mut character,
        }])
        .unwrap();
    let crate_momentum = world.body(world.props[index].body).momentum();
    assert!(crate_momentum.x > 50., "{crate_momentum}");
    assert!(character.external.x < 5.);
    let after = character.external.x * 75. + crate_momentum.x;
    assert!((after - before).abs() < 1e-9, "{before} {after}");
    assert!(world.ledger.external.contains_key("character contact"));
}

#[test]
fn spell_bodies_joints_and_fields_leave_when_their_cast_ends() {
    let mut world = floor();
    let first = world.begin_cast(14, true).unwrap();
    let panel = world
        .add_prop(
            life(1),
            "panel",
            PropSpec::reference(PropKind::SpellBody),
            DVec3::new(0., 0.375, 4.),
            0.,
            Some(first),
        )
        .unwrap();
    world
        .add_field(SpellField {
            cast: first,
            spell: "test".into(),
            owner: 14,
            area: Area::Sphere {
                center: DVec3::ZERO,
                radius: 50.,
            },
            acceleration: DVec3::X * 10.,
            expires: 60.,
            concentration: true,
        })
        .unwrap();
    settle(&mut world, 0.5);
    assert!(world.body(world.props[panel].body).vel.x > 0.1);
    // A second concentration spell ends the first.
    let second = world.begin_cast(14, true).unwrap();
    assert!(world.props[panel].removed && world.fields.is_empty());
    assert_eq!(world.concentration[&14], second);
    settle(&mut world, 0.2);
    let error = world.ledger_error();
    assert!(error.linear < LEDGER_TOLERANCE, "{error:?}");
    world.end_concentration(14).unwrap();
    assert!(world.concentration.is_empty());
}

#[test]
fn a_saved_world_with_props_in_flight_continues_identically() {
    let mut world = floor();
    for n in 0..6 {
        world
            .add_prop(
                life(n),
                "crate",
                PropSpec::reference(PropKind::Crate),
                DVec3::new(0., 0.3 + n as f64 * 0.6, 0.),
                0.1 * n as f64,
                None,
            )
            .unwrap();
    }
    settle(&mut world, 1.);
    world.push_prop(0, DVec3::Z, 10. * FEET, "test").unwrap();
    settle(&mut world, 0.2);
    let saved = serde_json::to_vec(&world).unwrap();
    let mut restored: SpellWorld = serde_json::from_slice(&saved).unwrap();
    restored.validate(0).unwrap();
    settle(&mut world, 2.);
    settle(&mut restored, 2.);
    assert_eq!(
        serde_json::to_vec(&world).unwrap(),
        serde_json::to_vec(&restored).unwrap()
    );
}
