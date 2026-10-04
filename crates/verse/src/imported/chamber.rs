//! Shared imported chamber assets and presentation for recording and play.
use super::Instance;
use crate::ui::Atlas;
use glam::{Mat4, Quat, Vec3};
use verse_engine::{assets::Pack, source_position as position_from_wow};
pub fn basis() -> Mat4 {
    Mat4::from_cols_array(&[
        0.0, 0.0, -0.9144, 0.0, -0.9144, 0.0, 0.0, 0.0, 0.0, 0.9144, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ])
}

pub fn instances(
    pack: &Pack,
    frame: &verse_engine::director::Frame,
) -> Result<Vec<Instance>, String> {
    let mut actors: Vec<_> = frame
        .actors
        .iter()
        .filter(|a| a.visible)
        .map(|a| Instance {
            actor: Some(a.life.unwrap_or(verse_engine::core::LifeId {
                instance: 0,
                actor: a.actor.id,
                generation: 0,
            })),
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
        .filter(|a| a.visible && a.actor.model == "adventurer" && !a.animation.casting())
    {
        let model = pack
            .models
            .get("adventurer")
            .ok_or("Missing adventurer model")?;
        if let Some(hand) = model.attachments.iter().find(|a| a.id == 2) {
            let pose =
                verse_engine::animation::pose_selected(model, a.animation, a.animation_time)?;
            let transform = Mat4::from_translation(a.actor.position)
                * Mat4::from_rotation_y(a.actor.yaw)
                * Mat4::from_scale(Vec3::splat(a.actor.scale))
                * basis()
                * pose[hand.bone]
                * Mat4::from_translation(hand.position.into());
            actors.push(Instance {
                actor: None,
                model: "bow".into(),
                transform,
                animation: 0.into(),
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
            animation: 0.into(),
            time: frame.time,
            emission: Vec3::ONE,
        });
    }
    Ok(actors)
}

/// Projects admitted world props using the same bounds as collision and routing.
pub fn blocker_instances(pack: &Pack, game: &super::play::Game) -> Vec<Instance> {
    if !pack.models.contains_key("navigation-blocker") {
        return vec![];
    }
    game.navigation_blockers()
        .active_bounds()
        .filter(|(life, _, _)| {
            !(pack.models.contains_key("prop/Table_Large")
                && (10_000..10_256).contains(&life.entity))
                && !game.physics_bodies().get(*life).is_some_and(|body| {
                    matches!(body.phase, physics::lifetimes::Phase::Corpse { .. })
                })
        })
        .map(|(_, min, max)| Instance {
            actor: None,
            model: "navigation-blocker".into(),
            transform: Mat4::from_translation(((min + max) * 0.5).as_vec3())
                * Mat4::from_scale((max - min).as_vec3() / 0.9144)
                * basis(),
            animation: 0.into(),
            time: game.time,
            emission: Vec3::ONE,
        })
        .collect()
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
    atlas.add_font("combat", &font, 28.0)?;
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
            animation: 0.into(),
            time: 0.0,
            emission: if matches!(
                p.model.as_str(),
                "prop/flame" | "prop/ritual-liquid" | "prop/summoning-seal"
            ) {
                Vec3::splat(4.)
            } else if pack.models[&p.model]
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
    lights.ambient = Vec3::new(0.009, 0.012, 0.017);
    lights.exposure = 1.1;
    lights.fog = Vec3::new(0.0015, 0.003, 0.004);
    lights.density = 0.012;
    // Place the light outside each vessel so its housing does not hide the spill.
    for (p, c, intensity, range) in [
        ([-7.5, 124.2, 88.0], [0.1, 1.0, 0.035], 155.0, 14.0),
        ([-7.5, 160.7, 89.0], [0.1, 1.0, 0.035], 155.0, 14.0),
        ([-26.66, 138.575, 86.4], [0.13, 0.85, 0.06], 65.0, 10.0),
        ([-26.5, 144.56, 86.4], [0.13, 0.85, 0.06], 65.0, 10.0),
        ([19.066, 133.143, 86.4], [1.0, 0.3, 0.04], 75.0, 10.0),
        ([18.752, 151.100, 86.4], [1.0, 0.3, 0.04], 75.0, 10.0),
    ] {
        lights.lights.push(super::lighting::Light {
            position: position_from_wow(p) - origin,
            color: c.into(),
            intensity,
            range,
        });
    }
    if origin == Vec3::ZERO {
        lights.lights.clear();
        lights.ambient = Vec3::new(0.021, 0.018, 0.025);
        lights.exposure = 1.1;
        lights.density = 0.008;
        // Warm visible flames lead into the cooler summoning circle.
        for z in [-12., 0., -24., 11.] {
            for x in [-14., 14.] {
                lights.lights.push(super::lighting::Light {
                    position: Vec3::new(x, 4.3, z),
                    color: Vec3::new(1., 0.54, 0.19),
                    intensity: 90.,
                    range: 20.,
                });
            }
        }
        for x in [-9., 9.] {
            lights.lights.push(super::lighting::Light {
                position: Vec3::new(x, 2.7, -6.7),
                color: Vec3::new(1., 0.65, 0.3),
                intensity: 45.,
                range: 12.,
            });
            lights.lights.push(super::lighting::Light {
                position: Vec3::new(x, 1.8, 5.),
                color: Vec3::new(0.2, 1., 0.32),
                intensity: 65.,
                range: 10.,
            });
            lights.lights.push(super::lighting::Light {
                position: Vec3::new(x * 0.7, 2.5, 2.5),
                color: Vec3::new(1., 0.65, 0.3),
                intensity: 35.,
                range: 10.,
            });
        }
        lights.lights.push(super::lighting::Light {
            position: Vec3::new(0., 6.8, -10.),
            color: Vec3::new(1., 0.7, 0.35),
            intensity: 95.,
            range: 20.,
        });
        lights.lights.push(super::lighting::Light {
            position: Vec3::new(0., 4., -1.),
            color: Vec3::new(0.55, 0.13, 1.),
            intensity: 120.,
            range: 14.,
        });
    }
    lights
}
/// Bind transient illumination to the same combat events that draw the effects.
pub fn combat_lighting(game: &super::play::Game) -> super::lighting::Lighting {
    use super::lighting::{Light, MAX_LIGHTS};
    use verse_world::utilities::Utility;
    let mut lighting = lighting(position_from_wow(game.scene.origin_wow));
    lighting.time = game.time;
    let mut effects = Vec::new();
    if let Some(position) = game.controls.light {
        effects.push(Light {
            position,
            color: Vec3::new(1.0, 0.8, 0.4),
            intensity: 55.0,
            range: 10.0,
        });
    }
    for p in game.snapshot().projectiles {
        if p.kind == verse_world::rules::ProjectileKind::Bow {
            continue;
        }
        let (color, intensity, range) = match p.kind {
            verse_world::rules::ProjectileKind::Fireball => {
                (Vec3::new(1.0, 0.23, 0.025), 260.0, 13.0)
            }
            verse_world::rules::ProjectileKind::MagicMissile => {
                (Vec3::new(0.2, 0.3, 1.0), 42.0, 6.0)
            }
            _ => (Vec3::new(1.0, 0.3, 0.04), 95.0, 8.0),
        };
        effects.push(Light {
            position: p.pos.into(),
            color,
            intensity,
            range,
        });
    }
    for (position, at, kind) in &game.impacts {
        if *kind == 3 {
            continue;
        }
        let age = game.time - at;
        if !(0.0..0.6).contains(&age) {
            continue;
        }
        let fade = (1.0 - age / 0.6).powi(2);
        effects.push(Light {
            position: *position + Vec3::Y * 0.35,
            color: match *kind {
                2 => Vec3::new(0.2, 0.3, 1.0),
                3 => Vec3::new(0.65, 0.04, 1.0),
                _ => Vec3::new(1.0, 0.32, 0.055),
            },
            intensity: if *kind == 1 {
                850.0 * fade
            } else {
                110.0 * fade
            },
            range: if *kind == 1 { 15.0 } else { 7.0 },
        });
    }
    if game.controls.shield > 0 && game.time < game.controls.shield_until {
        effects.push(Light {
            position: game.player + Vec3::Y * 1.3,
            color: Vec3::new(0.08, 0.5, 1.0),
            intensity: 22.0,
            range: 4.5,
        });
    }
    for area in &game.controls.areas {
        let left = area.until - game.time;
        if left <= 0.0 {
            continue;
        }
        let (intensity, range) = match area.kind {
            Utility::Thunderwave => (115.0 * (left / 0.7).clamp(0.0, 1.0).powi(2), 9.0),
            Utility::MistyStep => (30.0 * (left / 0.7).clamp(0.0, 1.0), 4.5),
            _ => continue,
        };
        effects.push(Light {
            position: area.position + Vec3::Y,
            color: Vec3::new(0.18, 0.45, 1.0),
            intensity,
            range,
        });
    }
    if let Some(encounter) = &game.encounter {
        for cast in &encounter.casts {
            let position = if game.time < cast.release {
                cast.origin
            } else {
                cast.position.unwrap_or(cast.origin)
            };
            let charge =
                ((game.time - cast.started) / (cast.release - cast.started)).clamp(0.0, 1.0);
            effects.push(Light {
                position,
                color: Vec3::new(0.65, 0.04, 1.0),
                intensity: if cast.boss { 100.0 } else { 35.0 } * (0.25 + 0.75 * charge),
                range: if cast.boss { 9.0 } else { 5.0 },
            });
        }
    }
    // Reserve two shadow slots for the brightest nearby effects; keep fixture light stable.
    let priority = |l: &Light| l.intensity / (1.0 + l.position.distance_squared(game.player));
    effects.sort_by(|a, b| priority(b).total_cmp(&priority(a)));
    effects.truncate(MAX_LIGHTS - lighting.lights.len());
    let shadowed_effects = effects.len().min(2);
    lighting
        .lights
        .splice(2..2, effects.drain(..shadowed_effects));
    lighting.lights.extend(effects);
    lighting
}
fn particle_quad() -> (Vec<verse_engine::assets::Vertex>, Vec<u32>) {
    use verse_engine::assets::Vertex;
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
        .map_err(|e| format!("Missing particle texture {file}: {e}"))?;
    let reader = png::Decoder::new(std::io::Cursor::new(&bytes))
        .read_info()
        .map_err(|e| e.to_string())?;
    let i = pack.textures.len();
    pack.textures.push(verse_engine::assets::Texture {
        file,
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        width: reader.info().width,
        height: reader.info().height,
    });
    Ok(i)
}
pub fn add_effect_models(pack: &mut Pack, dir: &std::path::Path) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    use verse_engine::assets::{Model, Surface, Texture, Vertex};
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
                states: Default::default(),
                skin: None,
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
                states: Default::default(),
                skin: None,
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
            states: Default::default(),
            skin: None,
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
                states: Default::default(),
                skin: None,
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
        if p.kind == verse_world::rules::ProjectileKind::Bow {
            continue;
        }
        let force = p.kind == verse_world::rules::ProjectileKind::MagicMissile;
        let scale = if p.kind == verse_world::rules::ProjectileKind::Fireball {
            0.32
        } else {
            0.15
        };
        out.push(Instance {
            actor: None,
            model: if force { "effect-force" } else { "effect-fire" }.into(),
            transform: Mat4::from_translation(p.pos.into()) * Mat4::from_scale(Vec3::splat(scale)),
            animation: 0.into(),
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
                animation: 0.into(),
                time: game.time,
                emission: Vec3::ONE,
            });
        }
    }
    use verse_world::utilities::Utility;
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
            animation: 0.into(),
            time: game.time,
            emission: Vec3::splat(alpha),
        });
    }
    if let Some(position) = game.controls.light {
        out.push(Instance {
            actor: None,
            model: "effect-light".into(),
            transform: Mat4::from_translation(position) * Mat4::from_scale(Vec3::splat(0.12)),
            animation: 0.into(),
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
            animation: 0.into(),
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
                animation: 0.into(),
                time: game.time,
                emission: Vec3::splat(0.6),
            });
            let position = if game.time < cast.release {
                cast.origin
            } else {
                cast.position.unwrap_or(cast.origin)
            };
            let size = if cast.boss { 0.55 } else { 0.23 };
            out.push(Instance {
                actor: None,
                model: "effect-shadow".into(),
                transform: Mat4::from_translation(position) * Mat4::from_scale(Vec3::splat(size)),
                animation: 0.into(),
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
                    animation: 0.into(),
                    time: game.time,
                    emission: Vec3::splat(0.75),
                });
            }
        }
    }
    for (position, at, kind) in &game.impacts {
        if *kind == 3 {
            continue;
        }
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
            animation: 0.into(),
            time: game.time,
            emission: Vec3::splat((1.0 - elapsed / 0.6).max(0.0)),
        });
    }
    for p in &game.snapshot().projectiles {
        if matches!(
            p.kind,
            verse_world::rules::ProjectileKind::MagicMissile
                | verse_world::rules::ProjectileKind::Bow
        ) {
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
        if *kind == 3 {
            continue;
        }
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
        animation: 0.into(),
        time,
        emission: Vec3::splat(opacity.clamp(0.0, 1.0)),
    }
}

/// Renders model headshots through the owned GPU pipeline for unit-frame portraits.
pub fn portrait_atlas(dir: &std::path::Path, pack: &Pack) -> Result<Atlas, String> {
    let atlas = classic_atlas(dir)?;
    portraits(dir, pack, atlas)
}
/// Renders portraits from original geometry and UI graphics.
pub fn original_portrait_atlas(dir: &std::path::Path, pack: &Pack) -> Result<Atlas, String> {
    portraits(dir, pack, super::original::atlas()?)
}
fn portraits(dir: &std::path::Path, pack: &Pack, mut atlas: Atlas) -> Result<Atlas, String> {
    use super::{Renderer, lighting::Lighting};
    use crate::{render::View, ui::UiBatch};
    let mut renderer = Renderer::new(pack.clone(), dir, 128, 128, &atlas, &[])?;
    for name in [
        "adventurer",
        "cultist",
        "cultist-female",
        "cultist-peasant",
        "cultist-peasant-female",
        "claude",
    ] {
        if !pack.models.contains_key(name) {
            continue;
        }
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
                animation: 0.into(),
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
    fn explosions_light_the_room_then_fade_with_the_effect() {
        let scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/wow/anthropic.json"
        ))
        .unwrap();
        let mut game = super::super::play::Game::new(scene).unwrap();
        game.time = 30.0;
        game.impacts.push((game.player, 30.0, 1));
        let flash = combat_lighting(&game);
        assert!(flash.ambient.max_element() < 0.02);
        assert!(flash.lights[2].intensity > 800.0);
        assert!(flash.lights[2].color.x > flash.lights[2].color.y);
        game.time = 30.5;
        let fading = combat_lighting(&game);
        assert!(fading.lights[2].intensity < flash.lights[2].intensity * 0.05);
        game.time = 30.7;
        assert_eq!(combat_lighting(&game).lights.len(), 6);
    }
    #[test]
    fn many_effects_keep_the_brightest_local_lights_and_respect_the_gpu_bound() {
        let scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/wow/anthropic.json"
        ))
        .unwrap();
        let mut game = super::super::play::Game::new(scene).unwrap();
        game.time = 30.0;
        game.impacts = (0..100)
            .map(|n| (game.player + Vec3::X * n as f32, 30.0, 1))
            .collect();
        let lights = combat_lighting(&game);
        assert_eq!(lights.lights.len(), super::super::lighting::MAX_LIGHTS);
        assert_eq!(lights.lights[2].position, game.player + Vec3::Y * 0.35);
        assert!(
            lights
                .lights
                .iter()
                .all(|l| l.position.is_finite() && l.intensity.is_finite())
        );
    }
    #[test]
    fn simultaneous_impacts_keep_a_finite_deterministic_particle_budget() {
        let scene = verse_engine::director::Scene::from_json(include_bytes!(
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
