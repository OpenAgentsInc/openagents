//! Versioned imported meshes, textures, skeletons, and animation tracks.
//!
//! Asset payloads remain private. This schema carries data only; shaders and
//! executable scene behavior belong to the compiled Verse engine.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub joints: [u32; 4],
    pub weights: [f32; 4],
}

fn white() -> [f32; 3] {
    [1.0; 3]
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Surface {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub texture: usize,
    /// 0 opaque, 1 cutout, 2 alpha blend, 3 additive.
    pub blend: u8,
    pub emissive: bool,
    #[serde(default = "white")]
    pub tint: [f32; 3],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bone {
    pub parent: i16,
    pub pivot: [f32; 3],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoneKeys {
    pub bone: usize,
    pub translation: Vec<(f32, [f32; 3])>,
    pub rotation: Vec<(f32, [f32; 4])>,
    pub scale: Vec<(f32, [f32; 3])>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Clip {
    pub id: u16,
    pub duration: f32,
    pub bones: Vec<BoneKeys>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Attachment {
    pub id: u16,
    pub bone: usize,
    pub position: [f32; 3],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Model {
    pub source: String,
    pub source_sha256: String,
    pub surfaces: Vec<Surface>,
    pub bones: Vec<Bone>,
    pub clips: Vec<Clip>,
    pub height: f32,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Texture {
    pub file: String,
    pub sha256: String,
    pub width: u32,
    pub height: u32,
}

/// Geometry stays in source model coordinates; placements convert it to meters.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Placement {
    pub model: String,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pack {
    pub version: u32,
    pub source_revision: String,
    pub models: BTreeMap<String, Model>,
    pub textures: Vec<Texture>,
    #[serde(default)]
    pub placements: Vec<Placement>,
}

impl Pack {
    pub fn read(path: &Path) -> Result<Self, String> {
        let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
        if size > 128 * 1024 * 1024 {
            return Err("Asset manifest exceeds 128 MiB".into());
        }
        let pack: Self = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        pack.validate()?;
        Ok(pack)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1
            || self.models.is_empty()
            || self.models.len() > 256
            || self.textures.len() > 512
        {
            return Err("Unsupported or oversized asset pack".into());
        }
        for texture in &self.textures {
            if texture.file.contains('/')
                || texture.file.contains('\\')
                || !texture.file.ends_with(".png")
                || texture.width == 0
                || texture.height == 0
                || texture.width > 4096
                || texture.height > 4096
            {
                return Err("Invalid private texture reference".into());
            }
        }
        let mut vertices = 0;
        for model in self.models.values() {
            if model
                .attachments
                .iter()
                .any(|a| a.bone >= model.bones.len() || a.position.iter().any(|v| !v.is_finite()))
            {
                return Err("Invalid model attachment".into());
            }
            if model.bones.len() > 256 || model.clips.len() > 512 || !model.height.is_finite() {
                return Err("Invalid model skeleton".into());
            }
            for (i, bone) in model.bones.iter().enumerate() {
                if bone.parent < -1
                    || bone.parent >= i as i16
                    || bone.pivot.iter().any(|x| !x.is_finite())
                {
                    return Err("Invalid bone hierarchy".into());
                }
            }
            for surface in &model.surfaces {
                vertices += surface.vertices.len();
                if vertices > 2_000_000
                    || surface.texture >= self.textures.len()
                    || surface.blend > 3
                    || surface.indices.len() % 3 != 0
                    || surface
                        .indices
                        .iter()
                        .any(|i| *i as usize >= surface.vertices.len())
                {
                    return Err("Invalid imported surface".into());
                }
                for v in &surface.vertices {
                    if v.position
                        .iter()
                        .chain(&v.normal)
                        .chain(&v.uv)
                        .chain(&v.weights)
                        .any(|x| !x.is_finite())
                        || (!model.bones.is_empty()
                            && v.joints.iter().any(|i| *i as usize >= model.bones.len()))
                    {
                        return Err("Invalid imported vertex".into());
                    }
                }
            }
            for clip in &model.clips {
                if !clip.duration.is_finite()
                    || clip.duration <= 0.0
                    || clip.bones.iter().any(|k| k.bone >= model.bones.len())
                {
                    return Err("Invalid animation clip".into());
                }
                for keys in &clip.bones {
                    validate_keys(&keys.translation, clip.duration)?;
                    validate_keys(&keys.rotation, clip.duration)?;
                    validate_keys(&keys.scale, clip.duration)?;
                }
            }
        }
        Ok(())
    }
}
fn validate_keys<const N: usize>(keys: &[(f32, [f32; N])], duration: f32) -> Result<(), String> {
    if keys.len() > 8192
        || keys.iter().any(|(t, v)| {
            !t.is_finite() || *t < 0.0 || *t > duration + 0.01 || v.iter().any(|x| !x.is_finite())
        })
        || keys.windows(2).any(|w| w[0].0 > w[1].0)
    {
        return Err("Invalid animation key timeline".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_path_traversal_and_cyclic_skeletons() {
        let mut pack = Pack {
            version: 1,
            source_revision: String::new(),
            models: BTreeMap::from([(
                "room".into(),
                Model {
                    source: String::new(),
                    source_sha256: String::new(),
                    surfaces: vec![],
                    bones: vec![Bone {
                        parent: 0,
                        pivot: [0.0; 3],
                    }],
                    clips: vec![],
                    height: 1.0,
                    attachments: vec![],
                },
            )]),
            placements: vec![],
            textures: vec![Texture {
                file: "../private.png".into(),
                sha256: String::new(),
                width: 1,
                height: 1,
            }],
        };
        assert!(pack.validate().is_err());
        pack.textures[0].file = "0.png".into();
        assert!(pack.validate().is_err());
        pack.models.get_mut("room").unwrap().bones[0].parent = -1;
        assert!(pack.validate().is_ok());
    }
}
