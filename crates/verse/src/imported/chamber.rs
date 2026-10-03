//! Shared imported chamber assets and presentation for recording and play.
use super::Instance;
use crate::ui::Atlas;
use glam::{Mat4, Quat, Vec3};
use verse_wow::{assets::Pack, position_from_wow};
pub fn basis() -> Mat4 {
    Mat4::from_cols_array(&[
        0.0, 0.0, -0.9144, 0.0, -0.9144, 0.0, 0.0, 0.0, 0.0, 0.9144, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ])
}

pub fn instances(pack: &Pack, frame: &verse_wow::director::Frame) -> Vec<Instance> {
    let mut actors: Vec<_> = frame
        .actors
        .iter()
        .filter(|a| a.visible)
        .map(|a| Instance {
            model: a.actor.model.clone(),
            transform: Mat4::from_translation(a.actor.position)
                * Mat4::from_rotation_y(a.actor.yaw)
                * Mat4::from_scale(Vec3::splat(a.actor.scale))
                * basis(),
            animation: a.animation,
            time: a.animation_time,
            emission: Vec3::ONE,
        })
        .collect();
    for a in frame
        .actors
        .iter()
        .filter(|a| a.visible && a.actor.model == "adventurer" && ![52, 53].contains(&a.animation))
    {
        let model = &pack.models["adventurer"];
        if let Some(hand) = model.attachments.iter().find(|a| a.id == 2) {
            let pose = verse_wow::animation::pose(model, a.animation, a.animation_time);
            let hand_position = (Mat4::from_translation(a.actor.position)
                * Mat4::from_rotation_y(a.actor.yaw)
                * basis()
                * pose[hand.bone])
                .transform_point3(hand.position.into());
            let transform = Mat4::from_translation(hand_position)
                * Mat4::from_rotation_y(a.actor.yaw)
                * basis()
                * Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2)
                * Mat4::from_rotation_y(-std::f32::consts::FRAC_PI_2);
            actors.push(Instance {
                model: "bow".into(),
                transform,
                animation: 0,
                time: frame.time,
                emission: Vec3::ONE,
            });
        }
    }
    for arrow in &frame.projectiles {
        actors.push(Instance {
            model: "arrow".into(),
            transform: Mat4::from_translation(arrow.position)
                * Mat4::from_quat(Quat::from_rotation_arc(-Vec3::Z, arrow.direction))
                * basis(),
            animation: 0,
            time: frame.time,
            emission: Vec3::ONE,
        });
    }
    actors
}

pub fn classic_atlas(dir: &std::path::Path) -> Result<Atlas, String> {
    let font = std::fs::read(dir.join("FRIZQT__.TTF"))
        .map_err(|e| format!("Import Classic UI assets with wow-import --ui-only: {e}"))?;
    let mut atlas = Atlas::from_font(&font, 18.0)?;
    for name in [
        "status-bar",
        "nameplate-border",
        "action-frame",
        "bow-icon",
        "fire-bolt-icon",
        "magic-missile-icon",
        "fireball-icon",
    ] {
        let decoder = png::Decoder::new(std::io::BufReader::new(
            std::fs::File::open(dir.join(format!("{name}.png"))).map_err(|e| e.to_string())?,
        ));
        let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
        let mut rgba = vec![0; reader.output_buffer_size().ok_or("Invalid UI image")?];
        let info = reader.next_frame(&mut rgba).map_err(|e| e.to_string())?;
        if info.color_type != png::ColorType::Rgba {
            return Err("Expected RGBA UI sprite".into());
        }
        if name == "nameplate-border" {
            if info.height != 32 || info.width != 128 {
                return Err("Unexpected Classic nameplate dimensions".into());
            }
            atlas.add_sprite(name, 128, 16, &rgba[128 * 16 * 4..128 * 32 * 4])?;
        } else {
            atlas.add_sprite(name, info.width, info.height, &rgba[..info.buffer_size()])?;
        }
    }
    Ok(atlas)
}

pub fn static_instances(pack: &Pack, origin: Vec3) -> Vec<Instance> {
    let conversion = Mat4::from_translation(-origin) * basis();
    pack.placements
        .iter()
        .map(|p| Instance {
            model: p.model.clone(),
            transform: conversion
                * Mat4::from_scale_rotation_translation(
                    Vec3::splat(p.scale),
                    Quat::from_array(p.rotation),
                    p.position.into(),
                ),
            animation: 0,
            time: 0.0,
            emission: if pack.models[&p.model]
                .source
                .ends_with("scholme_greencandelabra.m2")
            {
                Vec3::new(1.0, 0.38, 0.1)
            } else {
                Vec3::new(0.08, 1.0, 0.03)
            },
        })
        .collect()
}
pub fn lighting(origin: Vec3) -> super::lighting::Lighting {
    let mut lights = super::lighting::Lighting::default();
    lights.ambient = Vec3::new(0.055, 0.06, 0.075);
    lights.exposure = 1.35;
    for (p, c, intensity) in [
        ([-4.1, 124.2, 87.0], [1.0, 0.38, 0.1], 450.0),
        ([-4.1, 160.7, 88.0], [1.0, 0.38, 0.1], 450.0),
        ([-26.66, 138.575, 86.4], [0.18, 0.8, 0.12], 80.0),
        ([-26.5, 144.56, 86.4], [0.18, 0.8, 0.12], 80.0),
        ([19.066, 133.143, 86.4], [1.0, 0.38, 0.1], 250.0),
        ([18.752, 151.100, 86.4], [1.0, 0.38, 0.1], 250.0),
    ] {
        lights.lights.push(super::lighting::Light {
            position: position_from_wow(p) - origin,
            color: c.into(),
            intensity,
            range: 38.0,
        });
    }
    lights
}
pub fn add_effect_models(pack: &mut Pack, dir: &std::path::Path) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    use verse_wow::assets::{Model, Surface, Texture, Vertex};
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .map_err(|e| e.to_string())?
            .write_image_data(&[255; 4])
            .map_err(|e| e.to_string())?;
    }
    std::fs::write(dir.join("verse-effect-white.png"), &bytes).map_err(|e| e.to_string())?;
    let texture = pack.textures.len();
    pack.textures.push(Texture {
        file: "verse-effect-white.png".into(),
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        width: 1,
        height: 1,
    });
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for row in 0..=12 {
        for col in 0..=16 {
            let y = row as f32 / 12.0 * std::f32::consts::PI;
            let a = col as f32 / 16.0 * std::f32::consts::TAU;
            let p = Vec3::new(y.sin() * a.cos(), y.cos(), y.sin() * a.sin());
            vertices.push(Vertex {
                position: p.into(),
                normal: p.into(),
                uv: [0.5, 0.5],
                joints: [0; 4],
                weights: [1.0, 0.0, 0.0, 0.0],
            });
        }
    }
    for row in 0..12 {
        for col in 0..16 {
            let a = row * 17 + col;
            indices.extend_from_slice(&[a, a + 1, a + 17, a + 1, a + 18, a + 17]);
        }
    }
    for (name, color) in [
        ("effect-fire", [1.0, 0.15, 0.015]),
        ("effect-force", [0.18, 0.25, 1.0]),
        ("effect-impact", [1.0, 0.14, 0.015]),
    ] {
        pack.models.insert(
            name.into(),
            Model {
                source: format!("verse/procedural/{name}"),
                source_sha256: format!("{:x}", Sha256::digest(name.as_bytes())),
                height: 2.0,
                bones: vec![],
                clips: vec![],
                attachments: vec![],
                surfaces: vec![Surface {
                    vertices: vertices.clone(),
                    indices: indices.clone(),
                    texture,
                    blend: 3,
                    emissive: true,
                    tint: color,
                }],
            },
        );
    }
    pack.validate()
}
pub fn spell_instances(game: &super::play::Game) -> Vec<Instance> {
    let mut out = Vec::new();
    for p in game.snapshot().projectiles {
        let force = p.kind == verse_ruins::Spell::MagicMissile;
        let scale = if p.kind == verse_ruins::Spell::Fireball {
            0.32
        } else {
            0.15
        };
        out.push(Instance {
            model: if force { "effect-force" } else { "effect-fire" }.into(),
            transform: Mat4::from_translation(p.pos.into()) * Mat4::from_scale(Vec3::splat(scale)),
            animation: 0,
            time: game.time,
            emission: Vec3::ONE,
        });
        for trail in 1..4 {
            out.push(Instance {
                model: if force { "effect-force" } else { "effect-fire" }.into(),
                transform: Mat4::from_translation(
                    Vec3::from(p.pos) - Vec3::from(p.vel).normalize_or_zero() * trail as f32 * 0.2,
                ) * Mat4::from_scale(Vec3::splat(scale * (1.0 - trail as f32 * 0.2))),
                animation: 0,
                time: game.time,
                emission: Vec3::ONE,
            });
        }
    }
    for (position, at, kind) in &game.impacts {
        let elapsed = game.time - at;
        let scale = if *kind == 1 {
            0.6 + elapsed * 10.0
        } else {
            0.15 + elapsed * 1.8
        };
        out.push(Instance {
            model: "effect-impact".into(),
            transform: Mat4::from_translation(*position) * Mat4::from_scale(Vec3::splat(scale)),
            animation: 0,
            time: game.time,
            emission: Vec3::splat((1.0 - elapsed / 0.6).max(0.0)),
        });
    }
    out
}
