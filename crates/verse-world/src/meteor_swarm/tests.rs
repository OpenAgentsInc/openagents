use super::*;
use crate::spells::Dice;
use physics::trace::{Tolerance, Trace};
use physics::{Ledger, Momentum, Uniform};

const DT: f64 = 1.0 / 120.0;
const G: f64 = 9.81;
const DOWN: DVec3 = DVec3::new(0.0, -G, 0.0);
const DC: i32 = 15;

/// A bench whose damage dice and d20s can show fixed faces.
struct Rigged {
    bench: Bench,
    die: Option<u32>,
    d20: Option<u32>,
}

impl Host for Rigged {
    fn world(&self) -> &World {
        &self.bench.world
    }
    fn spawn_meteor(&mut self, start: DVec3, velocity: DVec3) -> Result<BodyId, String> {
        self.bench.spawn_meteor(start, velocity)
    }
    fn remove_meteor(&mut self, body: BodyId) -> Result<(), String> {
        self.bench.remove_meteor(body)
    }
    fn impulse(&mut self, body: BodyId, impulse: DVec3, at: DVec3) -> Result<(), String> {
        self.bench.impulse(body, impulse, at)
    }
    fn shatter(&mut self, body: BodyId) -> Result<Vec<BodyId>, String> {
        self.bench.shatter(body)
    }
    fn damage_die(&mut self, sides: u32) -> u32 {
        self.die.unwrap_or_else(|| self.bench.damage_die(sides))
    }
    fn save_d20(&mut self, creature: &Creature) -> u32 {
        self.d20.unwrap_or_else(|| self.bench.save_d20(creature))
    }
}

fn rigged(world: World, seed: u64) -> Rigged {
    Rigged {
        bench: Bench {
            world,
            dice: Dice::new(seed),
        },
        die: None,
        d20: None,
    }
}

fn static_box(world: &mut World, center: DVec3, half: DVec3) -> BodyId {
    let id = world.add(Body::new(1.0, DVec3::ONE, center).with_kind(BodyKind::Static));
    world.add_collider(Collider::new(id, Shape::Cuboid { half }));
    id
}

fn floor(world: &mut World) -> BodyId {
    static_box(
        world,
        DVec3::new(0.0, -0.5, 0.0),
        DVec3::new(200.0, 0.5, 200.0),
    )
}

fn block(world: &mut World, mass: f64, half: DVec3, at: DVec3) -> BodyId {
    let id = world.add(Body::new(mass, Body::box_inertia(mass, half * 2.0), at));
    world.add_collider(Collider::new(id, Shape::Cuboid { half }));
    id
}

fn crate_at(world: &mut World, x: f64, z: f64) -> BodyId {
    block(world, 20.0, DVec3::splat(0.25), DVec3::new(x, 0.25, z))
}

fn dummy(id: u64, x: f64, z: f64) -> Creature {
    Creature {
        id,
        feet: DVec3::new(x, 0.0, z),
        radius: 0.35,
        height: 1.8,
        dexterity: 0,
    }
}

/// Four points far apart on a line 40 m from the caster.
fn points() -> [DVec3; METEORS] {
    [-45.0, -15.0, 15.0, 45.0].map(|x| DVec3::new(x, 0.0, 40.0))
}

fn flat() -> World {
    let mut world = World::new(DT);
    floor(&mut world);
    world
}

fn settle(world: &mut World) {
    for _ in 0..60 {
        world.step(&Uniform(DOWN));
    }
}

struct Scene {
    host: Rigged,
    swarm: MeteorSwarm,
    creatures: Vec<Creature>,
    objects: Vec<Unattended>,
}

impl Scene {
    fn cast(world: World, points: [DVec3; METEORS], seed: u64) -> Self {
        Self::cast_rigged(rigged(world, seed), points)
    }

    fn cast_rigged(mut host: Rigged, points: [DVec3; METEORS]) -> Self {
        let swarm = MeteorSwarm::cast(
            &mut host,
            DVec3::ZERO,
            points,
            G,
            Flight::STANDARD,
            DC,
            |_| true,
        )
        .unwrap();
        Self {
            host,
            swarm,
            creatures: Vec::new(),
            objects: Vec::new(),
        }
    }

    fn step(&mut self) -> Vec<Impact> {
        self.host.bench.world.step(&Uniform(DOWN));
        let impacts = self
            .swarm
            .after_step(&mut self.host, &self.creatures, &mut self.objects)
            .unwrap();
        burn(&mut self.host, &mut self.objects).unwrap();
        impacts
    }

    fn run(&mut self, seconds: f64) -> Vec<Impact> {
        let mut impacts = Vec::new();
        for _ in 0..ticks(&self.host.bench.world, seconds) {
            impacts.extend(self.step());
        }
        impacts
    }

    fn world(&self) -> &World {
        &self.host.bench.world
    }
}

#[test]
fn four_meteors_fall_a_quarter_second_apart_and_strike_their_points() {
    let mut scene = Scene::cast(flat(), points(), 1);
    assert_eq!(scene.swarm.falling().count(), 1);
    let spawns: Vec<u64> = scene.swarm.meteors.iter().map(|m| m.spawn_tick).collect();
    assert_eq!(spawns, vec![0, 30, 60, 90]);
    for meteor in &scene.swarm.meteors {
        assert!((meteor.start.y - meteor.point.y - SPAWN_HEIGHT - METEOR_RADIUS).abs() < 1e-9);
        assert!((meteor.velocity.length() - INITIAL_SPEED).abs() < 1e-9);
        let slant = meteor.velocity.angle_between(-DVec3::Y).to_degrees();
        assert!((slant - SLANT_DEGREES).abs() < 1e-9);
        // From the caster's side: it moves away from the caster.
        let away = meteor.point - DVec3::ZERO;
        assert!(meteor.velocity.x * away.x + meteor.velocity.z * away.z > 0.0);
    }
    let impacts = scene.run(4.0);
    assert!(scene.swarm.finished());
    assert_eq!(impacts.len(), METEORS);
    for (i, impact) in impacts.iter().enumerate() {
        assert_eq!(impact.meteor, i);
        assert_eq!(impact.radius, RADIUS);
        assert!((impact.radius - 12.192).abs() < 1e-9);
        assert!(!impact.obstructed, "{impact:?}");
        assert!(impact.center.distance(impact.point) < 0.6, "{impact:?}");
    }
    // Staggered starts and equal flights give staggered impacts.
    for pair in impacts.windows(2) {
        assert_eq!(pair[1].tick - pair[0].tick, 30);
    }
}

#[test]
fn the_sphere_reaches_exactly_forty_feet() {
    let mut scene = Scene::cast(flat(), points(), 2);
    let p = points()[0];
    scene.creatures = vec![
        dummy(1, p.x + RADIUS - 1.0, p.z),
        dummy(2, p.x + RADIUS + 1.2, p.z),
    ];
    let impacts = scene.run(4.0);
    let ids: Vec<u64> = impacts
        .iter()
        .flat_map(|i| i.creatures.iter().map(|c| c.id))
        .collect();
    assert_eq!(ids, vec![1]);
}

#[test]
fn a_creature_in_two_spheres_is_damaged_once() {
    // The first two points are 10 m apart; the dummy between them is in
    // both Spheres.
    let points = [
        DVec3::new(-5.0, 0.0, 40.0),
        DVec3::new(5.0, 0.0, 40.0),
        DVec3::new(60.0, 0.0, 40.0),
        DVec3::new(-60.0, 0.0, 40.0),
    ];
    let mut scene = Scene::cast(flat(), points, 3);
    scene.creatures = vec![dummy(7, 0.0, 41.0), dummy(8, 9.0, 40.0)];
    let impacts = scene.run(4.0);
    let hits: Vec<&CreatureHit> = impacts.iter().flat_map(|i| &i.creatures).collect();
    assert_eq!(hits.iter().filter(|h| h.id == 7).count(), 1);
    assert_eq!(hits.iter().filter(|h| h.id == 8).count(), 1);
    let first = hits.iter().find(|h| h.id == 7).unwrap();
    assert_eq!(first.meteor, 0);
    // The second Sphere also covers dummy 7 but adds nothing.
    assert!(impacts[1].creatures.iter().all(|h| h.id != 7));
    assert_eq!(scene.swarm.affected.len(), 2);
}

#[test]
fn damage_is_twenty_d6_of_each_type_or_half_on_a_save() {
    let damage = Damage::roll(&mut |_| 6);
    assert_eq!(
        damage,
        Damage {
            fire: 120,
            bludgeoning: 120
        }
    );
    // The first twenty dice are Fire, the next twenty Bludgeoning.
    let mut n = 0;
    let damage = Damage::roll(&mut |_| {
        n += 1;
        if n <= 20 { 1 } else { 3 }
    });
    assert_eq!(
        damage,
        Damage {
            fire: 20,
            bludgeoning: 60
        }
    );
    assert_eq!(
        damage.halved(),
        Damage {
            fire: 10,
            bludgeoning: 30
        }
    );
    // Odd totals round down per type.
    let odd = Damage {
        fire: 71,
        bludgeoning: 69,
    };
    assert_eq!(
        odd.halved(),
        Damage {
            fire: 35,
            bludgeoning: 34
        }
    );
    // Rolled dice stay in range.
    let mut dice = Dice::new(9);
    for _ in 0..200 {
        let d = Damage::roll(&mut |s| dice.roll(s));
        assert!((20..=120).contains(&d.fire) && (20..=120).contains(&d.bludgeoning));
    }

    // In a cast: every die shows 4 and every d20 shows 1; a dummy with +20
    // Dexterity saves.
    let mut host = rigged(flat(), 1);
    host.die = Some(4);
    host.d20 = Some(1);
    let mut scene = Scene::cast_rigged(host, points());
    assert_eq!(
        scene.swarm.damage,
        Damage {
            fire: 80,
            bludgeoning: 80
        }
    );
    scene.creatures = vec![
        dummy(1, -45.0, 42.0),
        Creature {
            dexterity: 20,
            ..dummy(2, -43.0, 40.0)
        },
    ];
    let hits: Vec<CreatureHit> = scene
        .run(3.0)
        .into_iter()
        .flat_map(|i| i.creatures)
        .collect();
    assert_eq!(hits.len(), 2);
    assert!(!hits[0].save.success);
    assert_eq!(
        hits[0].damage,
        Damage {
            fire: 80,
            bludgeoning: 80
        }
    );
    assert!(hits[1].save.success);
    assert_eq!(
        hits[1].damage,
        Damage {
            fire: 40,
            bludgeoning: 40
        }
    );
}

#[test]
fn creatures_are_not_displaced() {
    let mut scene = Scene::cast(flat(), points(), 4);
    let before = vec![dummy(1, -45.0, 41.0), dummy(2, -44.0, 38.0)];
    scene.creatures = before.clone();
    let bodies = scene.world().bodies().len();
    let impacts = scene.run(4.0);
    assert_eq!(impacts[0].creatures.len(), 2);
    // The spell only reads creatures and adds no body for them; the only
    // bodies it creates are the three meteors spawned after the cast.
    assert_eq!(scene.creatures, before);
    assert_eq!(scene.world().bodies().len(), bodies + 3);
}

#[test]
fn a_crate_three_meters_out_flies_about_fifteen_meters() {
    let mut world = flat();
    let crate_ = crate_at(&mut world, 3.0, 0.0);
    settle(&mut world);
    let start = world[crate_].pos;
    let impulse = blast(DVec3::ZERO, start);
    let launch = impulse / 20.0;
    assert!((launch.length() - 13.0).abs() < 0.1, "{launch}");
    let elevation = launch.angle_between(DVec3::new(launch.x, 0.0, launch.z));
    assert!((elevation.to_degrees() - BLAST_UPWARD_DEGREES).abs() < 1e-9);
    world[crate_].apply_impulse_at(impulse, start);
    let mut airborne = false;
    let mut landed = None;
    for _ in 0..ticks(&world, 4.0) {
        world.step(&Uniform(DOWN));
        let body = world[crate_];
        airborne |= body.pos.y > start.y + 0.5;
        if airborne && body.vel.y <= 0.0 && body.pos.y <= start.y + 0.05 {
            landed = Some(body.pos);
            break;
        }
    }
    let landed = landed.expect("the crate lands");
    let carried = (landed - start).with_y(0.0).length();
    assert!((carried - 15.0).abs() < 1.0, "carried {carried} m");
}

#[test]
fn unattended_objects_take_full_damage_break_and_feel_the_blast() {
    let p = points()[0];
    let mut world = flat();
    let crate_ = crate_at(&mut world, p.x + 3.0, p.z);
    let stone = block(
        &mut world,
        1_000.0,
        DVec3::splat(0.5),
        DVec3::new(p.x - 4.0, 0.5, p.z),
    );
    let far = crate_at(&mut world, p.x, p.z + RADIUS + 2.0);
    settle(&mut world);
    let mut scene = Scene::cast(world, points(), 5);
    scene.objects = vec![
        Unattended::new(crate_, Some(10), false),
        Unattended::new(stone, None, false),
        Unattended::new(far, Some(10), false),
    ];
    let mut impacts = Vec::new();
    while impacts.is_empty() {
        impacts.extend(scene.step());
    }
    let stone_after = scene.world()[stone].vel;
    let impact = &impacts[0];
    let damage = scene.swarm.damage;
    let crate_hit = impact.objects.iter().find(|h| h.body == crate_).unwrap();
    assert_eq!(crate_hit.damage, Some(damage), "full damage, no save");
    assert!(crate_hit.broke);
    assert_eq!(crate_hit.debris.len(), DEBRIS_CHUNKS);
    assert!(scene.world()[crate_].removed);
    // Debris carries the crate's mass and its momentum after the blast.
    let chunks: Vec<Body> = crate_hit.debris.iter().map(|&d| scene.world()[d]).collect();
    let mass: f64 = chunks.iter().map(|c| c.mass).sum();
    assert!((mass - 20.0).abs() < 1e-9);
    let momentum: DVec3 = chunks.iter().map(Body::momentum).sum();
    assert!((momentum - crate_hit.impulse).length() < 1e-6, "{momentum}");
    // A 1,000 kg block survives and is shoved by the blast.
    let stone_hit = impact.objects.iter().find(|h| h.body == stone).unwrap();
    assert!(stone_hit.debris.is_empty() && !stone_hit.broke);
    assert!(stone_hit.impulse.length() > 0.0);
    let expected = stone_hit.impulse / 1_000.0;
    assert!((stone_after - expected).length() < 0.05, "{expected}");
    // Outside the Sphere: nothing.
    assert!(impact.objects.iter().all(|h| h.body != far));
    assert_eq!(scene.objects[2].hp, Some(10));
}

#[test]
fn debris_of_a_spinning_box_keeps_its_momentum() {
    let mut world = World::new(DT);
    let id = block(
        &mut world,
        40.0,
        DVec3::new(0.4, 0.3, 0.2),
        DVec3::new(1.0, 2.0, 3.0),
    );
    world[id].orientation = glam::DQuat::from_rotation_y(0.7);
    world[id].vel = DVec3::new(1.0, 2.0, -3.0);
    world[id].omega = DVec3::new(0.5, -1.0, 2.0);
    let origin = DVec3::ZERO;
    let before = Momentum::of(&world[id], origin);
    let chunks = debris(&world, id);
    assert_eq!(chunks.len(), DEBRIS_CHUNKS);
    let after = chunks
        .iter()
        .fold(Momentum::ZERO, |sum, c| sum + Momentum::of(&c.body, origin));
    assert!((after.linear - before.linear).length() < 1e-9);
    assert!((after.angular - before.angular).length() < 1e-9);
}

#[test]
fn overlapping_spheres_damage_an_object_once_but_blast_it_twice() {
    let points = [
        DVec3::new(-4.0, 0.0, 40.0),
        DVec3::new(4.0, 0.0, 40.0),
        DVec3::new(60.0, 0.0, 40.0),
        DVec3::new(-60.0, 0.0, 40.0),
    ];
    let mut world = flat();
    let stone = block(
        &mut world,
        1_000.0,
        DVec3::splat(0.5),
        DVec3::new(0.0, 0.5, 46.0),
    );
    let mut scene = Scene::cast(world, points, 6);
    scene.objects = vec![Unattended::new(stone, Some(500), false)];
    let impacts = scene.run(4.0);
    let hits: Vec<&ObjectHit> = impacts
        .iter()
        .flat_map(|i| &i.objects)
        .filter(|h| h.body == stone)
        .collect();
    assert_eq!(hits.len(), 2);
    assert!(hits[0].damage.is_some());
    assert!(hits[1].damage.is_none());
    assert!(hits.iter().all(|h| h.impulse.length() > 0.0));
    assert_eq!(scene.objects[0].hp, Some(500 - scene.swarm.damage.total()));
}

#[test]
fn flammable_objects_ignite_and_burn_only_themselves() {
    let p = points()[0];
    let mut world = flat();
    let barrel = block(
        &mut world,
        60.0,
        DVec3::new(0.3, 0.45, 0.3),
        DVec3::new(p.x + 6.0, 0.45, p.z),
    );
    let stone = block(
        &mut world,
        1_000.0,
        DVec3::splat(0.5),
        DVec3::new(p.x - 6.0, 0.5, p.z),
    );
    settle(&mut world);
    // Every damage die shows 1: 20 Fire + 20 Bludgeoning.
    let mut host = rigged(world, 1);
    host.die = Some(1);
    let mut scene = Scene::cast_rigged(host, points());
    scene.objects = vec![
        Unattended::new(barrel, Some(50), true),
        Unattended::new(stone, Some(500), false),
    ];
    let mut ignited = Vec::new();
    let mut broken = Vec::new();
    for _ in 0..ticks(scene.world(), 10.0) {
        scene.host.bench.world.step(&Uniform(DOWN));
        for impact in scene
            .swarm
            .after_step(&mut scene.host, &[], &mut scene.objects)
            .unwrap()
        {
            ignited.extend(impact.objects.iter().filter(|h| h.ignited).map(|h| h.body));
        }
        broken.extend(burn(&mut scene.host, &mut scene.objects).unwrap());
    }
    let tick = scene.world().tick;
    assert_eq!(ignited, vec![barrel]);
    assert!(!scene.objects[1].burning(tick));
    assert_eq!(scene.objects[1].hp, Some(460));
    // 50 - 40 = 10 left, then 3 a second: broken by the fourth burn.
    assert_eq!(broken, vec![barrel]);
    assert_eq!(scene.objects[0].hp, Some(0));
    let chunks = &scene.objects[2..];
    assert_eq!(chunks.len(), DEBRIS_CHUNKS);
    assert!(chunks.iter().all(|c| c.burning(tick)));
}

#[test]
fn an_obstructed_meteor_detonates_at_the_obstruction() {
    let mut world = flat();
    let p = points()[1];
    // An overhang 5 m up over the second point, wide enough to catch the
    // slanted path.
    let overhang = static_box(
        &mut world,
        DVec3::new(p.x, 5.25, p.z),
        DVec3::new(4.0, 0.25, 4.0),
    );
    let mut scene = Scene::cast(world, points(), 7);
    scene.creatures = vec![dummy(3, p.x, p.z)];
    let impacts = scene.run(4.0);
    let blocked = impacts.iter().find(|i| i.meteor == 1).unwrap();
    assert!(blocked.obstructed);
    assert_eq!(blocked.struck, Some(overhang));
    assert!(
        (blocked.center.y - 5.5).abs() < 0.05,
        "{:?}",
        blocked.center
    );
    // The Sphere is centered on the overhang, so the dummy below it is
    // still inside.
    assert_eq!(blocked.creatures.len(), 1);
    assert!(
        impacts
            .iter()
            .filter(|i| i.meteor != 1)
            .all(|i| !i.obstructed)
    );
}

#[test]
fn path_clear_matches_where_the_meteor_detonates() {
    let mut world = flat();
    let p = points()[1];
    let overhang = static_box(
        &mut world,
        DVec3::new(p.x, 5.25, p.z),
        DVec3::new(4.0, 0.25, 4.0),
    );
    // The overhang stops the flight to its point, not the others.
    assert!(!path_clear(
        &world,
        DVec3::ZERO,
        p,
        G,
        Flight::STANDARD,
        |_| true
    ));
    assert!(path_clear(
        &world,
        DVec3::ZERO,
        points()[0],
        G,
        Flight::STANDARD,
        |_| true
    ));
    // A body the caller lets meteors detonate on does not refuse the point.
    assert!(path_clear(
        &world,
        DVec3::ZERO,
        p,
        G,
        Flight::STANDARD,
        |b| b != overhang
    ));
    // The ground at the point never counts.
    assert!(path_clear(
        &flat(),
        DVec3::ZERO,
        p,
        G,
        Flight::STANDARD,
        |_| true
    ));
}

#[test]
fn a_slower_lower_flight_keeps_the_rules_and_lands_on_its_points() {
    let flight = Flight {
        height: 30.0,
        speed: 8.0,
        stagger: 0.5,
    };
    let mut host = rigged(flat(), 13);
    let swarm =
        MeteorSwarm::cast(&mut host, DVec3::ZERO, points(), G, flight, DC, |_| true).unwrap();
    let mut scene = Scene {
        host,
        swarm,
        creatures: Vec::new(),
        objects: Vec::new(),
    };
    let spawns: Vec<u64> = scene.swarm.meteors.iter().map(|m| m.spawn_tick).collect();
    assert_eq!(spawns, vec![0, 60, 120, 180]);
    let impacts = scene.run(5.0);
    assert_eq!(impacts.len(), METEORS);
    for impact in &impacts {
        assert!(!impact.obstructed, "{impact:?}");
        assert_eq!(impact.radius, RADIUS);
    }
    let expected = ticks(scene.world(), fall_time(G, flight));
    assert!(
        impacts[0].tick.abs_diff(expected) <= 2,
        "{}",
        impacts[0].tick
    );
    let bad = Flight {
        height: f64::NAN,
        ..flight
    };
    let mut host = rigged(flat(), 13);
    assert!(MeteorSwarm::cast(&mut host, DVec3::ZERO, points(), G, bad, DC, |_| true).is_err());
}

#[test]
fn a_meteor_detonates_on_a_creature_in_its_path() {
    let mut scene = Scene::cast(flat(), points(), 8);
    let p = points()[0];
    // A tall creature standing at the point: the meteor strikes it.
    scene.creatures = vec![Creature {
        height: 6.0,
        radius: 1.5,
        ..dummy(4, p.x, p.z)
    }];
    let impacts = scene.run(3.0);
    assert_eq!(impacts[0].struck, None);
    assert!(impacts[0].center.y > 2.0);
    assert_eq!(impacts[0].creatures.len(), 1);
}

#[test]
fn sweeping_several_steps_at_once_finds_the_same_impacts() {
    let run = |every: u64| {
        let mut scene = Scene::cast(flat(), points(), 12);
        let mut impacts = Vec::new();
        for n in 1..=ticks(scene.world(), 4.0) {
            scene.host.bench.world.step(&Uniform(DOWN));
            if n % every == 0 {
                impacts.extend(
                    scene
                        .swarm
                        .after_step(&mut scene.host, &[], &mut scene.objects)
                        .unwrap(),
                );
            }
        }
        impacts
    };
    let (one, four) = (run(1), run(4));
    assert_eq!(one.len(), METEORS);
    assert_eq!(four.len(), METEORS);
    for (a, b) in one.iter().zip(&four) {
        assert!(a.center.distance(b.center) < 0.1, "{a:?} {b:?}");
    }
}

#[test]
fn placement_is_validated() {
    let ok = points();
    assert_eq!(validate(DVec3::ZERO, &ok, |_| true), Ok(()));
    let mut far = ok;
    far[2] = DVec3::new(RANGE + 1.0, 0.0, 0.0);
    assert_eq!(
        validate(DVec3::ZERO, &far, |_| true),
        Err(Refusal::OutOfRange)
    );
    let mut same = ok;
    same[3] = same[0] + DVec3::X * 0.1;
    assert_eq!(
        validate(DVec3::ZERO, &same, |_| true),
        Err(Refusal::SamePoint)
    );
    assert_eq!(
        validate(DVec3::ZERO, &ok, |p| p.x < 40.0),
        Err(Refusal::NotVisible)
    );
    let mut bad = ok;
    bad[0] = DVec3::NAN;
    assert_eq!(validate(DVec3::ZERO, &bad, |_| true), Err(Refusal::Invalid));
    let mut host = rigged(flat(), 1);
    let bodies = host.bench.world.bodies().len();
    assert!(
        MeteorSwarm::cast(&mut host, DVec3::ZERO, far, G, Flight::STANDARD, DC, |_| {
            true
        })
        .is_err()
    );
    assert_eq!(host.bench.world.bodies().len(), bodies);
    assert_eq!(host.bench.dice, Dice::new(1));
}

#[test]
fn blast_impulses_balance_the_ledger() {
    let p = points()[0];
    let mut world = World::new(DT);
    world.sleep.enabled = false;
    let a = block(
        &mut world,
        20.0,
        DVec3::splat(0.25),
        p + DVec3::new(3.0, 0.25, 0.0),
    );
    let b = block(
        &mut world,
        60.0,
        DVec3::splat(0.3),
        p + DVec3::new(-2.0, 0.3, 4.0),
    );
    let mut objects = vec![
        Unattended::new(a, None, false),
        Unattended::new(b, None, false),
    ];
    let mut swarm = MeteorSwarm {
        caster: DVec3::ZERO,
        dc: DC,
        damage: Damage {
            fire: 1,
            bludgeoning: 1,
        },
        meteors: vec![Meteor {
            point: p,
            spawn_tick: 0,
            start: p,
            velocity: DVec3::ZERO,
            body: None,
            last: p,
            done: true,
        }],
        affected: BTreeSet::new(),
        damaged: BTreeSet::new(),
        impacts: Vec::new(),
        flight: Flight::STANDARD,
    };
    let origin = DVec3::ZERO;
    let mut ledger = Ledger::new(origin, world.momentum(origin));
    let mut host = rigged(world, 1);
    let impact = swarm
        .detonate(&mut host, 0, p, None, &[], &mut objects)
        .unwrap();
    for hit in &impact.objects {
        ledger.add_impulse(LEDGER_TERM, hit.impulse, host.bench.world[hit.body].pos);
    }
    let now: Momentum = host.bench.world.momentum(origin);
    let error = ledger.error(now);
    assert!(error.linear < 1e-12 && error.angular < 1e-12, "{error:?}");
}

#[test]
fn a_checkpoint_mid_fall_replays_identically() {
    let p = points()[0];
    let mut world = flat();
    let mut ids = Vec::new();
    for (i, x) in [2.0, 4.0, 6.0].into_iter().enumerate() {
        ids.push(crate_at(&mut world, p.x + x, p.z + i as f64));
    }
    let mut scene = Scene::cast(world, points(), 11);
    scene.objects = ids
        .iter()
        .map(|&id| Unattended::new(id, Some(10), true))
        .collect();
    scene.creatures = vec![dummy(1, p.x, p.z + 3.0), dummy(2, points()[1].x, 40.0)];
    // Mid-fall: three meteors in the air, none detonated.
    scene.run(0.7);
    assert_eq!(scene.swarm.falling().count(), 3);
    assert!(scene.swarm.impacts.is_empty());
    let saved = serde_json::to_string(&(
        &scene.host.bench.world,
        &scene.swarm,
        &scene.objects,
        &scene.host.bench.dice,
    ))
    .unwrap();
    let (world, swarm, objects, dice): (World, MeteorSwarm, Vec<Unattended>, Dice) =
        serde_json::from_str(&saved).unwrap();
    let mut restored = Scene {
        host: Rigged {
            bench: Bench { world, dice },
            die: None,
            d20: None,
        },
        swarm,
        creatures: scene.creatures.clone(),
        objects,
    };
    let (mut a, mut b) = (Trace::default(), Trace::default());
    let (mut ia, mut ib) = (Vec::new(), Vec::new());
    for _ in 0..ticks(scene.world(), 5.0) {
        ia.extend(scene.step());
        ib.extend(restored.step());
        a.record(scene.world());
        b.record(restored.world());
    }
    a.compare(&b, Tolerance::EXACT).unwrap();
    assert_eq!(ia.len(), METEORS);
    assert_eq!(ia, ib);
    assert_eq!(scene.swarm, restored.swarm);
    assert_eq!(scene.objects, restored.objects);
}
