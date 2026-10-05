//! The pack's forms: the skinned, animated models of a forms set, such as
//! the Grove's Wild Shape beasts, compiled to [`Character`]s.
//!
//! A form's glTF has one skin on one mesh. Its joints keep the source's
//! hierarchy; a node above the joints, such as Blender's armature object,
//! becomes a joint of its own that no vertex follows, so its transform
//! still places the skeleton. Each animation becomes a clip on its first
//! key's clock: step keys become linear keys that jump, and cubic-spline
//! keys keep their values. A gait's distance per loop is measured from the
//! clip itself: how far its farthest-reaching foot moves along the body.

use std::collections::BTreeMap;

use glam::{Mat4, Quat, Vec3};

use super::{
    Builder, Character, Clip, Joint, SkinnedPrimitive, SkinnedVertex, SourceSet, Track, Vertex,
    format, pack_name, reduce_keys, unit_weights, weld,
};

/// Clips that move the body, whose distance per loop is measured.
pub const GAITS: [&str; 2] = ["walk", "run"];
/// How long before a step key the previous value holds, s.
const STEP_EDGE: f32 = 1e-3;
/// Samples per loop when measuring a gait.
const GAIT_SAMPLES: usize = 48;

impl Builder<'_> {
    /// Compiles the skinned model `file` of `set` into a form named
    /// `set/<file stem>`.
    pub(super) fn form(&mut self, set: &SourceSet, file: &str) -> Result<Character, String> {
        let label = format!("{}/{file}", set.name);
        let gltf = gltf::Gltf::from_slice(&set.files[file])
            .map_err(|error| format!("{label}: {error}"))?;
        let document = &gltf.document;
        if document.extensions_required().next().is_some()
            || document.nodes().count() > super::MAX_NODES
        {
            return Err(format!(
                "{label}: forms must not require extensions and must be within the node limit"
            ));
        }
        let mut buffers = Vec::new();
        for buffer in document.buffers() {
            let gltf::buffer::Source::Uri(uri) = buffer.source() else {
                return Err(format!("{label}: binary glTF buffers are not admitted"));
            };
            let bytes = set
                .files
                .get(uri)
                .filter(|_| uri.ends_with(".bin"))
                .ok_or(format!("{label}: buffer {uri} is not admitted"))?;
            if bytes.len() < buffer.length() {
                return Err(format!("{label}: buffer {uri} is shorter than declared"));
            }
            buffers.push(bytes.as_slice());
        }
        let skins: Vec<_> = document.skins().collect();
        let [skin] = skins.as_slice() else {
            return Err(format!("{label}: a form has exactly one skin"));
        };
        let skinned: Vec<_> = document.nodes().filter(|n| n.mesh().is_some()).collect();
        let [node] = skinned.as_slice() else {
            return Err(format!("{label}: a form has exactly one mesh"));
        };
        if node.skin().map(|s| s.index()) != Some(skin.index()) {
            return Err(format!("{label}: a form's mesh must use its skin"));
        }
        let mesh = node.mesh().ok_or(format!("{label}: no mesh"))?;

        // The hierarchy, then the skeleton: every joint and every node above
        // one, parents first.
        let mut parent: BTreeMap<usize, usize> = BTreeMap::new();
        for n in document.nodes() {
            for child in n.children() {
                parent.insert(child.index(), n.index());
            }
        }
        let skin_joints: Vec<usize> = skin.joints().map(|j| j.index()).collect();
        let mut kept = std::collections::BTreeSet::new();
        for &joint in &skin_joints {
            let mut at = Some(joint);
            let mut depth = 0;
            while let Some(n) = at {
                depth += 1;
                if depth > super::MAX_NODE_DEPTH {
                    return Err(format!("{label}: node hierarchy is too deep"));
                }
                kept.insert(n);
                at = parent.get(&n).copied();
            }
        }
        if kept.contains(&node.index()) {
            return Err(format!("{label}: a form's mesh must not be a joint"));
        }
        let scene = document
            .default_scene()
            .or_else(|| document.scenes().next())
            .ok_or(format!("{label}: no scene"))?;
        let mut order = Vec::new();
        let mut stack: Vec<gltf::Node<'_>> = scene.nodes().collect();
        stack.reverse();
        while let Some(n) = stack.pop() {
            if kept.contains(&n.index()) {
                order.push(n.index());
            }
            let mut children: Vec<_> = n.children().collect();
            children.reverse();
            stack.extend(children);
        }
        if order.len() != kept.len() || order.len() > format::MAX_JOINTS {
            return Err(format!(
                "{label}: the skeleton must sit in the scene within {} joints",
                format::MAX_JOINTS
            ));
        }
        let index_of: BTreeMap<usize, usize> =
            order.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        let reader = skin.reader(|buffer| buffers.get(buffer.index()).copied());
        let binds: Vec<[[f32; 4]; 4]> = match reader.read_inverse_bind_matrices() {
            Some(matrices) => matrices.collect(),
            None => vec![Mat4::IDENTITY.to_cols_array_2d(); skin_joints.len()],
        };
        if binds.len() != skin_joints.len() {
            return Err(format!("{label}: the skin's inverse binds do not match"));
        }
        let nodes: Vec<gltf::Node<'_>> = document.nodes().collect();
        let joints: Vec<Joint> = order
            .iter()
            .map(|&n| {
                let (translation, rotation, scale) = nodes[n].transform().decomposed();
                let inverse_bind = skin_joints
                    .iter()
                    .position(|&j| j == n)
                    .map_or(Mat4::IDENTITY.to_cols_array(), |i| {
                        Mat4::from_cols_array_2d(&binds[i]).to_cols_array()
                    });
                Joint {
                    parent: parent
                        .get(&n)
                        .and_then(|p| index_of.get(p))
                        .map_or(-1, |&p| p as i16),
                    translation,
                    rotation,
                    scale,
                    inverse_bind,
                }
            })
            .collect();
        // The skin's joint list, as skeleton indices.
        let skin_to_joint: Vec<u8> = skin_joints.iter().map(|j| index_of[j] as u8).collect();

        let mut primitives: Vec<SkinnedPrimitive> = Vec::new();
        for primitive in mesh.primitives() {
            if primitive.mode() != gltf::mesh::Mode::Triangles
                || primitive.morph_targets().next().is_some()
            {
                return Err(format!("{label}: only skinned triangle lists are admitted"));
            }
            let source = primitive.material();
            if source.index().is_none() {
                return Err(format!("{label}: a primitive has no material"));
            }
            let material = self.material(set, source)?;
            let reader = primitive.reader(|buffer| buffers.get(buffer.index()).copied());
            let positions: Vec<[f32; 3]> = reader
                .read_positions()
                .ok_or(format!("{label}: a primitive has no positions"))?
                .collect();
            let normals: Vec<[f32; 3]> = reader
                .read_normals()
                .ok_or(format!("{label}: a primitive has no normals"))?
                .collect();
            let uvs: Vec<[f32; 2]> = match reader.read_tex_coords(0) {
                Some(coords) => coords.into_f32().collect(),
                None if self.contents.materials[material as usize].texture.is_none() => {
                    vec![[0.0; 2]; positions.len()]
                }
                None => return Err(format!("{label}: a textured primitive has no coordinates")),
            };
            let influences: Vec<[u16; 4]> = reader
                .read_joints(0)
                .ok_or(format!("{label}: a primitive has no joints"))?
                .into_u16()
                .collect();
            let weights: Vec<[f32; 4]> = reader
                .read_weights(0)
                .ok_or(format!("{label}: a primitive has no weights"))?
                .into_f32()
                .collect();
            let count = positions.len();
            if count == 0
                || count > u16::MAX as usize + 1
                || normals.len() != count
                || uvs.len() != count
                || influences.len() != count
                || weights.len() != count
            {
                return Err(format!("{label}: a primitive has mismatched attributes"));
            }
            let indices: Vec<u32> = match reader.read_indices() {
                Some(indices) => indices.into_u32().collect(),
                None => (0..count as u32).collect(),
            };
            if indices.is_empty()
                || !indices.len().is_multiple_of(3)
                || indices.iter().any(|&i| i as usize >= count)
            {
                return Err(format!("{label}: a primitive has invalid indices"));
            }
            let mut vertices = Vec::with_capacity(count);
            for i in 0..count {
                let mut joints = [0u8; 4];
                for k in 0..4 {
                    let j = usize::from(influences[i][k]);
                    joints[k] = if weights[i][k] > 0.0 {
                        *skin_to_joint
                            .get(j)
                            .ok_or(format!("{label}: a vertex names a missing joint"))?
                    } else {
                        0
                    };
                }
                vertices.push(SkinnedVertex {
                    vertex: Vertex {
                        position: positions[i],
                        normal: Vec3::from(normals[i]).normalize_or_zero().to_array(),
                        uv: uvs[i],
                        color: [255; 4],
                    },
                    joints,
                    weights: unit_weights(weights[i]),
                });
            }
            let limit = u16::MAX as usize + 1;
            if let Some(existing) = primitives
                .iter_mut()
                .find(|p| p.material == material && p.vertices.len() + count <= limit)
            {
                let base = existing.vertices.len() as u32;
                existing.vertices.extend(vertices);
                existing.indices.extend(indices.iter().map(|i| i + base));
            } else {
                primitives.push(SkinnedPrimitive {
                    material,
                    vertices,
                    indices,
                });
            }
        }
        for primitive in &mut primitives {
            weld(primitive);
        }

        let mut clips = Vec::new();
        for animation in document.animations() {
            let name = animation
                .name()
                .ok_or(format!("{label}: an animation has no name"))?;
            let mut tracks: BTreeMap<u16, Track> = BTreeMap::new();
            let mut start = f32::INFINITY;
            let mut end = 0.0f32;
            let mut channels = Vec::new();
            for channel in animation.channels() {
                let Some(&joint) = index_of.get(&channel.target().node().index()) else {
                    continue;
                };
                let reader = channel.reader(|buffer| buffers.get(buffer.index()).copied());
                let times: Vec<f32> = reader
                    .read_inputs()
                    .ok_or(format!("{label}: a channel has no times"))?
                    .collect();
                let outputs = reader
                    .read_outputs()
                    .ok_or(format!("{label}: a channel has no values"))?;
                let interpolation = channel.sampler().interpolation();
                let values: Vec<Vec<f32>> = match outputs {
                    gltf::animation::util::ReadOutputs::Translations(v) => {
                        v.map(|x| x.to_vec()).collect()
                    }
                    gltf::animation::util::ReadOutputs::Rotations(v) => {
                        v.into_f32().map(|x| x.to_vec()).collect()
                    }
                    gltf::animation::util::ReadOutputs::Scales(v) => {
                        v.map(|x| x.to_vec()).collect()
                    }
                    gltf::animation::util::ReadOutputs::MorphTargetWeights(_) => continue,
                };
                let values = match interpolation {
                    // In-tangent, value, out-tangent per key.
                    gltf::animation::Interpolation::CubicSpline => {
                        values.chunks_exact(3).map(|c| c[1].clone()).collect()
                    }
                    _ => values,
                };
                if times.is_empty() || times.len() != values.len() {
                    return Err(format!("{label}: a channel's keys do not match"));
                }
                if times.iter().any(|t| !t.is_finite()) || times.windows(2).any(|w| w[1] < w[0]) {
                    return Err(format!("{label}: a channel's times are invalid"));
                }
                start = start.min(times[0]);
                end = end.max(times[times.len() - 1]);
                let step = interpolation == gltf::animation::Interpolation::Step;
                let mut keys: Vec<(f32, Vec<f32>)> = Vec::new();
                for (i, (&t, v)) in times.iter().zip(&values).enumerate() {
                    if step && i > 0 {
                        let held = (t - STEP_EDGE).max(times[i - 1]);
                        keys.push((held, values[i - 1].clone()));
                    }
                    keys.push((t, v.clone()));
                }
                channels.push((joint as u16, channel.target().property(), keys));
            }
            if channels.is_empty() || !start.is_finite() {
                return Err(format!("{label}: animation {name} keys no joint"));
            }
            let duration = end - start;
            if duration <= 0.0 || duration > format::MAX_CLIP_SECONDS {
                return Err(format!("{label}: animation {name} has an invalid length"));
            }
            for (joint, property, keys) in channels {
                let track = tracks.entry(joint).or_insert_with(|| Track {
                    joint,
                    translation: Vec::new(),
                    rotation: Vec::new(),
                    scale: Vec::new(),
                });
                let shifted = keys
                    .iter()
                    .map(|(t, v)| ((t - start).clamp(0.0, duration), v));
                use gltf::animation::Property;
                match property {
                    Property::Translation => {
                        let keys: Vec<(f32, [f32; 3])> =
                            shifted.map(|(t, v)| (t, [v[0], v[1], v[2]])).collect();
                        track.translation = reduce_keys(&keys);
                    }
                    Property::Rotation => {
                        let keys: Vec<(f32, [f32; 4])> = shifted
                            .map(|(t, v)| {
                                let q = Quat::from_array([v[0], v[1], v[2], v[3]]).normalize();
                                (t, q.to_array())
                            })
                            .collect();
                        track.rotation = reduce_keys(&keys);
                    }
                    Property::Scale => {
                        let keys: Vec<(f32, [f32; 3])> =
                            shifted.map(|(t, v)| (t, [v[0], v[1], v[2]])).collect();
                        track.scale = reduce_keys(&keys);
                    }
                    Property::MorphTargetWeights => {}
                }
            }
            let mut clip = Clip {
                name: pack_name(name),
                duration,
                distance: 0.0,
                tracks: tracks.into_values().collect(),
            };
            if GAITS.contains(&clip.name.as_str()) {
                clip.distance = stride(&joints, &clip);
            }
            clips.push(clip);
        }
        if clips.is_empty() || clips.len() > format::MAX_CLIPS {
            return Err(format!(
                "{label}: a form has 1 to {} clips",
                format::MAX_CLIPS
            ));
        }
        clips.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(Character {
            name: format!("{}/{}", set.name, pack_name(file.trim_end_matches(".gltf"))),
            joints,
            primitives,
            clips,
        })
    }
}

/// Samples a channel at `t` with linear interpolation, or `rest` without
/// keys.
fn sample<const N: usize>(keys: &[(f32, [f32; N])], t: f32, rest: [f32; N]) -> [f32; N] {
    match keys {
        [] => rest,
        [only] => only.1,
        _ => {
            let after = keys.iter().position(|k| k.0 > t).unwrap_or(keys.len());
            if after == 0 {
                return keys[0].1;
            }
            if after == keys.len() {
                return keys[keys.len() - 1].1;
            }
            let (a, b) = (keys[after - 1], keys[after]);
            let span = (b.0 - a.0).max(1e-6);
            let k = ((t - a.0) / span).clamp(0.0, 1.0);
            std::array::from_fn(|c| a.1[c] + (b.1[c] - a.1[c]) * k)
        }
    }
}

/// The meters a gait carries the body over one loop: how far along the
/// body's axis (+Z) its farthest-reaching joint without children moves, a
/// foot's stride. A foot planted on the ground slides back by one stride
/// while the body moves forward by the same distance.
pub fn stride(joints: &[Joint], clip: &Clip) -> f32 {
    let leaves: Vec<usize> = (0..joints.len())
        .filter(|&i| !joints.iter().any(|j| j.parent == i as i16))
        .collect();
    let mut low = vec![f32::INFINITY; joints.len()];
    let mut high = vec![f32::NEG_INFINITY; joints.len()];
    for s in 0..GAIT_SAMPLES {
        let t = clip.duration * s as f32 / GAIT_SAMPLES as f32;
        let mut world = vec![Mat4::IDENTITY; joints.len()];
        for (i, joint) in joints.iter().enumerate() {
            let track = clip.tracks.iter().find(|k| usize::from(k.joint) == i);
            let (translation, rotation, scale) = match track {
                Some(track) => (
                    sample(&track.translation, t, joint.translation),
                    sample(&track.rotation, t, joint.rotation),
                    sample(&track.scale, t, joint.scale),
                ),
                None => (joint.translation, joint.rotation, joint.scale),
            };
            let local = Mat4::from_scale_rotation_translation(
                scale.into(),
                Quat::from_array(rotation).normalize(),
                translation.into(),
            );
            world[i] = match usize::try_from(joint.parent) {
                Ok(p) => world[p] * local,
                Err(_) => local,
            };
        }
        for &leaf in &leaves {
            let z = world[leaf].w_axis.z;
            low[leaf] = low[leaf].min(z);
            high[leaf] = high[leaf].max(z);
        }
    }
    leaves
        .iter()
        .map(|&leaf| high[leaf] - low[leaf])
        .filter(|d| d.is_finite())
        .fold(0.0, f32::max)
        .min(format::MAX_COORDINATE)
}
