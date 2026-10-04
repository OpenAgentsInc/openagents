use glam::{DQuat, DVec3};
use physics::trace::{Tolerance, Trace};
use physics::{Body, BodyId, BodyKind, Collider, Shape, Uniform, World};

use super::creatures::{Creature, Enclosure, push_out, resolve_enclosure};
use super::rig::{BondKind, Event, GRANITE, Wall};
use super::shapes::{bridge, enclosure, ramp, straight, tower};
use super::validate::{Refusal, Stone, force, validate};
use super::{DURATION, DamageType, FEET, Form, SPAN_LIMIT, hits_panel};

const DT: f64 = 1.0 / 120.0;
const GRAVITY: Uniform = Uniform(DVec3::new(0.0, -9.81, 0.0));

struct Scene {
    world: World,
    stone: Vec<Stone>,
    bodies: Vec<BodyId>,
}

impl Scene {
    fn new() -> Self {
        Self {
            world: World::new(DT),
            stone: Vec::new(),
            bodies: Vec::new(),
        }
    }

    fn rock(&mut self, min: DVec3, max: DVec3) {
        let stone = Stone::aabb(min, max);
        let body = self
            .world
            .add(Body::new(1.0, DVec3::ONE, stone.center).with_kind(BodyKind::Static));
        self.world.add_collider(
            Collider::new(body, Shape::Cuboid { half: stone.half }).with_material(GRANITE),
        );
        self.stone.push(stone);
        self.bodies.push(body);
    }

    /// A chasm `gap` wide centered on x = 0, lips at y = 0, floor 20 m down.
    fn chasm(gap: f64) -> Self {
        let mut scene = Self::new();
        let h = gap * 0.5;
        scene.rock(DVec3::new(-h - 8.0, -6.0, -6.0), DVec3::new(-h, 0.0, 6.0));
        scene.rock(DVec3::new(h, -6.0, -6.0), DVec3::new(h + 8.0, 0.0, 6.0));
        scene.rock(
            DVec3::new(-30.0, -21.0, -30.0),
            DVec3::new(30.0, -20.0, 30.0),
        );
        scene
    }

    fn floor() -> Self {
        let mut scene = Self::new();
        scene.rock(DVec3::new(-30.0, -1.0, -30.0), DVec3::new(30.0, 0.0, 30.0));
        scene
    }

    fn raise(&mut self, panels: &[super::Placement]) -> Wall {
        let plan = validate(panels, &self.stone, DVec3::new(0.0, 1.0, 0.0)).expect("admitted");
        let time = self.world.time();
        Wall::raise(&mut self.world, &plan, &self.bodies, time)
    }

    fn load(&mut self, mass: f64, at: DVec3) -> BodyId {
        let size = DVec3::splat(0.6);
        let body = self.world.add(Body::new(
            mass,
            Body::box_inertia(mass, size),
            at + DVec3::Y * 0.3,
        ));
        self.world.add_collider(
            Collider::new(body, Shape::Cuboid { half: size * 0.5 }).with_material(GRANITE),
        );
        body
    }

    fn run(&mut self, wall: &mut Wall, seconds: f64) -> Vec<Event> {
        let mut events = Vec::new();
        for _ in 0..(seconds / DT).round() as usize {
            self.world.step(&GRAVITY);
            let time = self.world.time();
            events.extend(wall.after_step(&mut self.world, time));
        }
        events
    }
}

fn broke(events: &[Event]) -> Vec<usize> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Broke { bond } => Some(*bond),
            _ => None,
        })
        .collect()
}

#[test]
fn panel_hit_points_follow_thickness() {
    assert_eq!(Form::Thick.hit_points(), 180);
    assert_eq!(Form::Thin.hit_points(), 90);
    assert_eq!(Form::Half.hit_points(), 180);
    // 3.048 x 3.048 x 0.1524 m of granite, and the thin panel's equal volume.
    assert!(
        (Form::Thick.mass() - 3822.9).abs() < 0.5,
        "{}",
        Form::Thick.mass()
    );
    assert!((Form::Thin.mass() - Form::Thick.mass()).abs() < 1e-9);
    assert!((Form::Half.mass() * 4.0 - Form::Thick.mass()).abs() < 1e-9);
    assert!(hits_panel(15) && !hits_panel(14));
}

#[test]
fn damage_respects_immunities_and_destroys_at_zero() {
    let mut scene = Scene::chasm(3.0);
    let mut wall = scene.raise(&bridge(
        DVec3::new(-3.048, 0.0, 0.0),
        DVec3::new(3.048, 0.0, 0.0),
        1,
        Form::Thick,
    ));
    assert!(
        wall.damage(&mut scene.world, 0, 500, DamageType::Poison, 0.0, 0)
            .is_empty()
    );
    assert!(
        wall.damage(&mut scene.world, 0, 500, DamageType::Psychic, 0.0, 0)
            .is_empty()
    );
    assert_eq!(wall.panels[0].hit_points, 180);
    for _ in 0..22 {
        assert!(
            wall.damage(&mut scene.world, 0, 8, DamageType::Fire, 0.0, 0)
                .is_empty()
        );
    }
    assert_eq!(wall.panels[0].hit_points, 4);
    let events = wall.damage(&mut scene.world, 0, 8, DamageType::Fire, 0.0, 0);
    assert!(matches!(events[..], [Event::Destroyed { panel: 0, .. }]));
    assert!(scene.world[wall.panels[0].body].removed);
}

#[test]
fn the_validator_refuses_unsupported_and_disconnected_walls_and_long_spans() {
    let scene = Scene::chasm(20.0 * FEET);
    let caster = DVec3::new(-6.0, 1.0, 0.0);
    // Floating in the air.
    let floating = straight(DVec3::new(-1.0, 5.0, 0.0), DVec3::X, 2, Form::Thick);
    assert_eq!(
        validate(&floating, &scene.stone, caster),
        Err(Refusal::Unsupported)
    );
    // Two panels that do not touch.
    let mut apart = straight(DVec3::new(-12.0, 0.0, 0.0), DVec3::X, 1, Form::Thick);
    apart.extend(straight(
        DVec3::new(-12.0, 0.0, 3.0),
        DVec3::X,
        1,
        Form::Thick,
    ));
    assert_eq!(
        validate(&apart, &scene.stone, caster),
        Err(Refusal::Disconnected { panel: 1 })
    );
    // A 20-foot bridge is at the limit.
    let span = bridge(
        DVec3::new(-15.0 * FEET, 0.0, 0.0),
        DVec3::new(15.0 * FEET, 0.0, 0.0),
        1,
        Form::Thick,
    );
    let plan = validate(&span, &scene.stone, caster).expect("a 20-foot bridge stands");
    assert!((plan.span - SPAN_LIMIT).abs() < 0.3, "{}", plan.span);
    // A 30-foot cantilever off the left lip.
    let cantilever = bridge(
        DVec3::new(-20.0 * FEET, 0.0, 0.0),
        DVec3::new(20.0 * FEET, 0.0, 0.0),
        1,
        Form::Thick,
    );
    let ledge = [Stone::aabb(
        DVec3::new(-20.0, -6.0, -6.0),
        DVec3::new(-10.0 * FEET, 0.0, 6.0),
    )];
    assert!(matches!(
        validate(&cantilever, &ledge, caster),
        Err(Refusal::SpanTooLong { span }) if span > 50.0 * FEET
    ));
    // Out of range and over budget.
    assert!(matches!(
        validate(&span, &scene.stone, DVec3::new(-60.0, 0.0, 0.0)),
        Err(Refusal::OutOfRange { .. })
    ));
    let eleven = straight(DVec3::new(-30.0, 0.0, 0.0), DVec3::X, 11, Form::Thick);
    assert_eq!(
        validate(&eleven, &scene.stone, caster),
        Err(Refusal::OverBudget { cost: 44 })
    );
}

/// A 30-foot gap with a support standing on the chasm floor in the middle.
fn supported_span(form: Form) -> (Vec<super::Placement>, Vec<Stone>) {
    let stone = vec![
        Stone::aabb(
            DVec3::new(-20.0, -6.0, -6.0),
            DVec3::new(-15.0 * FEET, 0.0, 6.0),
        ),
        Stone::aabb(
            DVec3::new(15.0 * FEET, -6.0, -6.0),
            DVec3::new(20.0, 0.0, 6.0),
        ),
        Stone::aabb(
            DVec3::new(-20.0, -7.0, -6.0),
            DVec3::new(20.0, -10.0 * FEET, 6.0),
        ),
    ];
    let size = form.size();
    let lanes = (10.0 * FEET / size.y).round() as usize;
    let mut panels = bridge(
        DVec3::new(-20.0 * FEET, 0.0, 0.0),
        DVec3::new(20.0 * FEET, 0.0, 0.0),
        lanes,
        form,
    );
    // An upright support across the deck at x = 0, standing on the floor.
    let across = DQuat::from_rotation_y(std::f64::consts::FRAC_PI_2);
    let levels = (10.0 * FEET / size.y).round() as usize;
    for lane in 0..lanes {
        for level in 0..levels {
            panels.push(super::Placement {
                form,
                center: DVec3::new(
                    0.0,
                    -10.0 * FEET + size.y * (level as f64 + 0.5),
                    (lane as f64 + 0.5 - lanes as f64 * 0.5) * size.x,
                ),
                orientation: across,
            });
        }
    }
    (panels, stone)
}

#[test]
fn spans_over_20_feet_need_half_size_supports() {
    let caster = DVec3::new(-6.0, 1.0, 0.0);
    let (full, stone) = supported_span(Form::Thick);
    assert!(matches!(
        validate(&full, &stone, caster),
        Err(Refusal::NeedsHalfPanels { span }) if span > 29.0 * FEET
    ));
    let (half, stone) = supported_span(Form::Half);
    let plan = validate(&half, &stone, caster).expect("half-size panels with a support");
    assert!(plan.uses_supports);
    assert!(plan.span <= SPAN_LIMIT, "{}", plan.span);
    // Without the support the same deck is too long.
    let deck: Vec<_> = half.into_iter().filter(|p| !p.vertical()).collect();
    assert!(matches!(
        validate(&deck, &stone, caster),
        Err(Refusal::SpanTooLong { .. })
    ));
}

#[test]
fn shapes_stand_by_the_rules() {
    let scene = Scene::floor();
    let caster = DVec3::new(0.0, 1.0, -8.0);
    let ring = validate(&enclosure(DVec3::ZERO, Form::Thick), &scene.stone, caster).unwrap();
    assert_eq!(ring.panels.len(), 5);
    assert!(!ring.uses_supports);
    let tall = validate(&tower(DVec3::ZERO, 2, Form::Thick), &scene.stone, caster).unwrap();
    assert_eq!(tall.panels.len(), 9);
    let line = validate(
        &straight(DVec3::new(-15.0, 0.0, 0.0), DVec3::X, 10, Form::Thick),
        &scene.stone,
        DVec3::new(0.0, 1.0, 0.0),
    )
    .unwrap();
    assert_eq!(line.seams.len(), 9);
    // A ramp up a 6 m ledge needs half-size panels and supports.
    let mut scene = Scene::floor();
    scene.rock(DVec3::new(4.0, 0.0, -6.0), DVec3::new(12.0, 6.0, 6.0));
    let (foot, lip) = (DVec3::new(-6.6, 0.0, 0.0), DVec3::new(4.0, 6.0, 0.0));
    assert!(validate(&ramp(foot, lip, 1, Form::Thick), &scene.stone, caster).is_err());
    let plan = validate(&ramp(foot, lip, 2, Form::Half), &scene.stone, caster)
        .expect("a supported ramp stands");
    assert!(plan.uses_supports);
    assert!(plan.panels.len() as u32 * Form::Half.cost() <= super::BUDGET);
}

#[test]
fn an_intact_20_foot_bridge_carries_a_large_creature() {
    for form in [Form::Thick, Form::Half] {
        let mut scene = Scene::chasm(20.0 * FEET);
        let reach = if form == Form::Thick { 15.0 } else { 12.5 } * FEET;
        let panels = bridge(
            DVec3::new(-reach, 0.0, 0.0),
            DVec3::new(reach, 0.0, 0.0),
            1,
            form,
        );
        let mut wall = scene.raise(&panels);
        let deck = form.size().z;
        let load = scene.load(300.0, DVec3::new(0.0, deck, 0.0));
        let events = scene.run(&mut wall, 6.0);
        assert!(broke(&events).is_empty(), "{form:?}: {events:?}");
        for panel in &wall.panels {
            let body = &scene.world[panel.body];
            assert!(
                body.pos.y > deck * 0.5 - 0.02,
                "{form:?} sagged to {}",
                body.pos.y
            );
        }
        assert!(scene.world[load].pos.y > deck + 0.25, "{form:?}");
    }
}

/// A 20-foot bridge of five half-size panels; the outer two half on the lips.
fn five_panel_bridge() -> (Scene, Wall) {
    let mut scene = Scene::chasm(20.0 * FEET);
    let panels = bridge(
        DVec3::new(-12.5 * FEET, 0.0, 0.0),
        DVec3::new(12.5 * FEET, 0.0, 0.0),
        1,
        Form::Half,
    );
    assert_eq!(panels.len(), 5);
    let mut wall = scene.raise(&panels);
    assert!(broke(&scene.run(&mut wall, 1.0)).is_empty());
    (scene, wall)
}

#[test]
fn destroying_the_middle_panel_drops_both_halves() {
    let (mut scene, mut wall) = five_panel_bridge();
    let time = scene.world.time();
    let events = wall.destroy(&mut scene.world, 2, time, 7);
    let Event::Destroyed { debris, .. } = &events else {
        panic!("{events:?}")
    };
    assert!((4..=8).contains(&debris.len()));
    // The middle panel's two seams went with it.
    let gone: Vec<BondKind> = wall
        .bonds
        .iter()
        .filter(|b| !b.intact())
        .map(|b| b.kind)
        .collect();
    assert_eq!(
        gone,
        vec![BondKind::Seam { a: 1, b: 2 }, BondKind::Seam { a: 2, b: 3 }]
    );
    // Each half tips off its lip and tears its footing.
    let events = scene.run(&mut wall, 5.0);
    let mut torn: Vec<BondKind> = broke(&events).iter().map(|&i| wall.bonds[i].kind).collect();
    torn.sort_by_key(|k| format!("{k:?}"));
    assert_eq!(
        torn,
        vec![
            BondKind::Footing { panel: 0, stone: 0 },
            BondKind::Footing { panel: 4, stone: 1 }
        ],
        "{events:?}"
    );
    for panel in [0, 1, 3, 4] {
        let y = scene.world[wall.panels[panel].body].pos.y;
        assert!(y < -3.0, "panel {panel} only fell to {y}");
    }
}

#[test]
fn a_forced_30_foot_cantilever_breaks_at_its_root() {
    let mut scene = Scene::new();
    scene.rock(DVec3::new(-20.0, -6.0, -6.0), DVec3::new(0.0, 0.0, 6.0));
    scene.rock(
        DVec3::new(-30.0, -21.0, -30.0),
        DVec3::new(30.0, -20.0, 30.0),
    );
    let panels = bridge(
        DVec3::new(-10.0 * FEET, 0.0, 0.0),
        DVec3::new(30.0 * FEET, 0.0, 0.0),
        1,
        Form::Thick,
    );
    assert!(validate(&panels, &scene.stone, DVec3::ZERO).is_err());
    let plan = force(&panels, &scene.stone).unwrap();
    let mut wall = Wall::raise(&mut scene.world, &plan, &scene.bodies, 0.0);
    let events = scene.run(&mut wall, 4.0);
    let torn: Vec<BondKind> = broke(&events).iter().map(|&i| wall.bonds[i].kind).collect();
    assert!(torn.contains(&BondKind::Seam { a: 0, b: 1 }), "{torn:?}");
    assert!(scene.world[wall.panels[0].body].pos.y > 0.0);
    for panel in 1..4 {
        assert!(scene.world[wall.panels[panel].body].pos.y < -5.0);
    }
}

#[test]
fn debris_keeps_the_panel_mass_and_momentum() {
    let mut scene = Scene::floor();
    let mut wall = scene.raise(&straight(DVec3::ZERO, DVec3::X, 1, Form::Thick));
    let id = wall.panels[0].body;
    scene.world[id].vel = DVec3::new(0.4, 1.0, -0.2);
    scene.world[id].omega = DVec3::new(0.0, 0.3, 0.1);
    let before = scene.world[id];
    for seed in 0..3 {
        let mut w = scene.world.clone();
        let mut wall = wall.clone();
        let Event::Destroyed { debris, .. } = wall.destroy(&mut w, 0, 0.0, seed) else {
            panic!()
        };
        assert!((4..=8).contains(&debris.len()));
        let mass: f64 = debris.iter().map(|&b| w[b].mass).sum();
        let momentum: DVec3 = debris.iter().map(|&b| w[b].momentum()).sum();
        assert!((mass - before.mass).abs() < 1e-9);
        assert!((momentum - before.momentum()).length() < 1e-9);
    }
    // Debris despawns after 20 s.
    let Event::Destroyed { debris, .. } = wall.destroy(&mut scene.world, 0, 0.0, 1) else {
        panic!()
    };
    let events = scene.run(&mut wall, 20.5);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::DebrisGone { .. }))
            .count(),
        debris.len()
    );
    assert!(debris.iter().all(|&b| scene.world[b].removed));
}

#[test]
fn ending_concentration_early_drops_what_stood_on_the_wall() {
    let mut scene = Scene::floor();
    let mut wall = scene.raise(&tower(DVec3::ZERO, 2, Form::Thick));
    let roof = 20.0 * FEET + Form::Thick.size().z;
    let load = scene.load(75.0, DVec3::new(0.0, roof, 0.0));
    assert!(broke(&scene.run(&mut wall, 2.0)).is_empty());
    assert!(scene.world[load].pos.y > roof);
    assert_eq!(wall.end(&mut scene.world, 2.0), vec![Event::Vanished]);
    assert!(wall.panels.iter().all(|p| scene.world[p.body].removed));
    scene.run(&mut wall, 3.0);
    assert!(scene.world[load].pos.y < 0.4, "{}", scene.world[load].pos.y);
}

#[test]
fn a_wall_held_for_ten_minutes_is_permanent() {
    let mut scene = Scene::floor();
    let mut wall = scene.raise(&enclosure(DVec3::ZERO, Form::Thick));
    scene.run(&mut wall, 1.0);
    assert!(
        wall.after_step(&mut scene.world, DURATION - 0.01)
            .is_empty()
    );
    assert_eq!(
        wall.after_step(&mut scene.world, DURATION),
        vec![Event::Permanent]
    );
    assert!(wall.end(&mut scene.world, DURATION + 1.0).is_empty());
    assert!(wall.panels.iter().all(|p| !scene.world[p.body].removed));
    assert_eq!(wall.intact().count(), wall.bonds.len());
}

#[test]
fn a_wall_through_a_creature_pushes_it_to_the_chosen_side() {
    let dummy = Creature {
        feet: DVec3::new(0.3, 0.0, 0.05),
        radius: 0.35,
        height: 1.8,
    };
    let wall = straight(DVec3::new(-1.524, 0.0, 0.0), DVec3::X, 1, Form::Thick);
    let clear = Form::Thick.size().z * 0.5 + 0.35 + super::creatures::CLEARANCE;
    for side in [1.0, -1.0] {
        let to = push_out(&wall, dummy, DVec3::Z * side).expect("in the wall's space");
        assert!((to.z - clear * side).abs() < 1e-9, "{to}");
        assert_eq!((to.x, to.y), (0.3, 0.0));
    }
    let aside = Creature {
        feet: DVec3::new(0.0, 0.0, 2.0),
        ..dummy
    };
    assert_eq!(push_out(&wall, aside, DVec3::Z), None);
}

#[test]
fn an_enclosed_creature_saves_to_escape() {
    let scene = Scene::floor();
    let ring = enclosure(DVec3::ZERO, Form::Thick);
    let dummy = Creature {
        feet: DVec3::ZERO,
        radius: 0.35,
        height: 1.8,
    };
    match resolve_enclosure(&ring, &scene.stone, dummy, 14, 2, 15) {
        Enclosure::Escaped {
            total,
            distance,
            to,
            ..
        } => {
            assert_eq!(total, 16);
            // Out past the 3.2 m footprint's nearest face.
            assert!((distance - (1.6 + 0.35 + 0.02)).abs() < 0.01, "{distance}");
            assert!(to.x.abs().max(to.z.abs()) > 1.6 + 0.35);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        resolve_enclosure(&ring, &scene.stone, dummy, 3, 2, 15),
        Enclosure::Trapped { roll: 3, total: 5 }
    );
    let outside = Creature {
        feet: DVec3::new(5.0, 0.0, 0.0),
        ..dummy
    };
    assert_eq!(
        resolve_enclosure(&ring, &scene.stone, outside, 3, 2, 15),
        Enclosure::Free
    );
    // Without its roof the ring does not enclose.
    assert_eq!(
        resolve_enclosure(&ring[..4], &scene.stone, dummy, 3, 2, 15),
        Enclosure::Free
    );
}

#[test]
fn a_checkpoint_with_broken_joints_replays_identically() {
    let (mut scene, mut wall) = five_panel_bridge();
    let time = scene.world.time();
    wall.destroy(&mut scene.world, 2, time, 4);
    let events = scene.run(&mut wall, 0.8);
    assert!(
        wall.bonds.iter().filter(|b| !b.intact()).count() >= 2,
        "{events:?}"
    );
    let saved = serde_json::to_string(&(&scene.world, &wall)).unwrap();
    let (world, restored): (World, Wall) = serde_json::from_str(&saved).unwrap();
    let mut copy = Scene {
        world,
        stone: scene.stone.clone(),
        bodies: scene.bodies.clone(),
    };
    let mut restored = restored;
    let (mut a, mut b) = (Trace::default(), Trace::default());
    for _ in 0..360 {
        scene.run(&mut wall, DT);
        copy.run(&mut restored, DT);
        a.record(&scene.world);
        b.record(&copy.world);
    }
    a.compare(&b, Tolerance::EXACT).unwrap();
    assert_eq!(wall, restored);
}
