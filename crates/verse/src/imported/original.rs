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
        graph: None,
        markers: Vec::new(),
        states: Default::default(),
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
        material: Default::default(),
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
fn equipment_samples(pack: &mut Pack) {
    let mut hat = model("gear-hat", 0.45);
    let mut surface = Surface {
        material: Default::default(),
        vertices: vec![],
        indices: vec![],
        texture: 0,
        blend: 0,
        emissive: false,
        tint: [0.34, 0.13, 0.56],
    };
    for i in 0..16 {
        let angle = i as f32 * std::f32::consts::TAU / 16.;
        for point in [
            [0., 0., 0.45],
            [0.22 * angle.cos(), 0.22 * angle.sin(), 0.],
            {
                let next = angle + std::f32::consts::TAU / 16.;
                [0.22 * next.cos(), 0.22 * next.sin(), 0.]
            },
        ] {
            let normal = Vec3::new(angle.cos(), angle.sin(), 0.49).normalize();
            let index = surface.vertices.len() as u32;
            surface.vertices.push(Vertex {
                position: point,
                normal: normal.to_array(),
                uv: [0.5; 2],
                joints: [0; 4],
                weights: [1., 0., 0., 0.],
            });
            surface.indices.push(index);
        }
    }
    hat.surfaces.push(surface);
    cuboid(
        &mut hat,
        [0., 0., 0.],
        [0.27, 0.27, 0.015],
        [0.34, 0.13, 0.56],
        0,
        false,
    );
    pack.models.insert("gear-hat".into(), hat);
    let mut wand = model("gear-wand", 0.6);
    cuboid(
        &mut wand,
        [0., 0., 0.18],
        [0.022, 0.022, 0.28],
        [0.32, 0.14, 0.045],
        0,
        false,
    );
    cuboid(
        &mut wand,
        [0., 0., 0.5],
        [0.055, 0.055, 0.07],
        [0.17, 0.75, 1.],
        0,
        true,
    );
    pack.models.insert("gear-wand".into(), wand);
}
fn bow() -> Model {
    let mut bow = model("bow", 1.4);
    // Tapered curved limbs in the pack's Z-up coordinates, centered on the grip.
    let points = [
        [0.0, -0.18, -0.7],
        [0.0, -0.08, -0.58],
        [0.0, 0.06, -0.36],
        [0.0, 0.0, 0.0],
        [0.0, 0.06, 0.36],
        [0.0, -0.08, 0.58],
        [0.0, -0.18, 0.7],
    ];
    for (i, ends) in points.windows(2).enumerate() {
        let start = Vec3::from_array(ends[0]);
        let end = Vec3::from_array(ends[1]);
        let direction = end - start;
        let thickness = if i == 0 || i == 5 { 0.012 } else { 0.022 };
        cuboid(
            &mut bow,
            [0.; 3],
            [thickness, thickness, direction.length() * 0.5],
            [0.42, 0.21, 0.075],
            0,
            false,
        );
        let rotation = Quat::from_rotation_arc(Vec3::Z, direction.normalize());
        for vertex in &mut bow.surfaces.last_mut().unwrap().vertices {
            vertex.position =
                (rotation * Vec3::from_array(vertex.position) + (start + end) * 0.5).to_array();
            vertex.normal = (rotation * Vec3::from_array(vertex.normal)).to_array();
        }
    }
    cuboid(
        &mut bow,
        [0.; 3],
        [0.026, 0.028, 0.09],
        [0.16, 0.095, 0.055],
        0,
        false,
    );
    cuboid(
        &mut bow,
        [0., -0.18, 0.],
        [0.002, 0.002, 0.7],
        [0.75, 0.7, 0.58],
        0,
        false,
    );
    bow
}
/// Compiles chamber semantics into explicit per-model clip bindings.
/// Clip numbers are compiler-local references, never gameplay selectors.
pub(super) fn bind_states(model: &mut Model) {
    use verse_engine::motion::{Binding, Mode, State};
    let defaults = [
        (State::Idle, 0),
        (State::Death, 1),
        (State::Walk, 4),
        (State::Run, 5),
        (State::Backpedal, 13),
        (State::StrafeLeft, 14),
        (State::StrafeRight, 15),
        (State::Airborne, 37),
        (State::CombatReadyAlternate, 25),
        (State::CombatReady, 51),
        (State::Cast, 52),
        (State::SpellRelease, 53),
        (State::BowReady, 109),
        (State::BowRelease, 46),
        (State::Prone, 100),
        (State::Yell, 64),
        (State::Affirm, 68),
    ];
    for (state, preferred) in defaults {
        let clip = if model.clips.iter().any(|c| c.id == preferred) {
            preferred
        } else if state == State::Prone {
            1
        } else if matches!(state, State::BowReady | State::BowRelease) {
            51
        } else {
            0
        };
        model.states.insert(
            state,
            Binding {
                clip,
                mode: if matches!(
                    state,
                    State::Death
                        | State::Prone
                        | State::SpellRelease
                        | State::BowRelease
                        | State::Yell
                        | State::Affirm
                ) {
                    Mode::Hold
                } else {
                    Mode::Loop
                },
                transition_seconds: if state == State::Death { 0.12 } else { 0.22 },
            },
        );
    }
    // Authored alternating foot cues at quarter and three-quarter cycle phases.
    // These are presentation events; they do not move actors or apply damage.
    for clip_id in [4, 5, 13, 14, 15] {
        let Some(clip) = model.clips.iter().find(|clip| clip.id == clip_id) else {
            continue;
        };
        if clip.duration <= 0. || model.markers.iter().any(|track| track.clip == clip_id) {
            continue;
        }
        let duration = f64::from(clip.duration);
        model.markers.push(verse_engine::markers::ClipTrack {
            clip: clip_id,
            track: verse_engine::markers::Track {
                duration,
                markers: vec![
                    verse_engine::markers::Marker {
                        id: verse_engine::markers::FOOTSTEP_LEFT,
                        seconds: duration * 0.25,
                    },
                    verse_engine::markers::Marker {
                        id: verse_engine::markers::FOOTSTEP_RIGHT,
                        seconds: duration * 0.75,
                    },
                ],
            },
        });
    }
    model.graph = Some(verse_engine::animation_graph::Authored::from_bindings(
        model,
    ));
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
    for id in [5, 13, 14, 15] {
        let mut walk = m.clips.iter().find(|c| c.id == 4).unwrap().clone();
        walk.id = id;
        m.clips.push(walk);
    }
    m.attachments.push(Attachment {
        id: 2,
        bone: 1,
        position: [0., -0.52, 0.88],
    });
    m.attachments.extend([
        Attachment {
            id: 5,
            bone: 0,
            position: [0., 0., 2.12],
        },
        Attachment {
            id: 6,
            bone: 2,
            position: [0., 0.52, 0.88],
        },
    ]);
    bind_states(&mut m);
    m
}
pub use verse_world::room::{colliders, room_boxes};
/// Generates a complete pack in the caller's directory.
pub fn generate(dir: &Path) -> Result<Pack, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    png_file(dir, "original-white", 1, &[255; 4])?;
    let mut pack = Pack {
        inventory: None,
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
    pack.models
        .insert("dummy".into(), actor("dummy", [0.72, 0.58, 0.3], false));
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
    // The spell playground hall is drawn only by `--spell-playground`.
    let mut hall = model("playground-hall", 12.);
    {
        let mut world_box = |center: Vec3, half: Vec3, color, emissive| {
            let c = inverse.transform_point3(center).to_array();
            let h = inverse.transform_vector3(half).abs().to_array();
            cuboid(&mut hall, c, h, color, 0, emissive);
        };
        for s in &verse_world::playground::hall()?.solids {
            world_box(s.center, s.half, s.color, s.emissive);
        }
        // Floor tiles every 2 m make slides easy to read against the floor.
        for x in -11..11 {
            for z in -12..7 {
                if (x + z) % 2 == 0 {
                    world_box(
                        Vec3::new(x as f32 * 2. + 1., 0.002, z as f32 * 2. + 1.),
                        Vec3::new(0.98, 0.002, 0.98),
                        [0.37, 0.355, 0.33],
                        false,
                    );
                }
            }
        }
    }
    pack.models.insert("playground-hall".into(), hall);
    // Dynamic props are unit cubes (half 0.5 in pack units) that instances
    // scale to their collision box; stripes show how they turn.
    for (name, body, band) in [
        ("prop-crate", [0.55, 0.36, 0.17], [0.32, 0.19, 0.08]),
        ("prop-crate-secured", [0.45, 0.33, 0.2], [0.55, 0.57, 0.62]),
        ("prop-barrel", [0.5, 0.28, 0.13], [0.3, 0.3, 0.32]),
        ("prop-dummy", [0.75, 0.62, 0.32], [0.45, 0.3, 0.12]),
        ("prop-anvil", [0.22, 0.23, 0.26], [0.12, 0.12, 0.14]),
        ("prop-stone", [0.5, 0.49, 0.46], [0.36, 0.35, 0.33]),
    ] {
        let mut m = model(name, 1.);
        cuboid(&mut m, [0.; 3], [0.5; 3], body, 0, false);
        for k in 0..3 {
            let mut half = [0.506; 3];
            half[k] = 0.08;
            let mut other = [0.506; 3];
            other[(k + 1) % 3] = 0.08;
            cuboid(&mut m, [0.; 3], half, band, 0, false);
            if name == "prop-crate-secured" {
                cuboid(&mut m, [0.; 3], other, band, 0, false);
            }
        }
        pack.models.insert(name.into(), m);
    }
    let mut blocker = model("navigation-blocker", 1.);
    cuboid(&mut blocker, [0.; 3], [0.5; 3], [0.65, 0.32, 0.1], 0, false);
    pack.models.insert("navigation-blocker".into(), blocker);
    pack.placements.push(Placement {
        model: "chamber".into(),
        position: [0.; 3],
        rotation: [0., 0., 0., 1.],
        scale: 1.,
    });
    pack.models.insert("bow".into(), bow());
    equipment_samples(&mut pack);
    for (name, half, color) in [("arrow", [0.55, 0.025, 0.025], [0.75, 0.65, 0.35])] {
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
    let mut atlas = Atlas::from_font(font, 54.)?;
    for (name, size) in [
        ("small", 10.),
        ("combat", 28.),
        ("hotkey", 12.),
        ("numbers", 14.),
    ] {
        atlas.add_font(name, font, size * 3.)?;
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
        "spell-slot-empty",
    ]
    .into_iter()
    .chain(verse_world::spells::CATALOG.iter().map(|s| s.icon))
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
    atlas.use_logical_metrics(3.);
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
    fn original_presentation_survives_arbitrary_clip_ids() {
        use verse_engine::{
            animation::pose_selected,
            motion::{Selection, State},
        };
        let mut actor = actor("semantic-fixture", [0.2; 3], false);
        assert_eq!(actor.states.len(), State::ALL.len());
        let before: Vec<_> = State::ALL
            .iter()
            .map(|state| pose_selected(&actor, (*state).into(), 0.7).unwrap())
            .collect();
        for clip in &mut actor.clips {
            clip.id += 1000;
        }
        for binding in actor.states.values_mut() {
            binding.clip += 1000;
        }
        for (state, expected) in State::ALL.iter().zip(before) {
            assert_eq!(
                pose_selected(&actor, (*state).into(), 0.7).unwrap(),
                expected
            );
        }
        let scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let mut game = super::super::play::Game::combat(scene, true).unwrap();
        for _ in 0..4200 {
            game.tick(1. / 30., [0.; 2]).unwrap();
            for actor in game.frame().actors {
                assert!(matches!(actor.animation, Selection::Named(_)));
                assert!(actor.life.is_some());
            }
        }
    }
    #[test]
    fn original_timeline_runs_the_full_kit_and_actual_defeat() {
        let scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        assert_eq!(scene.origin_wow, [0.; 3]);
        let mut game = super::super::play::Game::combat(scene, true).unwrap();
        for _ in 0..4500 {
            game.tick(1. / 30., [0.; 2]).unwrap();
            if game.encounter.as_ref().unwrap().ended.is_some() {
                break;
            }
        }
        let encounter = game.encounter.as_ref().unwrap();
        assert_eq!(encounter.boss_max, 300_000);
        assert!(encounter.boss_remaining > 0 && encounter.boss_remaining < encounter.boss_max);
        assert_eq!(game.snapshot().player.hp, 0);
        assert!(encounter.enemy_casts > 0 && encounter.absorbed > 0);
        assert_eq!(encounter.used.len(), 10);
    }
}
