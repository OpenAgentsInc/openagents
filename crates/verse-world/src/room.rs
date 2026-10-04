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

/// Collision boxes of a scene's named collision profile; none without one.
pub fn profile_colliders(profile: Option<&str>) -> Result<Vec<physics::kinematic::Aabb>, String> {
    match profile {
        None => Ok(vec![]),
        Some("original-chamber-v1") => Ok(colliders()),
        Some(crate::playground::PROFILE) => Ok(crate::playground::hall()?.colliders()),
        Some(_) => Err("Unsupported scene collision profile".into()),
    }
}

/// Compiles the same authored room solids into scoped triangle query geometry.
pub fn query_scene(instance: u64) -> Result<physics::queries::Scene, String> {
    profile_query_scene(Some("original-chamber-v1"), instance)
}

/// Compiles a collision profile's solids into scoped triangle query geometry.
pub fn profile_query_scene(
    profile: Option<&str>,
    instance: u64,
) -> Result<physics::queries::Scene, String> {
    use physics::queries::{ColliderKey, Life, Mesh, MeshCollider, Scene, Usage};
    let mut scene = Scene::default();
    for (shape, bounds) in profile_colliders(profile)?.into_iter().enumerate() {
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

/// Caches immutable walkable geometry; each instance keeps its own blocker book.
pub fn navigation(instance: u64) -> Result<std::sync::Arc<physics::walkable::Navigation>, String> {
    use physics::walkable::{Config, Navigation};
    static COMPILED: std::sync::OnceLock<Result<std::sync::Arc<Navigation>, String>> =
        std::sync::OnceLock::new();
    let template = COMPILED
        .get_or_init(|| {
            let scene = query_scene(0)?;
            Ok(std::sync::Arc::new(Navigation::compile(
                &scene,
                Config {
                    instance: 0,
                    layers: 1,
                    min: glam::DVec3::new(-21., -0.01, -36.),
                    max: glam::DVec3::new(21., 2., 16.),
                    cell: 0.5,
                    character: physics::character::Settings::default(),
                    work_budget: 40_000_000,
                },
            )?))
        })
        .clone()?;
    Ok(if instance == 0 {
        template
    } else {
        std::sync::Arc::new(template.bind_instance(instance))
    })
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

#[cfg(test)]
mod navigation_tests {
    use super::*;
    #[test]
    fn original_chamber_compiles_stairs_and_column_routes() {
        let scene = query_scene(0).unwrap();
        let nav = navigation(0).unwrap();
        eprintln!("Chamber navigation: {:?}", nav.stats);
        let blockers = physics::walkable::Blockers::new(0);
        let route = nav
            .path(
                &scene,
                &blockers,
                0,
                glam::DVec3::new(13., 0., -13.),
                glam::DVec3::new(17., 0., -13.),
                None,
                Default::default(),
            )
            .unwrap()
            .unwrap();
        assert!(route.points.iter().any(|p| (p.z + 13.).abs() > 1.));
        let stairs = nav
            .path(
                &scene,
                &blockers,
                0,
                glam::DVec3::new(18., 0., -32.),
                glam::DVec3::new(18., 1.5, -25.),
                None,
                Default::default(),
            )
            .unwrap()
            .unwrap();
        assert!(stairs.points.iter().any(|p| p.y > 1.4));
    }
}
