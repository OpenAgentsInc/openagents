//! Compiled CC0 furnishing and authored lights for the original summoning lair.
use glam::{Mat4, Quat, Vec3};
use std::path::Path;
use verse_engine::assets::{Bone, Model, Pack, Placement, Surface, Vertex};

const MODELS: &[&str] = &[
    "Torch_Metal",
    "Lantern_Wall",
    "Cauldron",
    "Table_Large",
    "Book_Stack_1",
    "Scroll_1",
    "CandleStick_Triple",
    "CandleStick_Stand",
    "Chandelier",
    "Cage_Small",
    "Chest_Wood",
    "Barrel",
    "Banner_2",
    "Potion_1",
];
fn place(pack: &mut Pack, model: &str, position: Vec3, yaw: f32, scale: f32) {
    let inverse = super::chamber::basis().inverse();
    let rotation = inverse * Mat4::from_rotation_y(yaw) * super::chamber::basis();
    pack.placements.push(Placement {
        model: format!("prop/{model}"),
        position: inverse.transform_point3(position).to_array(),
        rotation: Quat::from_mat4(&rotation).to_array(),
        scale,
    });
}
pub fn install(pack: &mut Pack, dir: &Path, root: &Path) -> Result<(), String> {
    let basis = super::chamber::basis();
    for name in MODELS {
        let mut model = super::characters::import(pack, dir, &root.join(format!("{name}.gltf")))?;
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for vertex in model.surfaces.iter().flat_map(|s| &s.vertices) {
            let p = basis.transform_point3(vertex.position.into());
            min = min.min(p);
            max = max.max(p);
        }
        if !min.is_finite() || !max.is_finite() {
            return Err("Prop has no finite geometry".into());
        }
        let pivot = Vec3::new((min.x + max.x) * 0.5, min.y, (min.z + max.z) * 0.5);
        for vertex in model.surfaces.iter_mut().flat_map(|s| &mut s.vertices) {
            vertex.position = basis
                .inverse()
                .transform_point3(basis.transform_point3(vertex.position.into()) - pivot)
                .to_array();
            vertex.joints = [0; 4];
            vertex.weights = [1., 0., 0., 0.];
        }
        model.skin = None;
        model.bones = vec![Bone {
            parent: -1,
            pivot: [0.; 3],
        }];
        model.clips.clear();
        model.height = (max.y - min.y) / 0.9144;
        pack.models.insert(format!("prop/{name}"), model);
    }
    let mut flame = Model {
        graph: None,
        source: "verse/original/torch-flame".into(),
        source_sha256: String::new(),
        surfaces: vec![],
        bones: vec![Bone {
            parent: -1,
            pivot: [0.; 3],
        }],
        clips: vec![],
        height: 0.6,
        attachments: vec![],
        skin: None,
        markers: Vec::new(),
        states: Default::default(),
    };
    for (radius, height, tint) in [
        (0.16, 0.62, [1., 0.24, 0.025]),
        (0.10, 0.42, [1., 0.72, 0.10]),
    ] {
        let positions = [
            Vec3::new(-radius, 0., -radius),
            Vec3::new(radius, 0., -radius),
            Vec3::new(radius, 0., radius),
            Vec3::new(-radius, 0., radius),
            Vec3::new(0.04, height, 0.),
        ];
        let vertices = positions
            .iter()
            .map(|p| Vertex {
                position: basis.inverse().transform_point3(*p).to_array(),
                normal: [0., 0., 1.],
                uv: [0.5; 2],
                joints: [0; 4],
                weights: [1., 0., 0., 0.],
            })
            .collect();
        flame.surfaces.push(Surface {
            material: Default::default(),
            vertices,
            indices: vec![0, 4, 1, 1, 4, 2, 2, 4, 3, 3, 4, 0],
            texture: 0,
            blend: 0,
            emissive: true,
            tint,
        });
    }
    pack.models.insert("prop/flame".into(), flame);
    let mut liquid = pack.models["prop/flame"].clone();
    liquid.source = "verse/original/ritual-liquid".into();
    let mut vertices = vec![Vertex {
        position: [0.; 3],
        normal: [0., 0., 1.],
        uv: [0.5; 2],
        joints: [0; 4],
        weights: [1., 0., 0., 0.],
    }];
    for i in 0..32 {
        let angle = i as f32 * std::f32::consts::TAU / 32.;
        vertices.push(Vertex {
            position: basis
                .inverse()
                .transform_point3(Vec3::new(angle.cos() * 0.95, 0., angle.sin() * 0.95))
                .to_array(),
            ..vertices[0].clone()
        });
    }
    let mut indices = vec![];
    for i in 0..32 {
        indices.extend([0, (i + 1) % 32 + 1, i + 1]);
    }
    liquid.surfaces = vec![Surface {
        material: Default::default(),
        vertices,
        indices,
        texture: 0,
        blend: 0,
        emissive: true,
        tint: [0.03, 0.9, 0.13],
    }];
    pack.models.insert("prop/ritual-liquid".into(), liquid);
    let mut seal = pack.models["prop/flame"].clone();
    seal.source = "verse/original/summoning-seal".into();
    let mut vertices = vec![];
    let mut indices = vec![];
    let mut quad = |points: [Vec3; 4]| {
        let offset = vertices.len() as u32;
        vertices.extend(points.map(|point| Vertex {
            position: basis.inverse().transform_point3(point).to_array(),
            normal: [0., 0., 1.],
            uv: [0.5; 2],
            joints: [0; 4],
            weights: [1., 0., 0., 0.],
        }));
        indices.extend([
            offset,
            offset + 2,
            offset + 1,
            offset,
            offset + 3,
            offset + 2,
        ]);
    };
    for radius in [5.2, 3.9] {
        for i in 0..64 {
            let a = i as f32 * std::f32::consts::TAU / 64.;
            let b = (i + 1) as f32 * std::f32::consts::TAU / 64.;
            let point = |angle: f32, r: f32| Vec3::new(angle.cos() * r, 0.025, angle.sin() * r);
            quad([
                point(a, radius),
                point(b, radius),
                point(b, radius - 0.06),
                point(a, radius - 0.06),
            ]);
        }
    }
    for i in 0..5 {
        let a = i as f32 * std::f32::consts::TAU / 5.;
        let b = (i + 2) as f32 * std::f32::consts::TAU / 5.;
        let start = Vec3::new(a.cos() * 3.85, 0.028, a.sin() * 3.85);
        let end = Vec3::new(b.cos() * 3.85, 0.028, b.sin() * 3.85);
        let side = (end - start).normalize().cross(Vec3::Y) * 0.035;
        quad([start - side, end - side, end + side, start + side]);
    }
    seal.surfaces = vec![Surface {
        material: Default::default(),
        vertices,
        indices,
        texture: 0,
        blend: 0,
        emissive: true,
        tint: [0.45, 0.055, 0.9],
    }];
    pack.models.insert("prop/summoning-seal".into(), seal);
    place(pack, "summoning-seal", Vec3::ZERO, 0., 1.);
    for x in [-14., 14.] {
        for z in [-24., -12., 0., 11.] {
            place(
                pack,
                "Torch_Metal",
                Vec3::new(x, 2.8, z),
                if x < 0. { 1.57 } else { -1.57 },
                2.2,
            );
            place(pack, "flame", Vec3::new(x, 4.0, z), 0., 1.);
        }
    }
    for x in [-9., 9.] {
        place(pack, "Table_Large", Vec3::new(x, 0., -7.), 0., 1.8);
        for (dx, z) in [(-0.9, -7.1), (0.8, -6.8)] {
            place(pack, "Book_Stack_1", Vec3::new(x + dx, 1.47, z), 0.4, 2.);
        }
        place(pack, "Scroll_1", Vec3::new(x, 1.47, -7.2), 0.4, 2.8);
        place(
            pack,
            "CandleStick_Triple",
            Vec3::new(x, 1.47, -6.7),
            0.,
            2.2,
        );
        place(pack, "flame", Vec3::new(x, 2.5, -6.7), 0., 0.45);
        place(pack, "Potion_1", Vec3::new(x + 1.5, 1.47, -7.), 0., 2.5);
        place(pack, "Cauldron", Vec3::new(x, 0., 5.), 0., 2.5);
        place(pack, "ritual-liquid", Vec3::new(x, 1.7, 5.), 0., 1.);
        place(pack, "Cage_Small", Vec3::new(x * 1.8, 0., 7.), 0., 2.8);
        place(pack, "Chest_Wood", Vec3::new(x * 1.9, 0., -10.), 0., 2.);
        place(pack, "Barrel", Vec3::new(x * 1.9, 0., -13.), 0., 2.);
        place(pack, "Banner_2", Vec3::new(x, 3.8, 16.), 0., 2.5);
        place(
            pack,
            "CandleStick_Stand",
            Vec3::new(x * 0.7, 0., 2.5),
            0.,
            1.7,
        );
        place(pack, "flame", Vec3::new(x * 0.7, 2.2, 2.5), 0., 0.6);
    }
    place(pack, "Chandelier", Vec3::new(0., 7.2, -10.), 0., 3.);
    for i in 0..6 {
        let angle = i as f32 * std::f32::consts::TAU / 6.;
        place(
            pack,
            "flame",
            Vec3::new(angle.cos() * 1.6, 7.65, -10. + angle.sin() * 1.6),
            0.,
            0.35,
        );
    }
    for x in [-21., 21.] {
        place(
            pack,
            "Lantern_Wall",
            Vec3::new(x, 2.5, -18.),
            if x < 0. { 1.57 } else { -1.57 },
            1.5,
        );
        place(pack, "flame", Vec3::new(x, 2.8, -18.), 0., 0.35);
    }
    pack.validate()
}

/// Admit furniture bounds through the same world prop path as navigation blockers.
pub fn admit_collision(pack: &Pack, game: &mut super::play::Game) -> Result<(), String> {
    let life = game.player_life();
    for (index, placement) in pack.placements.iter().enumerate() {
        if placement.model == "prop/flame" {
            let position = super::chamber::basis()
                .transform_point3(placement.position.into())
                .as_dvec3();
            let id = index as u32;
            if !game.spells.flames.iter().any(|f| f.id == id) {
                game.spells.flames.push(verse_world::gust::Flame {
                    id,
                    position,
                    protected: position.x.abs() > 20. && (position.z + 18.).abs() < 1.,
                    lit: true,
                });
            }
        }
        if ![
            "Table_Large",
            "Cauldron",
            "Cage_Small",
            "Chest_Wood",
            "Barrel",
            "CandleStick_Stand",
        ]
        .iter()
        .any(|name| placement.model == format!("prop/{name}"))
        {
            continue;
        }
        let transform = super::chamber::basis()
            * Mat4::from_scale_rotation_translation(
                Vec3::splat(placement.scale),
                Quat::from_array(placement.rotation),
                placement.position.into(),
            );
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for vertex in pack.models[&placement.model]
            .surfaces
            .iter()
            .flat_map(|s| &s.vertices)
        {
            let p = transform.transform_point3(vertex.position.into());
            min = min.min(p);
            max = max.max(p);
        }
        game.set_navigation_blocker(
            physics::queries::Life {
                instance: life.instance,
                entity: 10_000 + index as u64,
                generation: life.generation,
            },
            min.as_dvec3(),
            max.as_dvec3(),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn static_props_compile_with_provenance_and_admitted_collision() {
        let dir = std::env::temp_dir().join(format!("verse-props-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/verse/props/quaternius");
        let frozen = dir.join("source");
        super::super::inventory::snapshot_props(&root, &frozen).unwrap();
        let mut pack = super::super::original::generate(&dir).unwrap();
        install(&mut pack, &dir, &frozen).unwrap();
        super::super::inventory::compile(&mut pack, &dir, None).unwrap();
        let inventory = pack.inventory.as_ref().unwrap();
        assert!(
            inventory
                .admit(
                    &pack,
                    verse_engine::inventory::Purpose::Redistribute,
                    &inventory.roots()
                )
                .is_ok()
        );
        assert!(pack.models["prop/Torch_Metal"].skin.is_none());
        let torch = &pack.models["prop/Torch_Metal"].surfaces[0];
        assert!(torch.material.normal_texture.is_some());
        assert!(torch.material.metallic_roughness_texture.is_some());
        assert_eq!(torch.material.roughness, 1.0);
        assert_eq!(torch.material.metallic, 1.0);
        assert_ne!(torch.texture, torch.material.normal_texture.unwrap());
        assert_ne!(
            torch.texture,
            torch.material.metallic_roughness_texture.unwrap()
        );
        assert!(torch.material.validate(pack.textures.len()).is_ok());
        let model_asset = inventory
            .assets
            .iter()
            .find(|asset| {
                asset.binding
                    == verse_engine::inventory::Binding::Model {
                        key: "prop/Torch_Metal".into(),
                    }
            })
            .unwrap();
        for slot in torch.texture_slots() {
            let image = inventory
                .assets
                .iter()
                .find(|asset| asset.binding == verse_engine::inventory::Binding::Texture { slot })
                .unwrap();
            assert!(model_asset.dependencies.contains(&image.id));
        }

        let mut game = super::super::play::Game::new(
            verse_engine::director::Scene::from_json(include_bytes!(
                "../../../../assets/verse/original/ritual.json"
            ))
            .unwrap(),
        )
        .unwrap();
        admit_collision(&pack, &mut game).unwrap();
        assert!(game.navigation_blockers().active_bounds().count() >= 10);
        assert!(
            game.navigation_blockers()
                .active_bounds()
                .all(|(_, min, max)| min.cmplt(max).all())
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
