//! The town's destructible buildings: lazy raising, the studio's
//! protection, collapse, the solids, restoring, and the caps.

use super::site::{Role, Status};
use super::town::{Building, MAX_CHUNKS, MAX_LIVE, Town};
use crate::controller::PlayerController;
use crate::pbr::textured::{IndexEdits, TexturedScene};
use crate::zones::everglade::layout::{self, COTTAGE, READING_ROOM, SHOPS};
use crate::zones::everglade::tests::{pack, world};
use crate::zones::everglade::{HALL, height};
use glam::Vec3;
use std::sync::Arc;

/// The zone's static scene with edits of its own, as each zone entry has.
fn scene() -> Arc<TexturedScene> {
    let shared = world()
        .mesh
        .textured
        .as_ref()
        .expect("the zone has a scene");
    Arc::new(TexturedScene {
        edits: IndexEdits::default(),
        ..shared.as_ref().clone()
    })
}

fn town_over(scene: Arc<TexturedScene>) -> Town {
    Town::new(pack(), &layout::placements(), scene).expect("the town builds")
}

fn town() -> Town {
    town_over(scene())
}

/// The building whose footprint is `rect`.
fn building(town: &Town, rect: ([f32; 2], [f32; 2])) -> usize {
    town.buildings()
        .iter()
        .position(|b| {
            (b.rect.0[0] - rect.0[0]).abs() < 0.01 && (b.rect.0[1] - rect.0[1]).abs() < 0.01
        })
        .expect("the survey found the building")
}

/// A caster `back` meters south of `at`, facing it.
fn caster(at: Vec3, back: f32) -> PlayerController {
    let mut player = PlayerController::new(Vec3::new(at.x, 0.0, at.z - back), 0.0);
    player.pos.y = height(player.pos.x, player.pos.z);
    player
}

/// Casts Meteor Swarm at `at` from `player` and runs the strike and
/// `after` more seconds.
fn strike(town: &mut Town, player: &PlayerController, at: Vec3, after: f32) {
    town.meteor_swarm(player).expect("the spell is ready");
    assert!(town.aim(at + Vec3::Y * 12.0, Vec3::NEG_Y, player));
    assert!(town.confirm(player));
    let dt = 1.0 / 60.0;
    let mut seconds = 0.0;
    while town.swarm().casting() || town.swarm().meteors_left() > 0 {
        town.tick(dt, player);
        seconds += dt;
        assert!(seconds < 8.0, "the strike ends");
    }
    run(town, player, after);
}

fn run(town: &mut Town, player: &PlayerController, seconds: f32) {
    let dt = 1.0 / 60.0;
    for _ in 0..(seconds / dt).ceil() as usize {
        town.tick(dt, player);
    }
}

/// The site pieces of `building` and their status.
fn pieces(town: &Town, building: usize) -> Vec<(usize, Status)> {
    town.refs()
        .iter()
        .enumerate()
        .filter(|(_, (b, _))| *b == building)
        .map(|(i, _)| (i, town.site().pieces()[i].status))
        .collect()
}

fn south_front(b: &Building) -> Vec3 {
    let ([cx, cz], [_, hz]) = b.rect;
    Vec3::new(cx, height(cx, cz - hz), cz - hz - 0.4)
}

#[test]
fn the_survey_maps_the_kit_buildings_and_carves_everything_else() {
    let town = town();
    let buildings = town.buildings();
    let kit: Vec<&Building> = buildings.iter().filter(|b| !b.is_carved()).collect();
    // The lane's and Main Street's houses and the city's storied
    // buildings, most of the town, map onto the kit rules.
    assert!(kit.len() >= 40, "{} kit buildings", kit.len());
    // The workshop hall is the studio's, and breaks like the rest.
    let hall = building(&town, HALL);
    assert!(buildings[hall].destructible());
    // Every generated building and landmark is carved into blocks.
    let placements = layout::placements();
    for instance in layout::generated() {
        let carved = buildings.iter().find(|b| {
            b.carved.iter().any(|c| {
                placements[c.placement].model == instance.model.name
                    && placements[c.placement].at == instance.at
            })
        });
        let b = carved.unwrap_or_else(|| panic!("{} is carved", instance.name));
        assert!(b.destructible(), "{} breaks", instance.name);
    }
    // The studio's furniture is too.
    assert!(buildings.iter().any(|b| {
        b.is_carved()
            && b.carved
                .iter()
                .any(|c| placements[c.placement].model == "props/Workbench")
    }));
    for b in kit.iter().filter(|b| b.destructible()) {
        // Every destructible building has a roof span over every 8 m and
        // walls on every story.
        let spans = b
            .pieces
            .iter()
            .filter(|p| matches!(p.draft.role, Role::Roof { .. }))
            .count();
        assert_eq!(spans, ((b.rect.1[0] * 2.0 / 8.0).round() as usize).max(1));
        let walls = b
            .pieces
            .iter()
            .filter(|p| matches!(p.draft.role, Role::Wall { .. }))
            .count();
        let sections = 2.0 * (b.rect.1[0] + b.rect.1[1]) * f32::from(b.stories);
        assert_eq!(walls, sections.round() as usize);
    }
    // Storied city buildings are among them.
    assert!(buildings.iter().any(|b| b.destructible() && b.stories >= 2));
    // The cottage, the reading room, and the shops are.
    for rect in [COTTAGE, READING_ROOM, SHOPS[0]] {
        assert!(buildings[building(&town, rect)].destructible());
    }
}

#[test]
fn untouched_buildings_stay_in_the_static_cells() {
    let scene = scene();
    let mut town = town_over(scene.clone());
    let player = caster(Vec3::new(0.0, 0.0, -20.0), 0.0);
    run(&mut town, &player, 0.5);
    assert!(town.raised().is_empty());
    assert_eq!(town.hidden(), 0);
    assert!(town.own_figure().is_none(), "nothing draws on its own");
    assert_eq!(scene.edits.revision(), 0, "no index was rewritten");
}

#[test]
fn a_meteor_strike_breaks_the_pieces_near_its_center() {
    let scene = scene();
    let mut town = town_over(scene.clone());
    let cottage = building(&town, COTTAGE);
    let at = south_front(&town.buildings()[cottage]);
    let player = caster(at, 16.0);
    strike(&mut town, &player, at, 2.0);
    assert!(town.raised().contains(&cottage));
    let broken = pieces(&town, cottage)
        .into_iter()
        .filter(|&(i, status)| {
            status == Status::Broken && town.site().specs()[i].center.as_vec3().distance(at) < 3.5
        })
        .count();
    assert!(broken >= 3, "{broken} pieces broke near the center");
    // The broken pieces left the static cells and draw as chunks, their
    // far levels of detail with them.
    assert!(town.hidden() >= broken);
    let ranges = scene.index_ranges();
    let fars = crate::zones::everglade::detail::far_placements(pack(), &layout::placements());
    let (edits, _) = scene.edits.since(0);
    let written: std::collections::BTreeSet<u32> = edits.iter().map(|(first, _)| *first).collect();
    assert!(
        fars.iter()
            .flatten()
            .any(|(_, at)| ranges[*at].iter().any(|r| written.contains(&r.first))),
        "a far level hides with its piece"
    );
    let figure = town.own_figure().expect("the chunks draw");
    figure.validate().expect("a valid figure");
    assert!(
        figure.vertices.iter().any(|v| v.pos[1] > -10.0),
        "some chunk is posed in the world"
    );
    // Only the buildings the blasts reached were raised, within the cap.
    assert!(town.raised().len() <= MAX_LIVE);
    for &b in &town.raised() {
        let reached = town.buildings()[b].rect;
        let ([cx, cz], [hx, hz]) = reached;
        let gap = ((at.x - cx).abs() - hx)
            .max(0.0)
            .hypot(((at.z - cz).abs() - hz).max(0.0));
        assert!(gap < 12.0, "building {b} is {gap} m from the strike");
    }
}

#[test]
fn the_studio_breaks_and_comes_back() {
    let mut town = town();
    let hall = building(&town, HALL);
    let at = south_front(&town.buildings()[hall]);
    let player = caster(at, 14.0);
    strike(&mut town, &player, at, 1.0);
    assert!(town.raised().contains(&hall));
    assert!(
        pieces(&town, hall)
            .iter()
            .any(|&(_, s)| s == Status::Broken),
        "the hall's front broke"
    );
    town.restore();
    assert!(town.raised().is_empty());
    assert_eq!(town.hidden(), 0);
}

#[test]
fn restoring_brings_every_building_back() {
    let scene = scene();
    let mut town = town_over(scene.clone());
    let cottage = building(&town, COTTAGE);
    let at = south_front(&town.buildings()[cottage]);
    let player = caster(at, 16.0);
    let first = {
        let mut fresh = self::town();
        fresh.restore();
        fresh.take_solids().expect("the restored town has solids")
    };
    strike(&mut town, &player, at, 1.0);
    assert!(town.hidden() > 0);
    town.restore();
    run(&mut town, &player, 0.1);
    assert!(town.raised().is_empty());
    assert_eq!(town.hidden(), 0);
    assert!(town.own_figure().is_none());
    // Every index range the strike rewrote holds its placement's own
    // triangles again.
    let (edits, _) = scene.edits.since(0);
    assert!(!edits.is_empty());
    let ranges = scene.index_ranges();
    for (first_index, indices) in edits {
        let (placement, range) = ranges
            .iter()
            .enumerate()
            .find_map(|(p, rs)| rs.iter().find(|r| r.first == first_index).map(|r| (p, *r)))
            .expect("an edit is a placement's range");
        assert_eq!(indices, scene.range_indices(placement, &range));
    }
    // The solids are the town's whole solids again.
    let restored = town.take_solids().expect("the solids changed back");
    let feet = height(at.x, at.z);
    assert_eq!(restored.blocking(feet).len(), first.blocking(feet).len());
}

#[test]
fn the_solids_follow_the_collapse() {
    let mut town = town();
    let cottage = building(&town, COTTAGE);
    let b = town.buildings()[cottage].clone();
    let ([cx, cz], [hx, _]) = b.rect;
    let solids = |town: &mut Town| town.take_solids();
    // Whole, the roof holds a levitating player over the cottage.
    let mut whole = self::town();
    whole.restore();
    let first = solids(&mut whole).unwrap();
    let ground = height(cx, cz);
    let over = first.floor(cx + 1.0, cz, ground + 9.0);
    assert!(over > ground + 3.0, "the roof is underfoot at {over}");
    // Blast both eave walls: the roof comes down, and so does what
    // stood on it.
    let player = caster(Vec3::new(cx, 0.0, cz), 18.0);
    strike(
        &mut town,
        &player,
        Vec3::new(cx - hx - 0.5, ground, cz),
        0.5,
    );
    strike(
        &mut town,
        &player,
        Vec3::new(cx + hx + 0.5, ground, cz),
        3.0,
    );
    let after = solids(&mut town).expect("the solids changed");
    let roof = pieces(&town, cottage)
        .into_iter()
        .find(|&(i, _)| matches!(town.site().specs()[i].role, Role::Roof { .. }))
        .expect("the roof was raised");
    assert_ne!(roof.1, Status::Standing, "the roof fell");
    assert!(
        after.floor(cx + 1.0, cz, ground + 9.0) < ground + 3.0,
        "no roof is left to stand on"
    );
    // The west wall's gap lets the player through.
    let blocked = |s: &crate::zones::everglade::solids::Solids| {
        s.blocking(ground)
            .iter()
            .filter(|f| {
                f.min[0] <= cx - hx + 0.3
                    && f.max[0] >= cx - hx - 0.3
                    && f.min[1] <= cz
                    && f.max[1] >= cz
            })
            .count()
    };
    assert!(blocked(&first) > 0, "the whole west wall blocks");
    assert_eq!(blocked(&after), 0, "the broken west wall doesn't");
}

#[test]
fn the_caps_hold_under_repeated_casts() {
    let mut town = town();
    let targets: Vec<usize> = town
        .buildings()
        .iter()
        .enumerate()
        .filter(|(_, b)| b.destructible())
        .map(|(i, _)| i)
        .take(6)
        .collect();
    for &b in &targets {
        let at = south_front(&town.buildings()[b]);
        let player = caster(at, 14.0);
        strike(&mut town, &player, at, 0.2);
        assert!(town.raised().len() <= MAX_LIVE, "{:?}", town.raised());
        let alive = town
            .site()
            .pieces()
            .iter()
            .flat_map(|p| &p.chunks)
            .filter(|c| !c.gone)
            .count();
        assert!(alive <= MAX_CHUNKS, "{alive} chunks");
    }
    // Buildings let go stand whole in the static cells again.
    let raised = town.raised();
    let hidden_raised = town
        .refs()
        .iter()
        .filter(|(b, _)| raised.contains(b))
        .count();
    assert!(town.hidden() <= hidden_raised);
}

#[test]
fn the_hammer_breaks_a_town_wall() {
    let mut town = town();
    let cottage = building(&town, COTTAGE);
    let at = south_front(&town.buildings()[cottage]);
    let mut player = caster(at, 1.0);
    player.yaw = 0.0;
    let mut swings = 0;
    while town.raised().is_empty()
        || pieces(&town, cottage)
            .iter()
            .all(|&(_, s)| s == Status::Standing)
    {
        assert!(town.swing());
        run(&mut town, &player, 1.2);
        swings += 1;
        assert!(swings < 12, "the hammer reaches the wall");
    }
    assert!(town.raised().contains(&cottage));
}

#[test]
fn the_hotbar_ends_with_meteor_swarm_and_the_sledgehammer() {
    use crate::zones::Intent;
    use crate::zones::everglade::hotbar::{COUNT, SLOTS, card};
    let tail: Vec<Intent> = SLOTS[COUNT - 2..].iter().map(|(i, ..)| *i).collect();
    assert_eq!(tail, [Intent::MeteorSwarm, Intent::Swing]);
    let meteor = card(COUNT - 2).expect("a card");
    assert!(
        meteor
            .details
            .iter()
            .any(|(d, _)| d.contains("No mana, no cooldown"))
    );
    assert!(SLOTS[COUNT - 2].2.text.contains("R restores the town"));
}

/// Breaks the cottage's south wall sections on the ground story within
/// 2.5 m of its middle, and lets the town catch up. Returns the cottage.
fn open_the_cottage(town: &mut Town, player: &PlayerController) -> usize {
    use super::site::Side;
    let cottage = building(town, COTTAGE);
    let ([cx, cz], [_, hz]) = town.buildings()[cottage].rect;
    // A blast of no damage raises it into the rules.
    town.blast(
        Vec3::new(cx, height(cx, cz) + 1.0, cz - hz),
        2.0,
        0,
        Vec3::NEG_Z,
    );
    let doomed: Vec<usize> = pieces(town, cottage)
        .into_iter()
        .map(|(i, _)| i)
        .filter(|&i| {
            let spec = &town.site().specs()[i];
            matches!(
                spec.role,
                Role::Wall {
                    side: Side::South,
                    story: 0,
                    ..
                }
            ) && (spec.center.x as f32 - cx).abs() < 2.5
        })
        .collect();
    assert!(!doomed.is_empty(), "the south wall has sections to break");
    for i in doomed {
        let at = town.site().specs()[i].center;
        town.site_mut().damage(i, 100_000, at, glam::DVec3::Z);
    }
    run(town, player, 0.5);
    cottage
}

#[test]
fn through_a_blown_out_wall_the_aim_finds_the_far_walls_inner_face() {
    let mut town = town();
    let ([cx, cz], [_, hz]) = COTTAGE;
    let player = caster(Vec3::new(cx, 0.0, cz - hz), 10.0);
    let cottage = open_the_cottage(&mut town, &player);
    let base = town.buildings()[cottage].base;
    let eye = player.pos + Vec3::Y * 1.6;
    town.meteor_swarm(&player).unwrap();
    // Through the hole at the north wall, at chest height.
    let inside = Vec3::new(cx + 0.3, base + 1.5, cz + hz);
    assert!(town.aim(eye, (inside - eye).normalize(), &player));
    let aimed = town.swarm().aimed().expect("the ring is down");
    assert!(aimed.wall(), "{aimed:?}");
    assert!(
        aimed.at.z > cz + hz - 1.0 && aimed.at.z < cz + hz + 0.05,
        "on the north wall's inner face: {aimed:?}"
    );
    assert!(aimed.normal.z < -0.9, "facing the caster: {aimed:?}");
    // A section of the south wall still standing takes the ray on its outer
    // face.
    let side = Vec3::new(cx - 3.4, base + 1.5, cz - hz);
    assert!(town.aim(eye, (side - eye).normalize(), &player));
    let outer = town.swarm().aimed().unwrap();
    assert!(outer.wall() && outer.normal.z < -0.9, "{outer:?}");
    assert!((outer.at.z - (cz - hz)).abs() < 0.6, "{outer:?}");
    // From above, the roof takes a flat ring.
    let high = Vec3::new(cx + 2.5, base + 30.0, cz - 4.0);
    let roof = Vec3::new(cx + 2.5, base + 4.0, cz + 1.0);
    assert!(town.aim(high, (roof - high).normalize(), &player));
    let flat = town.swarm().aimed().unwrap();
    assert!(!flat.wall() && flat.at.y > base + 3.0, "{flat:?}");
    // Back on the far wall, Meteor Swarm comes in through the hole and
    // damages it.
    assert!(town.aim(eye, (inside - eye).normalize(), &player));
    assert!(town.confirm(&player));
    run(&mut town, &player, super::meteor::CAST + 2.5);
    let site = town.site();
    let hurt = pieces(&town, cottage)
        .into_iter()
        .filter(|&(i, _)| {
            let (spec, piece) = (&site.specs()[i], &site.pieces()[i]);
            spec.center.z as f32 > cz + hz - 1.0 && piece.hit_points < spec.hit_points
        })
        .count();
    assert!(hurt >= 2, "{hurt} far wall pieces took damage");
}
