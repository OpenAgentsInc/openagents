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
            actor: Some(a.actor.id),
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
                actor: None,
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
            actor: None,
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
        "shield-icon",
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
            actor: None,
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
fn particle_quad() -> (Vec<verse_wow::assets::Vertex>, Vec<u32>) {
    use verse_wow::assets::Vertex;
    let vertices = [
        ([-1.0, -1.0, 0.0], [0.0, 1.0]),
        ([1.0, -1.0, 0.0], [1.0, 1.0]),
        ([1.0, 1.0, 0.0], [1.0, 0.0]),
        ([-1.0, 1.0, 0.0], [0.0, 0.0]),
    ]
    .into_iter()
    .map(|(position, uv)| Vertex {
        position,
        normal: [0.0, 0.0, 1.0],
        uv,
        joints: [0; 4],
        weights: [1.0, 0.0, 0.0, 0.0],
    })
    .collect();
    (vertices, vec![0, 1, 2, 0, 2, 3])
}
fn particle_texture(pack: &mut Pack, dir: &std::path::Path, name: &str) -> Result<usize, String> {
    use sha2::{Digest, Sha256};
    let file = format!("{name}.png");
    if let Some(i) = pack.textures.iter().position(|t| t.file == file) {
        return Ok(i);
    }
    let bytes = std::fs::read(dir.join(&file))
        .map_err(|e| format!("Import Classic particle textures with wow-import --ui-only: {e}"))?;
    let reader = png::Decoder::new(std::io::Cursor::new(&bytes))
        .read_info()
        .map_err(|e| e.to_string())?;
    let i = pack.textures.len();
    pack.textures.push(verse_wow::assets::Texture {
        file,
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        width: reader.info().width,
        height: reader.info().height,
    });
    Ok(i)
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
        ("effect-shadow", [0.7, 0.05, 1.0]),
        ("effect-light", [1.0, 0.85, 0.45]),
    ] {
        let sprite = match name {
            "effect-fire" | "effect-impact" => Some("particle-fire"),
            "effect-force" | "effect-light" => Some("particle-arcane"),
            "effect-mist" => Some("particle-smoke"),
            "effect-web" => Some("particle-web"),
            "effect-grease" => Some("particle-smoke"),
            "effect-shadow" => Some("particle-shadow"),
            _ => None,
        };
        let texture = if let Some(sprite) = sprite {
            particle_texture(pack, dir, sprite)?
        } else {
            texture
        };
        let (mesh, triangles) = if sprite.is_some() {
            particle_quad()
        } else {
            (vertices.clone(), indices.clone())
        };
        pack.models.insert(
            name.into(),
            Model {
                source: format!(
                    "verse/{}/{name}",
                    if ["effect-web", "effect-grease"].contains(&name) {
                        "ground"
                    } else if sprite.is_some() {
                        "particles"
                    } else {
                        "procedural"
                    }
                ),
                source_sha256: format!("{:x}", Sha256::digest(name.as_bytes())),
                height: 2.0,
                bones: vec![],
                clips: vec![],
                attachments: vec![],
                surfaces: vec![Surface {
                    vertices: mesh,
                    indices: triangles,
                    texture,
                    blend: if ["effect-grease", "effect-web", "effect-mist"].contains(&name) {
                        2
                    } else {
                        3
                    },
                    emissive: true,
                    tint: if name == "effect-grease" {
                        [0.1, 0.075, 0.04]
                    } else if sprite.is_some() {
                        [1.0; 3]
                    } else {
                        color
                    },
                }],
            },
        );
    }
    for (name, image, blend, tint) in [
        ("particle-smoke", "particle-smoke", 2, [0.28, 0.22, 0.2]),
        ("particle-spark", "particle-spark", 3, [1.0, 0.65, 0.2]),
    ] {
        let texture = particle_texture(pack, dir, image)?;
        let (vertices, indices) = particle_quad();
        pack.models.insert(
            name.into(),
            Model {
                source: format!("verse/particles/{name}"),
                source_sha256: format!("{:x}", Sha256::digest(name.as_bytes())),
                height: 2.0,
                bones: vec![],
                clips: vec![],
                attachments: vec![],
                surfaces: vec![Surface {
                    vertices,
                    indices,
                    texture,
                    blend,
                    emissive: true,
                    tint,
                }],
            },
        );
    }
    let texture = particle_texture(pack, dir, "particle-ribbon")?;
    let (mesh, triangles) = particle_quad();
    pack.models.insert(
        "effect-ribbon".into(),
        Model {
            source: "verse/ribbon/effect-ribbon".into(),
            source_sha256: format!("{:x}", Sha256::digest(b"effect-ribbon")),
            height: 2.0,
            bones: vec![],
            clips: vec![],
            attachments: vec![],
            surfaces: vec![Surface {
                vertices: mesh,
                indices: triangles,
                texture,
                blend: 3,
                emissive: true,
                tint: [1.0; 3],
            }],
        },
    );
    // Thin luminous rings keep the shield transparent around the character.
    for (name, planes, color) in [
        ("effect-rune", 1, [0.8, 0.04, 0.65]),
        ("effect-shield", 3, [0.08, 0.6, 1.0]),
    ] {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        for plane in 0..planes {
            let base = vertices.len() as u32;
            for segment in 0..=64 {
                let angle = segment as f32 / 64.0 * std::f32::consts::TAU;
                for radius in [0.98, 1.0] {
                    let (x, z) = (angle.cos() * radius, angle.sin() * radius);
                    let p = match plane {
                        0 => Vec3::new(x, 0.0, z),
                        1 => Vec3::new(x, z, 0.0),
                        _ => Vec3::new(0.0, x, z),
                    };
                    vertices.push(Vertex {
                        position: p.into(),
                        normal: Vec3::Y.into(),
                        uv: [0.5, 0.5],
                        joints: [0; 4],
                        weights: [1.0, 0.0, 0.0, 0.0],
                    });
                }
            }
            for segment in 0..64 {
                let i = base + segment * 2;
                indices.extend_from_slice(&[i, i + 1, i + 2, i + 1, i + 3, i + 2]);
            }
        }
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
                    vertices,
                    indices,
                    texture,
                    blend: 3,
                    emissive: true,
                    tint: color,
                }],
            },
        );
    }
    let texture = particle_texture(pack, dir, "particle-rune")?;
    let (quad, triangles) = particle_quad();
    pack.models.get_mut("effect-rune").unwrap().surfaces[0] = Surface {
        vertices: quad,
        indices: triangles,
        texture,
        blend: 3,
        emissive: true,
        tint: [0.65, 0.15, 0.85],
    };
    pack.models.get_mut("effect-rune").unwrap().source = "verse/ground/effect-rune".into();
    let mut wave = pack.models["effect-rune"].clone();
    wave.source = "verse/ground/effect-wave".into();
    wave.surfaces[0].tint = [0.2, 0.55, 1.0];
    pack.models.insert("effect-wave".into(), wave);
    let shell = &mut pack.models.get_mut("effect-shield").unwrap().surfaces[0];
    shell.vertices = vertices;
    shell.indices = indices;
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
            actor: None,
            model: if force { "effect-force" } else { "effect-fire" }.into(),
            transform: Mat4::from_translation(p.pos.into()) * Mat4::from_scale(Vec3::splat(scale)),
            animation: 0,
            time: game.time,
            emission: Vec3::ONE,
        });
        for trail in 1..4 {
            out.push(Instance {
                actor: None,
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
                "effect-wave",
                Vec3::new((0.7 - left) * 7.0 + 0.2, 0.5, (0.7 - left) * 7.0 + 0.2),
                left / 0.7,
            ),
            Utility::Web => ("effect-web", Vec3::new(3.048, 0.08, 3.048), 0.5),
            Utility::Grease => ("effect-grease", Vec3::new(1.524, 0.045, 1.524), 0.8),
            Utility::Light | Utility::Shield => continue,
        };
        out.push(Instance {
            actor: None,
            model: model.into(),
            transform: Mat4::from_translation(area.position + Vec3::Y * 0.12)
                * if matches!(
                    area.kind,
                    Utility::Web | Utility::Grease | Utility::Thunderwave
                ) {
                    Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2)
                        * Mat4::from_scale(Vec3::new(scale.x, scale.z, 1.0))
                } else {
                    Mat4::from_scale(scale)
                },
            animation: 0,
            time: game.time,
            emission: Vec3::splat(alpha),
        });
    }
    if let Some(position) = game.controls.light {
        out.push(Instance {
            actor: None,
            model: "effect-light".into(),
            transform: Mat4::from_translation(position) * Mat4::from_scale(Vec3::splat(0.12)),
            animation: 0,
            time: game.time,
            emission: Vec3::ONE,
        });
    }
    if game.controls.shield > 0 && game.time < game.controls.shield_until {
        out.push(Instance {
            actor: None,
            model: "effect-shield".into(),
            transform: Mat4::from_translation(game.player + Vec3::Y * 1.05)
                * Mat4::from_rotation_y(game.time * 0.7)
                * Mat4::from_scale(Vec3::new(1.1, 1.25, 1.1)),
            animation: 0,
            time: game.time,
            emission: Vec3::splat(0.8),
        });
    }
    if game.controls.shield > 0 && game.time < game.controls.shield_until {
        for n in 0..6 {
            let angle = game.time * 1.3 + n as f32 * std::f32::consts::TAU / 6.0;
            let center = game.player
                + Vec3::new(
                    angle.cos() * 1.05,
                    1.05 + (angle * 2.0).sin() * 0.8,
                    angle.sin() * 1.05,
                );
            out.push(particle("effect-force", center, 0.18, 0.5, game.time));
        }
    }
    if let Some(encounter) = &game.encounter {
        for cast in &encounter.casts {
            out.push(Instance {
                actor: None,
                model: "effect-rune".into(),
                transform: Mat4::from_translation(cast.target + Vec3::Y * 0.08)
                    * Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2)
                    * Mat4::from_scale(Vec3::splat(cast.radius)),
                animation: 0,
                time: game.time,
                emission: Vec3::splat(0.6),
            });
            let position = if game.time < cast.release {
                cast.origin
            } else {
                cast.origin.lerp(
                    cast.target + Vec3::Y,
                    ((game.time - cast.release) / (cast.impact - cast.release)).clamp(0.0, 1.0),
                )
            };
            let size = if cast.boss { 0.55 } else { 0.23 };
            out.push(Instance {
                actor: None,
                model: "effect-shadow".into(),
                transform: Mat4::from_translation(position) * Mat4::from_scale(Vec3::splat(size)),
                animation: 0,
                time: game.time,
                emission: Vec3::ONE,
            });
            if game.time >= cast.release {
                let direction = (cast.target + Vec3::Y - cast.origin).normalize_or_zero();
                out.push(Instance {
                    actor: None,
                    model: "effect-ribbon".into(),
                    transform: Mat4::from_translation(position - direction * 0.65)
                        * Mat4::from_quat(Quat::from_rotation_arc(Vec3::Y, direction))
                        * Mat4::from_scale(Vec3::new(size * 0.7, 0.85, 1.0)),
                    animation: 0,
                    time: game.time,
                    emission: Vec3::splat(0.75),
                });
            }
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
            actor: None,
            model: "effect-impact".into(),
            transform: Mat4::from_translation(*position) * Mat4::from_scale(Vec3::splat(scale)),
            animation: 0,
            time: game.time,
            emission: Vec3::splat((1.0 - elapsed / 0.6).max(0.0)),
        });
    }
    for p in &game.snapshot().projectiles {
        if p.kind == verse_ruins::Spell::MagicMissile {
            continue;
        }
        let direction = Vec3::from(p.vel).normalize_or_zero();
        for n in 0..8 {
            let age = (n as f32 + 0.5) * 0.045;
            let phase = game.time * 8.0 + n as f32 * 2.4 + p.id as f32;
            let center = Vec3::from(p.pos) - direction * Vec3::from(p.vel).length() * age
                + Vec3::new(phase.sin(), age * 3.0, phase.cos()) * age * 0.4;
            out.push(particle(
                "effect-fire",
                center,
                0.12 + age * 0.25,
                1.0 - age / 0.5,
                game.time,
            ));
            if n % 2 == 0 {
                out.push(particle(
                    "particle-smoke",
                    center + Vec3::Y * age,
                    0.2 + age,
                    age * 0.65,
                    game.time,
                ));
            }
        }
    }
    for (position, at, kind) in &game.impacts {
        let age = game.time - at;
        for n in 0..if *kind == 1 { 16 } else { 6 } {
            let seed = *at * 7.31 + position.dot(Vec3::new(0.17, 0.31, 0.73)) + n as f32 * 2.39996;
            let direction =
                Vec3::new(seed.cos(), 0.2 + (seed * 1.7).sin().abs(), seed.sin()).normalize();
            let speed = if *kind == 1 { 6.0 } else { 2.0 };
            let center = *position + direction * speed * age - Vec3::Y * age * age * 3.0;
            out.push(particle(
                "particle-spark",
                center,
                0.05 + 0.08 * (1.0 - age / 0.6),
                (1.0 - age / 0.6).max(0.0),
                game.time,
            ));
        }
    }
    // The actor palette has a fixed budget; retain primary spell and area cues first.
    out.truncate(220);
    out
}
fn particle(model: &str, position: Vec3, radius: f32, opacity: f32, time: f32) -> Instance {
    Instance {
        actor: None,
        model: model.into(),
        transform: Mat4::from_translation(position) * Mat4::from_scale(Vec3::splat(radius)),
        animation: 0,
        time,
        emission: Vec3::splat(opacity.clamp(0.0, 1.0)),
    }
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
                actor: None,
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn simultaneous_impacts_keep_a_finite_deterministic_particle_budget() {
        let scene = verse_wow::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/wow/anthropic.json"
        ))
        .unwrap();
        let mut game = super::super::play::Game::new(scene).unwrap();
        game.time = 30.0;
        game.impacts = (0..100)
            .map(|n| (Vec3::new(n as f32 * 0.01, 1.0, 0.0), 29.8, 1))
            .collect();
        let first = spell_instances(&game);
        let second = spell_instances(&game);
        assert!(first.len() <= 220);
        assert!(
            first
                .iter()
                .all(|i| i.transform.is_finite() && i.emission.is_finite())
        );
        assert!(first.iter().any(|i| i.model == "particle-spark"));
        assert!(
            first
                .iter()
                .zip(second)
                .all(|(a, b)| a.model == b.model && a.transform == b.transform)
        );
    }
}
