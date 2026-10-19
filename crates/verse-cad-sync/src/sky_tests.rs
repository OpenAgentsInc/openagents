//! Nothing stays in the air once what held it up is gone: debris frozen
//! on a piece moves again when the piece breaks or comes loose, debris
//! frozen on that debris follows, and a toppling top whose hinge is
//! destroyed falls free instead of hanging from it.

use super::*;

/// A carved block of `building`, `size` m, with its underside at `base`
/// over `(x, z)`, cut into a 2 by 2 by 2 grid of chunks.
fn block(
    building: usize,
    level: u8,
    x: f64,
    z: f64,
    base: f64,
    size: DVec3,
    link: Link,
) -> PieceSpec {
    let half = size * 0.5;
    let quarter = half * 0.5;
    let mut chunks = Vec::new();
    for i in 0..8 {
        let at = DVec3::new(
            if i & 1 == 0 { -quarter.x } else { quarter.x },
            if i & 2 == 0 { -quarter.y } else { quarter.y },
            if i & 4 == 0 { -quarter.z } else { quarter.z },
        );
        chunks.push(Cuboid {
            center: at,
            rotation: DQuat::IDENTITY,
            half: quarter,
        });
    }
    PieceSpec {
        building,
        role: Role::Block { level },
        matter: Matter::Plaster,
        center: DVec3::new(x, base + half.y, z),
        orientation: DQuat::IDENTITY,
        mass: (size.x * size.y * size.z * 40.0).clamp(30.0, 3000.0),
        size,
        hit_points: 27,
        colliders: vec![Cuboid {
            center: DVec3::ZERO,
            rotation: DQuat::IDENTITY,
            half: half - DVec3::splat(0.01),
        }],
        chunks,
        blocks: false,
        link,
    }
}

fn link(footing: bool, under: &[u16], beside: &[u16]) -> Link {
    Link {
        footing,
        under: under.to_vec(),
        beside: beside.to_vec(),
    }
}

fn run(site: &mut Site, seconds: f64) {
    for _ in 0..(seconds / STEP).ceil() as usize {
        site.tick(STEP as f32);
    }
}

/// Knocks piece `piece` out of its place: it breaks and its chunks fly off
/// level along `toward`, so they don't fill the gap it leaves.
fn blow_out(site: &mut Site, piece: usize, toward: DVec3) {
    let at = site.piece_pose(piece).w_axis.truncate().as_dvec3();
    assert!(site.damage(piece, 100, at, toward * 15.0));
}

/// The live chunks' world heights.
fn chunk_heights(site: &Site) -> Vec<f32> {
    let mut out = Vec::new();
    for (p, piece) in site.pieces().iter().enumerate() {
        for k in 0..piece.chunks.len() {
            if let Some(pose) = site.chunk_pose(p, k) {
                out.push(pose.w_axis.y);
            }
        }
    }
    out
}

/// Runs `case` with the physics world's island sleep on and off: with it
/// off, settled debris freezes rather than sleeps, which is what happens
/// to debris that still rocks a little when it comes to rest.
fn both(case: impl Fn(bool)) {
    case(true);
    case(false);
}

#[test]
fn debris_frozen_on_a_column_falls_when_the_column_is_cut_out_from_under_it() {
    both(column_cut_under_debris);
}

fn column_cut_under_debris(sleep: bool) {
    // A 1 m wide post 2 m high carrying two 3 m wide levels, and a small
    // cap on top.
    let wide = DVec3::new(3.0, 2.0, 3.0);
    let mut site = Site::new(
        vec![
            block(
                0,
                0,
                0.0,
                0.0,
                0.0,
                DVec3::new(1.0, 2.0, 1.0),
                link(true, &[], &[]),
            ),
            block(0, 1, 0.0, 0.0, 2.0, wide, link(false, &[0], &[])),
            block(0, 2, 0.0, 0.0, 4.0, wide, link(false, &[1], &[])),
            block(
                0,
                0,
                0.0,
                0.0,
                6.0,
                DVec3::splat(1.0),
                link(false, &[2], &[]),
            ),
        ],
        3,
    );
    site.world.sleep.enabled = sleep;
    // The cap breaks, and its chunks come to rest and freeze on the
    // column's top, 6 m up.
    assert!(site.damage(3, 100, DVec3::new(0.0, 6.5, 0.0), DVec3::ZERO));
    run(&mut site, 4.0);
    let high = chunk_heights(&site)
        .into_iter()
        .filter(|&y| y > 5.9)
        .count();
    assert!(high >= 4, "the cap's chunks lie on the column's top");
    if !sleep {
        assert!(!site.frozen.is_empty(), "and are frozen there");
    }
    // The column's foot breaks 5 m and more below them: the levels over it
    // fall, and so does everything frozen on them.
    blow_out(&mut site, 0, DVec3::X);
    assert_eq!(site.pieces()[2].status, Status::Loose);
    run(&mut site, 5.0);
    for y in chunk_heights(&site) {
        assert!(y < 4.6, "a chunk hangs where the column was, {y} m up");
    }
}

#[test]
fn debris_frozen_on_frozen_debris_falls_with_it() {
    both(debris_on_debris);
}

fn debris_on_debris(sleep: bool) {
    // A ledge 4 m up on a post, debris frozen on the ledge, and debris
    // frozen on that debris: when the column goes, every layer falls,
    // however far it lies from the break.
    let wide = DVec3::new(3.0, 2.0, 3.0);
    let mut site = Site::new(
        vec![
            block(
                0,
                0,
                0.0,
                0.0,
                0.0,
                DVec3::new(1.0, 2.0, 1.0),
                link(true, &[], &[]),
            ),
            block(0, 1, 0.0, 0.0, 2.0, wide, link(false, &[0], &[])),
            block(
                0,
                0,
                0.0,
                0.0,
                4.0,
                DVec3::splat(1.2),
                link(false, &[1], &[]),
            ),
            block(
                0,
                0,
                0.0,
                0.0,
                5.2,
                DVec3::splat(1.2),
                link(false, &[2], &[]),
            ),
        ],
        5,
    );
    site.world.sleep.enabled = sleep;
    // Break the lower cap, then the upper, each settling and freezing.
    assert!(site.damage(2, 100, DVec3::new(0.0, 4.6, 0.0), DVec3::ZERO));
    // The upper cap loses its footing and drops onto the debris.
    run(&mut site, 4.0);
    if site.pieces()[3].status != Status::Broken {
        let at = site.piece_pose(3).w_axis.truncate().as_dvec3();
        assert!(site.damage(3, 100, at, DVec3::ZERO));
    }
    run(&mut site, 4.0);
    assert!(
        chunk_heights(&site).iter().any(|&y| y > 4.0),
        "debris rests on the column"
    );
    // A chain: something frozen rests only on other frozen debris.
    if !sleep {
        let chained = site
            .rests
            .values()
            .any(|under| under.iter().all(|u| site.frozen.contains(u)));
        assert!(chained, "debris froze on debris: {:?}", site.rests);
    }
    blow_out(&mut site, 0, DVec3::X);
    run(&mut site, 5.0);
    for y in chunk_heights(&site) {
        assert!(y < 3.6, "a chunk hangs where the column was, {y} m up");
    }
    assert!(site.frozen.iter().all(|b| site.rests.contains_key(b)));
}

#[test]
fn a_toppling_top_whose_hinge_breaks_falls_free() {
    // A tower of 2 m levels, each two blocks 1 m wide and 2 m deep, eight
    // levels high.
    let cube = DVec3::new(1.0, 2.0, 2.0);
    let mut specs = Vec::new();
    for level in 0..8u8 {
        for (k, x) in [-0.5, 0.5].into_iter().enumerate() {
            let me = u16::from(level) * 2 + k as u16;
            let under: Vec<u16> = if level == 0 { Vec::new() } else { vec![me - 2] };
            let beside = [me ^ 1];
            specs.push(block(
                0,
                level,
                x,
                0.0,
                f64::from(level) * 2.0,
                cube,
                link(level == 0, &under, &beside),
            ));
        }
    }
    let mut site = Site::new(specs, 9);
    // Cutting the east block out of level 2 tips the top east over the
    // west block, its hinge.
    blow_out(&mut site, 5, DVec3::X);
    assert_eq!(site.toppling(), 1);
    assert_eq!(site.pieces()[4].status, Status::Standing, "the hinge");
    let lowest = |site: &Site| {
        (6..16)
            .map(|i| site.piece_pose(i).w_axis.y)
            .fold(f32::INFINITY, f32::min)
    };
    let start = lowest(&site);
    // A blast takes the hinge out at once and throws its chunks clear:
    // nothing holds the top up any more.
    blow_out(&mut site, 4, DVec3::NEG_X);
    run(&mut site, 0.8);
    let dropped = start - lowest(&site);
    assert!(
        dropped > 1.5,
        "the top fell {dropped} m in 0.8 s instead of hanging from its hinge"
    );
}
