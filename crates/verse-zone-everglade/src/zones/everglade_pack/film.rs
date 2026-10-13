//! Film characters added to a loaded zone pack in memory, for scenes that
//! draw through the zone renderer (`Beast`) without recompiling the pinned
//! pack: Draconia for the peacock scene (docs/verse/draconia/parity.md).
//!
//! [`add_character`] converts a skinned engine model (as
//! `verse_content::compiler::characters::import_with` and
//! `film_character` make it, clips included) into a pack [`Character`],
//! appending its base-colour textures and materials to the pack. The zone
//! renderer draws base colour only, so the character shading models don't
//! apply here, and additive surfaces (a cornea's highlight) are left out.
use std::collections::BTreeMap;
use std::path::Path;

use glam::{Mat4, Vec3};

use super::compile::{reduce_keys, unit_weights, weld};
use super::format::{
    AlphaMode, Character, Clip, Joint, Material, SkinnedPrimitive, SkinnedVertex, Texture, Track,
    Vertex, ZonePack,
};

/// Adds `model` to `pack` as the character `name`: its textures (read from
/// `dir`, the engine pack directory the importer wrote them to, bounded to
/// `max_edge`) and materials, and the clips `clips` (pack name, engine clip
/// ID). Returns the character; push it to `pack.forms` or hand it to
/// `Beast::new`.
///
/// # Errors
///
/// Returns a message when the model has no skin, too many joints, a missing
/// clip or image, or a surface over 65,536 vertices.
pub fn add_character(
    pack: &mut ZonePack,
    engine: &verse_engine::assets::Pack,
    dir: &Path,
    model: &verse_engine::assets::Model,
    name: &str,
    clips: &[(&str, u16)],
    max_edge: u32,
) -> Result<Character, String> {
    let skin = model.skin.as_ref().ok_or("The character has no skin")?;
    if model.bones.len() > 256 {
        return Err("The character has more than 256 joints".into());
    }
    let inverse = Mat4::from_cols_array(&skin.basis).as_dmat4().inverse();
    let joints = model
        .bones
        .iter()
        .enumerate()
        .map(|(i, bone)| Joint {
            parent: bone.parent,
            translation: skin.rest[i].translation,
            rotation: skin.rest[i].rotation,
            scale: skin.rest[i].scale,
            inverse_bind: skin.inverse_bind[i],
        })
        .collect();
    let mut textures: BTreeMap<usize, u16> = BTreeMap::new();
    let mut primitives: Vec<SkinnedPrimitive> = Vec::new();
    for surface in model.surfaces.iter().filter(|s| !s.indices.is_empty()) {
        let alpha = match surface.blend {
            0 => AlphaMode::Opaque,
            1 => AlphaMode::Mask {
                cutoff: surface.material.alpha_cutoff,
            },
            2 => AlphaMode::Blend,
            _ => continue,
        };
        let texture = match textures.get(&surface.texture) {
            Some(&t) => t,
            None => {
                let image = engine
                    .textures
                    .get(surface.texture)
                    .ok_or("A surface names a missing image")?;
                let t = u16::try_from(pack.textures.len()).map_err(|_| "Too many textures")?;
                pack.textures.push(decode(
                    &dir.join(&image.file),
                    &format!("{name}/{}", image.file),
                    max_edge,
                )?);
                textures.insert(surface.texture, t);
                t
            }
        };
        let material = u16::try_from(pack.materials.len()).map_err(|_| "Too many materials")?;
        pack.materials.push(Material {
            name: format!("{name}/{material}"),
            texture: Some(texture),
            base_color: [
                surface.tint[0],
                surface.tint[1],
                surface.tint[2],
                surface.material.opacity,
            ],
            alpha,
            double_sided: true,
        });
        let vertices: Vec<SkinnedVertex> = surface
            .vertices
            .iter()
            .map(|v| SkinnedVertex {
                vertex: Vertex {
                    position: inverse
                        .transform_point3(Vec3::from(v.position).as_dvec3())
                        .as_vec3()
                        .to_array(),
                    normal: inverse
                        .transform_vector3(Vec3::from(v.normal).as_dvec3())
                        .normalize_or_zero()
                        .as_vec3()
                        .to_array(),
                    uv: v.uv,
                    color: [255; 4],
                },
                joints: v.joints.map(|j| j as u8),
                weights: unit_weights(v.weights),
            })
            .collect();
        // Split surfaces over the per-primitive vertex limit by triangles.
        let limit = u16::MAX as usize + 1;
        if vertices.len() <= limit {
            primitives.push(SkinnedPrimitive {
                material,
                vertices,
                indices: surface.indices.clone(),
            });
        } else {
            let mut map: BTreeMap<u32, u32> = BTreeMap::new();
            let mut part = SkinnedPrimitive {
                material,
                vertices: vec![],
                indices: vec![],
            };
            for tri in surface.indices.chunks_exact(3) {
                if part.vertices.len() + 3 > limit {
                    primitives.push(std::mem::replace(
                        &mut part,
                        SkinnedPrimitive {
                            material,
                            vertices: vec![],
                            indices: vec![],
                        },
                    ));
                    map.clear();
                }
                for &i in tri {
                    let k = *map.entry(i).or_insert_with(|| {
                        part.vertices.push(vertices[i as usize]);
                        (part.vertices.len() - 1) as u32
                    });
                    part.indices.push(k);
                }
            }
            if !part.indices.is_empty() {
                primitives.push(part);
            }
        }
    }
    let mut out_clips = Vec::new();
    for (clip_name, id) in clips {
        let clip = model
            .clips
            .iter()
            .find(|c| c.id == *id)
            .ok_or(format!("The character has no clip {id} for {clip_name}"))?;
        out_clips.push(Clip {
            name: (*clip_name).into(),
            duration: clip.duration,
            distance: 0.0,
            tracks: clip
                .bones
                .iter()
                .map(|keys| Track {
                    joint: keys.bone as u16,
                    translation: reduce_keys(&keys.translation),
                    rotation: reduce_keys(&keys.rotation),
                    scale: reduce_keys(&keys.scale),
                })
                .collect(),
        });
    }
    for primitive in &mut primitives {
        weld(primitive);
    }
    Ok(Character {
        name: name.to_owned(),
        joints,
        primitives,
        clips: out_clips,
    })
}

/// A PNG as an sRGB RGBA texture, box-filtered down to `max_edge`.
fn decode(path: &Path, name: &str, max_edge: u32) -> Result<Texture, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("PNG too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let (w, h) = (info.width as usize, info.height as usize);
    let channels = info.color_type.samples();
    let mut rgba = Vec::with_capacity(w * h * 4);
    for p in buf[..w * h * channels].chunks_exact(channels) {
        rgba.extend(match channels {
            1 => [p[0], p[0], p[0], 255],
            2 => [p[0], p[0], p[0], p[1]],
            3 => [p[0], p[1], p[2], 255],
            _ => [p[0], p[1], p[2], p[3]],
        });
    }
    let mut factor = 1;
    while w / factor > max_edge as usize || h / factor > max_edge as usize {
        factor *= 2;
    }
    let (ow, oh) = ((w / factor).max(1), (h / factor).max(1));
    let mut out = vec![0u8; ow * oh * 4];
    for y in 0..oh {
        for x in 0..ow {
            let mut acc = [0u32; 4];
            for dy in 0..factor {
                for dx in 0..factor {
                    let i = ((y * factor + dy).min(h - 1) * w + (x * factor + dx).min(w - 1)) * 4;
                    for c in 0..4 {
                        acc[c] += u32::from(rgba[i + c]);
                    }
                }
            }
            let n = (factor * factor) as u32;
            for c in 0..4 {
                out[(y * ow + x) * 4 + c] = (acc[c] / n) as u8;
            }
        }
    }
    Ok(Texture {
        name: name.to_owned(),
        width: ow as u32,
        height: oh as u32,
        rgba: out,
    })
}
