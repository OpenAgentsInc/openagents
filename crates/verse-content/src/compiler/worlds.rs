//! Original world recipes share compiled assets and portable authority profiles.
use std::path::Path;
use verse_engine::{assets::Pack, director::Scene};
use verse_world::play::social::Profile;

pub fn compile(name: &str, dir: &Path) -> Result<(Pack, Scene, Option<Profile>), String> {
    if !matches!(name, "ritual" | "observatory") {
        return Err("World recipe must be ritual or observatory".into());
    }
    let mut pack = super::original::generate(dir)?;
    let mut scene = Scene::from_json(include_bytes!(
        "../../../../assets/verse/original/ritual.json"
    ))?;
    let profile = if name == "observatory" {
        use glam::{DVec3, Vec3};
        use physics::queries::{ColliderKey, Life, Mesh, MeshCollider, Scene as Geometry, Usage};
        use verse_world::play::social::{Kind, Object, PROFILE_REVISION, Zone};
        scene.cut_at = 0.;
        scene.cues.clear();
        scene.collision_profile = None;
        scene.actors.retain(|actor| actor.model == "adventurer");
        scene.actors[0].name = "Observatory visitor".into();
        scene.actors[0].position = Vec3::new(0., 0., -4.);
        scene.actors[0].friendly = false;
        pack.placements.clear();
        let mut floor = pack.models["chamber"].clone();
        floor.source = "verse/original/observatory-floor".into();
        floor.surfaces.truncate(1);
        let surface = &mut floor.surfaces[0];
        let vertex = surface.vertices[0].clone();
        let inverse = crate::basis().inverse();
        surface.vertices = [(-12., -12.), (-12., 12.), (12., 12.), (12., -12.)]
            .into_iter()
            .map(|(x, z)| {
                let mut vertex = vertex.clone();
                vertex.position = inverse.transform_point3(Vec3::new(x, 0., z)).to_array();
                vertex.normal = inverse.transform_vector3(Vec3::Y).normalize().to_array();
                vertex.uv = [(x + 12.) / 24., (z + 12.) / 24.];
                vertex
            })
            .collect();
        surface.indices = vec![0, 1, 2, 0, 2, 3];
        surface.tint = [0.2, 0.35, 0.5];
        use sha2::{Digest, Sha256};
        floor.source_sha256 = String::new();
        floor.source_sha256 = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&floor).map_err(|e| e.to_string())?)
        );
        pack.models.insert("chamber".into(), floor);
        let mut geometry = Geometry::default();
        geometry.insert(MeshCollider {
            key: ColliderKey {
                life: Life {
                    instance: 0,
                    entity: 0,
                    generation: 0,
                },
                shape: 0,
            },
            layers: 1,
            usage: Usage::Blocking,
            mesh: Mesh::from_box(DVec3::new(-12., -1., -12.), DVec3::new(12., 0., 12.))?,
        })?;
        Some(Profile {
            revision: PROFILE_REVISION,
            zone: Zone::Plaza,
            geometry: geometry.snapshot(0)?,
            objects: vec![
                Object {
                    id: 1,
                    feet: [2., 0., 0.],
                    yaw: 0.,
                    kind: Kind::Seat,
                },
                Object {
                    id: 2,
                    feet: [0., 0., -3.],
                    yaw: 0.,
                    kind: Kind::Switch,
                },
            ],
        })
    } else {
        None
    };
    scene.validate()?;
    if let Some(profile) = &profile {
        profile.validate()?;
    }
    super::inventory::compile(&mut pack, dir, None)?;
    std::fs::write(
        dir.join("pack.json"),
        serde_json::to_vec_pretty(&pack).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(
        dir.join("scene.json"),
        serde_json::to_vec_pretty(&scene).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(
        dir.join("profile.json"),
        serde_json::to_vec_pretty(&profile).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok((pack, scene, profile))
}
