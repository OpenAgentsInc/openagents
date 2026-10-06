//! The Grove's concrete tower against the pinned pack: aiming Meteor
//! Swarm and the Thunderbolt at its side, the crater a strike leaves, the
//! top toppling over an undercut, R restoring it, and the caps holding.

use crate::controller::{InputState, PlayerController};
use crate::runtime::WorldRuntime;
use crate::zones::everglade::demolition::meteor::{self, Strike};
use crate::zones::everglade::demolition::site::{Role, Status};
use crate::zones::everglade::demolition::town::{MAX_CHUNKS, MAX_PIECES, Town};
use crate::zones::everglade_pack::{self, ZonePack};
use crate::zones::grove::kit::Spell;
use crate::zones::grove::layout::TOWER;
use crate::zones::grove::{Grove, slots};
use crate::zones::{Intent, ZoneId};
use glam::Vec3;
use std::path::Path;
use std::sync::OnceLock;

const DT: f32 = 1.0 / 60.0;

fn pack() -> &'static ZonePack {
    static PACK: OnceLock<ZonePack> = OnceLock::new();
    PACK.get_or_init(|| {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(everglade_pack::PACK_DIRECTORY)
            .join(format!(
                "{}.{}",
                everglade_pack::PACK_SHA256,
                everglade_pack::PACK_EXTENSION
            ));
        ZonePack::load_local(&path).expect("the committed pack loads")
    })
}

/// In the Grove, standing `back` m south of the tower, facing it.
fn before_tower(back: f32) -> WorldRuntime {
    let mut runtime = WorldRuntime::new();
    runtime.install_grove(pack());
    assert_eq!(runtime.zone, ZoneId::Grove);
    runtime
        .set_spawn(Vec3::new(TOWER[0], 0.0, TOWER[1] - back), 0.0)
        .unwrap();
    runtime
}

fn idle(runtime: &mut WorldRuntime, seconds: f32) {
    for _ in 0..(seconds / DT).round() as usize {
        runtime.tick(&InputState::default(), DT);
    }
}

fn town(runtime: &WorldRuntime) -> &Town {
    runtime
        .zone_state
        .everglade
        .as_ref()
        .and_then(|glade| glade.town())
        .expect("the Grove's tower can break")
}

fn town_mut(runtime: &mut WorldRuntime) -> &mut Town {
    runtime
        .zone_state
        .everglade
        .as_mut()
        .and_then(|glade| glade.town_mut())
        .expect("the Grove's tower can break")
}

fn grove(runtime: &WorldRuntime) -> &Grove {
    runtime.zone_state.grove.as_ref().expect("in the Grove")
}

/// Casts `spell` from its slot on the bar.
fn cast(runtime: &mut WorldRuntime, spell: Spell) {
    let slot = slots::slot_of(spell, None, grove(runtime).land()).expect("on the bar");
    runtime
        .zone_intent(Intent::GroveSlot(slot as u8))
        .expect("the spell aims");
}

/// Aims the targeting at `point` from the player's eyes.
fn aim(runtime: &mut WorldRuntime, point: Vec3) {
    let player = runtime.player.clone();
    let eye = player.pos + Vec3::Y * 1.6;
    let glade = runtime.zone_state.everglade.as_mut().unwrap();
    assert!(glade.aim_swarm(eye, (point - eye).normalize(), &player));
}

/// The tower's pieces' indices in the site and their centers as built.
fn pieces(runtime: &WorldRuntime) -> Vec<(usize, Vec3)> {
    let site = town(runtime).site();
    site.specs()
        .iter()
        .enumerate()
        .filter(|(_, s)| matches!(s.role, Role::Block { .. }))
        .map(|(i, s)| (i, s.center.as_vec3()))
        .collect()
}

#[test]
fn the_tower_stands_in_the_field_past_the_dummies_and_is_carved() {
    let runtime = before_tower(15.0);
    let town = town(&runtime);
    let tower = town
        .buildings()
        .iter()
        .find(|b| b.is_carved())
        .expect("the tower is the Grove's one carved building");
    // Three blocks across and a level every 2.5 m or so, 30 m high.
    assert!(tower.pieces.len() >= 80, "{}", tower.pieces.len());
    assert!(tower.top > 28.0, "{}", tower.top);
    // Past every dummy, as seen from the spawn.
    for (_, at) in crate::zones::grove::dummies::FIELD {
        assert!(at[1] < TOWER[1] - 4.0);
    }
}

#[test]
fn aiming_at_the_towers_side_lays_the_ring_flat_against_it() {
    let mut runtime = before_tower(15.0);
    cast(&mut runtime, Spell::MeteorSwarm);
    assert!(runtime.demolition_targeting());
    let side = Vec3::new(TOWER[0] + 0.4, 9.0, TOWER[1] - 2.8);
    aim(&mut runtime, side);
    let aimed = town(&runtime).swarm().aimed().expect("the ring is down");
    assert!(aimed.wall(), "{aimed:?}");
    assert!(aimed.normal.dot(Vec3::NEG_Z) > 0.9, "{aimed:?}");
    assert!((aimed.at.z - (TOWER[1] - 2.8)).abs() < 0.4, "{aimed:?}");
    assert!((aimed.at.y - 9.0).abs() < 1.0, "{aimed:?}");
    // The ground in front of it still takes an upright ring.
    aim(&mut runtime, Vec3::new(TOWER[0], 0.0, TOWER[1] - 8.0));
    let aimed = town(&runtime).swarm().aimed().unwrap();
    assert!(!aimed.wall() && aimed.normal == Vec3::Y);
    // A wall past the spell's range drops the ring to the ground in range.
    runtime
        .set_spawn(Vec3::new(TOWER[0], 0.0, TOWER[1] - 45.0), 0.0)
        .unwrap();
    aim(&mut runtime, side);
    let aimed = town(&runtime).swarm().aimed().unwrap();
    assert!(!aimed.wall());
    let player = runtime.player.pos;
    assert!(
        Vec3::new(aimed.at.x - player.x, 0.0, aimed.at.z - player.z).length()
            <= meteor::RANGE + 0.01
    );
}

/// A cast over flat ground and the boxes `boxes`, each a min and a max
/// corner, as [`meteor::surface_aim`] takes it.
fn boxes_cast(boxes: Vec<(Vec3, Vec3)>) -> impl Fn(Vec3, Vec3) -> Option<f32> {
    use verse_world::social::sight::{box_hit, plane_hit};
    move |from, to| {
        boxes
            .iter()
            .filter_map(|&(min, max)| box_hit(from, to, 0.0, min, max))
            .chain(plane_hit(from, to, 0.0, 0.0, false))
            .min_by(f32::total_cmp)
    }
}

#[test]
fn a_ray_into_a_tall_column_finds_its_side_and_its_normal() {
    // A 5.6 m column 30 m tall at the origin, and flat ground.
    let cast = boxes_cast(vec![(
        Vec3::new(-2.8, 0.0, -2.8),
        Vec3::new(2.8, 30.0, 2.8),
    )]);
    let aim = meteor::surface_aim(Vec3::new(-20.0, 6.0, 0.5), Vec3::X, &cast).unwrap();
    assert!(aim.wall());
    assert!(aim.normal.dot(Vec3::NEG_X) > 0.95, "{aim:?}");
    assert!((aim.at.x + 2.8).abs() < 0.05 && (aim.at.y - 6.0).abs() < 1e-3);
    let down = meteor::surface_aim(
        Vec3::new(-20.0, 10.0, 0.0),
        Vec3::new(1.0, -1.0, 0.0),
        &cast,
    )
    .unwrap();
    assert!(!down.wall());
    assert!(
        (down.at.x + 10.0).abs() < 0.05 && down.at.y.abs() < 0.05,
        "{down:?}"
    );
    let (u, v) = aim.across();
    assert!(u.dot(aim.normal).abs() < 1e-4 && v.dot(aim.normal).abs() < 1e-4);
    assert!(v.y > 0.9, "up the wall");
    // The column's top is a roof: the ring lies on it.
    let roof = meteor::surface_aim(
        Vec3::new(-10.0, 40.0, 0.0),
        Vec3::new(1.0, -1.0, 0.0),
        &cast,
    )
    .unwrap();
    assert!(!roof.wall() && (roof.at.y - 30.0).abs() < 0.05, "{roof:?}");
}

#[test]
fn a_ray_through_a_hole_finds_the_far_walls_inner_face() {
    // A hollow room 6 m across with 0.3 m walls, its near (west) wall
    // blown open from the ground to 3 m, and a roof.
    let wall = 0.3;
    let cast = boxes_cast(vec![
        // The near wall over the hole, and beside it.
        (Vec3::new(-3.0, 3.0, -3.0), Vec3::new(-3.0 + wall, 5.0, 3.0)),
        (
            Vec3::new(-3.0, 0.0, -3.0),
            Vec3::new(-3.0 + wall, 3.0, -1.5),
        ),
        (Vec3::new(-3.0, 0.0, 1.5), Vec3::new(-3.0 + wall, 3.0, 3.0)),
        // The far (east) wall, the side walls, and the roof.
        (Vec3::new(3.0 - wall, 0.0, -3.0), Vec3::new(3.0, 5.0, 3.0)),
        (Vec3::new(-3.0, 0.0, -3.0), Vec3::new(3.0, 5.0, -3.0 + wall)),
        (Vec3::new(-3.0, 0.0, 3.0 - wall), Vec3::new(3.0, 5.0, 3.0)),
        (Vec3::new(-3.0, 5.0, -3.0), Vec3::new(3.0, 5.3, 3.0)),
    ]);
    let eye = Vec3::new(-14.0, 1.6, 0.2);
    let aim = meteor::surface_aim(eye, Vec3::X, &cast).unwrap();
    assert!(aim.wall(), "{aim:?}");
    assert!(
        (aim.at.x - (3.0 - wall)).abs() < 0.05,
        "the far wall's inner face: {aim:?}"
    );
    assert!(
        aim.normal.dot(Vec3::NEG_X) > 0.95,
        "facing the caster: {aim:?}"
    );
    // The intact wall above the hole still takes the ray on its outer face.
    let up =
        meteor::surface_aim(eye, (Vec3::new(-3.0, 4.0, 0.2) - eye).normalize(), &cast).unwrap();
    assert!(up.wall() && (up.at.x + 3.0).abs() < 0.05, "{up:?}");
    assert!(up.normal.dot(Vec3::NEG_X) > 0.95, "{up:?}");
    // The floor inside, seen through the hole, faces up.
    let floor =
        meteor::surface_aim(eye, (Vec3::new(0.0, 0.0, 0.2) - eye).normalize(), &cast).unwrap();
    assert!(
        !floor.wall() && floor.at.x > -3.0 && floor.at.y.abs() < 0.05,
        "{floor:?}"
    );
}

#[test]
fn a_thunderbolt_on_the_towers_side_takes_a_crater_and_spares_the_far_side() {
    let mut runtime = before_tower(14.0);
    cast(&mut runtime, Spell::Thunderbolt);
    let hit = Vec3::new(TOWER[0], 15.5, TOWER[1] - 2.8);
    aim(&mut runtime, hit);
    assert!(runtime.demolition_confirm());
    idle(&mut runtime, meteor::BOLT_CAST + 0.6);
    let site = town(&runtime).site();
    let all = pieces(&runtime);
    let broken: Vec<Vec3> = all
        .iter()
        .filter(|(i, _)| site.pieces()[*i].status == Status::Broken)
        .map(|(_, c)| *c)
        .collect();
    assert!(!broken.is_empty(), "the strike broke blocks");
    // Every broken block is near the strike, on the struck side.
    for c in &broken {
        assert!(c.distance(hit) < meteor::BOLT_BLAST + 2.5, "{c} from {hit}");
        assert!(c.z < TOWER[1] + 0.5, "{c}");
    }
    // The far side stands.
    for (i, c) in &all {
        if c.z > TOWER[1] + 1.5 {
            assert_eq!(site.pieces()[*i].status, Status::Standing, "{c}");
        }
    }
    assert_eq!(site.toppling(), 0, "one strike only chips it");
}

#[test]
fn a_meteor_swarm_on_the_towers_side_breaks_blocks_near_the_hit() {
    let mut runtime = before_tower(16.0);
    cast(&mut runtime, Spell::MeteorSwarm);
    let hit = Vec3::new(TOWER[0], 20.0, TOWER[1] - 2.8);
    aim(&mut runtime, hit);
    assert!(town(&runtime).swarm().aimed().unwrap().wall());
    assert!(runtime.demolition_confirm());
    idle(&mut runtime, meteor::CAST + 2.0);
    let site = town(&runtime).site();
    let all = pieces(&runtime);
    let near_broken = all
        .iter()
        .filter(|(i, c)| c.distance(hit) < 3.0 && site.pieces()[*i].status == Status::Broken)
        .count();
    assert!(near_broken >= 3, "{near_broken} blocks broke at the hit");
    // Nothing under the strike's levels on the far side broke.
    for (i, c) in &all {
        if c.z > TOWER[1] + 1.5 && c.y < 12.0 {
            assert_eq!(site.pieces()[*i].status, Status::Standing, "{c}");
        }
    }
}

#[test]
fn an_undercut_tower_tips_toward_the_cut_and_lies_on_the_ground_as_rubble() {
    let mut runtime = before_tower(40.0);
    // Blow the south face and the sides beside it out of the fourth level.
    let wreck = town_mut(&mut runtime);
    for x in [-1.8, 0.0, 1.8] {
        wreck.blast(
            Vec3::new(TOWER[0] + x, 8.6, TOWER[1] - 2.9),
            2.4,
            400,
            Vec3::NEG_Z,
        );
    }
    for x in [-2.9, 2.9] {
        wreck.blast(
            Vec3::new(TOWER[0] + x, 8.6, TOWER[1] - 0.6),
            1.6,
            400,
            Vec3::X * x.signum(),
        );
    }
    let site = wreck.site();
    assert_eq!(site.toppling(), 1, "the top topples as one body");
    let tops: Vec<usize> = site
        .pieces()
        .iter()
        .enumerate()
        .filter(|(_, p)| p.local.is_some())
        .map(|(i, _)| i)
        .collect();
    assert!(tops.len() > 40, "{} blocks in the top", tops.len());
    // A moment later it leans south, toward the cut, turning over its
    // hinge rather than dropping straight down.
    idle(&mut runtime, 1.2);
    let site = town_mut(&mut runtime).site();
    let lean = |site: &crate::zones::everglade::demolition::site::Site| {
        let (mut sum, mut n) = (Vec3::ZERO, 0.0);
        for &i in &tops {
            sum += site.piece_pose(i).w_axis.truncate();
            n += 1.0;
        }
        sum / n
    };
    let leaning = lean(site);
    assert!(leaning.z < TOWER[1] - 1.0, "it leans south: {leaning}");
    // Then it strikes the ground and breaks up.
    let mut crashed = false;
    let mut slowest = 0.0_f64;
    for _ in 0..(10.0 / DT) as usize {
        let start = std::time::Instant::now();
        runtime.tick(&InputState::default(), DT);
        slowest = slowest.max(start.elapsed().as_secs_f64());
        if town(&runtime).site().toppling() == 0 {
            crashed = true;
        }
    }
    eprintln!("slowest topple frame {:.1} ms (debug build)", slowest * 1e3);
    assert!(crashed, "the top reached the ground");
    let site = town(&runtime).site();
    let mut rubble = Vec::new();
    for &i in &tops {
        let piece = &site.pieces()[i];
        match piece.status {
            Status::Loose => rubble.push(site.piece_pose(i).w_axis.truncate()),
            Status::Broken => {
                for (k, _) in piece.chunks.iter().enumerate() {
                    if let Some(m) = site.chunk_pose(i, k) {
                        rubble.push(m.w_axis.truncate());
                    }
                }
            }
            Status::Standing => panic!("a block of the top still stands"),
        }
    }
    assert!(!rubble.is_empty());
    let mean = rubble.iter().copied().sum::<Vec3>() / rubble.len() as f32;
    // On the ground, out toward the cut, the length of the fallen top.
    assert!(mean.y < 4.0, "the rubble lies low: {mean}");
    assert!(mean.z < TOWER[1] - 5.0, "it fell south: {mean}");
    // Caps hold through the fall.
    let chunks = site
        .pieces()
        .iter()
        .flat_map(|p| &p.chunks)
        .filter(|c| !c.gone)
        .count();
    assert!(chunks <= MAX_CHUNKS, "{chunks}");
    assert!(site.pieces().len() <= MAX_PIECES);
    assert!(site.puffs().len() <= 400);
    // R stands it back up whole.
    runtime.zone_intent(Intent::Rebuild).unwrap();
    let wreck = town(&runtime);
    assert!(wreck.site().pieces().is_empty(), "every piece is let go");
    assert_eq!(wreck.hidden(), 0, "the tower draws whole again");
}

#[test]
fn a_long_rest_restores_the_tower_and_a_thunderbolt_hurts_dummies() {
    let mut runtime = WorldRuntime::new();
    runtime.install_grove(pack());
    let straw = grove(&runtime).dummies[0].pos;
    runtime.set_spawn(straw - Vec3::Z * 10.0, 0.0).unwrap();
    let before = grove(&runtime).dummies[0].hp;
    cast(&mut runtime, Spell::Thunderbolt);
    aim(&mut runtime, straw);
    assert!(runtime.demolition_confirm());
    idle(&mut runtime, meteor::BOLT_CAST + 0.5);
    assert!(
        grove(&runtime).dummies[0].hp < before,
        "the bolt hurt the dummy"
    );
    // Struck down to a stump, then rested.
    let wreck = town_mut(&mut runtime);
    wreck.blast(
        Vec3::new(TOWER[0], 3.0, TOWER[1] - 2.9),
        3.0,
        400,
        Vec3::NEG_Z,
    );
    assert!(!wreck.site().pieces().is_empty());
    runtime.zone_intent(Intent::LongRest).unwrap();
    assert!(town_mut(&mut runtime).site().pieces().is_empty());
    // Pressing the key again while aiming leaves the aim; the other
    // strike's key switches to it.
    cast(&mut runtime, Spell::MeteorSwarm);
    cast(&mut runtime, Spell::Thunderbolt);
    let swarm = town(&runtime).swarm();
    assert!(swarm.targeting() && swarm.strike() == Strike::Lightning);
    cast(&mut runtime, Spell::Thunderbolt);
    assert!(!town(&runtime).swarm().targeting());
}

#[test]
fn lightning_bolt_chips_the_tower_where_its_line_meets_it() {
    let mut runtime = before_tower(12.0);
    // Arid, Polar, then Temperate, whose fourth spell is Lightning Bolt.
    for _ in 0..2 {
        cast(&mut runtime, Spell::ChooseLand);
    }
    let player: PlayerController = runtime.player.clone();
    assert!(player.forward().z > 0.99);
    cast(&mut runtime, Spell::LightningBolt);
    idle(&mut runtime, 0.1);
    let site = town(&runtime).site();
    assert!(
        site.specs()
            .iter()
            .zip(site.pieces())
            .any(|(s, p)| p.hit_points < s.hit_points),
        "the bolt chipped the tower"
    );
}

#[test]
fn the_dragons_fire_breath_breaks_the_tower() {
    // The dragon stands before the tower's south face, close enough for its
    // cone to reach.
    let mut runtime = before_tower(9.0);
    cast(&mut runtime, Spell::Shapechange);
    idle(
        &mut runtime,
        crate::zones::grove::dragon::SWAP + crate::zones::grove::dragon::GROW + 0.1,
    );
    // The dragon's attacks take the beast row while it holds the shape.
    let breath = slots::slot_of(
        Spell::FireBreath,
        grove(&runtime).form(),
        grove(&runtime).land(),
    )
    .expect("the dragon's row holds Fire Breath");
    for _ in 0..3 {
        runtime
            .zone_intent(Intent::GroveSlot(breath as u8))
            .expect("the breath");
        idle(&mut runtime, 1.2);
    }
    assert_eq!(
        grove(&runtime).form(),
        Some(crate::zones::grove::shape::Form::Dragon)
    );
    let site = town(&runtime).site();
    let broken = pieces(&runtime)
        .iter()
        .filter(|(i, _)| site.pieces()[*i].status == Status::Broken)
        .count();
    assert!(broken > 0, "the breath broke the tower's blocks");
}

#[test]
fn through_a_hole_in_the_tower_the_aim_finds_the_far_side_inside() {
    let mut runtime = before_tower(30.0);
    // Raise the tower with a blast of no damage, then break the middle
    // column's south and center blocks out of one level.
    town_mut(&mut runtime).blast(
        Vec3::new(TOWER[0], 10.0, TOWER[1] - 2.9),
        2.0,
        0,
        Vec3::NEG_Z,
    );
    let all = pieces(&runtime);
    let level = all
        .iter()
        .map(|(_, c)| c.y)
        .min_by(|a, b| (a - 10.5).abs().total_cmp(&(b - 10.5).abs()))
        .unwrap();
    let column = all
        .iter()
        .map(|(_, c)| c.x)
        .min_by(|a, b| (a - TOWER[0]).abs().total_cmp(&(b - TOWER[0]).abs()))
        .unwrap();
    let cell: Vec<(usize, Vec3)> = all
        .iter()
        .copied()
        .filter(|(_, c)| (c.y - level).abs() < 0.3 && (c.x - column).abs() < 0.3)
        .collect();
    let far_z = cell.iter().map(|(_, c)| c.z).fold(f32::MIN, f32::max);
    let wreck = town_mut(&mut runtime);
    for (i, c) in &cell {
        if c.z < far_z - 0.3 {
            wreck
                .site_mut()
                .damage(*i, 100_000, c.as_dvec3(), glam::DVec3::Z);
        }
    }
    idle(&mut runtime, 0.5);
    cast(&mut runtime, Spell::MeteorSwarm);
    let target = Vec3::new(column, level + 0.5, far_z);
    aim(&mut runtime, target);
    let aimed = town(&runtime).swarm().aimed().expect("the ring is down");
    assert!(aimed.wall(), "{aimed:?}");
    // Past the tower's middle and short of its north face, 2.8 m out.
    assert!(
        aimed.at.z > TOWER[1] + 0.3 && aimed.at.z < TOWER[1] + 2.75,
        "on the far blocks' inner face: {aimed:?}"
    );
    assert!(aimed.normal.z < -0.9, "facing the caster: {aimed:?}");
    // The strike reaches it through the hole.
    assert!(runtime.demolition_confirm());
    idle(&mut runtime, meteor::CAST + 2.0);
    let site = town(&runtime).site();
    let hurt = pieces(&runtime)
        .iter()
        .filter(|(i, c)| {
            c.z > TOWER[1] + 1.0
                && c.distance(aimed.at) < 2.0
                && site.pieces()[*i].hit_points < site.specs()[*i].hit_points
        })
        .count();
    assert!(hurt >= 1, "the far side took the blast");
}

/// World boxes, min and max corners, of everything in `site` that is
/// standing, loose, or a live chunk, each with a label.
fn solid_boxes(
    site: &crate::zones::everglade::demolition::site::Site,
) -> Vec<(String, Vec3, Vec3)> {
    let mut out = Vec::new();
    let corners = |frame: glam::Mat4, half: Vec3| {
        let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
        for i in 0..8 {
            let c = Vec3::new(
                if i & 1 == 0 { -half.x } else { half.x },
                if i & 2 == 0 { -half.y } else { half.y },
                if i & 4 == 0 { -half.z } else { half.z },
            );
            let p = frame.transform_point3(c);
            lo = lo.min(p);
            hi = hi.max(p);
        }
        (lo, hi)
    };
    for (i, (spec, piece)) in site.specs().iter().zip(site.pieces()).enumerate() {
        match piece.status {
            Status::Standing | Status::Loose => {
                let pose = site.piece_pose(i);
                let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
                for c in &spec.colliders {
                    let (a, b) = corners(pose * c.frame(), c.half.as_vec3());
                    lo = lo.min(a);
                    hi = hi.max(b);
                }
                let label = format!(
                    "piece {i} {:?} {:?} rubble={} toppling={}",
                    spec.role,
                    piece.status,
                    piece.rubble,
                    piece.local.is_some()
                );
                out.push((label, lo, hi));
            }
            Status::Broken => {
                for (k, chunk) in spec.chunks.iter().enumerate() {
                    if let Some(pose) = site.chunk_pose(i, k) {
                        let (lo, hi) = corners(pose * chunk.frame(), chunk.half.as_vec3() * 0.94);
                        out.push((format!("chunk {i}.{k}"), lo, hi));
                    }
                }
            }
        }
    }
    out
}

/// Everything in `site` whose underside is more than 3 m up and that
/// rests on no chain of boxes down to the ground: what hangs in the sky.
/// A box rests on another that reaches up to its underside and overlaps
/// it across and whose top is near its underside, give or take half a
/// metre for tilted boxes: a wall beside a block does not hold it up.
fn floating(site: &crate::zones::everglade::demolition::site::Site) -> Vec<String> {
    let boxes = solid_boxes(site);
    let mut grounded: Vec<bool> = boxes.iter().map(|(_, lo, _)| lo.y < 1.0).collect();
    let mut changed = true;
    while changed {
        changed = false;
        for b in 0..boxes.len() {
            if grounded[b] {
                continue;
            }
            let (_, blo, bhi) = &boxes[b];
            let held = (0..boxes.len()).any(|a| {
                let (_, alo, ahi) = &boxes[a];
                grounded[a]
                    && a != b
                    && alo.x < bhi.x + 0.5
                    && blo.x < ahi.x + 0.5
                    && alo.z < bhi.z + 0.5
                    && blo.z < ahi.z + 0.5
                    && ahi.y >= blo.y - 0.5
                    && ahi.y <= blo.y + 1.2
                    && alo.y < blo.y + 0.2
            });
            if held {
                grounded[b] = true;
                changed = true;
            }
        }
    }
    boxes
        .iter()
        .zip(&grounded)
        .filter(|((_, lo, _), g)| !**g && lo.y > 3.0)
        .map(|((label, lo, hi), _)| format!("{label} from {lo} to {hi}"))
        .collect()
}

/// Lets the wreck settle, then fails if anything hangs in the air. It
/// looks before most chunks end, which would hide what hangs.
fn settles_with_nothing_in_the_air(runtime: &mut WorldRuntime, what: &str) {
    idle(runtime, 6.0);
    let site = town(runtime).site();
    let high = floating(site);
    assert!(
        high.is_empty(),
        "{what}: {} things hang in the air:\n{}",
        high.len(),
        high.join("\n")
    );
}

/// How many of the tower's blocks no longer stand.
fn fallen(runtime: &WorldRuntime) -> usize {
    let site = town(runtime).site();
    pieces(runtime)
        .iter()
        .filter(|(i, _)| site.pieces()[*i].status != Status::Standing)
        .count()
}

/// Blows the south face and the sides beside it out of the level `y` m
/// up, as the undercut test does, so the top topples south over a hinge.
fn undercut(runtime: &mut WorldRuntime, y: f32) {
    let wreck = town_mut(runtime);
    for x in [-1.8, 0.0, 1.8] {
        wreck.blast(
            Vec3::new(TOWER[0] + x, y, TOWER[1] - 2.9),
            2.4,
            400,
            Vec3::NEG_Z,
        );
    }
    for x in [-2.9, 2.9] {
        wreck.blast(
            Vec3::new(TOWER[0] + x, y, TOWER[1] - 0.6),
            1.6,
            400,
            Vec3::X * x.signum(),
        );
    }
}

#[test]
fn a_stump_blown_out_from_under_settled_debris_leaves_nothing_in_the_air() {
    // The owner's report: the top topples and its debris comes to rest
    // and freezes on the stump, then the stump's lower part goes. Debris
    // that froze on the stump, or on other frozen debris, more than a
    // few metres above the blasts used to stay frozen in the sky.
    let mut runtime = before_tower(40.0);
    undercut(&mut runtime, 8.6);
    idle(&mut runtime, 10.0);
    assert_eq!(town(&runtime).site().toppling(), 0, "the top came down");
    for y in [0.5f32, 2.0, 3.5, 5.0] {
        let wreck = town_mut(&mut runtime);
        for x in [-1.8, 0.0, 1.8] {
            wreck.blast(
                Vec3::new(TOWER[0] + x, y, TOWER[1] + 2.9),
                2.4,
                400,
                Vec3::Z,
            );
            wreck.blast(
                Vec3::new(TOWER[0] + x, y, TOWER[1] - 2.9),
                2.4,
                400,
                Vec3::NEG_Z,
            );
        }
        idle(&mut runtime, 1.0);
    }
    settles_with_nothing_in_the_air(&mut runtime, "the stump's foot blown out");
}

/// In the dragon's shape `back` m south of the tower, facing it, with its
/// Fire Breath slot.
fn dragon_before_tower(back: f32) -> (WorldRuntime, u8) {
    let mut runtime = before_tower(back);
    cast(&mut runtime, Spell::Shapechange);
    idle(
        &mut runtime,
        crate::zones::grove::dragon::SWAP + crate::zones::grove::dragon::GROW + 0.1,
    );
    let breath = slots::slot_of(
        Spell::FireBreath,
        grove(&runtime).form(),
        grove(&runtime).land(),
    )
    .expect("the dragon's row holds Fire Breath");
    (runtime, breath as u8)
}

#[test]
fn fire_breath_on_the_towers_foot_leaves_nothing_in_the_air() {
    for back in [9.0, 5.0] {
        let (mut runtime, breath) = dragon_before_tower(back);
        // Debris from an earlier undercut lies on the stump when the
        // breath takes its foot.
        undercut(&mut runtime, 8.6);
        idle(&mut runtime, 10.0);
        for _ in 0..10 {
            runtime
                .zone_intent(Intent::GroveSlot(breath))
                .expect("the breath");
            idle(&mut runtime, 1.5);
        }
        assert!(fallen(&runtime) > 40, "the breath took the stump down");
        settles_with_nothing_in_the_air(&mut runtime, &format!("Fire Breath from {back} m"));
    }
}

/// Casts `spell`, Meteor Swarm or the Thunderbolt, at the tower's south
/// face `y` m up, and waits `wait` seconds.
fn strike_the_face(runtime: &mut WorldRuntime, spell: Spell, y: f32, wait: f32) {
    cast(runtime, spell);
    aim(runtime, Vec3::new(TOWER[0], y, TOWER[1] - 2.8));
    assert!(runtime.demolition_confirm());
    idle(runtime, wait);
}

#[test]
fn meteor_swarms_on_the_towers_foot_leave_nothing_in_the_air() {
    let mut runtime = before_tower(16.0);
    // Topple the top, let its debris settle, then cut the stump's foot.
    for y in [8.6, 7.4, 8.6, 6.2] {
        strike_the_face(&mut runtime, Spell::MeteorSwarm, y, meteor::CAST + 2.5);
    }
    idle(&mut runtime, 8.0);
    for y in [2.0, 3.5, 1.5, 4.5] {
        strike_the_face(&mut runtime, Spell::MeteorSwarm, y, meteor::CAST + 2.5);
    }
    assert!(fallen(&runtime) > 40, "the meteors brought the tower down");
    settles_with_nothing_in_the_air(&mut runtime, "Meteor Swarm");
}

#[test]
fn thunderbolts_on_the_towers_foot_leave_nothing_in_the_air() {
    let mut runtime = before_tower(14.0);
    for y in [8.6, 7.4, 8.6, 6.2, 8.0] {
        strike_the_face(&mut runtime, Spell::Thunderbolt, y, meteor::BOLT_CAST + 2.5);
    }
    idle(&mut runtime, 8.0);
    for y in [2.0, 3.5, 1.5, 4.5, 2.5, 1.0] {
        strike_the_face(&mut runtime, Spell::Thunderbolt, y, meteor::BOLT_CAST + 2.5);
    }
    assert!(fallen(&runtime) > 40, "the bolts brought the tower down");
    settles_with_nothing_in_the_air(&mut runtime, "the Thunderbolt");
}

/// The tower's south face, z.
const SOUTH_FACE: f32 = TOWER[1] - 2.8;

/// Walks the player straight ahead for `seconds`.
fn walk(runtime: &mut WorldRuntime, seconds: f32) {
    let ahead = InputState {
        forward: true,
        ..InputState::default()
    };
    for _ in 0..(seconds / DT).round() as usize {
        runtime.tick(&ahead, DT);
    }
}

#[test]
fn the_dragon_pressed_against_the_tower_keeps_its_head_out_of_the_wall() {
    let (mut runtime, _) = dragon_before_tower(12.0);
    walk(&mut runtime, 4.0);
    let feet = runtime.player.pos;
    // Its jaws are 5 m ahead of its feet and its snout a little short of
    // them: walking into the wall stops it with its head before the face.
    assert!(
        feet.z <= SOUTH_FACE - 4.4,
        "the dragon's feet at {feet} put its head into the wall at z = {SOUTH_FACE}"
    );
    assert!(
        feet.z > SOUTH_FACE - 6.5,
        "it walked up to the wall: {feet}"
    );
    // Flying into it at the jaws' height stops it the same way.
    let glade = runtime.zone_state.everglade.as_ref().unwrap();
    let solids = glade.solids();
    use verse_world::social::sight::Sight;
    let from = feet + Vec3::Y * 3.1;
    assert_eq!(solids.sweep(from, from + Vec3::Z * 4.4, 0.3), 1.0);
}

#[test]
fn point_blank_fire_breath_breaks_the_near_wall_not_the_far_one() {
    let (mut runtime, breath) = dragon_before_tower(12.0);
    walk(&mut runtime, 4.0);
    runtime
        .zone_intent(Intent::GroveSlot(breath))
        .expect("the breath");
    // Looks the frame the fire lands, before what it breaks falls on the
    // rest.
    for _ in 0..(2.0 / DT) as usize {
        runtime.tick(&InputState::default(), DT);
        let site = town(&runtime).site();
        if site
            .specs()
            .iter()
            .zip(site.pieces())
            .any(|(s, p)| p.hit_points < s.hit_points)
        {
            break;
        }
    }
    let site = town(&runtime).site();
    let hurt: Vec<Vec3> = pieces(&runtime)
        .into_iter()
        .filter(|(i, _)| site.pieces()[*i].hit_points < site.specs()[*i].hit_points)
        .map(|(_, c)| c)
        .collect();
    assert!(
        hurt.iter().any(|c| c.z < TOWER[1] - 1.0),
        "the near wall took the fire: {hurt:?}"
    );
    assert!(
        hurt.iter().all(|c| c.z < TOWER[1] + 1.0),
        "the far wall was spared: {hurt:?}"
    );
}
