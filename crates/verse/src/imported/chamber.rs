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
        "main-bar",
        "end-cap",
        "empty-slot",
        "unit-frame",
        "elite-frame",
        "unit-name",
        "unit-skull",
        "page-up",
        "page-down",
        "backpack",
        "bag-empty",
        "micro-character",
        "micro-spellbook",
        "micro-talents",
        "micro-quest",
        "micro-socials",
        "micro-world",
        "micro-mainmenu",
        "micro-help",
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
        atlas.add_sprite(name, info.width, info.height, &rgba[..info.buffer_size()])?;
    }
    atlas.add_font("small", &font, 10.0)?;
    let numbers = std::fs::read(dir.join("ARIALN.TTF")).map_err(|e| e.to_string())?;
    atlas.add_font("hotkey", &numbers, 12.0)?;
    atlas.add_font("numbers", &numbers, 14.0)?;
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
        ("effect-mist", [0.5, 0.8, 1.0]),
        ("effect-web", [0.7, 0.8, 0.9]),
        ("effect-grease", [0.15, 0.12, 0.07]),
        ("effect-light", [1.0, 0.85, 0.45]),
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
                    blend: if name == "effect-grease" { 2 } else { 3 },
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
    use verse_ruins::chamber_spells::Utility;
    for area in game.controls.areas.iter().filter(|a| a.until > game.time) {
        let left = area.until - game.time;
        let (model, scale, alpha) = match area.kind {
            Utility::MistyStep => ("effect-mist", Vec3::new(0.9, 1.7, 0.9), left / 0.7),
            Utility::Thunderwave => (
                "effect-force",
                Vec3::new((0.7 - left) * 7.0 + 0.2, 0.5, (0.7 - left) * 7.0 + 0.2),
                left / 0.7,
            ),
            Utility::Web => ("effect-web", Vec3::new(3.048, 0.08, 3.048), 0.5),
            Utility::Grease => ("effect-grease", Vec3::new(1.524, 0.045, 1.524), 0.8),
            Utility::Light => continue,
        };
        out.push(Instance {
            model: model.into(),
            transform: Mat4::from_translation(area.position + Vec3::Y * 0.12)
                * Mat4::from_scale(scale),
            animation: 0,
            time: game.time,
            emission: Vec3::splat(alpha),
        });
        if area.kind == Utility::Web {
            out.pop();
            let point = |radius: f32, i: usize| {
                let angle = i as f32 * std::f32::consts::TAU / 12.0;
                area.position
                    + Vec3::new(
                        angle.cos() * radius,
                        0.18 + 0.08 * (angle * 3.0).sin(),
                        angle.sin() * radius,
                    )
            };
            let mut strand = |a: Vec3, b: Vec3| {
                let delta = b - a;
                out.push(Instance {
                    model: "effect-web".into(),
                    transform: Mat4::from_translation((a + b) * 0.5)
                        * Mat4::from_quat(Quat::from_rotation_arc(Vec3::X, delta.normalize()))
                        * Mat4::from_scale(Vec3::new(delta.length() * 0.5, 0.017, 0.017)),
                    animation: 0,
                    time: game.time,
                    emission: Vec3::splat(0.7),
                });
            };
            for i in 0..12 {
                strand(area.position + Vec3::Y * 0.3, point(3.048, i));
                for radius in [0.6, 1.2, 1.8, 2.4, 3.048] {
                    strand(point(radius, i), point(radius, (i + 1) % 12));
                }
            }
        }
    }
    if let Some(position) = game.controls.light {
        out.push(Instance {
            model: "effect-light".into(),
            transform: Mat4::from_translation(position) * Mat4::from_scale(Vec3::splat(0.12)),
            animation: 0,
            time: game.time,
            emission: Vec3::ONE,
        });
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

/// Renders model headshots through the owned GPU pipeline for unit-frame portraits.
pub fn portrait_atlas(dir: &std::path::Path, pack: &Pack) -> Result<Atlas, String> {
    use super::{Renderer, lighting::Lighting};
    use crate::{render::View, ui::UiBatch};
    let mut atlas = classic_atlas(dir)?;
    let mut renderer = Renderer::new(pack.clone(), dir, 128, 128, &atlas, &[])?;
    for name in ["adventurer", "cultist", "claude"] {
        let height = pack.models[name].height * 0.9144;
        let target = Vec3::Y * height * if name == "claude" { 0.82 } else { 0.92 };
        let eye = target - Vec3::Z * height * 0.4;
        let view = View {
            view_proj: Mat4::perspective_rh(35.0_f32.to_radians(), 1.0, 0.01, 30.0)
                * Mat4::look_at_rh(eye, target, Vec3::Y),
            eye,
        };
        let mut pixels = renderer.draw(
            view,
            &[Instance {
                model: name.into(),
                transform: basis(),
                animation: 0,
                time: 0.0,
                emission: Vec3::ONE,
            }],
            &UiBatch::default(),
            &Lighting {
                ambient: Vec3::splat(0.75),
                density: 0.0,
                shadowed: 0,
                fog: Vec3::splat(0.015),
                ..Lighting::default()
            },
        )?;
        for y in 0..128 {
            for x in 0..128 {
                let distance =
                    ((x as f32 + 0.5 - 64.0).powi(2) + (y as f32 + 0.5 - 64.0).powi(2)).sqrt();
                pixels[(y * 128 + x) * 4 + 3] = ((64.0 - distance).clamp(0.0, 1.0) * 255.0) as u8;
            }
        }
        atlas.add_sprite(&format!("portrait-{name}"), 128, 128, &pixels)?;
    }
    Ok(atlas)
}
