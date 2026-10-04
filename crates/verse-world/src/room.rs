//! Shared authored chamber solids for authority and presentation.
use glam::Vec3;
pub struct RoomBox {
    pub center: Vec3,
    pub half: Vec3,
    pub color: [f32; 3],
    pub emissive: bool,
}
pub fn room_boxes() -> Vec<RoomBox> {
    let mut boxes = vec![];
    let mut add = |center, half, color, emissive| {
        boxes.push(RoomBox {
            center,
            half,
            color,
            emissive,
        })
    };
    add(
        Vec3::new(0., -0.3, -7.),
        Vec3::new(22., 0.3, 30.),
        [0.19, 0.21, 0.24],
        false,
    );
    for x in [-22., 22.] {
        add(
            Vec3::new(x, 6., -7.),
            Vec3::new(0.5, 6., 30.),
            [0.12, 0.14, 0.18],
            false,
        );
    }
    for z in [-37., 17.] {
        add(
            Vec3::new(0., 6., z),
            Vec3::new(22., 6., 0.5),
            [0.12, 0.14, 0.18],
            false,
        );
    }
    for x in [-15., 15.] {
        for z in [-25., -13., 0., 12.] {
            add(
                Vec3::new(x, 5., z),
                Vec3::new(0.7, 5., 0.7),
                [0.23, 0.24, 0.27],
                false,
            );
            add(
                Vec3::new(x, 2.8, z),
                Vec3::new(0.9, 0.2, 0.9),
                [0.2, 0.24, 0.2],
                false,
            );
            add(
                Vec3::new(x, 3.15, z),
                Vec3::new(0.3, 0.3, 0.3),
                [0.07, 0.8, 0.18],
                true,
            );
        }
    }
    // The side stairs exercise the same authored solids in rendering and movement.
    for step in 0..6 {
        let height = (step + 1) as f32 * 0.25;
        add(
            Vec3::new(18., height * 0.5, -29.5 + step as f32 * 0.9),
            Vec3::new(1.5, height * 0.5, 0.45),
            [0.23, 0.24, 0.27],
            false,
        );
    }
    boxes
}
/// Compiles collision boxes from the same authored solids as the room mesh.
pub fn colliders() -> Vec<physics::kinematic::Aabb> {
    room_boxes()
        .into_iter()
        .filter(|b| !b.emissive)
        .map(|b| physics::kinematic::Aabb {
            min: (b.center - b.half).as_dvec3(),
            max: (b.center + b.half).as_dvec3(),
        })
        .collect()
}

/// Compiles the same authored room solids into scoped triangle query geometry.
pub fn query_scene(instance: u64) -> Result<physics::queries::Scene, String> {
    use physics::queries::{ColliderKey, Life, Mesh, MeshCollider, Scene, Usage};
    let mut scene = Scene::default();
    for (shape, bounds) in colliders().into_iter().enumerate() {
        scene.insert(MeshCollider {
            key: ColliderKey {
                life: Life {
                    instance,
                    entity: 0,
                    generation: 0,
                },
                shape: shape as u32,
            },
            layers: 1,
            usage: Usage::Blocking,
            mesh: Mesh::from_box(bounds.min, bounds.max)?,
        })?;
    }
    Ok(scene)
}

#[cfg(test)]
mod query_tests {
    use super::*;
    use physics::queries::{Capsule, Filter};
    #[test]
    fn compiled_room_queries_match_floor_column_and_instance_scope() {
        let scene = query_scene(7).unwrap();
        let origin = glam::DVec3::new(13., 0.9, -13.);
        let filter = Filter::blocking(7);
        let wall = scene
            .sweep(
                Capsule {
                    a: origin - glam::DVec3::Y * 0.55,
                    b: origin + glam::DVec3::Y * 0.55,
                    radius: 0.35,
                },
                glam::DVec3::X * 100.,
                filter,
            )
            .unwrap();
        assert!((wall.hits[0].distance - 0.95).abs() < 1e-6);
        let floor = scene.ray(origin, -glam::DVec3::Y, 3., filter).unwrap();
        assert!((floor.hits[0].distance - 0.9).abs() < 1e-9);
        assert!(
            scene
                .ray(origin, -glam::DVec3::Y, 3., Filter::blocking(8))
                .unwrap()
                .hits
                .is_empty()
        );
    }
}
