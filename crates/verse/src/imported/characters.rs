//! Licensed character composition and glTF animation compilation.
use glam::{Mat4, Quat, Vec3};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};
use verse_engine::assets::{
    Attachment, Bone, BoneKeys, Clip, Model, Pack, RestPose, Skin, Surface, Texture, Vertex,
};

pub const APPEARANCES: [&str; 6] = [
    "male-ranger",
    "female-ranger",
    "male-peasant",
    "female-peasant",
    "superhero-male",
    "superhero-female",
];
fn local(r: RestPose) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        r.scale.into(),
        Quat::from_array(r.rotation),
        r.translation.into(),
    )
}
fn globals(model: &Model) -> Vec<Mat4> {
    let skin = model.skin.as_ref().unwrap();
    let mut result = vec![Mat4::IDENTITY; model.bones.len()];
    for (i, b) in model.bones.iter().enumerate() {
        result[i] = if b.parent < 0 {
            local(skin.rest[i])
        } else {
            result[b.parent as usize] * local(skin.rest[i])
        };
    }
    result
}
fn texture(
    pack: &mut Pack,
    dir: &Path,
    image: &gltf::image::Data,
    file: String,
) -> Result<usize, String> {
    use gltf::image::Format;
    let channels = match image.format {
        Format::R8 => 1,
        Format::R8G8 => 2,
        Format::R8G8B8 => 3,
        Format::R8G8B8A8 => 4,
        _ => return Err("Unsupported character image format".into()),
    };
    // Compile a bounded GPU texture while retaining the original source image.
    let width = image.width.min(1024);
    let height = image.height.min(1024);
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let i = ((y * image.height / height * image.width + x * image.width / width) as usize)
                * channels;
            let p = &image.pixels[i..i + channels];
            rgba.extend(match channels {
                1 => [p[0], p[0], p[0], 255],
                2 => [p[0], p[0], p[0], p[1]],
                3 => [p[0], p[1], p[2], 255],
                _ => [p[0], p[1], p[2], p[3]],
            });
        }
    }
    let mut bytes = vec![];
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(&rgba)
        .map_err(|e| e.to_string())?;
    let digest = format!("{:x}", Sha256::digest(&bytes));

    if let Some(i) = pack.textures.iter().position(|t| t.file == file) {
        return Ok(i);
    }
    std::fs::write(dir.join(&file), bytes).map_err(|e| e.to_string())?;
    let index = pack.textures.len();
    pack.textures.push(Texture {
        file,
        sha256: digest,
        width,
        height,
    });
    Ok(index)
}
fn material_image(
    pack: &mut Pack,
    dir: &Path,
    path: &Path,
    buffers: &[gltf::buffer::Data],
    cache: &mut BTreeMap<usize, usize>,
    source: gltf::image::Image<'_>,
) -> Result<usize, String> {
    if let Some(index) = cache.get(&source.index()) {
        return Ok(*index);
    }
    let bytes = match source.source() {
        gltf::image::Source::Uri { uri, .. } => {
            std::fs::read(path.parent().unwrap().join(uri)).map_err(|e| e.to_string())?
        }
        gltf::image::Source::View { view, .. } => {
            buffers[view.buffer().index()].0[view.offset()..view.offset() + view.length()].to_vec()
        }
    };
    let file = format!("universal-source-{:x}.png", Sha256::digest(&bytes));
    let index = if let Some(i) = pack.textures.iter().position(|t| t.file == file) {
        i
    } else {
        let data = gltf::image::Data::from_source(source.source(), path.parent(), buffers)
            .map_err(|e| e.to_string())?;
        texture(pack, dir, &data, file)?
    };
    cache.insert(source.index(), index);
    Ok(index)
}
fn material_uv(set: u32) -> Result<(), String> {
    if set != 0 {
        return Err("Material requires an unsupported texture coordinate set".into());
    }
    Ok(())
}
/// Imports skinned triangle meshes using their full rest hierarchy and inverse binds.
pub fn import(pack: &mut Pack, dir: &Path, path: &Path) -> Result<Model, String> {
    let gltf = gltf::Gltf::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let buffers = gltf::import_buffers(&gltf.document, path.parent(), gltf.blob)
        .map_err(|e| e.to_string())?;
    let doc = gltf.document;
    let mut image_textures = BTreeMap::new();
    let nodes: Vec<_> = doc.nodes().collect();
    if nodes.len() > 256 {
        return Err("Character skeleton exceeds 256 nodes".into());
    }
    let mut parent = vec![None; nodes.len()];
    for n in &nodes {
        for child in n.children() {
            parent[child.index()] = Some(n.index());
        }
    }
    let mut order = vec![];
    let mut indices = vec![usize::MAX; nodes.len()];
    while order.len() < nodes.len() {
        let before = order.len();
        for n in &nodes {
            if indices[n.index()] == usize::MAX
                && parent[n.index()].is_none_or(|p| indices[p] != usize::MAX)
            {
                indices[n.index()] = order.len();
                order.push(n.index());
            }
        }
        if before == order.len() {
            return Err("Cyclic character hierarchy".into());
        }
    }
    let basis = super::chamber::basis().inverse() * Mat4::from_rotation_y(std::f32::consts::PI);
    let rest: Vec<_> = order
        .iter()
        .map(|i| {
            let (translation, rotation, scale) = nodes[*i].transform().decomposed();
            RestPose {
                translation,
                rotation,
                scale,
            }
        })
        .collect();
    let mut model = Model {
        markers: Vec::new(),
        states: Default::default(),
        source: format!(
            "verse/interchange/{}",
            path.file_name().unwrap().to_string_lossy()
        ),
        source_sha256: format!(
            "{:x}",
            Sha256::digest(std::fs::read(path).map_err(|e| e.to_string())?)
        ),
        surfaces: vec![],
        bones: order
            .iter()
            .map(|i| Bone {
                parent: parent[*i].map_or(-1, |p| indices[p] as i16),
                pivot: [0.; 3],
            })
            .collect(),
        clips: vec![],
        height: 2.,
        attachments: vec![],
        skin: Some(Skin {
            names: order
                .iter()
                .map(|i| nodes[*i].name().unwrap_or("").to_owned())
                .collect(),
            rest,
            inverse_bind: vec![Mat4::IDENTITY.to_cols_array(); nodes.len()],
            basis: basis.to_cols_array(),
        }),
    };
    let global = globals(&model);
    model.skin.as_mut().unwrap().inverse_bind =
        global.iter().map(|m| m.inverse().to_cols_array()).collect();
    for node in &nodes {
        let Some(mesh) = node.mesh() else {
            continue;
        };
        let skin = node.skin();
        let joints: Vec<_> = if let Some(skin) = skin.as_ref() {
            skin.joints().map(|n| indices[n.index()]).collect()
        } else {
            vec![indices[node.index()]]
        };
        if let Some(matrices) = skin.as_ref().and_then(|skin| {
            skin.reader(|b| Some(&buffers[b.index()].0))
                .read_inverse_bind_matrices()
        }) {
            for (joint, matrix) in joints.iter().zip(matrices) {
                model.skin.as_mut().unwrap().inverse_bind[*joint] =
                    Mat4::from_cols_array_2d(&matrix).to_cols_array();
            }
        }
        for primitive in mesh.primitives() {
            if primitive.mode() != gltf::mesh::Mode::Triangles {
                return Err("Character mesh is not triangulated".into());
            }
            let reader = primitive.reader(|b| Some(&buffers[b.index()].0));
            let positions: Vec<_> = reader
                .read_positions()
                .ok_or("Missing character positions")?
                .collect();
            let normals: Vec<_> = reader
                .read_normals()
                .ok_or("Missing character normals")?
                .collect();
            let uvs: Vec<_> = reader
                .read_tex_coords(0)
                .ok_or("Missing character UVs")?
                .into_f32()
                .collect();
            let bone_ids: Vec<_> = if skin.is_some() {
                reader
                    .read_joints(0)
                    .ok_or("Missing character joints")?
                    .into_u16()
                    .collect()
            } else {
                vec![[0; 4]; positions.len()]
            };
            let weights: Vec<_> = if skin.is_some() {
                reader
                    .read_weights(0)
                    .ok_or("Missing character weights")?
                    .into_f32()
                    .collect()
            } else {
                vec![[1., 0., 0., 0.]; positions.len()]
            };
            if normals.len() != positions.len()
                || uvs.len() != positions.len()
                || bone_ids.len() != positions.len()
                || weights.len() != positions.len()
            {
                return Err("Interchange vertex attributes have inconsistent lengths".into());
            }
            let vertex_basis = if skin.is_some() {
                basis
            } else {
                basis * global[indices[node.index()]]
            };
            let normal_basis = vertex_basis.inverse().transpose();
            let material = primitive.material();
            let color = material.pbr_metallic_roughness().base_color_factor();
            let pbr = material.pbr_metallic_roughness();
            let mut load =
                |image| material_image(pack, dir, path, &buffers, &mut image_textures, image);
            let texture = if let Some(t) = pbr.base_color_texture() {
                material_uv(t.tex_coord())?;
                load(t.texture().source())?
            } else {
                0
            };
            let mut authored = verse_engine::material::Material {
                roughness: pbr.roughness_factor(),
                metallic: pbr.metallic_factor(),
                opacity: color[3],
                alpha_cutoff: material.alpha_cutoff().unwrap_or(0.5),
                emissive_factor: material.emissive_factor(),
                ..Default::default()
            };
            if let Some(t) = material.normal_texture() {
                material_uv(t.tex_coord())?;
                authored.normal_scale = t.scale();
                authored.normal_texture = Some(load(t.texture().source())?);
            }
            if let Some(t) = pbr.metallic_roughness_texture() {
                material_uv(t.tex_coord())?;
                authored.metallic_roughness_texture = Some(load(t.texture().source())?);
            }
            if let Some(t) = material.occlusion_texture() {
                material_uv(t.tex_coord())?;
                authored.occlusion_strength = t.strength();
                authored.occlusion_texture = Some(load(t.texture().source())?);
            }
            if let Some(t) = material.emissive_texture() {
                material_uv(t.tex_coord())?;
                authored.emissive_texture = Some(load(t.texture().source())?);
            }
            authored.validate(pack.textures.len())?;
            let vertices = positions
                .into_iter()
                .enumerate()
                .map(|(i, p)| Vertex {
                    position: vertex_basis.transform_point3(p.into()).to_array(),
                    normal: normal_basis
                        .transform_vector3(normals[i].into())
                        .normalize()
                        .to_array(),
                    uv: uvs[i],
                    joints: bone_ids[i].map(|j| joints[j as usize] as u32),
                    weights: weights[i],
                })
                .collect();
            model.surfaces.push(Surface {
                material: authored,
                vertices,
                indices: reader
                    .read_indices()
                    .ok_or("Missing character indices")?
                    .into_u32()
                    .collect(),
                texture,
                blend: match material.alpha_mode() {
                    gltf::material::AlphaMode::Opaque => 0,
                    gltf::material::AlphaMode::Mask => 1,
                    gltf::material::AlphaMode::Blend => 2,
                },
                emissive: false,
                tint: [color[0], color[1], color[2]],
            });
        }
    }
    model.height = model
        .surfaces
        .iter()
        .flat_map(|s| s.vertices.iter())
        .map(|v| v.position[2])
        .fold(0., f32::max);
    Ok(model)
}
/// Composes a modular mesh onto the target rig by matching named joints.
pub fn compose(target: &mut Model, source: Model, head_only: bool) -> Result<(), String> {
    let target_skin = target.skin.as_ref().unwrap();
    let source_skin = source.skin.as_ref().unwrap();
    let mapping: Vec<_> = source_skin
        .names
        .iter()
        .map(|name| target_skin.names.iter().position(|n| n == name))
        .collect();
    let basis = Mat4::from_cols_array(&target_skin.basis);
    let inverse = basis.inverse();
    let global = globals(target);
    let convert: Vec<_> = source_skin
        .names
        .iter()
        .enumerate()
        .map(|(i, _)| {
            mapping[i].map(|j| {
                basis * global[j] * Mat4::from_cols_array(&source_skin.inverse_bind[i]) * inverse
            })
        })
        .collect();
    for mut surface in source.surfaces {
        if head_only {
            surface.indices = surface
                .indices
                .chunks_exact(3)
                .filter(|triangle| {
                    triangle.iter().all(|i| {
                        let v = &surface.vertices[*i as usize];
                        v.joints
                            .iter()
                            .zip(v.weights)
                            .filter(|(j, _)| {
                                matches!(
                                    source_skin.names[**j as usize].as_str(),
                                    "Head" | "head" | "neck_01"
                                )
                            })
                            .map(|(_, w)| w)
                            .sum::<f32>()
                            > 0.8
                    })
                })
                .flatten()
                .copied()
                .collect();
            if surface.indices.is_empty() {
                continue;
            }
        }
        // Remove covered body vertices as well as triangles, including their bounds.
        let mut used = BTreeMap::new();
        let mut vertices = Vec::new();
        for index in &mut surface.indices {
            let next = vertices.len() as u32;
            let mapped = *used.entry(*index).or_insert_with(|| {
                vertices.push(surface.vertices[*index as usize].clone());
                next
            });
            *index = mapped;
        }
        surface.vertices = vertices;
        for vertex in &mut surface.vertices {
            let mut p = Vec3::ZERO;
            let mut normal = Vec3::ZERO;
            for k in 0..4 {
                if vertex.weights[k] > 0. {
                    let old = vertex.joints[k] as usize;
                    let matrix = convert[old].ok_or_else(|| {
                        format!("Missing modular joint {}", source_skin.names[old])
                    })?;
                    p += matrix.transform_point3(vertex.position.into()) * vertex.weights[k];
                    normal += matrix.transform_vector3(vertex.normal.into()) * vertex.weights[k];
                    vertex.joints[k] = mapping[old].unwrap() as u32;
                } else {
                    vertex.joints[k] = 0;
                }
            }
            vertex.position = p.to_array();
            vertex.normal = normal.normalize_or_zero().to_array();
        }
        target.surfaces.push(surface);
    }
    Ok(())
}
fn fingers(model: &Model) -> Vec<BoneKeys> {
    let skin = model.skin.as_ref().unwrap();
    model
        .clips
        .iter()
        .find(|c| c.id == 250)
        .map_or(vec![], |clip| {
            clip.bones
                .iter()
                .filter(|b| {
                    ["thumb", "index", "middle", "ring", "pinky"]
                        .iter()
                        .any(|prefix| skin.names[b.bone].starts_with(prefix))
                })
                .cloned()
                .map(|mut b| {
                    b.rotation.truncate(1);
                    b.translation.truncate(1);
                    b.scale.truncate(1);
                    b
                })
                .collect()
        })
}
fn archery(model: &mut Model) -> Result<(), String> {
    let skin = model.skin.as_ref().unwrap();
    let global = globals(model);
    let mut hold = Clip {
        id: 109,
        duration: 1.,
        bones: fingers(model),
    };
    for side in ["l", "r"] {
        let find = |part| {
            skin.names
                .iter()
                .position(|n| n == &format!("{part}_{side}"))
                .ok_or("Missing archery joint")
        };
        let upper = find("upperarm")?;
        let lower = find("lowerarm")?;
        let hand = find("hand")?;
        let origin = global[upper].transform_point3(Vec3::ZERO);
        let elbow = global[lower].transform_point3(Vec3::ZERO);
        let wrist = global[hand].transform_point3(Vec3::ZERO);
        let target = Vec3::new(
            origin.x * if side == "l" { 0.6 } else { 0.2 },
            origin.y - 0.08,
            origin.z + if side == "l" { 0.57 } else { 0.13 },
        );
        let a = origin.distance(elbow);
        let b = elbow.distance(wrist);
        let d = origin
            .distance(target)
            .clamp((a - b).abs() + 0.001, a + b - 0.001);
        let direction = (target - origin).normalize();
        let bend = Vec3::new(origin.x.signum(), -0.3, 0.);
        let bend = (bend - direction * bend.dot(direction)).normalize();
        let x = (a * a - b * b + d * d) / (2. * d);
        let new_elbow = origin + direction * x + bend * (a * a - x * x).max(0.).sqrt();
        let upper_world = Quat::from_rotation_arc(
            (elbow - origin).normalize(),
            (new_elbow - origin).normalize(),
        ) * global[upper].to_scale_rotation_translation().1;
        let lower_world = Quat::from_rotation_arc(
            (wrist - elbow).normalize(),
            (target - new_elbow).normalize(),
        ) * global[lower].to_scale_rotation_translation().1;
        let parent = model.bones[upper].parent as usize;
        for (bone, rotation) in [
            (
                upper,
                global[parent].to_scale_rotation_translation().1.inverse() * upper_world,
            ),
            (lower, upper_world.inverse() * lower_world),
        ] {
            hold.bones.push(BoneKeys {
                bone,
                translation: vec![],
                scale: vec![],
                rotation: vec![(0., rotation.normalize().to_array())],
            });
        }
    }
    let mut shot = hold.clone();
    shot.id = 46;
    let right = skin.names.iter().position(|n| n == "upperarm_r").unwrap();
    for track in &mut shot.bones {
        if track.bone == right {
            let drawn = Quat::from_array(track.rotation[0].1);
            let release = (drawn * Quat::from_rotation_y(-0.25)).to_array();
            track.rotation = vec![
                (0., drawn.to_array()),
                (0.15, drawn.to_array()),
                (0.25, release),
                (0.7, release),
                (1., drawn.to_array()),
            ];
        }
    }
    model.clips.retain(|c| c.id != 109 && c.id != 46);
    model.clips.extend([hold, shot]);
    Ok(())
}
/// Solves a named two-bone chain in glTF rest space, with a stable bend plane.
fn chain(
    model: &Model,
    global: &[Mat4],
    parts: [&str; 3],
    side: &str,
    target: Vec3,
    bend: Vec3,
    level_end: bool,
) -> Result<Vec<(usize, Quat)>, String> {
    let skin = model.skin.as_ref().ok_or("Character has no named skin")?;
    let find = |name| {
        skin.names
            .iter()
            .position(|n| n == &format!("{name}_{side}"))
            .ok_or_else(|| format!("Missing motion joint {name}_{side}"))
    };
    let upper = find(parts[0])?;
    let lower = find(parts[1])?;
    let end = find(parts[2])?;
    let origin = global[upper].transform_point3(Vec3::ZERO);
    let elbow = global[lower].transform_point3(Vec3::ZERO);
    let wrist = global[end].transform_point3(Vec3::ZERO);
    let a = origin.distance(elbow);
    let b = elbow.distance(wrist);
    if a < 0.001 || b < 0.001 {
        return Err("Degenerate motion chain".into());
    }
    let direction = (target - origin).normalize_or_zero();
    let d = origin
        .distance(target)
        .clamp((a - b).abs() + 0.001, a + b - 0.001);
    let target = origin + direction * d;
    let bend = (bend - direction * bend.dot(direction)).normalize_or_zero();
    let x = (a * a - b * b + d * d) / (2. * d);
    let new_elbow = origin + direction * x + bend * (a * a - x * x).max(0.).sqrt();
    let upper_world = Quat::from_rotation_arc(
        (elbow - origin).normalize(),
        (new_elbow - origin).normalize(),
    ) * global[upper].to_scale_rotation_translation().1;
    let lower_world = Quat::from_rotation_arc(
        (wrist - elbow).normalize(),
        (target - new_elbow).normalize(),
    ) * global[lower].to_scale_rotation_translation().1;
    let parent = model.bones[upper].parent as usize;
    let mut result = vec![
        (
            upper,
            global[parent].to_scale_rotation_translation().1.inverse() * upper_world,
        ),
        (lower, upper_world.inverse() * lower_world),
    ];
    if level_end {
        result.push((
            end,
            lower_world.inverse() * global[end].to_scale_rotation_translation().1,
        ));
    }
    Ok(result)
}
fn humanoid_motion(model: &mut Model) -> Result<(), String> {
    let skin = model.skin.as_ref().unwrap();
    let rest_global = globals(model);
    for (id, duration, stride) in [
        (0, 3., 0.),
        (4, 0.72, 0.40),
        (5, 0.42, 0.53),
        (13, 0.55, -0.34),
        (14, 0.42, 0.45),
        (15, 0.42, -0.45),
        (25, 2., 0.),
        (51, 2., 0.),
        (52, 1., 0.),
        (53, 1.1, 0.),
    ] {
        let mut tracks: BTreeMap<usize, BoneKeys> =
            fingers(model).into_iter().map(|b| (b.bone, b)).collect();
        for sample in 0..=32 {
            let phase = sample as f32 / 32.;
            let time = phase * duration;
            let crouch = if matches!(id, 5 | 14 | 15) {
                0.17
            } else if stride != 0. {
                0.10
            } else if id == 25 || id == 51 {
                0.035
            } else {
                0.
            };
            let mut global = rest_global.clone();
            let pelvis = skin
                .names
                .iter()
                .position(|n| n == "pelvis")
                .ok_or("Missing pelvis")?;
            let delta = Vec3::Y * (-crouch + 0.005 * (phase * std::f32::consts::TAU * 2.).sin());
            for i in 0..model.bones.len() {
                let mut ancestor = i;
                while ancestor != pelvis && model.bones[ancestor].parent >= 0 {
                    ancestor = model.bones[ancestor].parent as usize;
                }
                if ancestor == pelvis {
                    global[i] = Mat4::from_translation(delta) * rest_global[i];
                }
            }
            let parent = model.bones[pelvis].parent as usize;
            let translation = Vec3::from(skin.rest[pelvis].translation)
                + rest_global[parent].inverse().transform_vector3(delta);
            tracks
                .entry(pelvis)
                .or_insert(BoneKeys {
                    bone: pelvis,
                    translation: vec![],
                    rotation: vec![],
                    scale: vec![],
                })
                .translation
                .push((time, translation.to_array()));
            for (side, offset) in [("l", 0.), ("r", 0.5)] {
                let foot = skin
                    .names
                    .iter()
                    .position(|n| n == &format!("foot_{side}"))
                    .ok_or("Missing foot")?;
                let p = (phase + offset) % 1.;
                // Contact occupies most of the cycle; only the returning foot lifts.
                let stance = if matches!(id, 5 | 14 | 15) {
                    0.40
                } else if id == 13 {
                    0.30
                } else {
                    0.46
                };
                let (forward, lift) = if p < stance {
                    (1. - 2. * p / stance, 0.)
                } else {
                    let swing = (p - stance) / (1. - stance);
                    (
                        -1. + 2. * swing,
                        (swing * std::f32::consts::PI).sin()
                            * if matches!(id, 5 | 14 | 15) {
                                0.18
                            } else {
                                0.10
                            },
                    )
                };
                let target = rest_global[foot].transform_point3(Vec3::ZERO)
                    + Vec3::new(
                        0.,
                        if stride == 0. { 0. } else { lift },
                        if matches!(id, 14 | 15) {
                            0.
                        } else {
                            forward * stride
                        },
                    )
                    + if matches!(id, 14 | 15) {
                        Vec3::X * forward * stride
                    } else {
                        Vec3::ZERO
                    };
                let legs = chain(
                    model,
                    &global,
                    ["thigh", "calf", "foot"],
                    side,
                    target,
                    Vec3::Z,
                    true,
                )?;
                let upper = skin
                    .names
                    .iter()
                    .position(|n| n == &format!("upperarm_{side}"))
                    .unwrap();
                let shoulder = global[upper].transform_point3(Vec3::ZERO);
                let sign = shoulder.x.signum();
                let pulse = (phase * std::f32::consts::PI).sin();
                let target = match id {
                    25 => shoulder + Vec3::new(-sign * 0.1, -0.12, 0.3),
                    51 => shoulder + Vec3::new(sign * 0.08, -0.28, 0.28),
                    52 => {
                        shoulder + Vec3::new(sign * 0.10, -0.22 + pulse * 0.30, 0.3 + pulse * 0.12)
                    }
                    53 => shoulder + Vec3::new(sign * 0.07, -0.02, 0.52 - 0.15 * phase),
                    5 | 14 | 15 => {
                        shoulder + Vec3::new(sign * 0.07, -0.27, 0.16 - forward * stride * 0.55)
                    }
                    13 => shoulder + Vec3::new(sign * 0.09, -0.37, 0.12 - forward * stride * 0.6),
                    _ => shoulder + Vec3::new(sign * 0.11, -0.48, 0.06 - forward * stride * 0.65),
                };
                let arms = chain(
                    model,
                    &global,
                    ["upperarm", "lowerarm", "hand"],
                    side,
                    target,
                    Vec3::new(sign, -0.25, -0.1),
                    false,
                )?;
                for (bone, rotation) in legs.into_iter().chain(arms) {
                    tracks
                        .entry(bone)
                        .or_insert(BoneKeys {
                            bone,
                            translation: vec![],
                            rotation: vec![],
                            scale: vec![],
                        })
                        .rotation
                        .push((time, rotation.normalize().to_array()));
                }
            }
        }
        model.clips.retain(|c| c.id != id);
        model.clips.push(Clip {
            id,
            duration,
            bones: tracks.into_values().collect(),
        });
    }
    Ok(())
}
fn animations(model: &mut Model, path: &Path) -> Result<(), String> {
    for (id, name) in [
        (37, "NinjaJump_Idle_Loop"),
        (64, "Idle_No_Loop"),
        (68, "Yes"),
    ] {
        retarget_clip(model, path, id, name)?;
    }
    retarget_clip(model, path, 250, "Idle_No_Loop")?;
    humanoid_motion(model)?;
    // The Standard library has no death clip. Author a fall on its actual root rig.
    let skin = model.skin.as_ref().unwrap();
    if let Some(root) = skin.names.iter().position(|n| n == "root") {
        let rest = skin.rest[root];
        let mut death = model.clips.iter().find(|c| c.id == 0).unwrap().clone();
        death.id = 1;
        death.duration = 0.85;
        for track in &mut death.bones {
            for keys in [&mut track.translation, &mut track.scale] {
                keys.truncate(1);
            }
            track.rotation.truncate(1);
        }
        death.bones.retain(|b| b.bone != root);
        let fallen = (Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)
            * Quat::from_array(rest.rotation))
        .to_array();
        death.bones.push(BoneKeys {
            bone: root,
            translation: vec![],
            scale: vec![],
            rotation: vec![(0., rest.rotation), (0.85, fallen)],
        });
        model.clips.retain(|c| c.id != 1 && c.id != 100);
        let mut prone = death.clone();
        prone.id = 100;
        prone.bones.iter_mut().for_each(|b| {
            if b.bone == root {
                b.rotation = vec![(0., fallen)];
            }
        });
        model.clips.extend([death, prone]);
    }
    archery(model)?;
    model.clips.retain(|c| c.id != 250);
    let global = globals(model);
    let skin = model.skin.as_ref().unwrap();
    // Bow attachments: 2 is the left palm, between the wrist and the middle
    // knuckle, so the grip sits inside the closed fist; 3 is the upper back;
    // 4 is the left elbow, which with the palm gives the bow arm's direction.
    let basis = Mat4::from_cols_array(&skin.basis);
    let at = |name: &str| {
        skin.names.iter().position(|n| n == name).map(|bone| {
            (
                bone,
                basis.transform_point3(global[bone].transform_point3(Vec3::ZERO)),
            )
        })
    };
    if let (Some((hand, wrist)), Some((_, knuckle))) = (at("hand_l"), at("middle_01_l")) {
        model.attachments.push(Attachment {
            id: 2,
            bone: hand,
            position: wrist.lerp(knuckle, 0.55).to_array(),
        });
    }
    for (id, name) in [(3, "spine_03"), (4, "lowerarm_l")] {
        if let Some((bone, position)) = at(name) {
            model.attachments.push(Attachment {
                id,
                bone,
                position: position.to_array(),
            });
        }
    }
    super::original::bind_states(model);
    Ok(())
}
/// Retargets any named clip from the retained 43-clip library to a composed rig.
/// The caller selects an unused state ID before adding the clip to a pack.
pub fn retarget_clip(model: &mut Model, path: &Path, id: u16, name: &str) -> Result<(), String> {
    use gltf::animation::util::ReadOutputs;
    if model.clips.iter().any(|c| c.id == id) {
        return Err("Character animation ID already exists".into());
    }
    let (doc, buffers, _) = gltf::import(path).map_err(|e| e.to_string())?;
    let skin = model.skin.as_ref().ok_or("Character has no named skin")?;

    let animation = doc
        .animations()
        .find(|a| a.name() == Some(name))
        .ok_or_else(|| format!("Missing Universal animation {name}"))?;
    let mut tracks: BTreeMap<usize, BoneKeys> = BTreeMap::new();
    let mut duration: f32 = 0.;
    for channel in animation.channels() {
        if channel.sampler().interpolation() != gltf::animation::Interpolation::Linear {
            return Err("Unsupported character animation interpolation".into());
        }
        let node = channel.target().node();
        let Some(bone) = skin
            .names
            .iter()
            .position(|n| Some(n.as_str()) == node.name())
        else {
            continue;
        };
        let reader = channel.reader(|b| Some(&buffers[b.index()].0));
        let times: Vec<_> = reader
            .read_inputs()
            .ok_or("Missing animation time")?
            .collect();
        duration = duration.max(times.last().copied().unwrap_or(0.));
        let track = tracks.entry(bone).or_insert(BoneKeys {
            bone,
            translation: vec![],
            rotation: vec![],
            scale: vec![],
        });
        let (source_t, source_r, source_s) = node.transform().decomposed();
        let target = skin.rest[bone];
        match reader.read_outputs().ok_or("Missing animation values")? {
            ReadOutputs::Translations(values) => {
                track.translation = times
                    .into_iter()
                    .zip(values.map(|v| {
                        (Vec3::from(v) - Vec3::from(source_t) + Vec3::from(target.translation))
                            .to_array()
                    }))
                    .collect()
            }
            ReadOutputs::Rotations(values) => {
                track.rotation = times
                    .into_iter()
                    .zip(values.into_f32().map(|v| {
                        (Quat::from_array(target.rotation)
                            * Quat::from_array(source_r).inverse()
                            * Quat::from_array(v))
                        .normalize()
                        .to_array()
                    }))
                    .collect()
            }
            ReadOutputs::Scales(values) => {
                track.scale = times
                    .into_iter()
                    .zip(values.map(|v| {
                        (Vec3::from(v) / Vec3::from(source_s) * Vec3::from(target.scale)).to_array()
                    }))
                    .collect()
            }
            _ => return Err("Unsupported character morph animation".into()),
        }
    }
    model.clips.push(Clip {
        id,
        duration,
        bones: tracks.into_values().collect(),
    });
    Ok(())
}
/// Installs a locally licensed Bestiary monster without redistributing source files.
pub fn install_bestiary(
    pack: &mut Pack,
    dir: &Path,
    path: &Path,
    library: &Path,
) -> Result<(), String> {
    let mut monster = import(pack, dir, path)?;
    animations(&mut monster, library)?;
    pack.models.insert("claude".into(), monster);
    pack.source_revision = "verse-bestiary-ritual-v1".into();
    pack.validate()
}
/// Installs all six Standard appearances and binds the selected player outfit.
pub fn install(pack: &mut Pack, dir: &Path, root: &Path, appearance: &str) -> Result<(), String> {
    if !APPEARANCES.contains(&appearance) {
        return Err("Unknown Universal character appearance".into());
    }
    let recipes = [
        ("male-ranger", "Male_Ranger", "Male", false),
        ("female-ranger", "Female_Ranger", "Female", false),
        ("male-peasant", "Male_Peasant", "Male", false),
        ("female-peasant", "Female_Peasant", "Female", false),
        ("superhero-male", "", "Male", true),
        ("superhero-female", "", "Female", true),
    ];
    for (name, outfit, sex, full) in recipes {
        let base_path = root.join(format!("base/Superhero_{sex}_FullBody.gltf"));
        let base = import(pack, dir, &base_path)?;
        let mut model = if full {
            base
        } else {
            let mut outfit = import(pack, dir, &root.join(format!("outfits/{outfit}.gltf")))?;
            compose(&mut outfit, base, true)?;
            outfit
        };
        if full || name.ends_with("peasant") {
            let hair = if sex == "Male" {
                "Hair_Buzzed"
            } else {
                "Hair_Buns"
            };
            compose(
                &mut model,
                import(pack, dir, &root.join(format!("hair/{hair}.gltf")))?,
                false,
            )?;
        }
        model.height = model
            .surfaces
            .iter()
            .flat_map(|s| {
                s.indices
                    .iter()
                    .map(|i| s.vertices[*i as usize].position[2])
            })
            .fold(0., f32::max);
        animations(&mut model, &root.join("animations.glb"))?;
        pack.models.insert(format!("universal-{name}"), model);
    }
    for (role, appearance) in [
        ("adventurer", appearance),
        ("cultist", "male-ranger"),
        ("cultist-female", "female-ranger"),
        ("cultist-peasant", "male-peasant"),
        ("cultist-peasant-female", "female-peasant"),
        ("claude", "male-ranger"),
        ("dummy", "male-peasant"),
    ] {
        let mut model = pack.models[&format!("universal-{appearance}")].clone();
        if role.starts_with("cultist") {
            for surface in &mut model.surfaces {
                surface.tint = [0.62, 0.32, 0.8];
            }
        }
        if role == "claude" {
            for surface in &mut model.surfaces {
                surface.tint = [0.65, 0.24, 0.35];
            }
        }
        if role == "dummy" {
            // Straw training dummies for the spell playground.
            for surface in &mut model.surfaces {
                surface.tint = [0.95, 0.78, 0.4];
            }
        }
        pack.models.insert(role.into(), model);
    }
    pack.validate()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn material_coordinates_are_explicit() {
        assert!(material_uv(0).is_ok());
        assert!(material_uv(1).is_err());
        assert!(material_uv(u32::MAX).is_err());
    }
    #[test]
    fn standard_outfits_retarget_and_expose_runtime_states() {
        let dir = std::env::temp_dir().join(format!("verse-universal-test-{}", std::process::id()));
        let mut pack = super::super::original::generate(&dir).unwrap();
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/verse/characters/quaternius");
        install(&mut pack, &dir, &root, "female-ranger").unwrap();
        for name in APPEARANCES {
            let model = &pack.models[&format!("universal-{name}")];
            assert!(model.surfaces.iter().all(|s| !s.indices.is_empty()));
            for id in [4, 5, 13, 14, 15] {
                let track = model.markers.iter().find(|track| track.clip == id).unwrap();
                track.track.validate().unwrap();
                assert_eq!(track.track.markers.len(), 2);
                assert_eq!(
                    track.track.markers[0].id,
                    verse_engine::markers::FOOTSTEP_LEFT
                );
                assert_eq!(
                    track.track.markers[1].id,
                    verse_engine::markers::FOOTSTEP_RIGHT
                );
            }

            for id in [
                0, 1, 4, 5, 13, 14, 15, 25, 37, 46, 51, 52, 53, 64, 68, 100, 109,
            ] {
                assert!(model.clips.iter().any(|c| c.id == id));
                let pose = verse_engine::animation::pose(model, id, 0.7);
                assert!(pose.iter().all(|m| m.is_finite()));
                let bounds = model
                    .surfaces
                    .iter()
                    .flat_map(|s| {
                        s.indices.iter().map(|i| {
                            let v = &s.vertices[*i as usize];
                            let p: Vec3 = v.position.into();
                            super::super::chamber::basis()
                                .transform_point3(
                                    (0..4)
                                        .map(|k| {
                                            pose[v.joints[k] as usize].transform_point3(p)
                                                * v.weights[k]
                                        })
                                        .sum(),
                                )
                                .y
                        })
                    })
                    .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), y| {
                        (lo.min(y), hi.max(y))
                    });

                assert!(bounds.0 > -3. && bounds.1 < 4.);
            }
            for id in [4, 5, 13, 14, 15] {
                let duration = model.clips.iter().find(|c| c.id == id).unwrap().duration;
                let start = verse_engine::animation::pose(model, id, 0.);
                let end = verse_engine::animation::pose(model, id, duration - 0.000001);
                assert!(
                    start.iter().zip(end).all(|(a, b)| a.abs_diff_eq(b, 0.001)),
                    "Gait seam {name} {id}"
                );
            }
            assert!(model.height > 1.5 && model.height < 3.);
        }
        let mut model = pack.models["adventurer"].clone();
        retarget_clip(
            &mut model,
            &root.join("animations.glb"),
            200,
            "TreeChopping_Loop",
        )
        .unwrap();
        assert!(model.clips.iter().any(|c| c.id == 200));
        assert!(retarget_clip(&mut model, &root.join("animations.glb"), 200, "Yes").is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
