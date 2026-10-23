//! Portable creator content. Engine code, compiled meshes, credentials, and player state are absent.
use crate::{Error, MAX_SAFE_INTEGER, Project, StudioVisit, protocol::id};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const PROJECT_SCHEMA: &str = "openagents.verse.project.v1";
pub const MAX_PROJECT_BYTES: usize = 48 * 1024;
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum WorldNamespace {
    HauntedHouse,
    UniversePreferences,
    MedievalTown,
    PersonalPlanet,
}
impl WorldNamespace {
    pub fn key(self) -> &'static str {
        match self {
            Self::HauntedHouse => "haunted-house",
            Self::UniversePreferences => "universe-preferences",
            Self::MedievalTown => "medieval-town",
            Self::PersonalPlanet => "personal-planet",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WorldTemplate {
    pub world: String,
    pub engine_release: String,
    pub asset_set_sha256: String,
    pub scene_sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WorldEntity {
    pub asset: String,
    pub position: [f64; 3],
    pub yaw: f64,
    pub scale: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WorldRoom {
    pub center: [f64; 2],
    pub size: [f64; 2],
    pub height: f64,
    pub floor_y: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProjectResource {
    pub sha256: String,
    pub bytes: u64,
    pub media_type: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MaterialLayer {
    pub material: String,
    pub surface: String,
    pub texture: ProjectResource,
    pub uv_set: u8,
    pub kind: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OriginalImport {
    pub name: String,
    pub sha256: String,
    pub text: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProjectDocument {
    pub schema: String,
    pub namespace: WorldNamespace,
    pub template: WorldTemplate,
    pub source_revision: u64,
    pub entities: BTreeMap<String, WorldEntity>,
    pub rooms: BTreeMap<String, WorldRoom>,
    pub removed_walls: Vec<String>,
    pub materials: BTreeMap<String, BTreeMap<String, f64>>,
    pub environment: BTreeMap<String, f64>,
    pub source_draft: String,
    pub active_source: String,
    pub layers: Vec<MaterialLayer>,
    pub originals: Vec<OriginalImport>,
}
pub fn project_hash(bytes: &[u8]) -> String {
    use sha2::Digest;
    format!("{:x}", sha2::Sha256::digest(bytes))
}
fn hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn numbers(v: &[f64]) -> bool {
    v.iter().all(|n| n.is_finite() && n.abs() <= 100_000.)
}
impl ProjectDocument {
    pub fn validate(&self) -> Result<(), Error> {
        let invalid = || Error::Invalid("project document");
        if self.schema != PROJECT_SCHEMA
            || !id(&self.template.world)
            || !id(&self.template.engine_release)
            || !hash(&self.template.asset_set_sha256)
            || !hash(&self.template.scene_sha256)
            || self.source_revision > MAX_SAFE_INTEGER
            || self.entities.len() > 128
            || self.rooms.len() > 32
            || self.removed_walls.len() > 128
            || self.materials.len() > 32
            || self.environment.len() > 24
            || self.layers.len() > 32
            || self.originals.len() > 8
        {
            return Err(invalid());
        }
        if self.entities.iter().any(|(k, e)| {
            !id(k)
                || !id(&e.asset)
                || !numbers(&e.position)
                || !e.yaw.is_finite()
                || e.yaw.abs() > 3600.
                || !e.scale.is_finite()
                || !(0.05..=20.).contains(&e.scale)
        }) || self.rooms.iter().any(|(k, r)| {
            !id(k)
                || !numbers(&r.center)
                || !numbers(&r.size)
                || r.size.iter().any(|n| !(0.5..=100.).contains(n))
                || !r.height.is_finite()
                || !(1.5..=20.).contains(&r.height)
                || !r.floor_y.is_finite()
                || r.floor_y.abs() > 1000.
        }) || self.removed_walls.iter().any(|s| !id(s))
            || self.materials.iter().any(|(k, p)| {
                !id(k)
                    || p.len() > 8
                    || p.iter()
                        .any(|(k, v)| !id(k) || !v.is_finite() || v.abs() > 100_000.)
            })
            || self
                .environment
                .iter()
                .any(|(k, v)| !id(k) || !v.is_finite() || v.abs() > 100_000.)
            || [&self.source_draft, &self.active_source]
                .iter()
                .any(|s| s.len() > 4096 || s.contains('\0'))
            || self.layers.iter().any(|l| {
                !id(&l.material)
                    || !id(&l.surface)
                    || l.uv_set != 0
                    || !matches!(l.kind.as_str(), "paint" | "generated")
                    || !hash(&l.texture.sha256)
                    || l.texture.bytes == 0
                    || l.texture.bytes > 4 * 1024 * 1024
                    || l.texture.media_type != "image/png"
            })
            || self.originals.iter().any(|o| {
                o.name.is_empty()
                    || o.name.len() > 128
                    || o.name.contains(['\0', '/', '\\'])
                    || o.text.len() > 24 * 1024
                    || project_hash(o.text.as_bytes()) != o.sha256
            })
        {
            return Err(invalid());
        }
        crate::material::validate_project_materials(self)?;
        let bytes = serde_json::to_vec(self).map_err(|_| invalid())?;
        if bytes.len() > MAX_PROJECT_BYTES {
            return Err(Error::Invalid("project size"));
        }
        Ok(())
    }
    pub fn sha256(&self) -> Result<String, Error> {
        self.validate()?;
        Ok(project_hash(
            &serde_json::to_vec(self).map_err(|_| Error::Invalid("project document"))?,
        ))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProjectCheckpoint {
    pub project: String,
    pub namespace: WorldNamespace,
    pub branch: String,
    pub saved_version: u64,
    pub visit: StudioVisit,
}
impl ProjectCheckpoint {
    pub fn validate(&self) -> Result<(), Error> {
        if !id(&self.project) || !id(&self.branch) || self.saved_version > MAX_SAFE_INTEGER {
            return Err(Error::Invalid("project checkpoint"));
        }
        self.visit.validate()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SavedProject {
    pub project: String,
    pub namespace: WorldNamespace,
    pub branch: String,
    pub version: u64,
    pub document_sha256: String,
}
impl SavedProject {
    pub fn validate(&self) -> Result<(), Error> {
        Project {
            id: self.project.clone(),
            branch: self.branch.clone(),
            revision: self.version,
        }
        .validate()?;
        if self.version == 0 || !hash(&self.document_sha256) {
            return Err(Error::Invalid("saved project"));
        }
        Ok(())
    }
}

/// Converts only explicitly mapped legacy props. The untouched original remains exportable.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyConversion {
    pub document: ProjectDocument,
    pub unsupported: Vec<String>,
}
pub fn convert_legacy(
    original: &str,
    mapping: &BTreeMap<String, String>,
    mut document: ProjectDocument,
) -> Result<LegacyConversion, Error> {
    if original.len() > 24 * 1024 || document.originals.len() >= 8 {
        return Err(Error::Invalid("legacy export size"));
    }
    let value: serde_json::Value =
        serde_json::from_str(original).map_err(|_| Error::Invalid("legacy export"))?;
    let mut unsupported = Vec::new();
    if let Some(entities) = value.get("entities").and_then(|v| v.as_array()) {
        for (index, e) in entities.iter().enumerate() {
            let Some(kind) = e["kind"].as_str() else {
                unsupported.push(format!("Object {index}: no asset name"));
                continue;
            };
            let Some(asset) = mapping.get(kind) else {
                unsupported.push(format!(
                    "{kind}: choose a replacement; the original is kept"
                ));
                continue;
            };
            let Some(key) = e["id"].as_str() else {
                unsupported.push(format!("{kind}: no object ID"));
                continue;
            };
            let entity = WorldEntity {
                asset: asset.clone(),
                position: serde_json::from_value(e["position"].clone())
                    .map_err(|_| Error::Invalid("legacy position"))?,
                yaw: e["yaw"].as_f64().unwrap_or(0.),
                scale: e["scale"].as_f64().unwrap_or(1.),
            };
            if document.entities.contains_key(key) {
                unsupported.push(format!("{key}: an object already uses this name"));
            } else {
                document.entities.insert(key.into(), entity);
            }
        }
    }
    for key in value
        .as_object()
        .ok_or(Error::Invalid("legacy export"))?
        .keys()
    {
        if key != "entities" {
            unsupported.push(format!("{key}: original export kept for inspection"));
        }
    }
    document.originals.push(OriginalImport {
        name: "legacy-export.json".into(),
        sha256: project_hash(original.as_bytes()),
        text: original.into(),
    });
    document.validate()?;
    Ok(LegacyConversion {
        document,
        unsupported,
    })
}

/// Files must be exported by the old origin and explicitly selected by the person.
pub fn convert_legacy_files(
    files: Vec<OriginalImport>,
    mapping: &BTreeMap<String, String>,
    mut document: ProjectDocument,
) -> Result<LegacyConversion, Error> {
    if files.is_empty() || files.len() + document.originals.len() > 8 {
        return Err(Error::Invalid("legacy files"));
    }
    let mut unsupported = Vec::new();
    for file in files {
        if project_hash(file.text.as_bytes()) != file.sha256 {
            return Err(Error::Invalid("legacy file hash"));
        }
        let value = serde_json::from_str::<serde_json::Value>(&file.text).ok();
        if value.as_ref().is_some_and(|v| v.get("entities").is_some()) {
            let mut converted = convert_legacy(&file.text, mapping, document)?;
            *converted.document.originals.last_mut().unwrap() = file;
            unsupported.extend(converted.unsupported);
            document = converted.document;
        } else {
            unsupported.push(format!(
                "{}: original kept; no supported scene objects to convert",
                file.name
            ));
            document.originals.push(file);
        }
    }
    document.validate()?;
    Ok(LegacyConversion {
        document,
        unsupported,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn document() -> ProjectDocument {
        serde_json::from_value(serde_json::json!({"schema":PROJECT_SCHEMA,"namespace":"haunted-house","template":{"world":"synthetic-house","engine_release":"fixture-1","asset_set_sha256":"a".repeat(64),"scene_sha256":"b".repeat(64)},"source_revision":0,"entities":{"prop-1":{"asset":"custom:haunt:pumpkin","position":[0,0,-8],"yaw":0,"scale":1}},"rooms":{},"removed_walls":[],"materials":{"fur":{"roughness":0.8}},"environment":{"fogDensity":0.01},"source_draft":crate::DEFAULT_EFFECT,"active_source":crate::DEFAULT_EFFECT,"layers":[],"originals":[]})).unwrap()
    }
    #[test]
    fn editable_content_excludes_presence_and_preserves_invalid_drafts() {
        let mut d = document();
        d.source_draft = "fn broken draft".into();
        d.validate().unwrap();
        let bytes = serde_json::to_vec(&d).unwrap();
        let restored: ProjectDocument = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(d, restored);
        let mut value = serde_json::to_value(&d).unwrap();
        value["checkpoint"] = serde_json::json!({"eye":[0,1,2]});
        assert!(serde_json::from_value::<ProjectDocument>(value).is_err());
        d.entities.get_mut("prop-1").unwrap().scale = 0.;
        assert!(d.validate().is_err());
    }
    #[test]
    fn legacy_mapping_is_explicit_and_original_exports_are_unchanged() {
        let original = r#"{"entities":[{"id":"old-ghost","kind":"primitive-ghost","position":[2,0,-4]}],"changes":["old unsupported operation"]}"#;
        let unmapped = convert_legacy(original, &BTreeMap::new(), document()).unwrap();
        assert_eq!(unmapped.document.entities.len(), 1);
        assert_eq!(unmapped.unsupported.len(), 2);
        assert_eq!(unmapped.document.originals[0].text, original);
        let mapped = convert_legacy(
            original,
            &BTreeMap::from([("primitive-ghost".into(), "custom:haunt:ghost".into())]),
            document(),
        )
        .unwrap();
        assert_eq!(
            mapped.document.entities["old-ghost"].asset,
            "custom:haunt:ghost"
        );
        assert_eq!(
            mapped.document.originals[0].sha256,
            project_hash(original.as_bytes())
        );
    }
}
