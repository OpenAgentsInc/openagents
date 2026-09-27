//! Scenes shared with the offline Genesis oracle (`oracle/`, GP-8 #9785).
//!
//! A [`Scene`] is a plain description (bodies with one shape each, initial
//! motion, materials, step length, and step count) that both this crate and
//! `oracle/genesis_oracle.py` build. Each side writes a [`Trace`]; comparing
//! them within stated tolerances is what a fidelity claim has to rest on.
//! Gravity is zero, so the two engines' axis conventions do not matter.

use glam::{DQuat, DVec3};
use serde::{Deserialize, Serialize};

use crate::{Body, BodyKind, Collider, Material, NoField, Shape, Trace, World};

/// One body with one collider at its center of mass.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneBody {
    pub fixed: bool,
    pub mass: f64,
    pub inertia: DVec3,
    pub shape: Shape,
    pub pos: DVec3,
    pub orientation: DQuat,
    pub vel: DVec3,
    pub omega_world: DVec3,
    pub material: Material,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scene {
    pub name: String,
    pub dt: f64,
    pub steps: u32,
    pub bodies: Vec<SceneBody>,
}

impl Scene {
    /// Build the scene in a physics world.
    #[must_use]
    pub fn world(&self) -> World {
        let mut world = World::new(self.dt);
        for b in &self.bodies {
            let mut body = Body::new(b.mass, b.inertia, b.pos);
            body.orientation = b.orientation;
            body.prev_orientation = b.orientation;
            body.vel = b.vel;
            body.omega = b.orientation.inverse() * b.omega_world;
            if b.fixed {
                body = body.with_kind(BodyKind::Static);
            }
            let id = world.add(body);
            world.add_collider(Collider::new(id, b.shape).with_material(b.material));
        }
        world
    }

    /// Run the scene and record every step.
    #[must_use]
    pub fn run(&self) -> Trace {
        let mut world = self.world();
        let mut trace = Trace::default();
        for _ in 0..self.steps {
            world.step(&NoField);
            trace.record(&world);
        }
        trace
    }
}

/// The audit's experiment: a spinning 320 kg tank strikes a fixed panel's
/// edge at 0.85 m/s.
#[must_use]
pub fn tank_into_panel() -> Scene {
    let tank_material = Material {
        friction: 0.4,
        torsional: 0.0,
        restitution: 0.3,
    };
    Scene {
        name: "tank_into_panel".into(),
        dt: 1.0 / 120.0,
        steps: 360,
        bodies: vec![
            SceneBody {
                fixed: true,
                mass: 1.0,
                inertia: DVec3::ONE,
                shape: Shape::Cuboid {
                    half: DVec3::new(3.0, 0.15, 2.0),
                },
                pos: DVec3::ZERO,
                orientation: DQuat::IDENTITY,
                vel: DVec3::ZERO,
                omega_world: DVec3::ZERO,
                material: Material::default(),
            },
            SceneBody {
                fixed: false,
                mass: 320.0,
                inertia: Body::shell_inertia(320.0, 1.3, 3.6),
                shape: Shape::Capsule {
                    radius: 1.3,
                    half_length: 0.5,
                },
                pos: DVec3::new(3.2, 2.0, 0.0),
                orientation: DQuat::from_rotation_x(std::f64::consts::FRAC_PI_2),
                vel: DVec3::new(-0.3, -0.8, 0.0),
                omega_world: DVec3::new(0.05, 0.1, 0.4),
                material: tank_material,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tolerance;

    #[test]
    fn scenes_round_trip_and_run_deterministically() {
        let scene = tank_into_panel();
        let json = serde_json::to_string(&scene).unwrap();
        let back: Scene = serde_json::from_str(&json).unwrap();
        back.run().compare(&scene.run(), Tolerance::EXACT).unwrap();
    }

    /// Compare against a Genesis trace written by `oracle/genesis_oracle.py`
    /// into `PHYSICS_ORACLE_DIR`: the bar a fidelity claim would need. The
    /// 2026-09-27 run does not meet it, because the restitution models
    /// differ (see `oracle/README.md`), so no fidelity is claimed.
    #[test]
    #[ignore = "needs a Genesis trace from oracle/genesis_oracle.py"]
    fn tank_into_panel_matches_genesis() {
        let dir = std::env::var("PHYSICS_ORACLE_DIR").expect("set PHYSICS_ORACLE_DIR");
        let path = std::path::Path::new(&dir).join("tank_into_panel.genesis.json");
        let genesis: Trace = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let tolerance = Tolerance {
            pos: 0.05,
            vel: 0.1,
            angle: 0.05,
            omega: 0.1,
        };
        tank_into_panel()
            .run()
            .compare(&genesis, tolerance)
            .unwrap();
    }
}
