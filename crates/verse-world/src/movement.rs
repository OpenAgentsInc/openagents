//! Shared walking and capsule substeps for authority and movement replay.
use glam::{DVec3, Vec3};
use physics::{
    character::{Character, Settings},
    queries::{Filter, Scene},
};

/// Movement leases last half a simulated second, rounded to an authority interval.
pub const HELD_STEPS: u64 = 60;
#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Held {
    pub axes: [f32; 2],
    pub until: u64,
}
impl Held {
    pub fn refresh(&mut self, axes: [f32; 2], step: u64) -> Result<(), String> {
        if axes.iter().any(|v| !v.is_finite() || v.abs() > 1.) {
            return Err("Invalid held movement axes".into());
        }
        *self = Self {
            axes,
            until: step
                .checked_add(HELD_STEPS)
                .ok_or("Held movement clock exhausted")?,
        };
        Ok(())
    }
    pub fn axes(&self, step: u64) -> [f32; 2] {
        if step < self.until {
            self.axes
        } else {
            [0.; 2]
        }
    }
    pub fn validate(&self, step: u64) -> Result<(), String> {
        if self.axes.iter().any(|v| !v.is_finite() || v.abs() > 1.)
            || self.until.saturating_sub(step) > HELD_STEPS
        {
            return Err("Invalid held movement checkpoint".into());
        }
        Ok(())
    }
}

/// Authoritative movement state after all admitted movement and jump input is consumed.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Baseline {
    pub life: verse_engine::core::LifeId,
    pub epoch: u64,
    pub applied_sequence: u64,
    pub physics_step: u64,
    pub held: Held,
    pub character: Character,
    pub yaw: f32,
}
impl Baseline {
    pub fn validate(&self) -> Result<(), String> {
        self.character.validate()?;
        self.held.validate(self.physics_step)?;
        if self.life.actor == 0 || !self.yaw.is_finite() {
            return Err("Invalid authoritative movement baseline".into());
        }
        Ok(())
    }
}

pub struct Walk {
    pub direction: Vec3,
    pub speed: f32,
}
pub fn walk(axes: [f32; 2], yaw: f32) -> Result<Walk, String> {
    if !yaw.is_finite() || axes.iter().any(|v| !v.is_finite()) {
        return Err("Invalid walking input".into());
    }
    let forward = Vec3::new(-yaw.sin(), 0., -yaw.cos());
    let right = Vec3::new(-forward.z, 0., forward.x);
    let input = Vec3::new(axes[0].clamp(-1., 1.), 0., axes[1].clamp(-1., 1.));
    let input = input / input.length().max(1.);
    Ok(Walk {
        direction: right * input.x + forward * input.z,
        speed: if input.z < 0. { 4.1148 } else { 6.4008 },
    })
}
pub struct Travel {
    pub path: Vec<[f32; 3]>,
    pub fallen: f64,
}
/// Commits character state only after all bounded substeps succeed.
pub fn advance(
    character: &mut Character,
    scene: &Scene,
    filter: Filter,
    velocity: DVec3,
    jump: bool,
    steps: u32,
    dt: f64,
) -> Result<Travel, String> {
    if steps > 120
        || !dt.is_finite()
        || !(0. ..=0.1).contains(&dt)
        || dt == 0.
        || !velocity.is_finite()
    {
        return Err("Invalid movement replay step budget".into());
    }
    character.validate()?;
    let mut next = *character;
    let mut path = Vec::with_capacity(steps as usize + 1);
    path.push(next.feet.as_vec3().to_array());
    let mut fallen = 0.;
    for step in 0..steps {
        next.step(
            scene,
            filter,
            Settings::default(),
            velocity,
            jump && step == 0,
            dt,
        )?;
        fallen += next.landed.unwrap_or(0.);
        path.push(next.feet.as_vec3().to_array());
    }
    *character = next;
    Ok(Travel { path, fallen })
}
#[cfg(test)]
mod tests {
    use super::*;
    use physics::queries::{ColliderKey, Life, Mesh, MeshCollider, Usage};
    fn box_in(scene: &mut Scene, shape: u32, min: DVec3, max: DVec3) {
        scene
            .insert(MeshCollider {
                key: ColliderKey {
                    life: Life {
                        instance: 1,
                        entity: 0,
                        generation: 0,
                    },
                    shape,
                },
                layers: 1,
                usage: Usage::Blocking,
                mesh: Mesh::from_box(min, max).unwrap(),
            })
            .unwrap();
    }
    fn floor() -> Scene {
        let mut scene = Scene::default();
        box_in(
            &mut scene,
            0,
            DVec3::new(-20., -1., -20.),
            DVec3::new(20., 0., 20.),
        );
        scene
    }
    #[test]
    fn walking_matches_speed_diagonal_and_yaw_conventions() {
        let forward = walk([0., 1.], 0.).unwrap();
        assert_eq!(forward.direction, Vec3::NEG_Z);
        assert_eq!(forward.speed, 6.4008);
        let back = walk([0., -1.], 0.).unwrap();
        assert_eq!(back.direction, Vec3::Z);
        assert_eq!(back.speed, 4.1148);
        assert!((walk([1., 1.], 0.).unwrap().direction.length() - 1.).abs() < 1e-6);
        assert!(
            (walk([0., 1.], std::f32::consts::FRAC_PI_2)
                .unwrap()
                .direction
                - Vec3::NEG_X)
                .length()
                < 1e-6
        );
        assert!(walk([f32::NAN, 0.], 0.).is_err());
    }
    #[test]
    fn replay_preserves_exact_capsule_state_and_cannot_cross_a_thin_wall() {
        let mut scene = floor();
        box_in(
            &mut scene,
            1,
            DVec3::new(1., 0., -5.),
            DVec3::new(1.01, 5., 5.),
        );
        let mut replay = Character::new(DVec3::ZERO);
        let mut direct = replay;
        let velocity = DVec3::X * 6.4008;
        for _ in 0..15 {
            let travel = advance(
                &mut replay,
                &scene,
                Filter::blocking(1),
                velocity,
                false,
                4,
                1. / 120.,
            )
            .unwrap();
            assert_eq!(travel.path.len(), 5);
            for _ in 0..4 {
                direct
                    .step(
                        &scene,
                        Filter::blocking(1),
                        Settings::default(),
                        velocity,
                        false,
                        1. / 120.,
                    )
                    .unwrap();
            }
        }
        assert_eq!(
            serde_json::to_vec(&replay).unwrap(),
            serde_json::to_vec(&direct).unwrap()
        );
        assert!(replay.feet.x < 0.651);
        let before = serde_json::to_vec(&replay).unwrap();
        assert!(
            advance(
                &mut replay,
                &scene,
                Filter::blocking(1),
                velocity,
                true,
                121,
                1. / 120.
            )
            .is_err()
        );
        assert_eq!(before, serde_json::to_vec(&replay).unwrap());
    }
    #[test]
    fn replay_climbs_an_admitted_step_and_matches_direct_substeps() {
        let mut scene = floor();
        box_in(
            &mut scene,
            1,
            DVec3::new(0.6, 0., -2.),
            DVec3::new(3., 0.3, 2.),
        );
        let mut replay = Character::new(DVec3::ZERO);
        let mut direct = replay;
        for _ in 0..8 {
            advance(
                &mut replay,
                &scene,
                Filter::blocking(1),
                DVec3::X * 6.4008,
                false,
                4,
                1. / 120.,
            )
            .unwrap();
            for _ in 0..4 {
                direct
                    .step(
                        &scene,
                        Filter::blocking(1),
                        Settings::default(),
                        DVec3::X * 6.4008,
                        false,
                        1. / 120.,
                    )
                    .unwrap();
            }
        }
        assert!(replay.feet.x > 1. && replay.feet.y > 0.299);
        assert_eq!(
            serde_json::to_vec(&replay).unwrap(),
            serde_json::to_vec(&direct).unwrap()
        );
    }
    #[test]
    fn replay_jumps_once_and_lands_on_the_same_ground() {
        let scene = floor();
        let mut c = Character::new(DVec3::ZERO);
        advance(
            &mut c,
            &scene,
            Filter::blocking(1),
            DVec3::ZERO,
            false,
            4,
            1. / 120.,
        )
        .unwrap();
        advance(
            &mut c,
            &scene,
            Filter::blocking(1),
            DVec3::ZERO,
            true,
            20,
            1. / 120.,
        )
        .unwrap();
        assert!(c.feet.y > 0.5 && c.support.is_none());
        advance(
            &mut c,
            &scene,
            Filter::blocking(1),
            DVec3::ZERO,
            false,
            120,
            1. / 120.,
        )
        .unwrap();
        assert!(c.feet.y.abs() < 0.001 && c.support.is_some());
    }
}
