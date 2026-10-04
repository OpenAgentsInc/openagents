//! Shared imported chamber assets and presentation for recording and play.
use super::Instance;
use crate::ui::Atlas;
use glam::{Mat4, Quat, Vec3};
use verse_engine::{assets::Pack, source_position as position_from_wow};
/// Assembles admitted remote actors and effects without a local simulation.
#[cfg(feature = "remote-chamber")]
pub struct RemoteScene {
    pub frame: verse_engine::director::Frame,
    pub instances: Vec<Instance>,
    pub lighting: super::lighting::Lighting,
}
#[cfg(feature = "remote-chamber")]
pub fn remote_scene(
    pack: &Pack,
    view: &verse_world::service::view::View,
    alpha: f32,
    camera: verse_world::service::view::Camera,
    origin: Vec3,
    playground: bool,
    focus: Vec3,
) -> Result<Option<RemoteScene>, String> {
    remote_scene_predicted(pack, view, alpha, camera, origin, playground, focus, None)
}
#[cfg(feature = "remote-chamber")]
pub fn remote_scene_predicted(
    pack: &Pack,
    view: &verse_world::service::view::View,
    alpha: f32,
    camera: verse_world::service::view::Camera,
    origin: Vec3,
    playground: bool,
    focus: Vec3,
    predicted: Option<verse_world::prediction::Pose>,
) -> Result<Option<RemoteScene>, String> {
    if !origin.is_finite() || !focus.is_finite() {
        return Err("Invalid remote scene lighting position".into());
    }
    let Some(sample) = view.scene_sample_predicted(alpha, camera, predicted)? else {
        return Ok(None);
    };
    render_remote_sample(pack, sample, origin, playground, focus).map(Some)
}
#[cfg(feature = "remote-chamber")]
fn render_remote_sample(
    pack: &Pack,
    sample: verse_world::service::view::SceneSample,
    origin: Vec3,
    playground: bool,
    focus: Vec3,
) -> Result<RemoteScene, String> {
    let outfits: std::collections::BTreeMap<_, _> = sample
        .presentation
        .actors
        .iter()
        .filter_map(|p| {
            p.outfit_model
                .as_ref()
                .map(|model| (p.life.actor, model.clone()))
        })
        .collect();
    for name in outfits.values() {
        super::remote_content::outfit_model(pack, name)?;
    }
    let mut drawn = instances_with_outfits(pack, &sample.frame, &outfits)?;
    append_equipment(
        pack,
        &sample.frame,
        &outfits,
        &sample.presentation.actors,
        &mut drawn,
    )?;
    drawn.extend(prop_instances_from_poses(
        pack,
        &sample.presentation.props,
        sample.frame.time,
    ));
    drawn.extend(blocker_instances_from_bounds(
        pack,
        &sample.presentation.blockers,
        sample.frame.time,
    ));
    drawn.extend(spell_instances_from_visuals(&sample.combat));
    let lighting = lighting_from_visuals(&sample.combat, origin, playground, focus);
    Ok(RemoteScene {
        frame: sample.frame,
        instances: drawn,
        lighting,
    })
}

#[cfg(feature = "remote-chamber")]
fn append_equipment(
    pack: &Pack,
    frame: &verse_engine::director::Frame,
    outfits: &std::collections::BTreeMap<u64, String>,
    presentation: &[verse_world::service::presentation::Pose],
    drawn: &mut Vec<Instance>,
) -> Result<(), String> {
    for actor in frame
        .actors
        .iter()
        .filter(|a| a.visible && a.actor.model == "adventurer")
    {
        let Some(pose) = presentation
            .iter()
            .find(|p| Some(p.life.into()) == actor.life)
        else {
            continue;
        };
        if pose.equipment.is_empty() {
            continue;
        }
        let name = outfits
            .get(&actor.actor.id)
            .map(String::as_str)
            .unwrap_or(&actor.actor.model);
        let model = pack
            .models
            .get(name)
            .ok_or("Equipment parent rig is missing")?;
        verse_engine::sockets::Sockets::admit(model)?;
        for gear in &pose.equipment {
            super::remote_content::equipment_model(pack, gear)?;
            // A readied bow occupies the hands; holster the main-hand model for that pose.
            if gear.slot == verse_world::service::equipment::Slot::MainHand
                && bow_drawn(actor.animation)
            {
                continue;
            }
            if !model.attachments.iter().any(|a| a.id == gear.slot.socket()) {
                return Err("Equipment parent socket is missing".into());
            }
            let local = Mat4::from_translation(Vec3::from(gear.offset.map(|v| v as f32 / 1000.)));
            drawn.push(Instance {
                mount: Some(verse_engine::presentation::Mount {
                    parent: pose.life.into(),
                    parent_model: name.into(),
                    socket: gear.slot.socket(),
                    local,
                }),
                actor: None,
                model: gear.model.clone(),
                transform: Mat4::IDENTITY,
                animation: 0.into(),
                time: frame.time,
                emission: Vec3::ONE,
            });
        }
    }
    Ok(())
}

pub fn basis() -> Mat4 {
    Mat4::from_cols_array(&[
        0.0, 0.0, -0.9144, 0.0, -0.9144, 0.0, 0.0, 0.0, 0.0, 0.9144, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ])
}

pub fn instances(
    pack: &Pack,
    frame: &verse_engine::director::Frame,
) -> Result<Vec<Instance>, String> {
    instances_with_outfits(pack, frame, &std::collections::BTreeMap::new())
}
fn instances_with_outfits(
    pack: &Pack,
    frame: &verse_engine::director::Frame,
    outfits: &std::collections::BTreeMap<u64, String>,
) -> Result<Vec<Instance>, String> {
    let render_model = |actor: &verse_engine::director::Actor| {
        outfits
            .get(&actor.id)
            .cloned()
            .unwrap_or_else(|| actor.model.clone())
    };
    let mut actors: Vec<_> = frame
        .actors
        .iter()
        .filter(|a| a.visible)
        .map(|a| Instance {
            mount: None,
            actor: Some(a.life.unwrap_or(verse_engine::core::LifeId {
                instance: 0,
                actor: a.actor.id,
                generation: 0,
            })),
            model: render_model(&a.actor),
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
        .filter(|a| a.visible && a.actor.model == "adventurer")
    {
        let model = pack
            .models
            .get(&render_model(&a.actor))
            .ok_or("Missing adventurer model")?;
        if model.attachments.iter().any(|a| a.id == 2)
            && model.attachments.iter().any(|a| a.id == 3)
            && model.attachments.iter().any(|a| a.id == 4)
        {
            actors.push(Instance {
                mount: Some(verse_engine::presentation::Mount {
                    parent: a.life.unwrap_or(verse_engine::core::LifeId {
                        instance: 0,
                        actor: a.actor.id,
                        generation: 0,
                    }),
                    parent_model: render_model(&a.actor),
                    socket: 2,
                    local: Mat4::IDENTITY,
                }),
                actor: None,
                model: "bow".into(),
                transform: Mat4::IDENTITY,
                animation: 0.into(),
                time: frame.time,
                emission: Vec3::ONE,
            });
        }
    }
    for arrow in &frame.projectiles {
        actors.push(Instance {
            mount: None,
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
    blocker_instances_from_bounds(pack, &verse_world::visuals::blocker_bounds(game), game.time)
}
/// Draws admitted cover bounds without constructing local navigation or physics.
pub fn blocker_instances_from_bounds(
    pack: &Pack,
    blockers: &[verse_world::visuals::Blocker],
    time: f32,
) -> Vec<Instance> {
    if !pack.models.contains_key("navigation-blocker") {
        return vec![];
    }
    blockers
        .iter()
        .filter(|b| !(b.table_proxy && pack.models.contains_key("prop/Table_Large")))
        .map(|b| Instance {
            mount: None,
            actor: None,
            model: "navigation-blocker".into(),
            transform: Mat4::from_translation(((b.min + b.max) * 0.5).as_vec3())
                * Mat4::from_scale((b.max - b.min).as_vec3() / 0.9144)
                * basis(),
            animation: 0.into(),
            time,
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
        .filter(|p| p.model != "prop/flame")
        .map(|p| Instance {
            mount: None,
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
/// The spell playground hall's even, warm light.
pub fn playground_lighting() -> super::lighting::Lighting {
    let mut lights = super::lighting::Lighting::default();
    let Ok(hall) = verse_world::playground::hall() else {
        return lights;
    };
    lights.ambient = hall.ambient;
    lights.exposure = 1.2;
    lights.fog = Vec3::new(0.004, 0.004, 0.005);
    lights.density = 0.004;
    for light in &hall.lights {
        lights.lights.push(super::lighting::Light {
            position: light.position,
            color: light.color,
            intensity: light.intensity,
            range: light.range,
        });
    }
    lights
}
/// The playground hall as a static instance, in place of the chamber.
pub fn playground_static_instances() -> Vec<Instance> {
    vec![Instance {
        mount: None,
        actor: None,
        model: "playground-hall".into(),
        transform: basis(),
        animation: 0.into(),
        time: 0.0,
        emission: Vec3::ONE,
    }]
}
/// Dynamic props at their interpolated poses, scaled to their collision boxes.
pub fn prop_instances(pack: &Pack, game: &super::play::Game, alpha: f32) -> Vec<Instance> {
    prop_instances_from_poses(
        pack,
        &verse_world::visuals::prop_poses(game, alpha),
        game.time,
    )
}
/// Draws local or admitted remote box poses without stepping physics.
pub fn prop_instances_from_poses(
    pack: &Pack,
    props: &[verse_world::visuals::Prop],
    time: f32,
) -> Vec<Instance> {
    props
        .iter()
        .filter_map(|p| {
            let model = p.kind.model(p.secured);
            pack.models.contains_key(model).then(|| Instance {
                mount: None,
                actor: None,
                model: model.into(),
                transform: Mat4::from_translation(p.center)
                    * Mat4::from_quat(p.rotation)
                    * Mat4::from_scale(p.dimensions / 0.9144)
                    * basis(),
                animation: 0.into(),
                time,
                emission: Vec3::ONE,
            })
        })
        .collect()
}
/// Bind transient illumination to the same combat events that draw the effects.
pub fn combat_lighting(game: &super::play::Game) -> super::lighting::Lighting {
    let mut lighting = lighting_from_visuals(
        &verse_world::visuals::Combat::extract(game),
        position_from_wow(game.scene.origin_wow),
        game.scene.collision_profile.as_deref() == Some(verse_world::playground::PROFILE),
        game.player,
    );
    if game.scene.collision_profile.as_deref() == Some(verse_world::playground::PROFILE)
        && !game.spells.reversed.is_empty()
    {
        lighting.lights.truncate(super::lighting::MAX_LIGHTS - 1);
        lighting.lights.push(super::lighting::Light {
            position: Vec3::new(0., 28., -3.),
            color: Vec3::new(0.8, 0.85, 1.),
            intensity: 260.,
            range: 50.,
        });
    }
    lighting
}
/// Uses the same effect lighting for local authority and admitted remote data.
pub fn lighting_from_visuals(
    visuals: &verse_world::visuals::Combat,
    origin: Vec3,
    playground: bool,
    focus: Vec3,
) -> super::lighting::Lighting {
    use super::lighting::{Light, MAX_LIGHTS};
    use verse_world::utilities::Utility;
    let mut lighting = if playground {
        playground_lighting()
    } else {
        lighting(origin)
    };
    lighting.time = visuals.time;
    let mut fire_lights = Vec::new();
    for flame in &visuals.flames {
        let position = flame.position.as_vec3();
        if flame.id >= 1_000_000 && flame.lit {
            fire_lights.push(Light {
                position,
                color: Vec3::new(1., 0.35, 0.03),
                intensity: 45.,
                range: 4.,
            });
        }
        for light in &mut lighting.lights {
            if light.position.distance(position) < 0.8 {
                light.intensity = if !flame.lit {
                    0.
                } else if flame.protected {
                    light.intensity
                        * (0.6 + 0.4 * (visuals.time * 31. + flame.id as f32).sin().abs())
                } else {
                    light.intensity
                };
            }
        }
    }
    let mut effects = fire_lights;
    for controls in &visuals.players {
        if let Some(position) = controls.light {
            effects.push(Light {
                position,
                color: Vec3::new(1.0, 0.8, 0.4),
                intensity: 55.0,
                range: 10.0,
            });
        }
    }
    for p in &visuals.projectiles {
        if matches!(
            p.kind,
            verse_world::rules::ProjectileKind::Bow
                | verse_world::rules::ProjectileKind::SiegeBoulder
        ) {
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
    for (position, at, kind) in &visuals.impacts {
        if *kind == 3 {
            continue;
        }
        let age = visuals.time - at;
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
    for controls in &visuals.players {
        if controls.shield > 0 && visuals.time < controls.shield_until {
            effects.push(Light {
                position: controls.position + Vec3::Y * 1.3,
                color: Vec3::new(0.08, 0.5, 1.0),
                intensity: 22.0,
                range: 4.5,
            });
        }
    }
    for area in visuals.players.iter().flat_map(|c| &c.areas) {
        let left = area.until - visuals.time;
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
    {
        for cast in &visuals.hostile {
            let position = if visuals.time < cast.release {
                cast.origin
            } else {
                cast.position.unwrap_or(cast.origin)
            };
            let charge =
                ((visuals.time - cast.started) / (cast.release - cast.started)).clamp(0.0, 1.0);
            effects.push(Light {
                position,
                color: Vec3::new(0.65, 0.04, 1.0),
                intensity: if cast.boss { 100.0 } else { 35.0 } * (0.25 + 0.75 * charge),
                range: if cast.boss { 9.0 } else { 5.0 },
            });
        }
    }
    // Reserve two shadow slots for the brightest nearby effects; keep fixture light stable.
    let priority = |l: &Light| l.intensity / (1.0 + l.position.distance_squared(focus));
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
                graph: None,
                markers: Vec::new(),
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
                    material: Default::default(),
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
                graph: None,
                markers: Vec::new(),
                states: Default::default(),
                skin: None,
                source: format!("verse/particles/{name}"),
                source_sha256: format!("{:x}", Sha256::digest(name.as_bytes())),
                height: 2.0,
                bones: vec![],
                clips: vec![],
                attachments: vec![],
                surfaces: vec![Surface {
                    material: Default::default(),
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
            graph: None,
            markers: Vec::new(),
            states: Default::default(),
            skin: None,
            source: "verse/ribbon/effect-ribbon".into(),
            source_sha256: format!("{:x}", Sha256::digest(b"effect-ribbon")),
            height: 2.0,
            bones: vec![],
            clips: vec![],
            attachments: vec![],
            surfaces: vec![Surface {
                material: Default::default(),
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
                graph: None,
                markers: Vec::new(),
                states: Default::default(),
                skin: None,
                source: format!("verse/procedural/{name}"),
                source_sha256: format!("{:x}", Sha256::digest(name.as_bytes())),
                height: 2.0,
                bones: vec![],
                clips: vec![],
                attachments: vec![],
                surfaces: vec![Surface {
                    material: Default::default(),
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
        material: Default::default(),
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
    let mut out = spell_instances_from_visuals(&verse_world::visuals::Combat::extract(game));
    out
}
/// Draws admitted visual values without borrowing local world authority.
pub fn spell_instances_from_visuals(visuals: &verse_world::visuals::Combat) -> Vec<Instance> {
    let mut out = Vec::new();
    for p in &visuals.projectiles {
        if p.kind == verse_world::rules::ProjectileKind::SiegeBoulder {
            out.push(Instance {
                mount: None,
                actor: None,
                model: "prop-stone".into(),
                transform: Mat4::from_translation(p.pos.into())
                    * Mat4::from_scale(Vec3::splat(1.2)),
                animation: 0.into(),
                time: visuals.time,
                emission: Vec3::ZERO,
            });
            continue;
        }
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
            mount: None,
            actor: None,
            model: if force { "effect-force" } else { "effect-fire" }.into(),
            transform: Mat4::from_translation(p.pos.into()) * Mat4::from_scale(Vec3::splat(scale)),
            animation: 0.into(),
            time: visuals.time,
            emission: Vec3::ONE,
        });
        for trail in 1..4 {
            out.push(Instance {
                mount: None,
                actor: None,
                model: if force { "effect-force" } else { "effect-fire" }.into(),
                transform: Mat4::from_translation(
                    Vec3::from(p.pos) - Vec3::from(p.vel).normalize_or_zero() * trail as f32 * 0.2,
                ) * Mat4::from_scale(Vec3::splat(scale * (1.0 - trail as f32 * 0.2))),
                animation: 0.into(),
                time: visuals.time,
                emission: Vec3::ONE,
            });
        }
    }
    use verse_world::utilities::Utility;
    for area in visuals
        .players
        .iter()
        .flat_map(|c| &c.areas)
        .filter(|a| a.until > visuals.time)
    {
        let left = area.until - visuals.time;
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
            mount: None,
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
            time: visuals.time,
            emission: Vec3::splat(alpha),
        });
    }
    for controls in &visuals.players {
        let player_position = controls.position;
        if let Some(position) = controls.light {
            out.push(Instance {
                mount: None,
                actor: None,
                model: "effect-light".into(),
                transform: Mat4::from_translation(position) * Mat4::from_scale(Vec3::splat(0.12)),
                animation: 0.into(),
                time: visuals.time,
                emission: Vec3::ONE,
            });
        }
        if controls.shield > 0 && visuals.time < controls.shield_until {
            out.push(Instance {
                mount: None,
                actor: None,
                model: "effect-shield".into(),
                transform: Mat4::from_translation(player_position + Vec3::Y * 1.05)
                    * Mat4::from_rotation_y(visuals.time * 0.7)
                    * Mat4::from_scale(Vec3::new(1.1, 1.25, 1.1)),
                animation: 0.into(),
                time: visuals.time,
                emission: Vec3::splat(0.8),
            });
        }
        if controls.shield > 0 && visuals.time < controls.shield_until {
            for n in 0..6 {
                let angle = visuals.time * 1.3 + n as f32 * std::f32::consts::TAU / 6.0;
                let center = player_position
                    + Vec3::new(
                        angle.cos() * 1.05,
                        1.05 + (angle * 2.0).sin() * 0.8,
                        angle.sin() * 1.05,
                    );
                out.push(particle("effect-force", center, 0.18, 0.5, visuals.time));
            }
        }
    }
    {
        for cast in &visuals.hostile {
            out.push(Instance {
                mount: None,
                actor: None,
                model: "effect-rune".into(),
                transform: Mat4::from_translation(cast.target + Vec3::Y * 0.08)
                    * Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2)
                    * Mat4::from_scale(Vec3::splat(cast.radius)),
                animation: 0.into(),
                time: visuals.time,
                emission: Vec3::splat(0.6),
            });
            let position = if visuals.time < cast.release {
                cast.origin
            } else {
                cast.position.unwrap_or(cast.origin)
            };
            let size = if cast.boss { 0.55 } else { 0.23 };
            out.push(Instance {
                mount: None,
                actor: None,
                model: "effect-shadow".into(),
                transform: Mat4::from_translation(position) * Mat4::from_scale(Vec3::splat(size)),
                animation: 0.into(),
                time: visuals.time,
                emission: Vec3::ONE,
            });
            if visuals.time >= cast.release {
                let direction = (cast.target + Vec3::Y - cast.origin).normalize_or_zero();
                out.push(Instance {
                    mount: None,
                    actor: None,
                    model: "effect-ribbon".into(),
                    transform: Mat4::from_translation(position - direction * 0.65)
                        * Mat4::from_quat(Quat::from_rotation_arc(Vec3::Y, direction))
                        * Mat4::from_scale(Vec3::new(size * 0.7, 0.85, 1.0)),
                    animation: 0.into(),
                    time: visuals.time,
                    emission: Vec3::splat(0.75),
                });
            }
        }
    }
    for (position, at, kind) in &visuals.impacts {
        if *kind == 3 {
            continue;
        }
        let elapsed = visuals.time - at;
        let scale = if *kind == 1 {
            0.6 + elapsed * 10.0
        } else {
            0.15 + elapsed * 1.8
        };
        out.push(Instance {
            mount: None,
            actor: None,
            model: "effect-impact".into(),
            transform: Mat4::from_translation(*position) * Mat4::from_scale(Vec3::splat(scale)),
            animation: 0.into(),
            time: visuals.time,
            emission: Vec3::splat((1.0 - elapsed / 0.6).max(0.0)),
        });
    }
    for p in &visuals.projectiles {
        if matches!(
            p.kind,
            verse_world::rules::ProjectileKind::MagicMissile
                | verse_world::rules::ProjectileKind::Bow
                | verse_world::rules::ProjectileKind::SiegeBoulder
        ) {
            continue;
        }
        let direction = Vec3::from(p.vel).normalize_or_zero();
        for n in 0..8 {
            let age = (n as f32 + 0.5) * 0.045;
            let phase = visuals.time * 8.0 + n as f32 * 2.4 + p.id as f32;
            let center = Vec3::from(p.pos) - direction * Vec3::from(p.vel).length() * age
                + Vec3::new(phase.sin(), age * 3.0, phase.cos()) * age * 0.4;
            out.push(particle(
                "effect-fire",
                center,
                0.12 + age * 0.25,
                1.0 - age / 0.5,
                visuals.time,
            ));
            if n % 2 == 0 {
                out.push(particle(
                    "particle-smoke",
                    center + Vec3::Y * age,
                    0.2 + age,
                    age * 0.65,
                    visuals.time,
                ));
            }
        }
    }
    for (position, at, kind) in &visuals.impacts {
        if *kind == 3 {
            continue;
        }
        let age = visuals.time - at;
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
                visuals.time,
            ));
        }
    }
    // The actor palette has a fixed budget; retain primary spell and area cues first.
    out.truncate(220);
    out
}
fn particle(model: &str, position: Vec3, radius: f32, opacity: f32, time: f32) -> Instance {
    Instance {
        mount: None,
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
                mount: None,
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
    #[cfg(feature = "remote-chamber")]
    #[test]
    fn assembled_remote_sample_matches_local_native_world_consumers() {
        let scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let game = verse_world::play::Game::new_in(scene, 160).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let pack = super::super::original::generate(dir.path()).unwrap();
        let frame = game.frame();
        let combat = verse_world::visuals::Combat::extract(&game);
        let presentation = verse_world::service::presentation::Presentation {
            flames: game.spells.flames.clone(),
            time: frame.time,
            actors: vec![],
            corpses: vec![],
            effects: vec![],
            hostile_casts: vec![],
            impacts: vec![],
            props: verse_world::visuals::prop_poses(&game, 1.),
            blockers: verse_world::visuals::blocker_bounds(&game),
        };
        let mut expected = instances(&pack, &frame).unwrap();
        expected.extend(prop_instances(&pack, &game, 1.));
        expected.extend(blocker_instances(&pack, &game));
        expected.extend(spell_instances(&game));
        let origin = position_from_wow(game.scene.origin_wow);
        let drawn = render_remote_sample(
            &pack,
            verse_world::service::view::SceneSample {
                frame,
                presentation,
                combat,
            },
            origin,
            false,
            game.player,
        )
        .unwrap();
        assert_eq!(format!("{:?}", drawn.instances), format!("{:?}", expected));
        let local = combat_lighting(&game);
        assert_eq!(drawn.lighting.time, local.time);
        assert_eq!(drawn.lighting.lights.len(), local.lights.len());
        for (a, b) in drawn.lighting.lights.iter().zip(&local.lights) {
            assert_eq!(a.position, b.position);
            assert_eq!(a.intensity, b.intensity);
            assert_eq!(a.color, b.color);
        }
    }
    #[cfg(feature = "remote-chamber")]
    #[test]
    fn remote_scene_waits_for_admission_and_rejects_invalid_view_inputs() {
        let dir = tempfile::tempdir().unwrap();
        let pack = super::super::original::generate(dir.path()).unwrap();
        let view = verse_world::service::view::View::new(160, 10., 0).unwrap();
        let camera = verse_world::service::view::Camera {
            eye: Vec3::new(0., 3., -5.),
            target: Vec3::Y,
            fov: 60.,
        };
        assert!(
            remote_scene(&pack, &view, 0.5, camera, Vec3::ZERO, false, Vec3::ZERO)
                .unwrap()
                .is_none()
        );
        assert!(
            remote_scene(
                &pack,
                &view,
                f32::NAN,
                camera,
                Vec3::ZERO,
                false,
                Vec3::ZERO
            )
            .is_err()
        );
        assert!(remote_scene(&pack, &view, 0.5, camera, Vec3::NAN, false, Vec3::ZERO).is_err());
        let invalid = verse_world::service::view::Camera { fov: 0., ..camera };
        assert!(remote_scene(&pack, &view, 0.5, invalid, Vec3::ZERO, false, Vec3::ZERO).is_err());
    }
    #[test]
    #[ignore = "Explicit animated outfit GPU acceptance"]
    fn capture_owned_outfit_models() {
        let output = std::env::var_os("VERSE_OUTFIT_CAPTURE").expect("Explicit output path");
        let dir = tempfile::tempdir().unwrap();
        let mut pack = super::super::original::generate(dir.path()).unwrap();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/verse/characters/quaternius");
        super::super::characters::install(&mut pack, dir.path(), &root, "male-ranger").unwrap();
        let mut frame = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap()
        .frame(9.);
        let mut player = frame
            .actors
            .iter()
            .find(|a| a.actor.model == "adventurer")
            .unwrap()
            .clone();
        player.visible = true;
        player.actor.position = Vec3::new(-1.4, 0., 0.);
        player.actor.scale = 1.;
        player.actor.yaw = 0.;
        player.animation = verse_engine::motion::State::Walk.into();
        player.animation_time = 0.25;
        let mut other = player.clone();
        other.actor.id += 1;
        other.life = None;
        other.actor.position.x = 1.4;
        let ids = [player.actor.id, other.actor.id];
        frame.actors = vec![player, other];
        frame.projectiles.clear();
        let outfits = std::collections::BTreeMap::from([
            (ids[0], "universal-male-peasant".to_string()),
            (ids[1], "universal-female-ranger".to_string()),
        ]);
        for name in outfits.values() {
            super::super::remote_content::outfit_model(&pack, name).unwrap();
        }
        for name in super::super::characters::APPEARANCES {
            let model = &pack.models[&format!("universal-{name}")];
            for id in [2, 3, 4, 5, 6] {
                assert!(
                    model.attachments.iter().any(|a| a.id == id),
                    "Missing socket {id} on {name}"
                );
            }
        }
        for actor in &mut frame.actors {
            actor.life = Some(verse_engine::core::LifeId {
                instance: 1,
                actor: actor.actor.id,
                generation: 0,
            });
        }
        let drawn = instances_with_outfits(&pack, &frame, &outfits).unwrap();
        assert_eq!(drawn[0].model, "universal-male-peasant");
        assert_eq!(drawn[1].model, "universal-female-ranger");
        assert_eq!(drawn.iter().filter(|i| i.model == "bow").count(), 2);
        let mut equipped = drawn.clone();
        let poses = frame
            .actors
            .iter()
            .map(|a| verse_world::service::presentation::Pose {
                actor: a.actor.clone(),
                outfit_model: Some(outfits[&a.actor.id].clone()),
                life: a.life.unwrap().into(),
                teleport_stamp: None,
                animation: a.animation,
                animation_time: a.animation_time,
                visible: a.visible,
                health: a.health,
                equipment: vec![
                    verse_world::service::equipment::Gear {
                        id: 3,
                        name: "Ritual hat".into(),
                        slot: verse_world::service::equipment::Slot::Head,
                        model: "gear-hat".into(),
                        offset: [0, 0, 230],
                        health: 100,
                        mana: 0,
                    },
                    verse_world::service::equipment::Gear {
                        id: 4,
                        name: "Ritual wand".into(),
                        slot: verse_world::service::equipment::Slot::MainHand,
                        model: "gear-wand".into(),
                        offset: [0; 3],
                        health: 0,
                        mana: 10,
                    },
                ],
            })
            .collect::<Vec<_>>();
        append_equipment(&pack, &frame, &outfits, &poses, &mut equipped).unwrap();
        assert_eq!(equipped.len() - drawn.len(), 4);
        let mut bow_frame = frame.clone();
        for a in &mut bow_frame.actors {
            a.animation = verse_engine::motion::State::BowReady.into();
        }
        let mut bow_gear = vec![];
        append_equipment(&pack, &bow_frame, &outfits, &poses, &mut bow_gear).unwrap();
        assert_eq!(bow_gear.len(), 2);
        assert!(bow_gear.iter().all(|g| g.model == "gear-hat"));
        let mut bad = poses.clone();
        bad[0].equipment[0].model = "missing-gear".into();
        assert!(append_equipment(&pack, &frame, &outfits, &bad, &mut vec![]).is_err());
        let atlas = super::super::original::atlas().unwrap();
        let mut renderer =
            super::super::Renderer::new(pack, dir.path(), 1920, 1080, &atlas, &[]).unwrap();
        let eye = Vec3::new(0., 2.7, -7.);
        let camera = crate::render::View {
            view_proj: Mat4::perspective_rh(45f32.to_radians(), 1920. / 1080., 0.1, 100.)
                * Mat4::look_at_rh(eye, Vec3::Y, Vec3::Y),
            eye,
        };
        let lighting = super::super::lighting::Lighting {
            ambient: Vec3::splat(0.75),
            density: 0.,
            shadowed: 0,
            fog: Vec3::splat(0.015),
            ..Default::default()
        };
        let pixels = renderer
            .draw(camera, &drawn, &crate::ui::UiBatch::default(), &lighting)
            .unwrap();
        let mut encoder = png::Encoder::new(std::fs::File::create(output).unwrap(), 1920, 1080);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&pixels)
            .unwrap();
        if let Some(path) = std::env::var_os("VERSE_MOUNT_CAPTURE") {
            use std::io::Write;
            let root = std::path::PathBuf::from(path);
            std::fs::create_dir_all(&root).unwrap();
            let mut encoder = std::process::Command::new("ffmpeg")
                .args([
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-y",
                    "-f",
                    "rawvideo",
                    "-pixel_format",
                    "rgba",
                    "-video_size",
                    "1920x1080",
                    "-framerate",
                    "30",
                    "-i",
                    "pipe:0",
                    "-an",
                    "-filter_threads",
                    "1",
                    "-c:v",
                    "libx264",
                    "-preset",
                    "veryfast",
                    "-crf",
                    "20",
                    "-threads",
                    "2",
                    "-pix_fmt",
                    "yuv420p",
                ])
                .arg(root.join("transitions.mp4"))
                .stdin(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            let mut input = encoder.stdin.take().unwrap();
            let mut maximum_attachment_error = 0f32;
            let mut largest_blend_difference = 0f32;
            let mut sampled_mounts = 0usize;
            let mut reversed_frames = 0usize;
            let mut respawn_rejection = false;
            let mut states = std::collections::BTreeSet::new();
            let mut lighting = lighting.clone();
            for n in 0..120 {
                use verse_engine::motion::State;
                let state = match n {
                    0..=14 => State::Walk,
                    15..=17 => State::Cast,
                    18..=21 => State::SpellRelease,
                    22..=44 => State::BowReady,
                    45..=59 => State::BowRelease,
                    60..=89 => State::Death,
                    _ => State::Idle,
                };
                states.insert(format!("{state:?}"));
                let time = n as f32 / 30. + 0.02;
                lighting.time = time;
                for (index, actor) in frame.actors.iter_mut().enumerate() {
                    actor.animation = state.into();
                    actor.animation_time = if n >= 90 {
                        (n - 90) as f32 / 30.
                    } else {
                        time + index as f32 * 0.13
                    };
                    if n == 90 {
                        actor.life.as_mut().unwrap().generation += 1;
                    }
                }
                let mut instances =
                    instances_with_outfits(&renderer.pack, &frame, &outfits).unwrap();
                let mut poses = poses.clone();
                for (p, a) in poses.iter_mut().zip(&frame.actors) {
                    p.life = a.life.unwrap().into();
                }
                append_equipment(&renderer.pack, &frame, &outfits, &poses, &mut instances).unwrap();
                if n == 90 {
                    let mut stale = instances.clone();
                    let mount = stale.iter_mut().find_map(|i| i.mount.as_mut()).unwrap();
                    mount.parent.generation -= 1;
                    assert!(
                        verse_engine::presentation::ResolvedInstances::extract(
                            &renderer.catalog,
                            &stale
                        )
                        .is_err()
                    );
                    respawn_rejection = true;
                }
                if n % 2 == 1 {
                    instances.reverse();
                    reversed_frames += 1;
                }
                let pixels = renderer
                    .draw(
                        camera,
                        &instances,
                        &crate::ui::UiBatch::default(),
                        &lighting,
                    )
                    .unwrap();
                input.write_all(&pixels).unwrap();
                for (i, instance) in instances.iter().enumerate() {
                    let Some(mount) = &instance.mount else {
                        continue;
                    };
                    let parent = instances
                        .iter()
                        .position(|p| {
                            p.actor == Some(mount.parent) && p.model == mount.parent_model
                        })
                        .unwrap();
                    let parent_pose = renderer.evaluated_poses[parent];
                    let model = &renderer.pack.models[&mount.parent_model];
                    let matrices = parent_pose.bones[..model.bones.len().max(1)]
                        .iter()
                        .map(Mat4::from_cols_array_2d)
                        .collect::<Vec<_>>();
                    let palette = verse_engine::sockets::Palette::admit(model, &matrices).unwrap();
                    let sockets = verse_engine::sockets::Sockets::admit(model).unwrap();
                    let body = Mat4::from_cols_array_2d(&parent_pose.model);
                    let expected = if instance.model == "bow" {
                        mounted_bow(
                            sockets,
                            palette,
                            body,
                            bow_drawn(instances[parent].animation),
                        )
                        .unwrap()
                            * mount.local
                    } else {
                        sockets
                            .frame(palette, body, mount.socket, mount.local)
                            .unwrap()
                    };
                    let actual = Mat4::from_cols_array_2d(&renderer.evaluated_poses[i].model);
                    if instance.model == "bow" && !bow_drawn(instances[parent].animation) {
                        let spine = sockets.frame(palette, body, 3, Mat4::IDENTITY).unwrap();
                        assert!(
                            actual
                                .y_axis
                                .truncate()
                                .normalize()
                                .distance(spine.x_axis.truncate().normalize())
                                < 0.0001
                        );
                    }
                    let error = expected
                        .to_cols_array()
                        .into_iter()
                        .zip(actual.to_cols_array())
                        .map(|(a, b)| (a - b).abs())
                        .fold(0f32, f32::max);
                    maximum_attachment_error = maximum_attachment_error.max(error);
                    assert!(error < 0.0001);
                    let selected = verse_engine::animation::pose_selected(
                        model,
                        instances[parent].animation,
                        instances[parent].time,
                    )
                    .unwrap();
                    let difference = selected
                        .iter()
                        .zip(&matrices)
                        .flat_map(|(a, b)| {
                            a.to_cols_array()
                                .into_iter()
                                .zip(b.to_cols_array())
                                .map(|(a, b)| (a - b).abs())
                        })
                        .fold(0f32, f32::max);
                    if (15..=25).contains(&n) {
                        largest_blend_difference = largest_blend_difference.max(difference);
                    }
                    sampled_mounts += 1;
                }
                if [17, 23, 52, 87, 95].contains(&n) {
                    let mut png = png::Encoder::new(
                        std::fs::File::create(root.join(format!("frame-{n:03}.png"))).unwrap(),
                        1920,
                        1080,
                    );
                    png.set_color(png::ColorType::Rgba);
                    png.set_depth(png::BitDepth::Eight);
                    png.write_header()
                        .unwrap()
                        .write_image_data(&pixels)
                        .unwrap();
                }
            }
            drop(input);
            assert!(encoder.wait().unwrap().success());
            assert!(largest_blend_difference > 0.01);
            assert!(respawn_rejection);
            println!(
                "VERSE_MOUNTS {}",
                serde_json::json!({"schema":"verse.mounts.fixture.v1","frames":120,"dimensions":[1920,1080],"fps":30,"players":2,"models":outfits,"states":states,"reversed_instance_frames":reversed_frames,"sampled_mounts":sampled_mounts,"maximum_attachment_error":maximum_attachment_error,"largest_final_vs_unblended_difference":largest_blend_difference,"stale_generation_rejected":respawn_rejection,"parent_generation_after":1,"video":"transitions.mp4"})
            );
        }
        if let Some(path) = std::env::var_os("VERSE_SOCKET_CAPTURE") {
            let pixels = renderer
                .draw(camera, &equipped, &crate::ui::UiBatch::default(), &lighting)
                .unwrap();
            let mut encoder = png::Encoder::new(std::fs::File::create(path).unwrap(), 1920, 1080);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&pixels)
                .unwrap();
            println!(
                "VERSE_SOCKETS {}",
                serde_json::json!({"schema":"verse.sockets.fixture.v1","models":["universal-male-peasant","universal-female-ranger"],"animation":"walk","animation_time":0.25,"socket_ids":[5,6],"geometric_items":["gear-hat","gear-wand"],"equipment_instances":equipped.len()-drawn.len(),"dimensions":[1920,1080]})
            );
        }
    }
    #[test]
    fn local_and_read_only_blocker_bounds_share_native_transforms_and_table_proxy_rules() {
        let scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let mut game = verse_world::play::Game::new_in(scene, 160).unwrap();
        let life = physics::queries::Life {
            instance: 160,
            entity: 10000,
            generation: 0,
        };
        game.set_navigation_blocker(
            life,
            glam::DVec3::new(-0.5, 0., -8.5),
            glam::DVec3::new(0.5, 1., -7.5),
        )
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let mut pack = super::super::original::generate(dir.path()).unwrap();
        let bounds = verse_world::visuals::blocker_bounds(&game);
        verse_world::visuals::validate_blockers(&bounds, 160).unwrap();
        let local = blocker_instances(&pack, &game);
        let remote = blocker_instances_from_bounds(&pack, &bounds, game.time);
        assert_eq!(local.len(), 1);
        assert_eq!(format!("{:?}", local), format!("{:?}", remote));
        let model = pack.models["navigation-blocker"].clone();
        pack.models.insert("prop/Table_Large".into(), model);
        assert!(blocker_instances_from_bounds(&pack, &bounds, game.time).is_empty());
    }

    #[test]
    fn local_and_read_only_prop_values_share_native_model_and_box_transforms() {
        let scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let mut game = verse_world::play::Game::new_in(scene, 160).unwrap();
        game.spawn_prop(
            "Crate",
            verse_world::spells::PropSpec::reference(verse_world::spells::PropKind::Crate)
                .secured(),
            Vec3::new(0., 1., -8.),
            0.4,
        )
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let pack = super::super::original::generate(dir.path()).unwrap();
        let poses = verse_world::visuals::prop_poses(&game, 0.5);
        verse_world::visuals::validate_props(&poses, 160).unwrap();
        let local = prop_instances(&pack, &game, 0.5);
        let remote = prop_instances_from_poses(&pack, &poses, game.time);
        assert!(local.iter().any(|p| p.model == "prop-crate-secured"));
        assert_eq!(format!("{:?}", local), format!("{:?}", remote));
        assert!(remote[0].transform.is_finite());
        assert!(
            (remote[0].transform.transform_point3(Vec3::ZERO) - poses[0].center).length() < 0.00001
        );
    }

    #[test]
    fn controlled_adventurers_keep_distinct_shield_and_light_projections() {
        let scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let mut game = verse_world::play::Game::new(scene).unwrap();
        game.time = game.scene.cut_at;
        game.tick(0., [0.; 2]).unwrap();
        let second = game
            .add_player(verse_world::Controller(10), Vec3::new(3., 0., -22.))
            .unwrap();
        for ability in [
            verse_world::play::Ability::Shield,
            verse_world::play::Ability::Light,
        ] {
            game.activate(ability).unwrap();
            let command = game
                .player_admission(second.actor)
                .unwrap()
                .command(
                    game.authority_tick,
                    verse_world::Intent::Cast {
                        ability,
                        target: None,
                        aim: [0., 0., 1.],
                    },
                )
                .unwrap();
            game.submit(verse_world::Controller(10), command).unwrap();
        }
        let effects = spell_instances(&game);
        let visuals = verse_world::visuals::Combat::extract(&game);
        assert_eq!(
            format!("{:?}", effects),
            format!("{:?}", spell_instances_from_visuals(&visuals))
        );
        assert_eq!(
            format!("{:?}", combat_lighting(&game)),
            format!(
                "{:?}",
                lighting_from_visuals(
                    &visuals,
                    position_from_wow(game.scene.origin_wow),
                    false,
                    game.player
                )
            )
        );
        let shields: Vec<_> = effects
            .iter()
            .filter(|i| i.model == "effect-shield")
            .map(|i| i.transform.transform_point3(Vec3::ZERO))
            .collect();
        assert_eq!(shields.len(), 2);
        assert!(
            shields
                .iter()
                .any(|p| p.distance(Vec3::new(3., 1.05, -22.)) < 0.001)
        );
        assert_eq!(
            effects.iter().filter(|i| i.model == "effect-light").count(),
            2
        );
        let lights = combat_lighting(&game);
        assert!(
            lights
                .lights
                .iter()
                .any(|l| l.position.distance(Vec3::new(3., 1.3, -22.)) < 0.001)
        );
        let posed = game.frame();
        let player = posed
            .actors
            .iter()
            .find(|a| a.actor.id == second.actor)
            .unwrap();
        assert_eq!(player.life, Some(second));
        assert_eq!(player.actor.position, Vec3::new(3., 0., -22.));
    }
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

/// The bow is drawn while the archer holds a bow stance; otherwise it is stowed.
pub fn bow_drawn(animation: verse_engine::motion::Selection) -> bool {
    use verse_engine::motion::State;
    matches!(
        animation,
        verse_engine::motion::Selection::Named(State::BowReady | State::BowRelease)
    )
}

/// Places the bow in world space from the adventurer's posed body and three
/// posed world points: the left palm, the upper back, and the left elbow.
///
/// The bow model is in the pack's Z-up units with its limbs along +Z and its
/// string on its -Y side. A drawn bow stands upright in the fist with its back
/// (+Y) pointing along the bow arm, so the string faces the archer. A stowed
/// bow hangs diagonally against the upper back with its string outward.
pub(super) fn mounted_bow(
    sockets: verse_engine::sockets::Sockets<'_>,
    palette: verse_engine::sockets::Palette<'_>,
    body: Mat4,
    drawn: bool,
) -> Result<Mat4, String> {
    if drawn {
        return Ok(bow_pose(
            body,
            sockets.point(palette, body, 2)?,
            sockets.point(palette, body, 3)?,
            sockets.point(palette, body, 4)?,
            true,
        ));
    }
    let spine = sockets.frame(palette, body, 3, Mat4::IDENTITY)?;
    let scale = spine.transform_vector3(Vec3::X).length();
    if !scale.is_finite() || scale < 0.000001 {
        return Err("Stowed bow spine frame is degenerate".into());
    }
    Ok(spine
        * Mat4::from_translation(-Vec3::X * (BOW_BACK_DEPTH / scale))
        * Mat4::from_rotation_x(BOW_BACK_TILT)
        * Mat4::from_rotation_z(-std::f32::consts::FRAC_PI_2))
}
pub fn bow_pose(body: Mat4, palm: Vec3, back: Vec3, elbow: Vec3, drawn: bool) -> Mat4 {
    let (scale, rotation, _) = body.to_scale_rotation_translation();
    // The body's forward is the pack's +X; see `basis`.
    let forward = flat(rotation * Vec3::X).unwrap_or(Vec3::NEG_Z);
    let up = Vec3::Y;
    let (at, facing, limbs) = if drawn {
        let arm = flat(palm - elbow).unwrap_or(forward);
        (palm, arm, up)
    } else {
        let right = forward.cross(up);
        let limbs = up * BOW_BACK_TILT.cos() + right * BOW_BACK_TILT.sin();
        (back - forward * BOW_BACK_DEPTH, forward, limbs)
    };
    let side = facing.cross(limbs).normalize();
    let limbs = side.cross(facing).normalize();
    Mat4::from_translation(at)
        * Mat4::from_mat3(glam::Mat3::from_cols(side, facing, limbs))
        * Mat4::from_scale(scale)
}
fn flat(v: Vec3) -> Option<Vec3> {
    Vec3::new(v.x, 0., v.z).try_normalize()
}
/// From the upper spine to the stowed bow's grip, in meters behind the body.
const BOW_BACK_DEPTH: f32 = 0.14;
/// How far the stowed bow leans from vertical across the back, in radians.
const BOW_BACK_TILT: f32 = 0.6;

/// Draw spell volumes and registered flames from their admitted state.
pub fn environment_instances(pack: &Pack, game: &super::play::Game) -> Vec<Instance> {
    let mut out = Vec::new();
    let mut ribbon = |position: Vec3, direction: Vec3, width: f32, length: f32| {
        if out.len() >= 512 || !pack.models.contains_key("effect-ribbon") {
            return;
        }
        out.push(Instance {
            mount: None,
            actor: None,
            model: "effect-ribbon".into(),
            transform: Mat4::from_translation(position)
                * Mat4::from_quat(Quat::from_rotation_arc(
                    Vec3::Y,
                    direction.normalize_or_zero(),
                ))
                * Mat4::from_scale(Vec3::new(width, length, 1.)),
            animation: 0.into(),
            time: game.time,
            emission: Vec3::splat(0.5),
        });
    };
    for effect in game
        .spells
        .wind_walls
        .iter()
        .filter(|e| e.wall.active(game.time as f64))
    {
        let wall = &effect.wall.wall;
        for segment in wall.path.windows(2) {
            for i in 0..12 {
                let p = segment[0].lerp(segment[1], i as f64 / 12.);
                let height = ((game.time * 1.7 + i as f32 * 0.27)
                    % verse_world::wind_wall::HEIGHT as f32)
                    + wall.base as f32;
                ribbon(
                    Vec3::new(p.x as f32, height, p.y as f32),
                    Vec3::Y,
                    0.12,
                    0.8,
                );
            }
        }
    }
    for effect in game
        .spells
        .gusts
        .iter()
        .filter(|e| e.gust.active(game.time as f64))
    {
        for i in 0..32 {
            let along = (game.time * 12. + i as f32 * 0.57) % verse_world::gust::LENGTH as f32;
            let side = (i as f32 % 5. - 2.) * 0.6;
            let point = effect.gust.line.origin.as_vec3()
                + effect.gust.line.direction.as_vec3() * along
                + effect.gust.line.side().as_vec3() * side
                + Vec3::Y * (0.6 + (i % 3) as f32 * 0.6);
            ribbon(point, effect.gust.line.direction.as_vec3(), 0.08, 0.9);
        }
    }
    drop(ribbon);
    for effect in &game.spells.meteors {
        for impact in &effect.swarm.impacts {
            if pack.models.contains_key("effect-scorch") {
                out.push(Instance {
                    mount: None,
                    actor: None,
                    model: "effect-scorch".into(),
                    transform: Mat4::from_translation(impact.point.as_vec3() + Vec3::Y * 0.02)
                        * Mat4::from_scale(Vec3::new(5., 0.5, 5.))
                        * basis(),
                    animation: 0.into(),
                    time: game.time,
                    emission: Vec3::ZERO,
                });
            }
        }
        if pack.models.contains_key("effect-fire") {
            for meteor in &effect.swarm.meteors {
                let Some(id) = meteor.body else { continue };
                let body = &game.spells.world[id];
                if body.removed {
                    continue;
                }
                for trail in 1..=6 {
                    out.push(Instance {
                        mount: None,
                        actor: None,
                        model: "effect-fire".into(),
                        transform: Mat4::from_translation(
                            (body.pos - body.vel.normalize_or_zero() * f64::from(trail) * 1.2)
                                .as_vec3(),
                        ) * Mat4::from_scale(Vec3::splat(0.5)),
                        animation: 0.into(),
                        time: game.time,
                        emission: Vec3::splat(3.),
                    });
                }
            }
        }
    }
    if pack.models.contains_key("prop/flame") {
        out.extend(
            verse_world::visuals::flame_states(game)
                .iter()
                .filter(|f| f.lit)
                .map(|f| {
                    let scale = pack
                        .placements
                        .get(f.id as usize)
                        .filter(|p| p.model == "prop/flame")
                        .map_or(0.6, |p| p.scale);
                    Instance {
                        mount: None,
                        actor: None,
                        model: "prop/flame".into(),
                        transform: Mat4::from_translation(f.position.as_vec3())
                            * Mat4::from_scale(Vec3::splat(scale))
                            * basis(),
                        animation: 0.into(),
                        time: game.time,
                        emission: Vec3::splat(4.),
                    }
                }),
        );
    }
    out
}
