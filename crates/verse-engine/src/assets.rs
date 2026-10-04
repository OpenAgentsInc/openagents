//! Versioned meshes, textures, skeletons, and animation tracks.
//!
//! This schema carries data only; shaders and
//! executable scene behavior belong to the compiled Verse engine.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io::Read, path::Path};

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
    #[serde(default)]
    pub material: crate::material::Material,
    /// 0 opaque, 1 cutout, 2 alpha blend, 3 additive.
    pub blend: u8,
    pub emissive: bool,
    #[serde(default = "white")]
    pub tint: [f32; 3],
}

impl Surface {
    /// Full image dependency closure, including the retained base-color slot.
    pub fn texture_slots(&self) -> impl Iterator<Item = usize> + '_ {
        std::iter::once(self.texture).chain(self.material.textures())
    }
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

/// Absolute local node transforms for a glTF-compatible skeleton.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct RestPose {
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}
/// Bind-space information for node-based skeletal animation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Skin {
    pub names: Vec<String>,
    pub rest: Vec<RestPose>,
    pub inverse_bind: Vec<[f32; 16]>,
    pub basis: [f32; 16],
}
fn read_states<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<crate::motion::State, crate::motion::Binding>, D::Error> {
    struct States;
    impl<'de> serde::de::Visitor<'de> for States {
        type Value = BTreeMap<crate::motion::State, crate::motion::Binding>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("unique named animation bindings")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(
            self,
            mut map: M,
        ) -> Result<Self::Value, M::Error> {
            let mut states = BTreeMap::new();
            while let Some((state, binding)) = map.next_entry()? {
                if states.insert(state, binding).is_some() {
                    return Err(serde::de::Error::custom(
                        "Duplicate animation state binding",
                    ));
                }
            }
            Ok(states)
        }
    }
    deserializer.deserialize_map(States)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Model {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub markers: Vec<crate::markers::ClipTrack>,
    #[serde(
        default,
        deserialize_with = "read_states",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub states: BTreeMap<crate::motion::State, crate::motion::Binding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skin: Option<Skin>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inventory: Option<crate::inventory::Inventory>,
    pub version: u32,
    pub source_revision: String,
    pub models: BTreeMap<String, Model>,
    pub textures: Vec<Texture>,
    #[serde(default)]
    pub placements: Vec<Placement>,
}

impl Pack {
    pub fn read(path: &Path) -> Result<Self, String> {
        const LIMIT: u64 = 128 * 1024 * 1024;
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let metadata = file.metadata().map_err(|e| e.to_string())?;
        if !metadata.is_file() || metadata.len() > LIMIT {
            return Err("Asset manifest is not a bounded regular file".into());
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        file.take(LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > LIMIT {
            return Err("Asset manifest exceeds 128 MiB".into());
        }
        let pack: Self = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        pack.validate()?;
        Ok(pack)
    }

    pub fn validate(&self) -> Result<(), String> {
        if let Some(inventory) = &self.inventory {
            inventory.validate(self)?;
        }
        if self.version != 1
            || self.models.is_empty()
            || self.models.len() > 256
            || self.textures.len() > 512
        {
            return Err("Unsupported or oversized asset pack".into());
        }
        if self.placements.len() > 100_000
            || self.placements.iter().any(|placement| {
                !self.models.contains_key(&placement.model)
                    || placement
                        .position
                        .iter()
                        .chain(&placement.rotation)
                        .any(|v| !v.is_finite())
                    || !placement.scale.is_finite()
                    || placement.scale == 0.
                    || glam::Quat::from_array(placement.rotation).length_squared() < 1e-8
            })
        {
            return Err("Invalid asset placement dependency or transform".into());
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
                return Err("Invalid texture reference".into());
            }
        }
        let mut vertices = 0;
        for model in self.models.values() {
            let mut ids = std::collections::BTreeSet::new();
            let duplicate_ids = model
                .clips
                .iter()
                .fold(false, |duplicate, clip| !ids.insert(clip.id) || duplicate);
            if !model.states.is_empty() && duplicate_ids
                || model.states.values().any(|binding| {
                    !ids.contains(&binding.clip)
                        || !binding.transition_seconds.is_finite()
                        || !(0. ..=2.).contains(&binding.transition_seconds)
                })
            {
                return Err("Invalid semantic animation bindings".into());
            }
            if model.markers.len() > 512 || (!model.markers.is_empty() && duplicate_ids) {
                return Err("Invalid animation marker track capacity or clip identity".into());
            }
            let mut marker_clips = std::collections::BTreeSet::new();
            for authored in &model.markers {
                authored.track.validate()?;
                let clip = model
                    .clips
                    .iter()
                    .find(|clip| clip.id == authored.clip)
                    .ok_or("Animation marker track references a missing clip")?;
                if !marker_clips.insert(authored.clip)
                    || authored.track.duration != f64::from(clip.duration)
                {
                    return Err(
                        "Animation marker track has duplicate identity or mismatched duration"
                            .into(),
                    );
                }
            }
            if let Some(skin) = &model.skin {
                if skin.names.len() != model.bones.len()
                    || skin.rest.len() != model.bones.len()
                    || skin.inverse_bind.len() != model.bones.len()
                    || skin.basis.iter().any(|v| !v.is_finite())
                    || glam::Mat4::from_cols_array(&skin.basis).determinant().abs() < 1e-8
                    || skin.inverse_bind.iter().flatten().any(|v| !v.is_finite())
                    || skin.rest.iter().any(|r| {
                        r.translation
                            .iter()
                            .chain(&r.rotation)
                            .chain(&r.scale)
                            .any(|v| !v.is_finite())
                            || glam::Quat::from_array(r.rotation).length_squared() < 1e-8
                    })
                {
                    return Err("Invalid skeletal bind data".into());
                }
            }
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
                surface.material.validate(self.textures.len())?;
                vertices += surface.vertices.len();
                if vertices > 2_000_000
                    || surface.texture >= self.textures.len()
                    || surface.blend > 3
                    || surface.tint.iter().any(|v| !v.is_finite())
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
                let mut tracks = std::collections::BTreeSet::new();
                if clip.bones.iter().any(|keys| !tracks.insert(keys.bone)) {
                    return Err("Duplicate animation bone track".into());
                }
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
            inventory: None,
            version: 1,
            source_revision: String::new(),
            models: BTreeMap::from([(
                "room".into(),
                Model {
                    markers: Vec::new(),
                    states: Default::default(),
                    skin: None,
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
                file: "../compiled.png".into(),
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
        use crate::motion::{Binding, Mode, State};
        let model = pack.models.get_mut("room").unwrap();
        model.states.insert(
            State::Idle,
            Binding {
                clip: 421,
                mode: Mode::Loop,
                transition_seconds: 0.22,
            },
        );
        assert!(pack.validate().is_err());
        pack.models.get_mut("room").unwrap().clips.push(Clip {
            id: 421,
            duration: 1.,
            bones: vec![],
        });
        assert!(pack.validate().is_ok());
        use crate::markers::{ClipTrack, Marker, Track};
        let authored = ClipTrack {
            clip: 421,
            track: Track {
                duration: 1.,
                markers: vec![Marker {
                    id: 7,
                    seconds: 0.5,
                }],
            },
        };
        pack.models
            .get_mut("room")
            .unwrap()
            .markers
            .push(authored.clone());
        assert!(pack.validate().is_ok());
        pack.models.get_mut("room").unwrap().markers[0].clip = 999;
        assert!(pack.validate().is_err());
        pack.models.get_mut("room").unwrap().markers[0] = authored.clone();
        pack.models.get_mut("room").unwrap().markers[0]
            .track
            .duration = 2.;
        assert!(pack.validate().is_err());
        pack.models.get_mut("room").unwrap().markers[0] = authored.clone();
        pack.models.get_mut("room").unwrap().markers.push(authored);
        assert!(pack.validate().is_err());
        pack.models.get_mut("room").unwrap().markers.pop();
        let duplicate = pack.models["room"].clips[0].clone();
        pack.models.get_mut("room").unwrap().clips.push(duplicate);
        assert!(pack.validate().is_err());
        pack.models.get_mut("room").unwrap().clips.pop();
        pack.models
            .get_mut("room")
            .unwrap()
            .states
            .get_mut(&State::Idle)
            .unwrap()
            .transition_seconds = f32::NAN;
        assert!(pack.validate().is_err());
        let model = pack.models.get_mut("room").unwrap();
        model
            .states
            .get_mut(&State::Idle)
            .unwrap()
            .transition_seconds = 0.22;
        let track = BoneKeys {
            bone: 0,
            translation: vec![],
            rotation: vec![],
            scale: vec![],
        };
        model.clips[0].bones = vec![track.clone(), track];
        assert!(pack.validate().is_err());
    }
    #[test]
    fn duplicate_semantic_names_are_refused_during_decoding() {
        let body = r#""skin":null,"source":"test","source_sha256":"","surfaces":[],"bones":[],"clips":[],"height":1,"attachments":[]"#;
        let binding = r#"{"clip":7,"mode":"hold","transition_seconds":0.12}"#;
        let json = format!("{{\"states\":{{\"death\":{binding},\"death\":{binding}}},{body}}}");
        let error = serde_json::from_str::<Model>(&json)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("Duplicate animation state binding"),
            "{error}"
        );
    }
}
