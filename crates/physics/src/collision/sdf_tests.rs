//! Scene tests for distance-grid colliders and decomposed parts (#11657):
//! hulls poured into a baked non-convex bowl, and a part split offline by
//! CoACD falling onto a floor.

use std::sync::OnceLock;

use glam::{DQuat, DVec3};

use crate::{
    Body, BodyKind, Collider, ConvexHull, HullRef, SdfGrid, SdfRef, Shape, Uniform, World,
};

/// A closed solid of revolution about y from a closed (r, y) profile.
pub(crate) fn revolve(profile: &[(f64, f64)], segments: u32) -> (Vec<DVec3>, Vec<[u32; 3]>) {
    let n = profile.len() as u32;
    let mut vertices = Vec::new();
    for j in 0..segments {
        let a = std::f64::consts::TAU * f64::from(j) / f64::from(segments);
        for &(r, y) in profile {
            vertices.push(DVec3::new(r * a.cos(), y, r * a.sin()));
        }
    }
    let at = |i: u32, j: u32| (j % segments) * n + (i % n);
    let mut triangles = Vec::new();
    for j in 0..segments {
        for i in 0..n {
            for t in [
                [at(i, j), at(i + 1, j), at(i + 1, j + 1)],
                [at(i, j), at(i + 1, j + 1), at(i, j + 1)],
            ] {
                let [a, b, c] = t.map(|k| vertices[k as usize]);
                if (b - a).cross(c - a).length() > 1e-12 {
                    triangles.push(t);
                }
            }
        }
    }
    (vertices, triangles)
}

/// The bowl's profile: a flat floor `T` thick and a flared wall, m.
const RB: f64 = 0.45;
const RT: f64 = 0.7;
const H: f64 = 0.4;
const T: f64 = 0.07;

fn bowl() -> SdfRef {
    static BOWL: OnceLock<SdfRef> = OnceLock::new();
    *BOWL.get_or_init(|| {
        let profile = [
            (0.0, 0.0),
            (RB, 0.0),
            (RT, H),
            (RT - T, H),
            (RB - T * 0.8, T),
            (0.0, T),
        ];
        let (v, t) = revolve(&profile, 48);
        SdfRef::new(SdfGrid::bake(&v, &t, 0.02, 0.06).unwrap())
    })
}

#[test]
fn a_baked_bowl_is_hollow_with_a_wall() {
    let grid = bowl().get();
    // In the wall, in the cavity, and outside.
    assert!(grid.sample(DVec3::new(0.0, T * 0.5, 0.0)).0 < 0.0);
    assert!(grid.sample(DVec3::new(0.0, 0.2, 0.0)).0 > 0.1);
    assert!(grid.sample(DVec3::new(1.2, 0.2, 0.0)).0 > 0.3);
    let (d, n) = grid.sample(DVec3::new(0.0, T + 0.03, 0.0));
    assert!(
        (d - 0.03).abs() < 0.01 && (n - DVec3::Y).length() < 0.1,
        "{d} {n}"
    );
}

#[test]
fn three_hundred_hulls_poured_into_a_baked_bowl_stay_inside() {
    let mut world = World::new(1.0 / 120.0);
    let floor = world.add(Body::new(1.0, DVec3::ONE, DVec3::ZERO).with_kind(BodyKind::Static));
    world.add_collider(Collider::new(floor, Shape::Sdf { grid: bowl() }));
    let rocks: Vec<HullRef> = (0..6_u64)
        .map(|k| {
            let s = 0.035 + 0.008 * k as f64;
            HullRef::new(ConvexHull::rock(k, DVec3::new(s * 1.2, s, s * 0.9), 16).unwrap())
        })
        .collect();
    for n in 0..300_usize {
        let (layer, cell) = (n / 36, n % 36);
        let pos = DVec3::new(
            (cell % 6) as f64 * 0.13 - 0.325,
            0.35 + layer as f64 * 0.14,
            (cell / 6) as f64 * 0.13 - 0.325,
        );
        let turn = DQuat::from_euler(
            glam::EulerRot::XYZ,
            n as f64,
            n as f64 * 0.3,
            n as f64 * 2.1,
        );
        let (id, _) = world.add_hull_body(rocks[n % rocks.len()], 2500.0, pos, turn);
        world[id].vel = DVec3::new(0.0, -3.0, 0.0);
    }
    let g = Uniform(DVec3::new(0.0, -9.81, 0.0));
    for _ in 0..(120 * 5) {
        world.step(&g);
    }
    let grid = bowl().get();
    let mut resting = 0;
    for b in world
        .bodies()
        .iter()
        .filter(|b| b.kind == BodyKind::Dynamic)
    {
        assert!(b.pos.is_finite());
        let r = DVec3::new(b.pos.x, 0.0, b.pos.z).length();
        // In the cavity: above the floor, inside the wall, never in it.
        assert!(b.pos.y > T, "fell through the floor: {}", b.pos);
        let wall = RB - T * 0.8 + (RT - T - (RB - T * 0.8)) * ((b.pos.y - T) / (H - T)).min(1.0);
        assert!(
            r < wall + 0.02,
            "escaped through the wall: {} (wall {wall})",
            b.pos
        );
        assert!(grid.sample(b.pos).0 > -0.01, "inside the wall: {}", b.pos);
        if b.sleeping || b.vel.length() < 0.05 {
            resting += 1;
        }
    }
    assert!(resting >= 285, "{resting} of 300 at rest");
}

/// The U bracket's pieces from the checked-in CoACD output.
fn bracket() -> Vec<HullRef> {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/u-bracket.coacd.json")).unwrap();
    assert_eq!(fixture["license"], "MIT");
    fixture["hulls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| {
            let points: Vec<DVec3> = h
                .as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    let a = p.as_array().unwrap();
                    DVec3::new(
                        a[0].as_f64().unwrap(),
                        a[1].as_f64().unwrap(),
                        a[2].as_f64().unwrap(),
                    )
                })
                .collect();
            HullRef::new(ConvexHull::new(&points).unwrap())
        })
        .collect()
}

#[test]
fn a_decomposed_u_bracket_falls_and_rests_on_the_floor() {
    let pieces = bracket();
    assert!(
        pieces.len() >= 3,
        "the notch splits the U into its posts and base"
    );
    // The pieces cover the bracket's volume (0.3 x 0.2 less the notch).
    let volume: f64 = pieces.iter().map(|h| h.get().volume()).sum();
    let solid = (0.3 * 0.2 - 0.14 * 0.14) * 0.1;
    assert!((volume - solid).abs() < 0.1 * solid, "{volume} vs {solid}");
    let mut world = World::new(1.0 / 120.0);
    let floor = world
        .add(Body::new(1.0, DVec3::ONE, DVec3::new(0.0, -0.5, 0.0)).with_kind(BodyKind::Static));
    world.add_collider(Collider::new(
        floor,
        Shape::Cuboid {
            half: DVec3::new(5.0, 0.5, 5.0),
        },
    ));
    // Upside down (posts first) and tipped, dropped from 0.6 m.
    let tilt = DQuat::from_rotation_z(std::f64::consts::PI + 0.3) * DQuat::from_rotation_x(0.2);
    let (id, colliders) = world.add_compound_body(&pieces, 2700.0, DVec3::new(0.0, 0.6, 0.0), tilt);
    assert_eq!(colliders.len(), pieces.len());
    let g = Uniform(DVec3::new(0.0, -9.81, 0.0));
    for k in 0..(120 * 4) {
        world.step(&g);
        if (70..82).contains(&k) && std::env::var("DEBUG_BRACKET").is_ok() {
            for c in &world.contacts {
                eprintln!(
                    "  {k} {:?} n{} s{:.4} p{} j{}",
                    (c.a, c.b),
                    c.normal,
                    c.separation,
                    c.point,
                    c.impulse
                );
            }
        }
        if k % 10 == 0 && std::env::var("DEBUG_BRACKET").is_ok() {
            eprintln!(
                "{k} {} {} {} m{}",
                world[id].pos,
                world[id].vel,
                world.contacts.len(),
                world.stats.manifolds
            );
        }
    }
    let body = world[id];
    assert!(body.sleeping || body.vel.length() < 0.01, "{:?}", body.vel);
    // Its lowest corner is on the floor.
    let lowest = colliders
        .iter()
        .flat_map(|&c| {
            let collider = world.colliders()[c.0 as usize];
            let (pos, rotation) = collider.pose(&world);
            let Shape::Hull { hull } = collider.shape else {
                unreachable!()
            };
            hull.get()
                .vertices()
                .iter()
                .map(move |v| (pos + rotation * *v).y)
                .collect::<Vec<_>>()
        })
        .fold(f64::INFINITY, f64::min);
    assert!(
        lowest > -world.solver.slop * 2.0 && lowest < 0.01,
        "{lowest}"
    );
}
