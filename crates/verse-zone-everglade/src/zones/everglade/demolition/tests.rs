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
    Role::Wall {
        side,
        index,
        count,
        story: 0,
    }
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
    let roof = find(&site, Role::Roof { span: 0, spans: 1 });
    assert_eq!(site.pieces()[roof].status, Status::Standing);
    run(&mut site, 1.0);
    assert_eq!(site.pieces()[roof].status, Status::Standing);
}

#[test]
fn knocking_out_an_eave_wall_drops_the_roof_onto_the_rest() {
    let mut site = site();
    let roof = find(&site, Role::Roof { span: 0, spans: 1 });
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
        site.pieces()[find(&site, Role::Chimney { span: 0 })].status,
        Status::Standing
    );
    assert_ne!(
        site.pieces()[find(
            &site,
            Role::Gable {
                side: Side::South,
                span: 0
            }
        )]
        .status,
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
    assert!(
        !yard
            .mesh(&at, at.pos + Vec3::new(0.0, 2.0, -4.0), None)
            .faces
            .is_empty()
    );
}

use super::super::player;

mod meteor_swarm {
    use super::super::meteor::{self, MAX_MANA, RANGE, Swarm};
    use super::*;

    /// The caster, south of the west cottage.
    fn caster() -> PlayerController {
        PlayerController::new(Vec3::new(-6.0, 0.0, -26.0), 0.0)
    }

    #[test]
    fn impacts_start_lights_at_the_particle_origin_and_reset_clears_them() {
        let mut site = site();
        let mut swarm = Swarm::default();
        let player = caster();
        swarm.target().unwrap();
        swarm.aim_at(Vec3::new(-6.0, 0.0, -18.0), &player);
        assert!(swarm.confirm(&player));
        let mut landed = 0;
        for _ in 0..360 {
            swarm.tick(1.0 / 60.0, &player, &mut site);
            let impacts = swarm.take_impacts();
            for impact in impacts {
                let lamps = swarm.flash_lamps(player.pos);
                assert!(
                    lamps
                        .iter()
                        .any(|lamp| lamp.lit() && lamp.position == impact.at)
                );
                landed += 1;
            }
            if landed == meteor::METEORS {
                break;
            }
        }
        assert_eq!(landed, meteor::METEORS);
        assert!(swarm.flash_lamps(player.pos).iter().any(|lamp| lamp.lit()));
        swarm.reset();
        assert!(swarm.flash_lamps(player.pos).iter().all(|lamp| !lamp.lit()));
    }

    #[test]
    fn thunderbolt_bursts_start_blue_direct_light() {
        for strike in [meteor::Strike::Lightning, meteor::Strike::MegaLightning] {
            let mut site = site();
            let mut swarm = Swarm::default();
            let player = caster();
            swarm.target_with(strike).unwrap();
            swarm.aim_at(Vec3::new(-6.0, 0.0, -18.0), &player);
            assert!(swarm.confirm(&player));
            let mut lit = false;
            for _ in 0..360 {
                swarm.tick(1.0 / 60.0, &player, &mut site);
                if let Some(impact) = swarm.take_impacts().first() {
                    let lamps = swarm.flash_lamps(player.pos);
                    let lamp = lamps.iter().find(|lamp| lamp.lit()).unwrap();
                    assert_eq!(lamp.position, impact.at);
                    assert!(lamp.color[2] > lamp.color[0]);
                    lit = true;
                    break;
                }
            }
            assert!(lit, "{} lights its impact", strike.name());
        }
    }

    /// Casts at `at` and runs the cast, the meteors, and `after` more
    /// seconds of the site.
    fn strike(site: &mut Site, swarm: &mut Swarm, at: Vec3, after: f64) {
        let player = caster();
        swarm.target().expect("the spell is ready");
        swarm.aim_at(at, &player);
        assert!(swarm.confirm(&player));
        let dt = 1.0 / 60.0;
        let mut seconds = 0.0;
        while swarm.casting() || swarm.meteors_left() > 0 {
            swarm.tick(dt, &player, site);
            site.tick(dt);
            seconds += dt;
            assert!(seconds < 6.0, "the strike ends");
        }
        run(site, after);
    }

    #[test]
    fn targeting_clamps_to_the_spells_range() {
        let player = caster();
        let mut swarm = Swarm::default();
        // Not targeting: aiming does nothing.
        swarm.aim_at(Vec3::new(0.0, 0.0, 0.0), &player);
        assert_eq!(swarm.aim(), None);
        swarm.target().unwrap();
        swarm.aim_at(Vec3::new(-6.0, 0.0, -14.0), &player);
        let near = swarm.aim().expect("the circle is down");
        assert!((near - Vec3::new(-6.0, 0.0, -14.0)).length() < 1e-4);
        swarm.aim_at(Vec3::new(-6.0, 0.0, 2.0 * RANGE + 40.0), &player);
        let far = swarm.aim().unwrap();
        let reach = Vec3::new(far.x - player.pos.x, 0.0, far.z - player.pos.z).length();
        assert!((reach - RANGE).abs() < 1e-3, "{reach}");
        assert!(far.z > player.pos.z, "it keeps the cursor's direction");
        assert!((far.y - crate::zones::everglade::height(far.x, far.z)).abs() < 1e-4);
        // A ray from a raised camera meets the ground where it points.
        let eye = Vec3::new(-6.0, 4.0, -30.0);
        let hit = meteor::ground_hit(eye, (Vec3::new(-6.0, 0.0, -18.0) - eye).normalize())
            .expect("the ray meets the ground");
        assert!((hit - Vec3::new(-6.0, 0.0, -18.0)).length() < 0.05, "{hit}");
        // A ray at the sky aims the circle ahead.
        let up = meteor::ground_hit(eye, Vec3::new(0.0, 1.0, 1.0)).unwrap();
        assert!(up.z > eye.z);
    }

    #[test]
    fn a_strike_breaks_the_pieces_near_its_center_and_leaves_far_ones_standing() {
        let mut site = site();
        let mut swarm = Swarm::default();
        // Just outside the middle of the west cottage's south front.
        let target = find(&site, wall(Side::South, 1));
        let center = site.specs()[target].center.as_vec3();
        let at = Vec3::new(center.x, 0.0, center.z - 0.5);
        strike(&mut site, &mut swarm, at, 2.0);
        assert_eq!(site.pieces()[target].status, Status::Broken);
        let near_broken = site
            .specs()
            .iter()
            .zip(site.pieces())
            .filter(|(s, _)| s.center.as_vec3().distance(at) < 3.0)
            .filter(|(_, p)| p.status == Status::Broken)
            .count();
        assert!(
            near_broken >= 3,
            "{near_broken} pieces broke near the center"
        );
        // The other cottage, far outside the circle, is untouched.
        for (spec, piece) in site.specs().iter().zip(site.pieces()) {
            if spec.building == 1 {
                assert_eq!(piece.status, Status::Standing, "{:?}", spec.role);
                assert_eq!(piece.hit_points, spec.hit_points, "{:?}", spec.role);
            }
        }
        // The debris stays within the cap.
        let alive = site
            .pieces()
            .iter()
            .flat_map(|p| &p.chunks)
            .filter(|c| !c.gone)
            .count();
        assert!(alive <= site::MAX_CHUNKS);
        assert!(alive > 0);
    }

    #[test]
    fn the_roof_falls_when_its_walls_are_destroyed() {
        let mut site = site();
        let mut swarm = Swarm::default();
        let roof = find(&site, Role::Roof { span: 0, spans: 1 });
        let start = site.specs()[roof].center.y as f32;
        // Just outside the west eave wall's middle.
        let ([cx, cz], [hx, _]) = COTTAGES[0];
        strike(
            &mut site,
            &mut swarm,
            Vec3::new(cx - hx - 0.5, 0.0, cz),
            3.0,
        );
        let west = (0..5)
            .filter(|&i| site.pieces()[find(&site, wall(Side::West, i))].status != Status::Standing)
            .count();
        assert!(west >= 4, "{west} of the west wall's sections are down");
        let state = &site.pieces()[roof];
        assert_ne!(state.status, Status::Standing, "the roof came down");
        if state.status == Status::Loose {
            assert!(
                site.piece_pose(roof).w_axis.y < start - 0.5,
                "the roof dropped"
            );
        }
    }

    #[test]
    fn casts_follow_each_other_at_once() {
        let mut site = site();
        let mut swarm = Swarm::default();
        assert!(swarm.status().ready);
        strike(&mut site, &mut swarm, Vec3::new(-6.0, 0.0, -16.0), 0.0);
        // Free and without a cooldown: the next cast can be aimed at once.
        let status = swarm.status();
        assert_eq!(status.mana, MAX_MANA);
        assert_eq!(swarm.cooldown_left(), 0.0);
        assert!(status.ready);
        assert!(swarm.target().is_ok());
        assert!(swarm.targeting());
    }

    #[test]
    fn walking_and_jumping_through_the_cast_still_brings_the_meteors_down_where_aimed() {
        let mut site = site();
        let mut player = caster();
        let mut swarm = Swarm::default();
        let at = Vec3::new(-6.0, 0.0, -16.0);
        swarm.target().unwrap();
        swarm.aim_at(at, &player);
        assert!(swarm.confirm(&player));
        let dt = 1.0 / 60.0;
        let mut seconds = 0.0;
        let mut landed = Vec::new();
        while swarm.casting() || swarm.meteors_left() > 0 || seconds < 0.1 {
            // Walking away to the east, and hopping.
            player.pos.x += 0.08;
            player.pos.y = if (seconds * 2.0_f32).fract() < 0.5 {
                0.6
            } else {
                0.0
            };
            swarm.tick(dt, &player, &mut site);
            landed.extend(swarm.take_impacts());
            seconds += dt;
            assert!(seconds < 6.0, "the strike ends");
        }
        assert_eq!(landed.len(), meteor::METEORS, "every meteor fell");
        for impact in &landed {
            let flat = Vec3::new(impact.at.x - at.x, 0.0, impact.at.z - at.z);
            assert!(flat.length() <= meteor::AREA + 0.5, "{impact:?}");
        }
    }

    #[test]
    fn cancelling_spends_nothing() {
        let mut site = site();
        let player = caster();
        let mut swarm = Swarm::default();
        swarm.target().unwrap();
        swarm.aim_at(Vec3::new(-6.0, 0.0, -16.0), &player);
        assert!(swarm.aim().is_some());
        swarm.cancel();
        assert!(!swarm.targeting());
        assert!(!swarm.confirm(&player), "nothing is aimed");
        // Pressing the slot again also leaves the aim.
        swarm.target().unwrap();
        swarm.target().unwrap();
        assert!(!swarm.targeting());
        // A cast stopped partway, as Esc stops it, spends nothing.
        swarm.target().unwrap();
        swarm.aim_at(Vec3::new(-6.0, 0.0, -16.0), &player);
        assert!(swarm.confirm(&player));
        let walked = player.clone();
        for _ in 0..30 {
            swarm.tick(1.0 / 60.0, &walked, &mut site);
        }
        swarm.cancel();
        assert!(!swarm.casting());
        for _ in 0..240 {
            swarm.tick(1.0 / 60.0, &walked, &mut site);
        }
        assert_eq!(swarm.meteors_left(), 0);
        assert_eq!(swarm.status().mana, MAX_MANA);
        assert_eq!(swarm.cooldown_left(), 0.0);
        assert!(swarm.status().ready);
        assert!(
            site.pieces()
                .iter()
                .zip(site.specs())
                .all(|(p, s)| p.hit_points == s.hit_points)
        );
    }
}
