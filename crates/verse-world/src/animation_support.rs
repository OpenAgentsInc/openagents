//! Read-only animation contacts over the authority or prediction's admitted scene.
use glam::{DVec3, Vec3};
use physics::queries::{Filter, Life, Scene};
use verse_engine::{
    core::LifeId,
    locomotion::{Ground, Support},
};

pub struct Queries<'a> {
    pub(crate) scene: &'a Scene,
}
impl<'a> Queries<'a> {
    pub fn new(scene: &'a Scene) -> Self {
        Self { scene }
    }
}
impl Support for Queries<'_> {
    fn sample(&self, life: LifeId, position: Vec3, reach: f32) -> Result<Option<Ground>, String> {
        if !position.is_finite() || !reach.is_finite() || !(0.01..=0.75).contains(&reach) {
            return Err("Invalid animation contact request".into());
        }
        let mut filter = Filter::blocking(life.instance);
        filter.ignore = Some(Life {
            instance: life.instance,
            entity: life.actor,
            generation: life.generation,
        });
        let result = self.scene.ray(
            position.as_dvec3() + DVec3::Y * f64::from(reach),
            DVec3::NEG_Y,
            f64::from(reach * 2.),
            filter,
        )?;
        // Uncertain support leaves the foot under ordinary animation, not on an invented surface.
        if result.truncated {
            return Ok(None);
        }
        Ok(result
            .hits
            .into_iter()
            .find(|hit| hit.triangle != usize::MAX && hit.surface_normal.y >= 0.5)
            .map(|hit| Ground {
                position: hit.position.as_vec3(),
                normal: hit.surface_normal.as_vec3().normalize(),
            }))
    }
}
impl crate::play::Game {
    pub fn animation_support(&self) -> Queries<'_> {
        Queries {
            scene: &self.query_scene,
        }
    }
}
impl crate::prediction::Local {
    pub fn animation_support(&self) -> Queries<'_> {
        Queries {
            scene: self.animation_scene(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contacts_are_scoped_and_do_not_advance_authority_or_prediction() {
        let scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let game = crate::play::Game::combat_in(scene, false, 77).unwrap();
        let before = game.checkpoint().unwrap();
        let life = game.player_life();
        let sample = game
            .animation_support()
            .sample(life, Vec3::new(0., 0.1, -24.), 0.45)
            .unwrap();
        assert!(sample.is_some());
        assert_eq!(game.checkpoint().unwrap(), before);
        assert!(
            game.animation_support()
                .sample(
                    LifeId {
                        instance: 78,
                        ..life
                    },
                    Vec3::new(0., 0.1, -24.),
                    0.45
                )
                .unwrap()
                .is_none()
        );
        let mut predicted = crate::prediction::Local::new(77);
        let before = serde_json::to_vec(&predicted.timing()).unwrap();
        assert!(
            predicted
                .animation_support()
                .sample(life, Vec3::ZERO, 0.45)
                .unwrap()
                .is_none()
        );
        assert_eq!(serde_json::to_vec(&predicted.timing()).unwrap(), before);
        predicted
            .observe_animation_geometry(&game.collision_geometry().unwrap())
            .unwrap();
        assert_eq!(serde_json::to_vec(&predicted.timing()).unwrap(), before);
        assert!(
            predicted
                .animation_support()
                .sample(life, Vec3::new(0., 0.1, -24.), 0.45)
                .unwrap()
                .is_some()
        );
        let mut foreign = game.collision_geometry().unwrap();
        foreign.instance = 78;
        assert!(predicted.observe_animation_geometry(&foreign).is_err());
        assert_eq!(serde_json::to_vec(&predicted.timing()).unwrap(), before);
        assert!(
            predicted
                .animation_support()
                .sample(life, Vec3::new(0., 0.1, -24.), 0.45)
                .unwrap()
                .is_some()
        );
    }
}
