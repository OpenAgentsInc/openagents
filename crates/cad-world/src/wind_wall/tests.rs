use super::*;
use physics::{Momentum, Uniform};

const DT: f64 = 1.0 / 120.0;
const GRAVITY: DVec3 = DVec3::new(0.0, -9.81, 0.0);

fn flat(_: DVec2) -> Option<f64> {
    Some(0.0)
}

/// A straight wall along x through the origin, 40 feet long.
fn straight() -> Wall {
    Wall::new(
        Wall::straight(DVec2::ZERO, DVec2::X, 40.0 * FOOT),
        DVec3::new(0.0, 0.0, -6.0),
        flat,
    )
    .unwrap()
}

fn floor(world: &mut World) {
    let floor = world
        .add(Body::new(1.0, DVec3::ONE, DVec3::new(0.0, -0.5, 0.0)).with_kind(BodyKind::Static));
    world.add_collider(Collider::new(
        floor,
        Shape::Cuboid {
            half: DVec3::new(30.0, 0.5, 30.0),
        },
    ));
}

fn cube(world: &mut World, mass: f64, side: f64, pos: DVec3, vel: DVec3) -> BodyId {
    let size = DVec3::splat(side);
    let mut body = Body::new(mass, Body::box_inertia(mass, size), pos);
    body.vel = vel;
    let id = world.add(body);
    world.add_collider(Collider::new(id, Shape::Cuboid { half: size / 2.0 }));
    id
}

/// A seeded uniform draw in [0, 1).
struct Dice(u64);

impl Dice {
    fn next(&mut self) -> f64 {
        self.0 = mix(self.0);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[test]
fn shapes_are_at_most_fifty_feet_continuous_and_grounded() {
    let caster = DVec3::ZERO;
    let wall = Wall::new(
        Wall::straight(DVec2::new(0.0, 3.0), DVec2::X, 50.0 * FOOT),
        caster,
        flat,
    )
    .unwrap();
    assert!((wall.length() - MAX_LENGTH).abs() < 1e-9);
    assert_eq!(
        Wall::new(
            Wall::straight(DVec2::new(0.0, 3.0), DVec2::X, 51.0 * FOOT),
            caster,
            flat
        ),
        Err(Refusal::TooLong)
    );
    // An arc of radius 3 m through 270 degrees and an L of 20 + 25 feet.
    let arc = Wall::new(
        Wall::arc(DVec2::ZERO, 3.0, 0.0, 1.5 * std::f64::consts::PI),
        caster,
        flat,
    )
    .unwrap();
    assert!(arc.length() <= MAX_LENGTH && arc.length() > 13.0);
    let l = Wall::new(
        Wall::l_shape(
            DVec2::new(2.0, 2.0),
            DVec2::X,
            20.0 * FOOT,
            DVec2::Y,
            25.0 * FOOT,
        ),
        caster,
        flat,
    )
    .unwrap();
    assert!((l.length() - 45.0 * FOOT).abs() < 1e-9);
    // Continuity: every segment starts where the last one ended, and the
    // volume covers each vertex.
    for p in &l.path {
        assert!(l.contains(DVec3::new(p.x, 1.0, p.y), 0.0));
    }
    let malformed = [
        vec![DVec2::ZERO],
        vec![DVec2::ZERO, DVec2::ZERO],
        vec![DVec2::ZERO, DVec2::new(f64::NAN, 0.0)],
    ];
    for path in malformed {
        assert_eq!(Wall::new(path, caster, flat), Err(Refusal::Malformed));
    }
    // A 3 m chasm across x in [1, 4] and a 6 m ledge beyond x = 8.
    let hall = |p: DVec2| match p.x {
        x if (1.0..4.0).contains(&x) => None,
        x if x >= 8.0 => Some(6.0),
        _ => Some(0.0),
    };
    let across_chasm = vec![DVec2::new(-1.0, 0.0), DVec2::new(5.0, 0.0)];
    let up_the_ledge = vec![DVec2::new(5.0, 0.0), DVec2::new(9.0, 0.0)];
    let beside = vec![DVec2::new(-6.0, 0.0), DVec2::new(0.5, 0.0)];
    assert_eq!(
        Wall::new(across_chasm, caster, hall),
        Err(Refusal::NotGrounded)
    );
    assert_eq!(
        Wall::new(up_the_ledge, caster, hall),
        Err(Refusal::NotGrounded)
    );
    assert!(Wall::new(beside, caster, hall).is_ok());
    let far = vec![DVec2::new(RANGE + 1.0, 0.0), DVec2::new(RANGE + 2.0, 0.0)];
    assert_eq!(Wall::new(far, caster, flat), Err(Refusal::OutOfRange));
    let wall = straight();
    assert!((wall.top() - HEIGHT).abs() < 1e-12);
    assert!(wall.contains(DVec3::new(0.0, HEIGHT - 0.01, THICKNESS / 2.0 - 0.001), 0.0));
    assert!(!wall.contains(DVec3::new(0.0, 1.0, THICKNESS / 2.0 + 0.001), 0.0));
    assert!(!wall.contains(DVec3::new(0.0, HEIGHT + 0.01, 0.0), 0.0));
}

/// Fly `flight` for up to `seconds`, returning whether it ever touched a
/// standing target capsule at `feet` and the step that touched.
fn fly(flight: &mut Flight, wall: Option<&Wall>, feet: DVec3, seconds: f64) -> Option<u32> {
    for step in 0..(seconds / DT) as u32 {
        let start = flight.pos;
        flight.advance(DT, GRAVITY, wall);
        let touched =
            physics::continuous::sphere_capsule(start, flight.pos, 0.06, feet, feet, 0.35, 1.8)
                .unwrap();
        if touched.is_some() {
            return Some(step);
        }
        if flight.pos.y < -1.0 {
            return None;
        }
    }
    None
}

#[test]
fn every_ordinary_flight_toward_a_target_behind_the_wall_is_deflected() {
    let wall = straight();
    let feet = DVec3::new(0.0, 0.0, 3.0);
    let mut dice = Dice(0x5EED);
    let mut crossing = 0;
    for _ in 0..500 {
        // Archers anywhere 2 to 30 m in front of the wall shoot at the
        // target's chest at 20 to 400 m/s, leading for the drop (the high
        // end tunnels through a 1-foot slab in a single step).
        let origin = DVec3::new(
            -5.0 + 10.0 * dice.next(),
            0.5 + 3.0 * dice.next(),
            -2.0 - 28.0 * dice.next(),
        );
        let speed = 20.0 + 380.0 * dice.next();
        let chest = feet + DVec3::Y * 1.1;
        let time = chest.distance(origin) / speed;
        let lead = chest - GRAVITY * (0.5 * time * time);
        let mut flight = Flight::new(origin, (lead - origin).normalize() * speed, Tag::Ordinary);
        // Only shots that would hit the target through the wall's volume
        // count as launched at a target behind it.
        let mut free = flight;
        if fly(&mut free, None, feet, 3.0).is_none() {
            continue;
        }
        crossing += 1;
        let mut entry = None;
        for _ in 0..(3.0 / DT) as u32 {
            let before = flight.vel;
            if let Some(point) = flight.advance(DT, GRAVITY, Some(&wall)) {
                entry = Some((point, before + GRAVITY * DT));
                break;
            }
        }
        let (point, incoming) = entry.expect("the flight crosses the wall and is deflected");
        assert!(flight.deflected && !flight.can_damage_target());
        // The entry is on the slab's face, exactly.
        assert!((point.z + THICKNESS / 2.0).abs() < 1e-9, "{point}");
        let flat = DVec2::new(flight.vel.x, flight.vel.z).length();
        let rise = flight.vel.y - DEFLECT_BOOST;
        assert!(rise.atan2(flat) >= DEFLECT_ELEVATION - 1e-9);
        let speed_after = DVec3::new(flight.vel.x, rise, flight.vel.z).length();
        assert!((speed_after - incoming.length()).abs() < 1e-6);
        // It deals no damage: the rule forbids it, and it flies over.
        assert_eq!(fly(&mut flight, Some(&wall), feet, 3.0), None);
    }
    assert!(
        crossing > 450,
        "most shots would hit without the wall: {crossing}"
    );
}

#[test]
fn siege_and_spell_flights_are_unaffected() {
    let wall = straight();
    let feet = DVec3::new(0.0, 0.0, 3.0);
    for tag in [Tag::Siege, Tag::Spell] {
        for speed in [15.0, 40.0, 300.0] {
            let origin = DVec3::new(0.0, 1.5, -12.0);
            let aim = (feet + DVec3::Y * 1.1 - origin).normalize();
            let launched = Flight::new(origin, aim * speed, tag);
            let (mut with, mut without) = (launched, launched);
            for _ in 0..240 {
                assert_eq!(with.advance(DT, DVec3::ZERO, Some(&wall)), None);
                without.advance(DT, DVec3::ZERO, None);
                assert_eq!(with, without);
            }
            assert!(!with.deflected && with.can_damage_target());
            let mut straight = Flight::new(origin, aim * speed, tag);
            let mut reached = false;
            for _ in 0..240 {
                let start = straight.pos;
                straight.advance(DT, DVec3::ZERO, Some(&wall));
                reached |= physics::continuous::sphere_capsule(
                    start,
                    straight.pos,
                    0.06,
                    feet,
                    feet,
                    0.35,
                    1.8,
                )
                .unwrap()
                .is_some();
            }
            assert!(reached, "{tag:?} at {speed} m/s reaches the target");
        }
    }
}

struct Scene {
    world: World,
    spell: WindWall,
    props: Vec<Prop>,
    ledger: Ledger,
}

impl Scene {
    fn new() -> Self {
        let mut world = World::new(DT);
        floor(&mut world);
        let spell = WindWall::raise(&mut world, straight(), 0.0, 7);
        let ledger = Ledger::new(DVec3::ZERO, Momentum::default());
        Self {
            world,
            spell,
            props: Vec::new(),
            ledger,
        }
    }

    fn prop(&mut self, mass: f64, side: f64, size: Size, pos: DVec3, vel: DVec3) -> BodyId {
        let body = cube(&mut self.world, mass, side, pos, vel);
        self.props.push(Prop {
            body,
            size,
            radius: side / 2.0,
        });
        body
    }

    fn step(&mut self) {
        self.spell
            .before_step(&mut self.world, &self.props, &mut self.ledger);
        self.world.step(&Uniform(GRAVITY));
    }
}

#[test]
fn lightweight_bodies_rise_and_a_crate_is_not_lifted() {
    let mut scene = Scene::new();
    // Paper and leaves resting 0.4 m in front of the wall, blown toward it.
    let light: Vec<BodyId> = [0.02, 0.1, 0.5, 2.0]
        .iter()
        .enumerate()
        .map(|(i, &mass)| {
            scene.prop(
                mass,
                0.15,
                Size::Tiny,
                DVec3::new(-3.0 + 2.0 * i as f64, 0.075, -0.4),
                DVec3::new(0.0, 0.0, 3.0),
            )
        })
        .collect();
    // A 20 kg crate pushed along the floor into the wall.
    let crate_body = scene.prop(
        20.0,
        0.6,
        Size::Small,
        DVec3::new(4.0, 0.3, -1.2),
        DVec3::new(0.0, 0.0, 4.0),
    );
    let mut highest = vec![0.0f64; light.len()];
    let mut crate_highest: f64 = 0.0;
    for _ in 0..(4.0 / DT) as u32 {
        scene.step();
        for (h, id) in highest.iter_mut().zip(&light) {
            *h = h.max(scene.world[*id].pos.y);
        }
        crate_highest = crate_highest.max(scene.world[crate_body].pos.y);
    }
    for (h, id) in highest.iter().zip(&light) {
        assert!(
            *h > HEIGHT,
            "a {} kg body rises out of the top (peak {h})",
            scene.world[*id].mass
        );
    }
    assert!(
        crate_highest < 0.31,
        "the crate is not lifted: {crate_highest}"
    );
    // Grounded, it slid through the wall instead of bouncing.
    assert!(scene.world[crate_body].pos.z > THICKNESS / 2.0 + 0.3);
    let updraft = scene.ledger.external[LEDGER_TERM].linear;
    assert!(updraft.y > 0.0);
}

#[test]
fn an_airborne_small_prop_bounces_and_grounded_things_pass() {
    let mut scene = Scene::new();
    // A crate thrown at the wall 1.5 m up, as Telekinesis would.
    let thrown = scene.prop(
        20.0,
        0.6,
        Size::Small,
        DVec3::new(0.0, 1.5, -2.0),
        DVec3::new(0.0, 1.0, 8.0),
    );
    let mut bounced = false;
    for _ in 0..(3.0 / DT) as u32 {
        scene.step();
        let body = scene.world[thrown];
        bounced |= body.vel.z < -1.0;
        assert!(body.pos.z < 0.0, "the thrown crate never crosses");
    }
    assert!(bounced);
    // A Medium body thrown the same way is not stopped.
    let mut scene = Scene::new();
    let dummy = scene.prop(
        75.0,
        0.6,
        Size::Medium,
        DVec3::new(0.0, 1.5, -2.0),
        DVec3::new(0.0, 1.0, 8.0),
    );
    for _ in 0..(1.5 / DT) as u32 {
        scene.step();
    }
    assert!(scene.world[dummy].pos.z > 1.0);
    // Creatures: only Small-or-smaller fliers are stopped.
    assert!(Wall::blocks_creature(Size::Small, true));
    assert!(Wall::blocks_creature(Size::Tiny, true));
    assert!(!Wall::blocks_creature(Size::Medium, true));
    assert!(!Wall::blocks_creature(Size::Small, false));
    assert!(!Wall::blocks_creature(Size::Huge, false));
}

#[test]
fn appearance_damage_is_four_d8_or_half_on_a_save() {
    let wall = straight();
    assert!(wall.in_area(DVec3::new(1.0, 0.0, 0.3), 0.35, 1.8));
    assert!(!wall.in_area(DVec3::new(1.0, 0.0, 0.6), 0.35, 1.8));
    assert!(!wall.in_area(DVec3::new(7.0, 0.0, 0.0), 0.35, 1.8));
    let mut faces = [8, 5, 3, 1].into_iter();
    let (rolls, failed) = appearance_damage(false, || faces.next().unwrap());
    assert_eq!(rolls, vec![8, 5, 3, 1]);
    assert_eq!(failed, 17);
    let mut faces = [8, 5, 3, 1].into_iter();
    assert_eq!(appearance_damage(true, || faces.next().unwrap()).1, 8);
    let mut dice = Dice(3);
    for _ in 0..200 {
        let (rolls, damage) = appearance_damage(false, || 1 + (dice.next() * 8.0) as i32);
        assert_eq!(rolls.len(), DAMAGE_DICE);
        assert!((4..=32).contains(&damage));
    }
}

#[test]
fn the_wall_clears_overlapping_gas() {
    let wall = straight();
    assert!(wall.clears_gas(DVec3::new(2.0, 1.0, 1.5), 2.0));
    assert!(!wall.clears_gas(DVec3::new(2.0, 1.0, 4.0), 2.0));
}

#[test]
fn concentration_end_removes_the_field_and_the_colliders() {
    let mut scene = Scene::new();
    let leaf = scene.prop(
        0.05,
        0.15,
        Size::Tiny,
        DVec3::new(0.0, 0.075, 0.0),
        DVec3::ZERO,
    );
    let props = scene.props.clone();
    scene.spell.end(&mut scene.world, &props);
    assert!(!scene.spell.active(0.0));
    for id in &scene.spell.bodies {
        assert!(scene.world[*id].removed);
    }
    for _ in 0..240 {
        scene.step();
    }
    assert!(scene.world[leaf].pos.y < 0.1, "nothing lifts the leaf");
    // A minute after the cast, concentration lapses on its own.
    let mut scene = Scene::new();
    scene.spell.until = 0.5;
    let leaf = scene.prop(
        0.05,
        0.15,
        Size::Tiny,
        DVec3::new(0.0, 0.075, 0.0),
        DVec3::ZERO,
    );
    for _ in 0..(0.25 / DT) as u32 {
        scene.step();
    }
    assert!(
        scene.world[leaf].pos.y > 0.2,
        "the wall lifts it while it stands"
    );
    for _ in 0..(3.0 / DT) as u32 {
        scene.step();
    }
    assert!(scene.spell.ended);
    assert!(scene.world[leaf].pos.y < 0.1, "it falls once the wall ends");
}

#[test]
fn a_checkpoint_with_a_deflected_flight_replays_identically() {
    #[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
    struct Saved {
        world: World,
        spell: WindWall,
        props: Vec<Prop>,
        ledger: Ledger,
        flights: Vec<Flight>,
    }
    let mut scene = Scene::new();
    for i in 0..6 {
        scene.prop(
            0.1,
            0.15,
            Size::Tiny,
            DVec3::new(-2.0 + i as f64 * 0.8, 0.075, -0.3),
            DVec3::new(0.0, 0.0, 2.0),
        );
    }
    let flights: Vec<Flight> = (0..5)
        .map(|i| {
            Flight::new(
                DVec3::new(i as f64 - 2.0, 1.5, -8.0),
                DVec3::new(0.0, 0.0, 30.0),
                Tag::Ordinary,
            )
        })
        .collect();
    let run = |s: &mut Saved, steps: u32| {
        for _ in 0..steps {
            s.spell.before_step(&mut s.world, &s.props, &mut s.ledger);
            s.world.step(&Uniform(GRAVITY));
            for f in &mut s.flights {
                f.advance(DT, GRAVITY, Some(&s.spell.wall));
            }
        }
    };
    let mut live = Saved {
        world: scene.world,
        spell: scene.spell,
        props: scene.props,
        ledger: scene.ledger,
        flights,
    };
    run(&mut live, 40);
    assert!(live.flights.iter().all(|f| f.deflected));
    assert!(
        live.flights.iter().all(|f| f.pos.y < 6.0),
        "still in flight"
    );
    let bytes = serde_json::to_string(&live).unwrap();
    let mut restored: Saved = serde_json::from_str(&bytes).unwrap();
    assert_eq!(restored, live);
    run(&mut live, 300);
    run(&mut restored, 300);
    assert_eq!(restored, live);
}
