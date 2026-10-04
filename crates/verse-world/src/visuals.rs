//! Read-only combat values shared by local and remote rendering adapters.
use crate::{play::Game, rules::Projectile, utilities::Area};
use glam::Vec3;

#[derive(Clone, Debug)]
pub struct Player {
    pub position: Vec3,
    pub shield: i32,
    pub shield_until: f32,
    pub light: Option<Vec3>,
    pub areas: Vec<Area>,
}
#[derive(Clone, Debug)]
pub struct Hostile {
    pub origin: Vec3,
    pub target: Vec3,
    pub position: Option<Vec3>,
    pub started: f32,
    pub release: f32,
    pub radius: f32,
    pub boss: bool,
}
/// Contains no command, damage execution, or authority handle.
#[derive(Clone, Debug)]
pub struct Combat {
    pub time: f32,
    pub projectiles: Vec<Projectile>,
    pub players: Vec<Player>,
    pub hostile: Vec<Hostile>,
    pub impacts: Vec<(Vec3, f32, u8)>,
}
impl Combat {
    pub fn extract(game: &Game) -> Self {
        Self {
            time: game.time,
            projectiles: game.snapshot().projectiles,
            players: game
                .controlled_effects()
                .map(|(_, position, c)| Player {
                    position,
                    shield: c.shield,
                    shield_until: c.shield_until,
                    light: c.light,
                    areas: c.areas.clone(),
                })
                .collect(),
            hostile: game
                .encounter
                .iter()
                .flat_map(|e| &e.casts)
                .map(|c| Hostile {
                    origin: c.origin,
                    target: c.target,
                    position: c.position,
                    started: c.started,
                    release: c.release,
                    radius: c.radius,
                    boss: c.boss,
                })
                .collect(),
            impacts: game.impacts.clone(),
        }
    }
}

/// Collision-box pose of an admitted live prop; contains no physics authority.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prop {
    pub life: physics::queries::Life,
    pub kind: crate::spells::PropKind,
    pub secured: bool,
    pub center: Vec3,
    pub rotation: glam::Quat,
    pub dimensions: Vec3,
}
pub fn prop_poses(game: &Game, alpha: f32) -> Vec<Prop> {
    game.spells
        .props
        .iter()
        .enumerate()
        .filter(|(_, p)| !p.removed)
        .map(|(i, p)| {
            let (center, rotation) = game.spells.prop_pose(i, alpha as f64);
            Prop {
                life: p.life,
                kind: p.spec.kind,
                secured: p.spec.secured,
                center: center.as_vec3(),
                rotation: rotation.as_quat(),
                dimensions: p.spec.dimensions.as_vec3(),
            }
        })
        .collect()
}
pub fn validate_props(props: &[Prop], instance: u64) -> Result<(), String> {
    let mut entities = std::collections::BTreeSet::new();
    if props.len() > crate::spells::MAX_PROPS
        || props.iter().any(|p| {
            p.life.instance != instance
                || p.life.entity < crate::spells::PROP_ENTITY_BASE
                || !entities.insert(p.life.entity)
                || !p.center.is_finite()
                || p.center.abs().max_element() > 1_000_000.
                || !p.rotation.is_finite()
                || !p.rotation.is_normalized()
                || !p.dimensions.is_finite()
                || p.dimensions.min_element() <= 0.
                || p.dimensions.max_element() > 1000.
        })
    {
        return Err("Invalid replicated prop poses or budget".into());
    }
    Ok(())
}

#[cfg(test)]
mod prop_tests {
    use super::*;
    #[test]
    fn extracted_props_follow_physics_box_poses_and_omit_removed_bodies() {
        let scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let mut game = Game::new_in(scene, 160).unwrap();
        let index = game
            .spawn_prop(
                "Crate",
                crate::spells::PropSpec::reference(crate::spells::PropKind::Crate),
                Vec3::new(0., 1., -8.),
                0.,
            )
            .unwrap();
        game.spells.begin_tick();
        let body = game.spells.props[index].body;
        game.spells.world[body].pos.x += 2.;
        let poses = prop_poses(&game, 0.5);
        validate_props(&poses, 160).unwrap();
        assert_eq!(poses.len(), 1);
        assert!((poses[0].center.x - 1.).abs() < 0.0001);
        assert_eq!(poses[0].dimensions, Vec3::splat(0.6));
        let saved = game.checkpoint().unwrap();
        let restored = Game::restore(&saved).unwrap();
        assert_eq!(
            serde_json::to_vec(&prop_poses(&game, 1.)).unwrap(),
            serde_json::to_vec(&prop_poses(&restored, 1.)).unwrap()
        );
        game.spells.remove_prop(index).unwrap();
        assert!(prop_poses(&game, 1.).is_empty());
    }
}
