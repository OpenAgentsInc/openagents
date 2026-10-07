use std::f64::consts::PI;

use glam::{DQuat, DVec2, DVec3};

use super::*;
use crate::Body;
use crate::collision::{Collider, Shape};
use crate::ledger::Ledger;
use crate::trace::{Tolerance, Trace};
use crate::world::{BodyId, Uniform, World};

const DT: f64 = 1.0 / 120.0;
const GRAVITY: Uniform = Uniform(DVec3::new(0.0, -9.81, 0.0));

fn still_pond() -> WaterBody {
    WaterBody::pond(
        WaterId(0),
        vec![
            DVec2::new(-50.0, -50.0),
            DVec2::new(50.0, -50.0),
            DVec2::new(50.0, 50.0),
            DVec2::new(-50.0, 50.0),
        ],
        0.0,
    )
}

fn swell() -> WaveSet {
    WaveSet::seeded(7, DT, 120 * 60, 6, DVec2::new(1.0, 0.3), 9.0, 0.12, 0.5).unwrap()
}

fn wavy_ocean() -> WaterBody {
    WaterBody::ocean(WaterId(1), 0.0).with_waves(swell())
}

/// A body of `mass` with one collider, at `pos`.
fn add(world: &mut World, shape: Shape, mass: f64, pos: DVec3, orientation: DQuat) -> BodyId {
    let inertia = match shape {
        Shape::Sphere { radius } => DVec3::splat(0.4 * mass * radius * radius),
        Shape::Capsule {
            radius,
            half_length,
        } => Body::box_inertia(
            mass,
            DVec3::new(2.0 * radius, 2.0 * radius, 2.0 * (half_length + radius)),
        ),
        Shape::Cuboid { half } => Body::box_inertia(mass, half * 2.0),
    };
    let mut body = Body::new(mass, inertia, pos);
    body.orientation = orientation;
    body.prev_orientation = orientation;
    let id = world.add(body);
    world.add_collider(Collider::new(id, shape));
    id
}

fn run<W: Water>(world: &mut World, water: &W, seconds: f64, settings: &Settings) {
    for _ in 0..(seconds / DT).round() as usize {
        apply_with(world, water, world.tick, DT, settings);
        world.step(&GRAVITY);
    }
}

/// Water's forces for the world's next step.
fn wet<W: Water>(world: &mut World, water: &W) -> Vec<Push> {
    let tick = world.tick;
    apply(world, water, tick, DT)
}

fn submersion<W: Water>(world: &World, id: BodyId, water: &W) -> Submersion {
    let c = world.colliders().iter().find(|c| c.body == id).unwrap();
    submerged(&c.shape, c.pose(world), water, world.tick)
}

// The surface.

#[test]
fn a_sample_at_tick_n_equals_stepping_from_tick_zero() {
    let body = wavy_ocean();
    let surface = body.surface();
    let mut phases = body.waves.phases(0);
    let points = [
        DVec2::new(0.3, -2.0),
        DVec2::new(17.5, 4.25),
        DVec2::new(-60.0, 33.0),
    ];
    for tick in 0..=30_000u64 {
        if tick % 997 == 0 || tick == 30_000 {
            assert_eq!(phases, body.waves.phases(tick), "tick {tick}");
            for p in points {
                let direct = surface.sample(p.x, p.y, tick).unwrap();
                let stepped = surface.sample_with(p.x, p.y, &phases).unwrap();
                assert_eq!(direct, stepped, "tick {tick}");
            }
        }
        body.waves.advance(&mut phases);
    }
    // The clock folds: a tick and the same tick a period later agree
    // exactly, however late.
    let late = 1u64 << 40;
    for p in points {
        assert_eq!(
            surface.sample(p.x, p.y, 1234),
            surface.sample(
                p.x,
                p.y,
                1234 + body.waves.period * (late / body.waves.period)
            )
        );
    }
    // A world stepped to tick N reads the same surface.
    let mut world = World::new(DT);
    for _ in 0..500 {
        world.step(&GRAVITY);
    }
    assert_eq!(
        surface.sample(1.0, 2.0, world.tick),
        surface.sample(1.0, 2.0, 500)
    );
}

#[test]
fn snapped_frequencies_stay_near_deep_water_dispersion() {
    let waves = swell();
    assert_eq!(waves.waves.len(), 6);
    assert_eq!(waves.seed, Some(7));
    for (i, w) in waves.waves.iter().enumerate() {
        let exact = (G * w.wavenumber()).sqrt();
        let snapped = waves.omega(i);
        // Within half a step of 2π / T.
        assert!(
            (snapped - exact).abs() <= PI / 60.0 + 1e-12,
            "{snapped} {exact}"
        );
    }
    assert!(WaveSet::new(DT, 100, 0.5, vec![Wave::new(DVec2::X, 0.1, 5.0, 0.0); 9]).is_err());
}

#[test]
fn the_inverted_surface_lands_on_the_query_point_with_a_consistent_normal() {
    let waves = swell();
    let phases = waves.phases(4321);
    let mut worst: f64 = 0.0;
    for i in 0..40 {
        let x = DVec2::new(i as f64 * 1.37 - 20.0, (i as f64 * 0.71).sin() * 9.0);
        let mut p0 = x;
        for _ in 0..INVERSIONS {
            p0 = x - waves.at_rest(p0, &phases).horizontal;
        }
        let d = waves.at_rest(p0, &phases);
        worst = worst.max((p0 + d.horizontal - x).length());
        // The normal matches the slope of the sampled height.
        let h = 1e-4;
        let s = waves.at(x, &phases);
        let dx = (waves.at(x + DVec2::X * h, &phases).height
            - waves.at(x - DVec2::X * h, &phases).height)
            / (2.0 * h);
        let dz = (waves.at(x + DVec2::Y * h, &phases).height
            - waves.at(x - DVec2::Y * h, &phases).height)
            / (2.0 * h);
        let numeric = DVec3::new(-dx, 1.0, -dz).normalize();
        assert!(
            numeric.angle_between(s.normal) < 0.02,
            "{numeric} {}",
            s.normal
        );
    }
    assert!(worst < 2e-3, "{worst}");
}

// Submersion geometry.

#[test]
fn a_plane_through_the_center_wets_exactly_half_of_every_shape() {
    let shapes = [
        Shape::Sphere { radius: 0.7 },
        Shape::Capsule {
            radius: 0.3,
            half_length: 0.8,
        },
        Shape::Cuboid {
            half: DVec3::new(0.2, 0.5, 1.1),
        },
    ];
    for shape in shapes {
        for k in 0..12 {
            let k = f64::from(k);
            let rotation = DQuat::from_euler(glam::EulerRot::XYZ, 0.37 * k, 1.1 * k, 0.2 * k);
            let center = DVec3::new(3.0, -1.0, 2.0);
            let plane = Plane {
                point: center,
                normal: DVec3::new((0.3 * k).sin() * 0.4, 1.0, 0.2).normalize(),
            };
            let (v, centroid, wet) = below(&shape, (center, rotation), plane);
            // A centrally symmetric body is halved by any plane through
            // its center.
            let error = (v / volume(&shape) - 0.5).abs();
            let tolerance = if matches!(shape, Shape::Capsule { .. }) {
                5e-3
            } else {
                1e-12
            };
            assert!(error < tolerance, "{shape:?} {k} {error}");
            assert!(
                (wet / area(&shape) - 0.5).abs() < tolerance.max(1e-9),
                "{shape:?} {wet}"
            );
            assert!(plane.distance(centroid) < 0.0);
        }
    }
}

#[test]
fn caps_and_slices_match_their_closed_forms() {
    let r = 0.5;
    let up = |height| Plane {
        point: DVec3::new(0.0, height, 0.0),
        normal: DVec3::Y,
    };
    // A sphere 0.2 m into the water: a cap of height 0.2.
    let h = 0.2;
    let (v, c, wet) = below(
        &Shape::Sphere { radius: r },
        (DVec3::new(0.0, r - h, 0.0), DQuat::IDENTITY),
        up(0.0),
    );
    assert!((v - PI * h * h * (3.0 * r - h) / 3.0).abs() < 1e-12);
    assert!((wet - 2.0 * PI * r * h).abs() < 1e-12);
    assert!(c.y < 0.0 && c.y > -h);
    // A vertical capsule under to 0.3 m above its center.
    let capsule = Shape::Capsule {
        radius: 0.25,
        half_length: 0.5,
    };
    let (v, _, wet) = below(
        &capsule,
        (DVec3::ZERO, DQuat::from_rotation_x(PI / 2.0)),
        up(0.3),
    );
    let expected = 2.0 / 3.0 * PI * 0.25f64.powi(3) + PI * 0.0625 * 0.8;
    assert!((v - expected).abs() / expected < 2e-3, "{v} {expected}");
    let expected_wet = 2.0 * PI * 0.0625 + 2.0 * PI * 0.25 * 0.8;
    assert!((wet - expected_wet).abs() / expected_wet < 2e-3);
    // A cuboid: a tilted box below a level plane by brute-force sampling.
    let half = DVec3::new(0.3, 0.2, 0.6);
    let rotation = DQuat::from_rotation_z(0.4) * DQuat::from_rotation_x(0.3);
    let center = DVec3::new(0.0, 0.1, 0.0);
    let (v, c, _) = below(&Shape::Cuboid { half }, (center, rotation), up(0.0));
    let n = 60;
    let (mut count, mut sum) = (0usize, DVec3::ZERO);
    for i in 0..n {
        for j in 0..n {
            for k in 0..n {
                let f = |a: usize| (a as f64 + 0.5) / n as f64 * 2.0 - 1.0;
                let p = center + rotation * (half * DVec3::new(f(i), f(j), f(k)));
                if p.y < 0.0 {
                    count += 1;
                    sum += p;
                }
            }
        }
    }
    let sampled = count as f64 / (n * n * n) as f64 * volume(&Shape::Cuboid { half });
    assert!((v - sampled).abs() / sampled < 5e-3, "{v} {sampled}");
    assert!(c.distance(sum / count as f64) < 5e-3);
}

// Floating and sinking.

#[test]
fn a_cube_of_half_water_density_settles_half_submerged() {
    let pond = still_pond();
    let mut world = World::new(DT);
    let shape = Shape::Cuboid {
        half: DVec3::splat(0.5),
    };
    let id = add(
        &mut world,
        shape,
        500.0,
        DVec3::new(0.0, 0.8, 0.0),
        DQuat::from_rotation_z(0.1),
    );
    run(&mut world, &pond, 60.0, &Settings::default());
    let fraction = submersion(&world, id, &pond).volume_fraction();
    assert!((fraction - 0.5).abs() < 0.005, "{fraction}");
    assert!(world[id].vel.length() < 0.01, "{}", world[id].vel);
}

/// The draft of a sphere of `ratio` times water's density, by bisection on
/// the cap volume.
fn analytic_draft(r: f64, ratio: f64) -> f64 {
    let want = ratio * 4.0 / 3.0 * PI * r.powi(3);
    let (mut lo, mut hi) = (0.0, 2.0 * r);
    for _ in 0..100 {
        let h = 0.5 * (lo + hi);
        if PI * h * h * (3.0 * r - h) / 3.0 < want {
            lo = h;
        } else {
            hi = h;
        }
    }
    0.5 * (lo + hi)
}

#[test]
fn light_spheres_float_at_the_analytic_draft_and_heavy_ones_sink() {
    let pond = still_pond();
    let r: f64 = 0.4;
    let mass = |density: f64| density * 4.0 / 3.0 * PI * r.powi(3);
    let mut world = World::new(DT);
    let light = add(
        &mut world,
        Shape::Sphere { radius: r },
        mass(900.0),
        DVec3::new(-5.0, 1.0, 0.0),
        DQuat::IDENTITY,
    );
    let heavy = add(
        &mut world,
        Shape::Sphere { radius: r },
        mass(1100.0),
        DVec3::new(5.0, 1.0, 0.0),
        DQuat::IDENTITY,
    );
    run(&mut world, &pond, 40.0, &Settings::default());
    let draft = r - world[light].pos.y;
    let expected = analytic_draft(r, 0.9);
    assert!(
        (draft - expected).abs() / expected < 0.01,
        "{draft} {expected}"
    );
    assert!(world[heavy].pos.y < -20.0, "{}", world[heavy].pos);
    assert!(world[heavy].vel.y < -0.5);
}

/// A light proxy body floats at its material's draft when its buoyancy is
/// scaled by its mass over the material's mass, and a stone proxy of the
/// same light mass sinks.
#[test]
fn scaled_buoyancy_floats_a_proxy_at_its_material_s_draft() {
    let pond = still_pond();
    let mut world = World::new(DT);
    let half = DVec3::new(0.5, 0.1, 0.25);
    let shape = Shape::Cuboid { half };
    let volume = 8.0 * half.x * half.y * half.z;
    // Both weigh 40 kg/m³ of their box, as debris tuned for its fall does.
    let mass = 40.0 * volume;
    let wood = add(
        &mut world,
        shape,
        mass,
        DVec3::new(-3.0, 0.5, 0.0),
        DQuat::IDENTITY,
    );
    let stone = add(
        &mut world,
        shape,
        mass,
        DVec3::new(3.0, 0.5, 0.0),
        DQuat::IDENTITY,
    );
    let lift = |id: BodyId| {
        let density = if id == wood { 600.0 } else { 2000.0 };
        Some(mass / (density * volume))
    };
    for _ in 0..(30.0 / DT) as usize {
        let tick = world.tick;
        apply_scaled(&mut world, &pond, tick, DT, &Settings::default(), lift);
        world.step(&GRAVITY);
    }
    let fraction = submersion(&world, wood, &pond).volume_fraction();
    assert!((fraction - 0.6).abs() < 0.01, "{fraction}");
    // The light stone proxy sinks slowly against its drag, but sinks.
    assert!(world[stone].pos.y < -3.0, "{}", world[stone].pos);
    assert!(world[stone].vel.y < -0.05, "{}", world[stone].vel);
}

#[test]
fn sinking_reaches_the_quadratic_drag_terminal_speed() {
    let pond = still_pond();
    let r: f64 = 0.3;
    let density = 1400.0;
    let v = 4.0 / 3.0 * PI * r.powi(3);
    let mut world = World::new(DT);
    let id = add(
        &mut world,
        Shape::Sphere { radius: r },
        density * v,
        DVec3::new(0.0, -2.0, 0.0),
        DQuat::IDENTITY,
    );
    let settings = Settings {
        linear: 40.0,
        ..Settings::default()
    };
    run(&mut world, &pond, 20.0, &settings);
    // (ρ_b − ρ) V g = c_l A v + ½ ρ C_d (A / 4) v², with A = 4π r².
    let area = 4.0 * PI * r * r;
    let weight = (density - FRESH) * v * 9.81;
    let a = 0.5 * FRESH * settings.quadratic * 0.25 * area;
    let b = settings.linear * area;
    let terminal = (-b + (b * b + 4.0 * a * weight).sqrt()) / (2.0 * a);
    let speed = -world[id].vel.y;
    assert!(
        (speed - terminal).abs() / terminal < 0.02,
        "{speed} {terminal}"
    );
    // Quadratic drag alone, as with C_d on a sphere's frontal area.
    let mut world = World::new(DT);
    let id = add(
        &mut world,
        Shape::Sphere { radius: r },
        density * v,
        DVec3::new(0.0, -2.0, 0.0),
        DQuat::IDENTITY,
    );
    let settings = Settings {
        linear: 0.0,
        quadratic: 0.47,
        ..Settings::default()
    };
    run(&mut world, &pond, 30.0, &settings);
    let terminal = (2.0 * weight / (FRESH * 0.47 * PI * r * r)).sqrt();
    let speed = -world[id].vel.y;
    assert!(
        (speed - terminal).abs() / terminal < 0.02,
        "{speed} {terminal}"
    );
}

#[test]
fn a_plank_floats_flat_and_rights_itself_after_a_push() {
    let pond = still_pond();
    let mut world = World::new(DT);
    let half = DVec3::new(0.12, 0.025, 1.0);
    let mass = 600.0 * 8.0 * half.x * half.y * half.z;
    // Dropped nearly on end.
    let id = add(
        &mut world,
        Shape::Cuboid { half },
        mass,
        DVec3::new(0.0, 1.2, 0.0),
        DQuat::from_rotation_x(1.3),
    );
    let flat = |world: &World| (world[id].orientation * DVec3::Y).y.abs();
    run(&mut world, &pond, 30.0, &Settings::default());
    assert!(flat(&world) > 0.995, "{}", flat(&world));
    // A shove that rolls it about its long axis.
    let push = world[id].orientation * DVec3::Z * (world[id].inertia.z * 6.0);
    world[id].apply_angular_impulse(push);
    let mut most: f64 = 0.0;
    for _ in 0..(20.0 / DT) as usize {
        wet(&mut world, &pond);
        world.step(&GRAVITY);
        most = most.max(1.0 - flat(&world));
    }
    assert!(most > 0.05, "the push tipped it: {most}");
    assert!(flat(&world) > 0.995, "{}", flat(&world));
}

#[test]
fn a_floating_body_sleeps_on_a_still_pond_but_not_on_waves() {
    let pond = still_pond();
    let ocean = wavy_ocean();
    let shape = Shape::Sphere { radius: 0.3 };
    let mass = 500.0 * volume(&shape);
    let mut still = World::new(DT);
    let a = add(
        &mut still,
        shape,
        mass,
        DVec3::new(0.0, 0.5, 0.0),
        DQuat::IDENTITY,
    );
    run(&mut still, &pond, 30.0, &Settings::default());
    assert!(still[a].sleeping);
    let height = still[a].pos.y;
    run(&mut still, &pond, 5.0, &Settings::default());
    assert_eq!(still[a].pos.y, height);
    let mut rough = World::new(DT);
    let b = add(
        &mut rough,
        shape,
        mass,
        DVec3::new(0.0, 0.5, 0.0),
        DQuat::IDENTITY,
    );
    let mut slept = false;
    for _ in 0..(30.0 / DT) as usize {
        wet(&mut rough, &ocean);
        rough.step(&GRAVITY);
        slept |= rough[b].sleeping;
    }
    assert!(!slept);
    // It rides the swell: its height follows the surface.
    let s = ocean
        .sample(rough[b].pos.x, rough[b].pos.z, rough.tick)
        .unwrap();
    assert!((rough[b].pos.y - s.height).abs() < 0.3);
}

// Currents.

fn bend() -> Course {
    Course::spline(
        &[
            DVec2::new(0.0, 0.0),
            DVec2::new(12.0, 0.0),
            DVec2::new(18.0, 4.0),
            DVec2::new(20.0, 12.0),
            DVec2::new(20.0, 24.0),
        ],
        &[4.0, 4.0, 4.5, 4.0, 4.0],
        6,
    )
}

fn river() -> WaterBody {
    let course = bend();
    let flow = FlowGrid::river(
        &course,
        &[Obstacle {
            center: DVec2::new(20.0, 17.0),
            radius: 0.5,
        }],
        0.5,
        1.2,
    );
    WaterBody::new(
        WaterId(2),
        Kind::River,
        Outline::River { course },
        Level::Profile {
            points: vec![[0.0, 0.3], [40.0, 0.0]],
        },
    )
    .with_flow(flow)
}

#[test]
fn generated_river_flow_is_divergence_free_and_carries_the_discharge() {
    let body = river();
    let Outline::River { course } = &body.outline else {
        unreachable!()
    };
    let grid = body.flow.as_ref().unwrap();
    let speed = 1.2;
    let mut worst: f64 = 0.0;
    for j in 1..grid.nz - 1 {
        for i in 1..grid.nx - 1 {
            // Nodes whose neighbors all lie in the river.
            let inside = [(0, 0), (1, 0), (0, 1), (2, 1), (1, 2), (1, 1)]
                .iter()
                .all(|&(a, b)| body.outline.contains(grid.position(i + a - 1, j + b - 1)));
            // Away from the rock, whose nodes hold the wrapped streamline.
            let clear =
                grid.position(i, j).distance(DVec2::new(20.0, 17.0)) > 0.5 + 2.0 * grid.cell;
            if inside && clear {
                worst = worst.max(grid.divergence(i, j).abs());
            }
        }
    }
    assert!(worst < 1e-9, "{worst}");
    // The flux across sections away from the ends is the discharge.
    let discharge = speed * 4.0;
    for along in [4.0, 10.0, 16.0, 22.0, 30.0] {
        let station = course.locate(course.point_at(along));
        let mid = course.point_at(along);
        let normal = DVec2::new(-station.tangent.y, station.tangent.x);
        let n = 200;
        let mut flux = 0.0;
        for k in 0..n {
            let t = ((k as f64 + 0.5) / n as f64 * 2.0 - 1.0) * station.half_width;
            flux += grid.sample(mid + normal * t).dot(station.tangent)
                * (2.0 * station.half_width / n as f64);
        }
        assert!(
            (flux - discharge).abs() / discharge < 0.03,
            "{along}: {flux}"
        );
        // On the centerline the water runs downstream.
        let v = grid.sample(mid);
        assert!(v.normalize().dot(station.tangent) > 0.9, "{along}: {v}");
    }
}

/// A float dropped at a seeded point upstream.
fn river_run(seed: u64) -> (World, BodyId, Trace) {
    let body = river();
    let set = WaterSet::new(vec![body, still_pond()], 4.0);
    let mut world = World::new(DT);
    let jitter = (seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) >> 40) as f64 / (1u64 << 24) as f64;
    let shape = Shape::Cuboid {
        half: DVec3::new(0.3, 0.15, 0.3),
    };
    let id = add(
        &mut world,
        shape,
        600.0 * volume(&shape),
        DVec3::new(2.0, 0.6, (jitter - 0.5) * 1.5),
        DQuat::IDENTITY,
    );
    let mut trace = Trace::default();
    for _ in 0..(30.0 / DT) as usize {
        wet(&mut world, &set);
        world.step(&GRAVITY);
        if world.tick % 12 == 0 {
            trace.record(&world);
        }
    }
    (world, id, trace)
}

#[test]
fn a_seeded_float_follows_the_river_round_the_bend_and_replays_exactly() {
    let course = bend();
    let (world, id, trace) = river_run(42);
    let at = world[id].pos;
    let station = course.locate(DVec2::new(at.x, at.z));
    // Past the bend (about 15 m to 25 m along) and into the northbound leg,
    // still between the banks and afloat.
    assert!(station.along > 26.0, "{station:?} {at}");
    assert!(station.offset.abs() < station.half_width, "{station:?}");
    assert!(at.y > -0.5, "{at}");
    let (_, _, again) = river_run(42);
    trace.compare(&again, Tolerance::EXACT).unwrap();
}

// Bookkeeping and determinism.

#[test]
fn the_momentum_ledger_balances_with_the_water_terms() {
    let ocean = wavy_ocean();
    let mut world = World::new(DT);
    let shapes = [
        Shape::Sphere { radius: 0.3 },
        Shape::Capsule {
            radius: 0.2,
            half_length: 0.6,
        },
        Shape::Cuboid {
            half: DVec3::new(0.4, 0.2, 0.7),
        },
    ];
    for (i, shape) in shapes.into_iter().enumerate() {
        let id = add(
            &mut world,
            shape,
            650.0 * volume(&shape),
            DVec3::new(i as f64 * 3.0, 1.5, 0.5),
            DQuat::from_rotation_y(0.3 * i as f64),
        );
        world[id].omega = DVec3::new(0.5, -0.2, 0.3);
    }
    let origin = DVec3::new(1.0, -2.0, 3.0);
    let mut ledger = Ledger::new(origin, world.momentum(origin));
    let mut worst: f64 = 0.0;
    for _ in 0..(15.0 / DT) as usize {
        let pushes = wet(&mut world, &ocean);
        record(&pushes, &mut ledger, DT);
        for b in world.bodies() {
            if b.responds() {
                ledger.add_impulse("gravity", GRAVITY.0 * b.mass * DT, b.pos);
            }
        }
        world.step(&GRAVITY);
        let error = ledger.error(world.momentum(origin));
        worst = worst.max(error.linear).max(error.angular);
    }
    assert!(ledger.external.contains_key("water.buoyancy"));
    assert!(ledger.external.contains_key("water.drag"));
    assert!(worst < 1e-9, "{worst}");
    // Leaving a term out shows.
    let mut missing = ledger.clone();
    missing.external.remove("water.buoyancy");
    assert!(missing.error(world.momentum(origin)).linear > 1e-3);
}

#[test]
fn a_restored_world_in_water_continues_bit_for_bit() {
    let set = WaterSet::new(vec![river(), wavy_ocean()], 4.0);
    let mut world = World::new(DT);
    add(
        &mut world,
        Shape::Cuboid {
            half: DVec3::new(0.3, 0.2, 0.5),
        },
        120.0,
        DVec3::new(3.0, 0.6, 0.0),
        DQuat::from_rotation_y(0.4),
    );
    add(
        &mut world,
        Shape::Sphere { radius: 0.35 },
        90.0,
        DVec3::new(-30.0, 0.5, -30.0),
        DQuat::IDENTITY,
    );
    add(
        &mut world,
        Shape::Capsule {
            radius: 0.3,
            half_length: 0.4,
        },
        100.0,
        DVec3::new(-34.0, 0.6, -30.0),
        DQuat::from_rotation_x(0.7),
    );
    for _ in 0..240 {
        wet(&mut world, &set);
        world.step(&GRAVITY);
    }
    let json = serde_json::to_string(&world).unwrap();
    let mut restored: World = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, world);
    // The water round-trips too.
    let set_json = serde_json::to_string(&set).unwrap();
    let set_back: WaterSet = serde_json::from_str(&set_json).unwrap();
    assert_eq!(set_back, set);
    let (mut a, mut b) = (Trace::default(), Trace::default());
    for _ in 0..600 {
        wet(&mut world, &set);
        world.step(&GRAVITY);
        wet(&mut restored, &set_back);
        restored.step(&GRAVITY);
        a.record(&world);
        b.record(&restored);
    }
    a.compare(&b, Tolerance::EXACT).unwrap();
}

#[test]
fn a_water_set_finds_the_same_water_as_testing_every_body() {
    let bodies = vec![
        still_pond().with_density(1010.0),
        river(),
        WaterBody::pond(
            WaterId(5),
            vec![
                DVec2::new(60.0, 0.0),
                DVec2::new(70.0, 0.0),
                DVec2::new(65.0, 8.0),
            ],
            2.0,
        ),
    ];
    let set = WaterSet::new(bodies.clone(), 3.0);
    for i in 0..400 {
        let x = (i as f64 * 0.731).sin() * 80.0;
        let z = (i as f64 * 1.913).cos() * 80.0;
        let brute = bodies.iter().find_map(|b| b.sample(x, z, 77));
        assert_eq!(set.sample(x, z, 77), brute, "{x} {z}");
    }
    assert_eq!(
        set.bodies_overlapping(DVec2::new(61.0, 1.0), DVec2::new(62.0, 2.0)),
        vec![WaterId(5)]
    );
    assert!(set.sample(200.0, 200.0, 0).is_none());
    // A river's level falls along its course.
    let up = set.sample(1.0, 0.0, 0).unwrap();
    assert_eq!(up.body, WaterId(0));
    let river = river();
    let a = river.sample(1.0, 0.0, 0).unwrap();
    let b = river.sample(20.0, 20.0, 0).unwrap();
    assert!(a.height > b.height && a.flow.length() > 0.5);
}
