//! The Grove's concrete tower against the pinned pack: aiming Meteor
//! Swarm and the Thunderbolt at its side, the crater a strike leaves, the
//! top toppling over an undercut, R restoring it, and the caps holding.

use super::kit::Spell;
use super::layout::TOWER;
use super::{Grove, slots};
use crate::controller::{InputState, PlayerController};
use crate::runtime::WorldRuntime;
use crate::zones::everglade::demolition::meteor::{self, Strike};
use crate::zones::everglade::demolition::site::{Role, Status};
use crate::zones::everglade::demolition::town::{MAX_CHUNKS, MAX_PIECES, Town};
use crate::zones::everglade_pack::{self, ZonePack};
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
    for (_, at) in super::dummies::FIELD {
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

#[test]
fn a_ray_into_a_tall_column_finds_its_side_and_its_normal() {
    // A 5.6 m column 30 m tall at the origin, and flat ground.
    let surface = |x: f32, z: f32| {
        if x.abs() < 2.8 && z.abs() < 2.8 {
            30.0
        } else {
            0.0
        }
    };
    let aim = meteor::surface_aim(Vec3::new(-20.0, 6.0, 0.5), Vec3::X, &surface).unwrap();
    assert!(aim.wall());
    assert!(aim.normal.dot(Vec3::NEG_X) > 0.95, "{aim:?}");
    assert!((aim.at.x + 2.8).abs() < 0.05 && (aim.at.y - 6.0).abs() < 1e-3);
    let down = meteor::surface_aim(
        Vec3::new(-20.0, 10.0, 0.0),
        Vec3::new(1.0, -1.0, 0.0),
        &surface,
    )
    .unwrap();
    assert!(!down.wall());
    let (u, v) = aim.across();
    assert!(u.dot(aim.normal).abs() < 1e-4 && v.dot(aim.normal).abs() < 1e-4);
    assert!(v.y > 0.9, "up the wall");
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
        super::dragon::SWAP + super::dragon::GROW + 0.1,
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
    assert_eq!(grove(&runtime).form(), Some(super::shape::Form::Dragon));
    let site = town(&runtime).site();
    let broken = pieces(&runtime)
        .iter()
        .filter(|(i, _)| site.pieces()[*i].status == Status::Broken)
        .count();
    assert!(broken > 0, "the breath broke the tower's blocks");
}
