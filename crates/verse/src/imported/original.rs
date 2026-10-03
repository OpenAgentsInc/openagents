//! Original procedural ritual assets. No external game files are read.
use crate::ui::Atlas;
use glam::{Quat, Vec3};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};
use verse_engine::assets::{
    Attachment, Bone, BoneKeys, Clip, Model, Pack, Placement, Surface, Texture, Vertex,
};

fn png_file(dir: &Path, name: &str, size: u32, pixels: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(dir.join(format!("{name}.png"))).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(file, size, size);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(pixels)
        .map_err(|e| e.to_string())
}
fn model(name: &str, height: f32) -> Model {
    Model {
        skin: None,
        source: format!("verse/original/{name}"),
        source_sha256: String::new(),
        surfaces: vec![],
        bones: vec![Bone {
            parent: -1,
            pivot: [0.0; 3],
        }],
        clips: vec![],
        height,
        attachments: vec![],
    }
}
fn cuboid(
    m: &mut Model,
    center: [f32; 3],
    half: [f32; 3],
    color: [f32; 3],
    joint: u32,
    emissive: bool,
) {
    let mut s = Surface {
        vertices: vec![],
        indices: vec![],
        texture: 0,
        blend: 0,
        emissive,
        tint: color,
    };
    for (normal, axes) in [
        ([1., 0., 0.], [1, 2]),
        ([-1., 0., 0.], [2, 1]),
        ([0., 1., 0.], [2, 0]),
        ([0., -1., 0.], [0, 2]),
        ([0., 0., 1.], [0, 1]),
        ([0., 0., -1.], [1, 0]),
    ] {
        let base = s.vertices.len() as u32;
        for (u, v) in [(-1., -1.), (1., -1.), (1., 1.), (-1., 1.)] {
            let mut position = center;
            for k in 0..3 {
                position[k] += normal[k] * half[k];
            }
            position[axes[0]] += u * half[axes[0]];
            position[axes[1]] += v * half[axes[1]];
            s.vertices.push(Vertex {
                position,
                normal,
                uv: [(u + 1.) * 0.5, (v + 1.) * 0.5],
                joints: [joint, 0, 0, 0],
                weights: [1., 0., 0., 0.],
            });
        }
        s.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    m.surfaces.push(s);
}
fn actor(name: &str, robe: [f32; 3], monster: bool) -> Model {
    let mut m = model(name, 2.35);
    for pivot in [
        [0., -0.48, 1.65],
        [0., 0.48, 1.65],
        [0., -0.2, 0.8],
        [0., 0.2, 0.8],
    ] {
        m.bones.push(Bone { parent: 0, pivot });
    }
    cuboid(&mut m, [0., 0., 1.2], [0.28, 0.4, 0.65], robe, 0, false);
    cuboid(&mut m, [0., 0., 2.04], [0.3, 0.32, 0.31], robe, 0, false);
    cuboid(
        &mut m,
        [0.305, 0., 2.02],
        [0.018, 0.22, 0.16],
        [0.035, 0.028, 0.04],
        0,
        false,
    );
    for (i, y) in [-0.52, 0.52].into_iter().enumerate() {
        cuboid(
            &mut m,
            [0., y, 1.3],
            [0.16, 0.16, 0.4],
            robe,
            i as u32 + 1,
            false,
        );
        cuboid(
            &mut m,
            [-0.035, y, 0.88],
            [0.17, 0.17, 0.13],
            [0.5, 0.36, 0.23],
            i as u32 + 1,
            false,
        );
        cuboid(
            &mut m,
            [0., y * 0.4, 0.42],
            [0.2, 0.16, 0.4],
            robe,
            i as u32 + 3,
            false,
        );
        cuboid(
            &mut m,
            [-0.12, y * 0.4, 0.1],
            [0.3, 0.18, 0.1],
            [0.08, 0.06, 0.05],
            i as u32 + 3,
            false,
        );
        cuboid(
            &mut m,
            [0.33, y * 0.25, 2.08],
            [0.025, 0.045, 0.035],
            if monster {
                [1., 0.16, 0.02]
            } else {
                [0.3, 0.9, 0.35]
            },
            0,
            true,
        );
        if monster {
            cuboid(
                &mut m,
                [0., y, 2.42],
                [0.1, 0.1, 0.38],
                [0.44, 0.33, 0.2],
                0,
                false,
            );
        }
    }
    for id in [0, 1, 4, 25, 51, 52, 53, 64, 68] {
        let duration = if id == 1 { 1.0 } else { 1.8 };
        let mut bones = vec![];
        for bone in 0..5 {
            let angles: [f32; 3] = if id == 1 && bone == 0 {
                [0., 0.8, 1.57]
            } else if bone == 0 {
                [0., 0.015, 0.]
            } else if id == 4 {
                let a = if bone % 2 == 0 { 0.65 } else { -0.65 };
                [a, -a, a]
            } else if bone < 3 && id != 0 {
                [-0.75, -1.1, -0.75]
            } else {
                [0., 0.035, 0.]
            };
            bones.push(BoneKeys {
                bone,
                translation: vec![],
                scale: vec![],
                rotation: angles
                    .into_iter()
                    .enumerate()
                    .map(|(i, a)| {
                        (
                            i as f32 * duration * 0.5,
                            Quat::from_rotation_y(a).to_array(),
                        )
                    })
                    .collect(),
            });
        }
        m.clips.push(Clip {
            id,
            duration,
            bones,
        });
    }
    m.attachments.push(Attachment {
        id: 2,
        bone: 1,
        position: [0., -0.52, 0.88],
    });
    m
}
struct RoomBox {
    center: Vec3,
    half: Vec3,
    color: [f32; 3],
    emissive: bool,
}
fn room_boxes() -> Vec<RoomBox> {
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
/// Generates a complete pack in the caller's directory.
pub fn generate(dir: &Path) -> Result<Pack, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    png_file(dir, "original-white", 1, &[255; 4])?;
    let mut pack = Pack {
        version: 1,
        source_revision: "verse-original-ritual-v1".into(),
        models: BTreeMap::new(),
        textures: vec![Texture {
            file: "original-white.png".into(),
            sha256: format!(
                "{:x}",
                Sha256::digest(
                    std::fs::read(dir.join("original-white.png")).map_err(|e| e.to_string())?
                )
            ),
            width: 1,
            height: 1,
        }],
        placements: vec![],
    };
    pack.models.insert(
        "adventurer".into(),
        actor("adventurer", [0.16, 0.26, 0.38], false),
    );
    pack.models.insert(
        "cultist".into(),
        actor("cultist", [0.22, 0.055, 0.3], false),
    );
    pack.models
        .insert("claude".into(), actor("claude", [0.26, 0.09, 0.075], true));
    let mut room = model("chamber", 12.);
    // Author world-space boxes, then convert to the pack's Z-up convention.
    let inverse = super::chamber::basis().inverse();
    let mut world_box = |center: Vec3, half: Vec3, color, emissive| {
        let c = inverse.transform_point3(center).to_array();
        let h = inverse.transform_vector3(half).abs().to_array();
        cuboid(&mut room, c, h, color, 0, emissive);
    };
    for b in room_boxes() {
        world_box(b.center, b.half, b.color, b.emissive);
    }
    pack.models.insert("chamber".into(), room);
    pack.placements.push(Placement {
        model: "chamber".into(),
        position: [0.; 3],
        rotation: [0., 0., 0., 1.],
        scale: 1.,
    });
    for (name, half, color) in [
        ("bow", [0.05, 0.07, 0.7], [0.55, 0.29, 0.1]),
        ("arrow", [0.55, 0.025, 0.025], [0.75, 0.65, 0.35]),
    ] {
        let mut m = model(name, 1.4);
        cuboid(&mut m, [0.; 3], half, color, 0, false);
        pack.models.insert(name.into(), m);
    }
    for (index, name) in [
        "particle-fire",
        "particle-arcane",
        "particle-smoke",
        "particle-web",
        "particle-shadow",
        "particle-spark",
        "particle-ribbon",
        "particle-rune",
    ]
    .into_iter()
    .enumerate()
    {
        let mut pixels = vec![];
        for y in 0..64 {
            for x in 0..64 {
                let dx = (x as f32 - 31.5) / 31.5;
                let dy = (y as f32 - 31.5) / 31.5;
                let r = (dx * dx + dy * dy).sqrt();
                let a = if index == 7 {
                    (1. - ((r - 0.72) * 18.).abs()).max(0.)
                } else if index == 3 {
                    ((dx * 12.).sin().abs().min((dy * 12.).sin().abs()) < 0.12) as u8 as f32
                        * (1. - r).max(0.)
                } else {
                    (1. - r).max(0.).powi(2)
                };
                pixels.extend([255, 255, 255, (a * 255.) as u8]);
            }
        }
        png_file(dir, name, 64, &pixels)?;
    }
    super::chamber::add_effect_models(&mut pack, dir)?;
    for model in pack.models.values_mut() {
        model.source_sha256 = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(model).map_err(|e| e.to_string())?)
        );
    }
    pack.validate()?;
    std::fs::write(
        dir.join("pack.json"),
        serde_json::to_vec_pretty(&pack).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(pack)
}
/// Builds original UI art and uses the bundled OFL font.
pub fn atlas() -> Result<Atlas, String> {
    let font = include_bytes!("../../assets/FiraMono-Medium.ttf");
    let mut atlas = Atlas::from_font(font, 18.)?;
    for (name, size) in [
        ("small", 10.),
        ("combat", 28.),
        ("hotkey", 12.),
        ("numbers", 14.),
    ] {
        atlas.add_font(name, font, size)?;
    }
    for (i, name) in [
        "unit-frame",
        "elite-frame",
        "unit-name",
        "unit-skull",
        "status-bar",
        "nameplate-border",
        "action-frame",
        "bow-icon",
        "fire-bolt-icon",
        "magic-missile-icon",
        "fireball-icon",
        "misty-step-icon",
        "thunderwave-icon",
        "web-icon",
        "grease-icon",
        "light-icon",
        "shield-icon",
        "portrait-adventurer",
        "portrait-claude",
        "portrait-cultist",
    ]
    .into_iter()
    .enumerate()
    {
        let mut pixels = vec![];
        for y in 0..64 {
            for x in 0..64 {
                let border = match name {
                    "action-frame" => {
                        (14..=49).contains(&x)
                            && (14..=49).contains(&y)
                            && (x < 17 || x > 46 || y < 17 || y > 46)
                    }
                    "nameplate-border" => {
                        (1..=55).contains(&x)
                            && (36..=58).contains(&y)
                            && (x < 3 || x > 53 || y < 39 || y > 55)
                    }
                    "unit-frame" | "elite-frame" => false,
                    _ => x < 3 || y < 3 || x > 60 || y > 60,
                };
                let icon = name.ends_with("icon");
                let mark = icon
                    && (((x as f32 - 32.).hypot(y as f32 - 32.) - 18.).abs() < 2.
                        || (x as i32 - y as i32).abs() < 3);
                let color = if border {
                    [140, 110, 65, 255]
                } else if mark {
                    [
                        (80 + i * 47 % 175) as u8,
                        (90 + i * 71 % 160) as u8,
                        (90 + i * 29 % 160) as u8,
                        255,
                    ]
                } else if name == "status-bar" {
                    [255; 4]
                } else if name == "nameplate-border"
                    || name == "action-frame"
                    || name == "unit-frame"
                    || name == "elite-frame"
                {
                    [0; 4]
                } else {
                    [18, 21, 29, 255]
                };
                pixels.extend(color);
            }
        }
        atlas.add_sprite(name, 64, 64, &pixels)?;
    }
    Ok(atlas)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_pack_uses_only_generated_files_and_animated_actors() {
        let dir = std::env::temp_dir().join(format!("verse-original-{}", std::process::id()));
        let pack = generate(&dir).unwrap();
        for texture in &pack.textures {
            let bytes = std::fs::read(dir.join(&texture.file)).unwrap();
            assert_eq!(texture.sha256, format!("{:x}", Sha256::digest(bytes)));
        }
        assert_eq!(
            Pack::read(&dir.join("pack.json")).unwrap().source_revision,
            pack.source_revision
        );
        assert!(pack.models.values().all(|m| m.source.starts_with("verse/")));
        for name in ["claude", "cultist", "adventurer"] {
            let m = &pack.models[name];
            assert!(m.clips.iter().any(|c| c.id == 1));
            assert_ne!(
                verse_engine::animation::pose(m, 4, 0.),
                verse_engine::animation::pose(m, 4, 0.9)
            );
        }
        atlas().unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn original_timeline_runs_the_full_kit_and_actual_defeat() {
        let scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        assert_eq!(scene.origin_wow, [0.; 3]);
        let mut game = super::super::play::Game::combat(scene, true).unwrap();
        for _ in 0..2400 {
            game.tick(1. / 30., [0.; 2]).unwrap();
        }
        let encounter = game.encounter.as_ref().unwrap();
        assert_eq!(encounter.boss_max, 300_000);
        assert!(encounter.boss_remaining > 0 && encounter.boss_remaining < encounter.boss_max);
        assert_eq!(game.snapshot().player.hp, 0);
        assert!(encounter.enemy_casts > 0 && encounter.absorbed > 0);
        assert_eq!(encounter.used.len(), 10);
    }
}
