//! Licensed character composition and glTF animation compilation.
//!
//! The rig, pose, and clip arithmetic runs in `f64` on glam's scalar types
//! and rounds to `f32` once, at the end. glam's `f32` quaternions and 4x4
//! matrices use SIMD whose horizontal sums add in a different order on
//! aarch64 than on x86_64, and the platform's `sin` rounds differently
//! between operating systems, so `f32` arithmetic compiled the same sources
//! to packs that differed in the last bit on different machines. Sines come
//! from `libm`, which is the same Rust code everywhere.
use glam::{DMat4 as Mat4, DQuat as Quat, DVec3 as Vec3};
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
/// Our original characters, beside the Universal appearances: each is a
/// whole model on the Universal rig rather than an outfit composed with a
/// base head.
pub const ORIGINAL_APPEARANCES: [&str; 1] = ["alice"];
/// Alice's variants and their triangle budgets
/// (`docs/verse/female-character.md`): `lod0` for the chamber, `lod1` for
/// the Everglade pack, `lod2` for phones once packs split by tier, and
/// `lod3` for distant players once skinned levels of detail exist.
pub const ALICE_VARIANTS: [(&str, usize); 4] = [
    ("lod0", 24_000),
    ("lod1", 16_000),
    ("lod2", 10_000),
    ("lod3", 3_000),
];
/// A position or scale read from a pack.
fn v3(v: [f32; 3]) -> Vec3 {
    glam::Vec3::from(v).as_dvec3()
}
/// A rotation read from a pack.
fn q4(q: [f32; 4]) -> Quat {
    glam::Quat::from_array(q).as_dquat()
}
/// A matrix read from a pack.
fn m4(m: &[f32; 16]) -> Mat4 {
    glam::Mat4::from_cols_array(m).as_dmat4()
}
/// A position or scale written to a pack.
fn f3(v: Vec3) -> [f32; 3] {
    v.as_vec3().to_array()
}
/// A rotation written to a pack.
fn f4(q: Quat) -> [f32; 4] {
    q.as_quat().to_array()
}
/// A matrix written to a pack.
fn f16(m: Mat4) -> [f32; 16] {
    m.as_mat4().to_cols_array()
}
/// A rotation of `angle` radians about the x axis, with `libm`'s sine.
fn rotation_x(angle: f64) -> Quat {
    Quat::from_xyzw(libm::sin(angle / 2.), 0., 0., libm::cos(angle / 2.))
}
/// A rotation of `angle` radians about the y axis, with `libm`'s sine.
fn rotation_y(angle: f64) -> Quat {
    Quat::from_xyzw(0., libm::sin(angle / 2.), 0., libm::cos(angle / 2.))
}
fn local(r: RestPose) -> Mat4 {
    Mat4::from_scale_rotation_translation(v3(r.scale), q4(r.rotation), v3(r.translation))
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
    base: Option<&Path>,
    buffers: &[gltf::buffer::Data],
    cache: &mut BTreeMap<usize, usize>,
    source: gltf::image::Image<'_>,
) -> Result<usize, String> {
    if let Some(index) = cache.get(&source.index()) {
        return Ok(*index);
    }
    let bytes = match source.source() {
        gltf::image::Source::Uri { uri, .. } => {
            let base = base.ok_or_else(|| format!("An in-memory model refers to {uri}"))?;
            std::fs::read(base.join(uri)).map_err(|e| e.to_string())?
        }
        gltf::image::Source::View { view, .. } => {
            buffers[view.buffer().index()].0[view.offset()..view.offset() + view.length()].to_vec()
        }
    };
    let file = format!("universal-source-{:x}.png", Sha256::digest(&bytes));
    let index = if let Some(i) = pack.textures.iter().position(|t| t.file == file) {
        i
    } else {
        let data = gltf::image::Data::from_source(source.source(), base, buffers)
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
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    import_bytes(pack, dir, &name, &bytes, path.parent())
}

/// Imports a model from the glTF file `bytes`, named `name`, as [`import`]
/// does. A buffer or image the file refers to by URI is read relative to
/// `base`; a binary glTF file that carries its own needs none.
///
/// # Errors
///
/// Returns a message when the file cannot be parsed or imported.
pub fn import_bytes(
    pack: &mut Pack,
    dir: &Path,
    name: &str,
    bytes: &[u8],
    base: Option<&Path>,
) -> Result<Model, String> {
    let gltf = gltf::Gltf::from_slice(bytes).map_err(|e| format!("{name}: {e}"))?;
    let buffers =
        gltf::import_buffers(&gltf.document, base, gltf.blob).map_err(|e| e.to_string())?;
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
    let basis =
        crate::basis().as_dmat4().inverse() * Mat4::from_quat(rotation_y(std::f64::consts::PI));
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
        graph: None,
        markers: Vec::new(),
        states: Default::default(),
        source: format!("verse/interchange/{name}"),
        source_sha256: format!("{:x}", Sha256::digest(bytes)),
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
            inverse_bind: vec![f16(Mat4::IDENTITY); nodes.len()],
            basis: f16(basis),
        }),
    };
    let global = globals(&model);
    model.skin.as_mut().unwrap().inverse_bind = global.iter().map(|m| f16(m.inverse())).collect();
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
                    glam::Mat4::from_cols_array_2d(&matrix).to_cols_array();
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
                |image| material_image(pack, dir, base, &buffers, &mut image_textures, image);
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
                    position: f3(vertex_basis.transform_point3(v3(p))),
                    normal: f3(normal_basis.transform_vector3(v3(normals[i])).normalize()),
                    uv: uvs[i],
                    joints: bone_ids[i].map(|j| joints[j as usize] as u32),
                    weights: weights[i],
                })
                .collect();
            model.surfaces.push(Surface {
                topology: Default::default(),
                unlit: false,
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
    let basis = m4(&target_skin.basis);
    let inverse = basis.inverse();
    let global = globals(target);
    let convert: Vec<_> = source_skin
        .names
        .iter()
        .enumerate()
        .map(|(i, _)| {
            mapping[i].map(|j| basis * global[j] * m4(&source_skin.inverse_bind[i]) * inverse)
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
                    p +=
                        matrix.transform_point3(v3(vertex.position)) * f64::from(vertex.weights[k]);
                    normal +=
                        matrix.transform_vector3(v3(vertex.normal)) * f64::from(vertex.weights[k]);
                    vertex.joints[k] = mapping[old].unwrap() as u32;
                } else {
                    vertex.joints[k] = 0;
                }
            }
            vertex.position = f3(p);
            vertex.normal = f3(normal.normalize_or_zero());
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
                rotation: vec![(0., f4(rotation.normalize()))],
            });
        }
    }
    let mut shot = hold.clone();
    shot.id = 46;
    let right = skin.names.iter().position(|n| n == "upperarm_r").unwrap();
    for track in &mut shot.bones {
        if track.bone == right {
            let drawn = track.rotation[0].1;
            let release = f4(q4(drawn) * rotation_y(-0.25));
            track.rotation = vec![
                (0., drawn),
                (0.15, drawn),
                (0.25, release),
                (0.7, release),
                (1., drawn),
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
/// The authored humanoid clips: ID, seconds per loop, and how far a foot
/// reaches ahead of and behind its rest point, m.
const MOTIONS: [(u16, f32, f32); 10] = [
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
];
/// Gait clips from the original Universal Animation Library (`gaits.glb`,
/// no root motion): state ID, clip name, and meters its root-motion twin
/// travels over one loop.
const GAITS: [(u16, &str, f32); 2] = [(4, "Walk_Loop", 1.3), (5, "Jog_Fwd_Loop", 5.0)];
/// The share of a gait cycle a foot spends on the ground.
fn stance(id: u16) -> f32 {
    if matches!(id, 5 | 14 | 15) {
        0.40
    } else if id == 13 {
        0.30
    } else {
        0.46
    }
}
/// Meters the body travels over one loop of authored clip `id`, while a
/// planted foot sweeps its full stride; zero for a clip without a stride.
#[must_use]
pub fn loop_distance(id: u16) -> f32 {
    if let Some(&(_, _, distance)) = GAITS.iter().find(|g| g.0 == id) {
        return distance;
    }
    MOTIONS
        .iter()
        .find(|m| m.0 == id)
        .map_or(0., |&(_, _, stride)| 2. * stride.abs() / stance(id))
}
fn humanoid_motion(model: &mut Model) -> Result<(), String> {
    let skin = model.skin.as_ref().unwrap();
    let rest_global = globals(model);
    for (id, duration, stride) in MOTIONS {
        let mut tracks: BTreeMap<usize, BoneKeys> =
            fingers(model).into_iter().map(|b| (b.bone, b)).collect();
        for sample in 0..=32 {
            let phase = sample as f32 / 32.;
            let time = phase * duration;
            let (phase, stride) = (f64::from(phase), f64::from(stride));
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
            let delta = Vec3::Y * (-crouch + 0.005 * libm::sin(phase * std::f64::consts::TAU * 2.));
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
            let translation = v3(skin.rest[pelvis].translation)
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
                .push((time, f3(translation)));
            for (side, offset) in [("l", 0.), ("r", 0.5)] {
                let foot = skin
                    .names
                    .iter()
                    .position(|n| n == &format!("foot_{side}"))
                    .ok_or("Missing foot")?;
                let p = (phase + offset) % 1.;
                // Contact occupies most of the cycle; only the returning foot lifts.
                let stance = f64::from(stance(id));
                let (forward, lift) = if p < stance {
                    (1. - 2. * p / stance, 0.)
                } else {
                    let swing = (p - stance) / (1. - stance);
                    (
                        -1. + 2. * swing,
                        libm::sin(swing * std::f64::consts::PI)
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
                // The arm's length at rest: shoulder to elbow to wrist.
                let joint = |name: &str| {
                    skin.names
                        .iter()
                        .position(|n| n == &format!("{name}_{side}"))
                        .map(|i| global[i].transform_point3(Vec3::ZERO))
                };
                let reach = match (joint("lowerarm"), joint("hand")) {
                    (Some(elbow), Some(wrist)) => shoulder.distance(elbow) + elbow.distance(wrist),
                    _ => 0.55,
                };
                let pulse = libm::sin(phase * std::f64::consts::PI);
                let target = match id {
                    25 => shoulder + Vec3::new(-sign * 0.1, -0.12, 0.3),
                    51 => shoulder + Vec3::new(sign * 0.08, -0.28, 0.28),
                    52 => {
                        shoulder + Vec3::new(sign * 0.10, -0.22 + pulse * 0.30, 0.3 + pulse * 0.12)
                    }
                    53 => shoulder + Vec3::new(sign * 0.07, -0.02, 0.52 - 0.15 * phase),
                    // Gaits: the hand hangs near the arm's full reach
                    // beside the thigh and swings against the leg on its
                    // side; a run bends the elbow more and swings further.
                    0 | 4 | 13 => {
                        shoulder
                            + Vec3::new(sign * 0.07, -1.0, 0.04 - forward * stride * 0.85)
                                .normalize()
                                * reach
                                * 0.95
                    }
                    5 | 14 | 15 => {
                        shoulder
                            + Vec3::new(sign * 0.08, -1.0, 0.30 - forward * stride * 1.1)
                                .normalize()
                                * reach
                                * 0.78
                    }
                    _ => shoulder + Vec3::new(sign * 0.11, -0.48, 0.06 - forward * stride * 0.65),
                };
                // A gait's elbow bends back, behind the arm, never out to
                // the side.
                let pole = if matches!(id, 0 | 4 | 5 | 13 | 14 | 15) {
                    Vec3::new(sign * 0.15, 0.0, -1.0)
                } else {
                    Vec3::new(sign, -0.25, -0.1)
                };
                let arms = chain(
                    model,
                    &global,
                    ["upperarm", "lowerarm", "hand"],
                    side,
                    target,
                    pole,
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
                        .push((time, f4(rotation.normalize())));
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
    // The original library's walk and jog replace the authored walk and run
    // (owner, 2026-10-04): real arm swing on the same rig.
    let gaits = path.with_file_name("gaits.glb");
    for (id, name, _) in GAITS {
        model.clips.retain(|c| c.id != id);
        retarget_clip(model, &gaits, id, name)?;
        // Close the loop exactly: the library's last key is a hair off its
        // first, which shows as a hitch once a cycle.
        if let Some(clip) = model.clips.iter_mut().find(|c| c.id == id) {
            for bone in &mut clip.bones {
                if let Some(first) = bone.translation.first().map(|k| k.1) {
                    bone.translation.last_mut().unwrap().1 = first;
                }
                if let Some(first) = bone.rotation.first().map(|k| k.1) {
                    bone.rotation.last_mut().unwrap().1 = first;
                }
                if let Some(first) = bone.scale.first().map(|k| k.1) {
                    bone.scale.last_mut().unwrap().1 = first;
                }
            }
        }
    }
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
        let fallen = f4(rotation_x(std::f64::consts::FRAC_PI_2) * q4(rest.rotation));
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
    let basis = m4(&skin.basis);
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
            position: f3(wrist.lerp(knuckle, 0.55)),
        });
    }
    for (id, name) in [(3, "spine_03"), (4, "lowerarm_l"), (5, "Head")] {
        if let Some((bone, position)) = at(name) {
            model.attachments.push(Attachment {
                id,
                bone,
                position: f3(position),
            });
        }
    }
    if let (Some((hand, wrist)), Some((_, knuckle))) = (at("hand_r"), at("middle_01_r")) {
        model.attachments.push(Attachment {
            id: 6,
            bone: hand,
            position: f3(wrist.lerp(knuckle, 0.55)),
        });
    }
    super::original::bind_states(model);
    model.graph = Some(verse_engine::animation_graph::Authored::from_locomotion(
        model,
        verse_engine::locomotion::Definition::universal(0),
    )?);
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
                    .zip(values.map(|v| f3(v3(v) - v3(source_t) + v3(target.translation))))
                    .collect()
            }
            ReadOutputs::Rotations(values) => {
                track.rotation = times
                    .into_iter()
                    .zip(values.into_f32().map(|v| {
                        f4((q4(target.rotation) * q4(source_r).inverse() * q4(v)).normalize())
                    }))
                    .collect()
            }
            ReadOutputs::Scales(values) => {
                track.scale = times
                    .into_iter()
                    .zip(values.map(|v| f3(v3(v) / v3(source_s) * v3(target.scale))))
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
/// Composes one Standard appearance on its outfit's rig: the outfit, the base
/// body's head, hair where the outfit has no hood, and the runtime clips and
/// states the chamber plays.
pub fn appearance(pack: &mut Pack, dir: &Path, root: &Path, name: &str) -> Result<Model, String> {
    let (outfit, sex, full) = match name {
        "male-ranger" => ("Male_Ranger", "Male", false),
        "female-ranger" => ("Female_Ranger", "Female", false),
        "male-peasant" => ("Male_Peasant", "Male", false),
        "female-peasant" => ("Female_Peasant", "Female", false),
        "superhero-male" => ("", "Male", true),
        "superhero-female" => ("", "Female", true),
        _ => return Err("Unknown Universal character appearance".into()),
    };
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
    Ok(model)
}
/// Imports Alice's `variant` (`alice.<variant>.gltf` under `alice_root`,
/// built by `scripts/blender/alice.py`) and gives her the runtime clips and
/// states every Universal appearance has, from the Universal Animation
/// Library under `universal_root`. Her skeleton is the Universal rig's, so
/// every clip retargets exactly.
///
/// # Errors
///
/// Returns a message when the variant is unknown, or its model cannot be
/// imported or animated.
pub fn alice(
    pack: &mut Pack,
    dir: &Path,
    universal_root: &Path,
    alice_root: &Path,
    variant: &str,
) -> Result<Model, String> {
    if !ALICE_VARIANTS.iter().any(|(name, _)| *name == variant) {
        return Err("Unknown Alice variant".into());
    }
    let mut model = import(pack, dir, &alice_root.join(format!("alice.{variant}.gltf")))?;
    // Her height is her crown's, not the staff's on her back.
    let head = model
        .skin
        .as_ref()
        .and_then(|skin| skin.names.iter().position(|n| n == "Head"))
        .ok_or("Alice has no head joint")? as u32;
    model.height = model
        .surfaces
        .iter()
        .flat_map(|s| s.vertices.iter())
        .filter(|v| (0..4).any(|k| v.joints[k] == head && v.weights[k] > 0.5))
        .map(|v| v.position[2])
        .fold(0., f32::max);
    animations(&mut model, &universal_root.join("animations.glb"))?;
    relax_arms(&mut model, ALICE_ARMS_OUT)?;
    Ok(model)
}
/// How far Alice's upper arms turn out from her sides in every clip,
/// radians: her coat flares over her hips, and the Universal clips hang a
/// slimmer figure's arms, which would sink her hands into it.
pub const ALICE_ARMS_OUT: f64 = 0.17;
/// Turns both upper arms out from the body by `angle` radians, about the
/// axis the character faces along, in every clip's keys.
fn relax_arms(model: &mut Model, angle: f64) -> Result<(), String> {
    let global = globals(model);
    let skin = model.skin.as_ref().ok_or("The model has no skin")?;
    let mut turns = Vec::new();
    for (side, sign) in [("l", 1.0), ("r", -1.0)] {
        let upper = skin
            .names
            .iter()
            .position(|n| *n == format!("upperarm_{side}"))
            .ok_or("Missing upper arm")?;
        let parent = usize::try_from(model.bones[upper].parent).map_err(|_| "Unparented arm")?;
        let rest = global[parent].to_scale_rotation_translation().1;
        // The model faces +Z with its left at +X: a turn about +Z raises the
        // left arm outward, and the opposite turn the right.
        let half = sign * angle / 2.;
        let turn = Quat::from_xyzw(0., 0., libm::sin(half), libm::cos(half));
        turns.push((upper, rest.inverse() * turn * rest));
    }
    for clip in &mut model.clips {
        for track in &mut clip.bones {
            if let Some((_, turn)) = turns.iter().find(|(bone, _)| *bone == track.bone) {
                for key in &mut track.rotation {
                    key.1 = f4((*turn * q4(key.1)).normalize());
                }
            }
        }
    }
    Ok(())
}
/// Installs Alice's chamber variant as `universal-alice`, a character a
/// scene can place. She is an NPC, never the player's body.
///
/// # Errors
///
/// Returns a message when her model cannot be built or the pack is invalid.
pub fn install_alice(
    pack: &mut Pack,
    dir: &Path,
    universal_root: &Path,
    alice_root: &Path,
) -> Result<(), String> {
    let model = alice(pack, dir, universal_root, alice_root, "lod0")?;
    pack.models.insert("universal-alice".into(), model);
    pack.validate()
}
/// Installs all six Standard appearances and binds the selected player outfit.
pub fn install(pack: &mut Pack, dir: &Path, root: &Path, appearance: &str) -> Result<(), String> {
    if !APPEARANCES.contains(&appearance) {
        return Err("Unknown Universal character appearance".into());
    }
    for name in APPEARANCES {
        let model = self::appearance(pack, dir, root, name)?;
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
        let mut pack = crate::compiler::original::generate(&dir).unwrap();
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
                            let p: glam::Vec3 = v.position.into();
                            crate::basis()
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

    /// Every Alice variant imports on the Universal rig with its rest
    /// transforms, keeps its triangle budget and one to four influences a
    /// vertex, plays every clip the states bind without leaving a person's
    /// bounds, and closes her gait loops.
    #[test]
    fn alice_variants_retarget_within_budget() {
        let dir = std::env::temp_dir().join(format!("verse-alice-test-{}", std::process::id()));
        let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/verse/characters");
        let root = assets.join("quaternius");
        let alice_root = assets.join("original/alice");
        super::super::inventory::verify_alice(&alice_root).unwrap();
        let mut pack = crate::compiler::original::generate(&dir).unwrap();
        let reference = self::appearance(&mut pack, &dir, &root, "female-ranger").unwrap();
        let reference = reference.skin.as_ref().unwrap();
        for (variant, budget) in ALICE_VARIANTS {
            let model = alice(&mut pack, &dir, &root, &alice_root, variant).unwrap();
            let skin = model.skin.as_ref().unwrap();
            // The Universal 65 joints, plus the mesh's and the armature's
            // own nodes, which the importer keeps as bones too.
            let joints = skin
                .names
                .iter()
                .filter(|n| !matches!(n.as_str(), "Alice" | "Armature"))
                .count();
            assert_eq!(joints, 65);
            assert_eq!(skin.names.len(), 67);
            for (i, name) in skin.names.iter().enumerate() {
                if matches!(name.as_str(), "Alice" | "Armature") {
                    continue;
                }
                let j = reference.names.iter().position(|n| n == name).expect(name);
                let (a, b) = (skin.rest[i], reference.rest[j]);
                for k in 0..3 {
                    assert!((a.translation[k] - b.translation[k]).abs() < 1e-4, "{name}");
                }
                for k in 0..4 {
                    assert!((a.rotation[k] - b.rotation[k]).abs() < 1e-4, "{name}");
                }
            }
            let triangles: usize = model.surfaces.iter().map(|s| s.indices.len() / 3).sum();
            assert!(triangles <= budget, "{variant}: {triangles} > {budget}");
            for v in model.surfaces.iter().flat_map(|s| &s.vertices) {
                let used = v.weights.iter().filter(|w| **w > 0.).count();
                assert!((1..=4).contains(&used));
                assert!((v.weights.iter().sum::<f32>() - 1.).abs() < 0.01);
            }
            assert!(
                model.height > 1.7 && model.height < 2.1,
                "{variant}: {}",
                model.height
            );
            for id in [
                0, 1, 4, 5, 13, 14, 15, 25, 37, 46, 51, 52, 53, 64, 68, 100, 109,
            ] {
                assert!(model.clips.iter().any(|c| c.id == id), "{variant} {id}");
                let pose = verse_engine::animation::pose(&model, id, 0.4);
                assert!(pose.iter().all(|m| m.is_finite()));
                let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
                for s in &model.surfaces {
                    for i in &s.indices {
                        let v = &s.vertices[*i as usize];
                        let p: glam::Vec3 = v.position.into();
                        let at: glam::Vec3 = (0..4)
                            .map(|k| pose[v.joints[k] as usize].transform_point3(p) * v.weights[k])
                            .sum();
                        let y = crate::basis().transform_point3(at).y;
                        lo = lo.min(y);
                        hi = hi.max(y);
                    }
                }
                assert!(lo > -1. && hi < 2.6, "{variant} clip {id}: {lo}..{hi}");
            }
            for id in [4, 5, 13, 14, 15] {
                let duration = model.clips.iter().find(|c| c.id == id).unwrap().duration;
                let start = verse_engine::animation::pose(&model, id, 0.);
                let end = verse_engine::animation::pose(&model, id, duration - 0.000001);
                assert!(start.iter().zip(end).all(|(a, b)| a.abs_diff_eq(b, 0.001)));
            }
        }
        // Standing idle and mid-walk, her hands hang clear of her coat's
        // hips (0.21 m from her center at most).
        let model = alice(&mut pack, &dir, &root, &alice_root, "lod1").unwrap();
        let skin = model.skin.as_ref().unwrap();
        for id in [0, 4] {
            let pose = verse_engine::animation::pose(&model, id, 0.3);
            for side in ["l", "r"] {
                let hand = skin
                    .names
                    .iter()
                    .position(|n| *n == format!("middle_01_{side}"))
                    .unwrap();
                let bind = glam::Mat4::from_cols_array(&skin.inverse_bind[hand])
                    .inverse()
                    .w_axis
                    .truncate();
                let at = pose[hand].transform_point3(bind);
                let at = crate::basis().transform_point3(at);
                assert!(at.x.abs() > 0.24, "clip {id} {side}: hand at {at}");
            }
        }
        assert!(alice(&mut pack, &dir, &root, &alice_root, "lod9").is_err());
        install(&mut pack, &dir, &root, "male-ranger").unwrap();
        install_alice(&mut pack, &dir, &root, &alice_root).unwrap();
        assert!(pack.models.contains_key("universal-alice"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
