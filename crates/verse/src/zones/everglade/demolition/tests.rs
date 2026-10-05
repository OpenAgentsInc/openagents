use super::cottage::{self, COTTAGES};
use super::site::{Role, STEP, Side, Site, Status};
use super::*;
use crate::zones::everglade::tests::pack;
use glam::DVec3;

fn site() -> Site {
    Site::new(cottage::specs_without_meshes(), 7)
}

/// The piece of building 0 with `role`.
fn find(site: &Site, role: Role) -> usize {
    site.specs()
        .iter()
        .position(|s| s.building == 0 && s.role == role)
        .expect("the cottage has the piece")
}

fn wall(side: Side, index: u8) -> Role {
    let count = if matches!(side, Side::South | Side::North) {
        4
    } else {
        5
    };
    Role::Wall { side, index, count }
}

/// A hammer path that passes through the outer face of `piece`.
fn path_at(site: &Site, piece: usize) -> (Vec<Vec3>, Vec3) {
    let spec = &site.specs()[piece];
    let center = spec.center.as_vec3();
    let outward = spec.orientation.as_quat() * Vec3::Z;
    let point = center + outward * 0.35;
    (
        vec![point + Vec3::Y * 0.3, point, point - Vec3::Y * 0.3],
        -outward,
    )
}

fn run(site: &mut Site, seconds: f64) {
    for _ in 0..(seconds / STEP).ceil() as usize {
        site.tick(STEP as f32);
    }
}

#[test]
fn a_hit_lowers_the_struck_pieces_hit_points() {
    let mut site = site();
    let target = find(&site, wall(Side::South, 2));
    let full = site.pieces()[target].hit_points;
    let (path, push) = path_at(&site, target);
    let blow = site.strike(&path, push, 0.5).expect("the swing lands");
    assert_eq!(blow.piece, target);
    assert!((8..=30).contains(&blow.damage), "{blow:?}");
    assert_eq!(
        site.pieces()[target].hit_points,
        (full - blow.damage).max(0)
    );
    // Everything else is untouched.
    for (i, piece) in site.pieces().iter().enumerate() {
        if i != target {
            assert_eq!(piece.hit_points, site.specs()[i].hit_points);
            assert_eq!(piece.status, Status::Standing);
        }
    }
}

#[test]
fn a_swing_at_nothing_misses() {
    let mut site = site();
    let far = Vec3::new(0.0, 1.2, -25.0);
    assert!(site.strike(&[far], Vec3::Z, 0.5).is_none());
}

#[test]
fn enough_hits_break_a_wall_into_chunks_that_fall_and_settle() {
    let mut site = site();
    let target = find(&site, wall(Side::South, 2));
    let (path, push) = path_at(&site, target);
    let mut hits = 0;
    while site.pieces()[target].status != Status::Broken {
        site.strike(&path, push, 0.5).expect("the swing lands");
        hits += 1;
        assert!(hits <= 4, "a wall section breaks in a few hits");
    }
    assert!(hits >= 2, "a single blow does not break a wall section");
    let chunks = site.pieces()[target].chunks.clone();
    assert!(chunks.len() >= 6);
    let before: Vec<Vec3> = (0..chunks.len())
        .map(|i| site.chunk_pose(target, i).unwrap().w_axis.truncate())
        .collect();
    run(&mut site, 4.0);
    for (i, start) in before.iter().enumerate() {
        let end = site.chunk_pose(target, i).unwrap().w_axis.truncate();
        // Chunks fell and came to rest on the ground, not under it.
        assert!(end.y <= start.y + 0.05, "chunk {i} rose: {start} to {end}");
        assert!(end.y > -0.05, "chunk {i} sank: {end}");
    }
    // The blow pushed the debris inward, along the swing.
    let moved: f32 = (0..chunks.len())
        .map(|i| (site.chunk_pose(target, i).unwrap().w_axis.truncate() - before[i]).z)
        .sum();
    assert!(moved > 0.0, "the debris flew with the blow");
    assert!(!site.puffs().is_empty() || site.time() > 2.0);
}

#[test]
fn breaking_two_south_sections_leaves_the_roof_up() {
    let mut site = site();
    for index in [1, 2] {
        let piece = find(&site, wall(Side::South, index));
        site.damage(piece, 1000, site.specs()[piece].center, DVec3::ZERO);
    }
    let roof = find(&site, Role::Roof);
    assert_eq!(site.pieces()[roof].status, Status::Standing);
    run(&mut site, 1.0);
    assert_eq!(site.pieces()[roof].status, Status::Standing);
}

#[test]
fn knocking_out_an_eave_wall_drops_the_roof_onto_the_rest() {
    let mut site = site();
    let roof = find(&site, Role::Roof);
    let start = site.specs()[roof].center.y as f32;
    for index in 0..4 {
        let piece = find(&site, wall(Side::West, index));
        site.damage(piece, 1000, site.specs()[piece].center, DVec3::ZERO);
        let expected = if index < 3 {
            Status::Standing
        } else {
            Status::Loose
        };
        assert_eq!(site.pieces()[roof].status, expected, "after {index}");
    }
    // The chimney and the gables go with it.
    assert_ne!(
        site.pieces()[find(&site, Role::Chimney)].status,
        Status::Standing
    );
    assert_ne!(
        site.pieces()[find(&site, Role::Gable { side: Side::South })].status,
        Status::Standing
    );
    run(&mut site, 3.0);
    let roof_state = &site.pieces()[roof];
    match roof_state.status {
        Status::Broken => {}
        _ => {
            let pose = site.piece_pose(roof);
            assert!(pose.w_axis.y < start - 0.5, "the roof dropped");
            // It lies tilted toward the side that gave way.
            let up = pose.transform_vector3(Vec3::Y);
            assert!(up.x < -0.05, "the roof leans west: {up}");
        }
    }
    // The other cottage is untouched.
    assert!(
        site.specs()
            .iter()
            .zip(site.pieces())
            .filter(|(s, _)| s.building == 1)
            .all(|(_, p)| p.status == Status::Standing)
    );
}

#[test]
fn a_wall_section_with_no_neighbours_left_topples_outward() {
    let mut site = site();
    let lone = find(&site, wall(Side::North, 1));
    for index in [0, 2] {
        let piece = find(&site, wall(Side::North, index));
        site.damage(piece, 1000, site.specs()[piece].center, DVec3::ZERO);
    }
    assert_eq!(site.pieces()[lone].status, Status::Loose);
    let z = site.specs()[lone].center.z as f32;
    run(&mut site, 3.0);
    match site.pieces()[lone].status {
        // It slammed down and broke.
        Status::Broken => {}
        _ => assert!(site.piece_pose(lone).w_axis.z > z + 0.3, "it fell outward"),
    }
}

#[test]
fn only_standing_walls_and_posts_block_the_player() {
    let mut site = site();
    let all = site.blocks().len();
    // Two buildings: 18 sections each (doors add a jamb), 4 posts each.
    assert_eq!(all, 2 * (18 + 1 + 4));
    let piece = find(&site, wall(Side::South, 3));
    site.damage(piece, 1000, site.specs()[piece].center, DVec3::ZERO);
    assert_eq!(site.blocks().len(), all - 1);
    // A doorway leaves room to walk through.
    let ([cx, cz], [hx, hz]) = COTTAGES[0];
    let door = Vec3::new(cx - hx + 3.0, 0.0, cz - hz);
    assert!(
        !site
            .blocks()
            .iter()
            .any(|(f, _)| f.contains(door.x, door.z, 0.3)),
        "the doorway is open"
    );
}

#[test]
fn debris_has_a_lifetime() {
    let mut site = site();
    let piece = find(&site, wall(Side::East, 2));
    site.damage(piece, 1000, site.specs()[piece].center, DVec3::ZERO);
    assert!(site.chunk_pose(piece, 0).is_some());
    run(&mut site, site::DEBRIS_LIFETIME + 5.0);
    assert!(site.pieces()[piece].chunks.iter().all(|c| c.gone));
    assert!(site.chunk_pose(piece, 0).is_none());
}

#[test]
fn rebuilding_restores_every_piece() {
    let mut site = site();
    let all = site.blocks();
    for index in 0..5 {
        let piece = find(&site, wall(Side::West, index));
        site.damage(piece, 1000, site.specs()[piece].center, DVec3::ZERO);
    }
    run(&mut site, 1.0);
    site.reset();
    for (spec, piece) in site.specs().iter().zip(site.pieces()) {
        assert_eq!(piece.status, Status::Standing);
        assert_eq!(piece.hit_points, spec.hit_points);
        assert!(piece.chunks.is_empty());
    }
    assert_eq!(site.blocks(), all);
    for i in 0..site.pieces().len() {
        let pose = site.piece_pose(i);
        assert!((pose.w_axis.truncate() - site.specs()[i].center.as_vec3()).length() < 1e-4);
    }
}

#[test]
fn the_yard_builds_from_the_pack_and_draws_with_the_character() {
    let pack = pack();
    let mut yard = Demolition::new(pack).expect("the kit is in the pack");
    // Every piece is cut into chunks whose boxes have volume.
    for spec in yard.site().specs() {
        assert!(spec.chunks.len() >= 2, "{:?}", spec.role);
        assert!(spec.chunks.iter().all(|c| c.half.min_element() > 0.0));
    }
    let alone = yard.figure(None);
    alone.validate().expect("the yard alone is a valid figure");
    let player = PlayerController::new(Vec3::new(0.0, 0.0, -20.0), 0.0);
    let cast = player::Cast::new(pack, &player)
        .unwrap()
        .expect("the pack has a character");
    let figure = cast.figure();
    yard.prepare(Some(&figure.scene));
    let joined = yard.figure(Some(figure.clone()));
    joined
        .validate()
        .expect("the character and the yard are one valid figure");
    assert_eq!(
        joined.vertices.len(),
        figure.vertices.len() + alone.vertices.len()
    );
    // Whole pieces draw where they were placed: the wall vertices sit on
    // the cottages' footprints.
    let ([cx, cz], [hx, hz]) = COTTAGES[0];
    assert!(
        alone
            .vertices
            .iter()
            .any(|v| { (v.pos[0] - cx).abs() < hx + 0.5 && (v.pos[2] - (cz - hz)).abs() < 0.5 })
    );
    // A swing in front of a wall damages it.
    let mut at = PlayerController::new(Vec3::new(cx - hx + 5.0, 0.0, cz - hz - 0.8), 0.0);
    at.yaw = 0.0;
    assert!(yard.swing());
    for _ in 0..60 {
        yard.tick(1.0 / 60.0, &at);
    }
    let hurt = yard
        .site()
        .pieces()
        .iter()
        .zip(yard.site().specs())
        .any(|(p, s)| p.hit_points < s.hit_points);
    assert!(hurt, "the hammer struck the south wall");
    assert!(yard.caption().contains("damage"));
    // The hammer and the damage draw.
    assert!(!yard.mesh(&at).faces.is_empty());
}

use super::super::player;
