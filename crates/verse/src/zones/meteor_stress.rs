//! A standalone castle under continuous Meteor Swarm bombardment.

use super::{Everglade, everglade, everglade_pack::ZonePack, grove};
use crate::{controller::PlayerController, world::World};
use everglade::layout::{Collision, Placement};
use everglade::studio::{Posture, SeatFigure};
use glam::Vec3;
use std::sync::Arc;

pub const SPAWN: Vec3 = Vec3::new(0.0, 0.0, -84.0);
pub const RETURN_PORTAL: Vec3 = Vec3::new(0.0, 0.0, -102.0);

/// The Grove's concrete tower forms the keep, turrets, and curtain walls.
/// Small gaps between sections keep each section independently destructible.
pub fn placements() -> Vec<Placement> {
    let mut castle = Vec::new();
    let mut tower = |x, z, scale| {
        castle.push(
            Placement::new(
                grove::layout::TOWER_MODEL,
                [x, z],
                std::f32::consts::PI,
                Collision::None,
            )
            .scale(scale),
        );
    };
    for x in [-32.0, 32.0] {
        for z in [-28.0, 28.0] {
            tower(x, z, 1.6);
        }
        for i in -6..=6 {
            tower(x, i as f32 * 3.4, 0.5);
        }
    }
    for i in -7..=7 {
        let x = i as f32 * 3.4;
        tower(x, 28.0, 0.5);
        if x.abs() > 14.0 {
            tower(x, -28.0, 0.5);
        }
    }
    for x in [-9.0, 9.0] {
        tower(x, -28.0, 1.25);
    }
    tower(0.0, 8.0, 1.65);
    for x in [-13.0, 13.0] {
        for z in [-5.0, 21.0] {
            tower(x, z, 1.3);
        }
    }
    castle
}

/// The five stationary casters around the castle perimeter.
pub fn figures() -> Vec<SeatFigure> {
    let positions = [
        [-24.0, -43.0],
        [24.0, -43.0],
        [-46.0, 0.0],
        [46.0, 0.0],
        [0.0, 44.0],
    ];
    positions
        .into_iter()
        .enumerate()
        .map(|(i, [x, z])| SeatFigure {
            name: format!("Meteor caster {}", i + 1),
            pos: Vec3::new(x, everglade::height(x, z), z),
            yaw: (-x).atan2(-z),
            speed: 0.0,
            posture: Posture::Talk,
            look: Some(Vec3::new(0.0, 12.0, 0.0)),
            tint: [
                [0.8, 0.18, 0.08],
                [0.65, 0.2, 0.8],
                [0.15, 0.55, 0.85],
                [0.8, 0.6, 0.1],
                [0.2, 0.7, 0.35],
            ][i],
            form: None,
        })
        .collect()
}

/// Builds the castle, characters, and shared destruction simulation.
///
/// # Errors
/// Returns a message if a required model or destruction scene cannot load.
pub fn build(pack: &ZonePack, player: &PlayerController) -> Result<(World, Everglade), String> {
    let placements = placements();
    let (mut scene, blockers) = everglade::scene::build(pack, &placements)?;
    everglade::draw::ground_with(&mut scene, false);
    scene.validate()?;
    let scene = Arc::new(scene);
    let mut world = World::default();
    world.mesh.textured = Some(scene.clone());
    world.blockers = blockers;
    let solids = everglade::solids::build_with(pack, &placements, &[])?;
    let mut glade = Everglade::with_solids(pack, player, solids)?;
    glade.set_free_casting();
    glade.set_look(grove::light::stage);
    glade.start_wreckage(pack, &placements, scene)?;
    let casters = figures();
    glade
        .town_mut()
        .ok_or("The castle has no destructible buildings")?
        .start_bombardment(std::array::from_fn(|i| casters[i].pos));
    Ok((world, glade))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn castle_reuses_grove_tower_and_has_five_separate_casters() {
        let castle = placements();
        assert!(castle.len() > 50);
        assert!(castle.iter().all(|p| p.model == grove::layout::TOWER_MODEL));
        assert!(castle.iter().any(|p| p.scale == 1.65));
        let casters = figures();
        assert_eq!(casters.len(), 5);
        assert!(
            casters
                .iter()
                .all(|caster| caster.pos.distance(SPAWN) > 10.0)
        );
        assert_eq!(
            super::super::ZoneId::from_name("meteor stress test"),
            Some(super::super::ZoneId::MeteorStressTest)
        );
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::{
        controller::InputState,
        runtime::WorldRuntime,
        zones::{Intent, ZoneId},
    };
    use everglade::demolition::{site::Status, town::MAX_PIECES};
    use std::path::Path;

    #[test]
    fn autonomous_swarm_breaks_castle_without_owning_player_targeting() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(super::super::everglade_pack::PACK_DIRECTORY)
            .join(format!(
                "{}.{}",
                super::super::everglade_pack::PACK_SHA256,
                super::super::everglade_pack::PACK_EXTENSION
            ));
        let pack = ZonePack::load_local(&path).unwrap();
        let mut runtime = WorldRuntime::new();
        runtime.install_meteor_stress_test(&pack);
        assert_eq!(runtime.zone, ZoneId::MeteorStressTest);
        let order = runtime.everglade_hotbar_order();
        assert_eq!(everglade::hotbar::SLOTS[order[0]].0, Intent::MeteorSwarm);
        assert_eq!(everglade::hotbar::SLOTS[order[5]].0, Intent::Levitate);
        let town = runtime
            .zone_state
            .everglade
            .as_ref()
            .unwrap()
            .town()
            .unwrap();
        assert_eq!(town.bombardment(), [5, 0]);
        assert!(
            town.buildings()
                .iter()
                .all(|building| building.destructible() && building.pieces.len() <= MAX_PIECES),
            "castle sections must fit the destruction budget: {:?}",
            town.buildings()
                .iter()
                .map(|b| b.pieces.len())
                .collect::<Vec<_>>()
        );
        // Four large towers cannot share the piece budget. Each immediate
        // hit must still break blocks while earlier hits are fresh.
        let town = runtime
            .zone_state
            .everglade
            .as_mut()
            .unwrap()
            .town_mut()
            .unwrap();
        let targets: Vec<_> = town
            .buildings()
            .iter()
            .take(4)
            .map(|building| {
                glam::Vec3::new(building.rect.0[0], building.base + 4.0, building.rect.0[1])
            })
            .collect();
        for at in targets {
            let blows = town.blast(at, 8.0, 1000, glam::Vec3::ZERO);
            assert!(
                blows.iter().any(|blow| blow.broke),
                "each fresh tower impact must produce debris"
            );
            assert!(town.site().pieces().len() <= MAX_PIECES);
        }
        runtime.zone_intent(Intent::Rebuild).unwrap();
        runtime.zone_intent(Intent::MeteorSwarm).unwrap();
        assert!(runtime.demolition_targeting());
        let mut flying = false;
        let mut damaged = false;
        for _ in 0..600 {
            runtime.tick(&InputState::default(), 1.0 / 60.0);
            let town = runtime
                .zone_state
                .everglade
                .as_ref()
                .unwrap()
                .town()
                .unwrap();
            flying |= town.bombardment()[1] > 0;
            damaged |= town
                .site()
                .pieces()
                .iter()
                .any(|piece| piece.status != Status::Standing);
            assert!(town.site().pieces().len() <= MAX_PIECES);
            assert!(town.shake().abs().max_element() <= 0.024001);
            let eye = runtime.view(1.6).eye;
            assert!(eye.y >= everglade::height(eye.x, eye.z) + 0.25);
        }
        assert!(flying, "NPCs must launch real meteors");
        assert!(damaged, "NPC meteors must break castle blocks");
        assert!(
            runtime.demolition_targeting(),
            "NPC casts must preserve the player's targeting"
        );
        let town = runtime
            .zone_state
            .everglade
            .as_ref()
            .unwrap()
            .town()
            .unwrap();
        assert!(
            !town.raised().is_empty(),
            "NPC meteors must reach castle sections"
        );
        assert!(runtime.demolition_confirm());
        runtime.zone_intent(Intent::Rebuild).unwrap();
        assert!(!runtime.demolition_targeting());
        assert_eq!(
            runtime
                .zone_state
                .everglade
                .as_ref()
                .unwrap()
                .town()
                .unwrap()
                .bombardment(),
            [5, 0]
        );
        runtime.zone_intent(Intent::Return).unwrap();
        assert_eq!(runtime.zone, ZoneId::Plaza);
        assert!(runtime.zone_state.everglade.is_none());
    }
}
