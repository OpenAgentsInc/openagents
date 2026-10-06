//! Complete, bounded public artifacts, separate from local generation builds.
use super::{Document, Workspace, admit, checked, parse, workspace};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};
use verse_engine::{assets::Pack, director::Scene, inventory::Purpose};

const LIMIT: usize = 640 * 1024 * 1024;
const MANIFEST_LIMIT: usize = 2 * 1024 * 1024;
const SCHEMA: &str = "verse.public.release.v1";
fn error(message: impl Into<String>) -> super::Diagnostic {
    super::Diagnostic::at("release.json", "$", message)
}
fn encode(value: &impl Serialize) -> super::Result<Vec<u8>> {
    serde_json::to_vec(value).map_err(|e| error(e.to_string()))
}
fn file_limit(name: &str) -> usize {
    if name == "pack.json" {
        128 * 1024 * 1024
    } else if name == verse_engine::mips::archive::PAYLOAD {
        verse_engine::mips::archive::MAX_BYTES
    } else if name == verse_engine::mips::archive::MANIFEST {
        8 * 1024 * 1024
    } else if name.ends_with(".json") {
        MANIFEST_LIMIT
    } else {
        64 * 1024 * 1024
    }
}
fn public_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b))
        && name != "."
        && name != ".."
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema: String,
    pub pack: u32,
    pub wire: u32,
    pub social: u32,
}
impl Default for Profile {
    fn default() -> Self {
        Self {
            schema: "verse.closed-data.v1".into(),
            pack: 1,
            wire: verse_world::service::wire::VERSION as u32,
            social: verse_world::play::social::PROFILE_REVISION as u32,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct File {
    pub sha256: String,
    pub bytes: u64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub generation: String,
    pub content: [u8; 32],
    pub profile: Profile,
    pub files: BTreeMap<String, File>,
    /// Retained declarations, not a legal certification.
    pub admission: serde_json::Value,
}
/// Immutable verified bytes. Serving adapters use this snapshot, not the source paths.
pub struct Verified {
    id: String,
    manifest: Manifest,
    files: BTreeMap<String, Vec<u8>>,
}
impl Verified {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn file(&self, name: &str) -> Option<&[u8]> {
        self.files.get(name).map(Vec::as_slice)
    }
}
fn names(pack: &Pack) -> super::Result<std::collections::BTreeSet<String>> {
    let mut names: std::collections::BTreeSet<String> = [
        "document.json",
        "pack.json",
        "scene.json",
        verse_engine::mips::archive::MANIFEST,
        verse_engine::mips::archive::PAYLOAD,
    ]
    .into_iter()
    .map(String::from)
    .collect();
    for texture in &pack.textures {
        if texture.file == "release.json"
            || !public_name(&texture.file)
            || !names.insert(texture.file.clone())
        {
            return Err(error("Texture path is unsafe, duplicated, or reserved"));
        }
    }
    Ok(names)
}
pub(super) fn admission(pack: &Pack) -> super::Result<serde_json::Value> {
    let inventory = pack
        .inventory
        .as_ref()
        .ok_or_else(|| error("Public releases require an asset inventory"))?;
    checked("inventory", inventory.verify(pack))?;
    let admitted = checked(
        "inventory",
        inventory.admit(pack, Purpose::Redistribute, &inventory.roots()),
    )?;
    // Every compiled model and texture must reach an admitted origin; a global
    // source declaration cannot excuse an unrelated orphan asset.
    for asset in &inventory.assets {
        if !matches!(
            asset.binding,
            verse_engine::inventory::Binding::Source { .. }
        ) {
            checked(
                "inventory",
                inventory.admit(pack, Purpose::Redistribute, std::slice::from_ref(&asset.id)),
            )?;
        }
    }
    serde_json::to_value(admitted).map_err(|e| error(e.to_string()))
}
fn validate_runtime(
    pack: &Pack,
    scene: &Scene,
    document: &Document,
    root: &Path,
) -> super::Result<[u8; 32]> {
    // Primitives are already compiled into this pack. Reapply the remaining
    // idempotent edits and exercise the same runtime catalog admission.
    let mut compiled = document.clone();
    compiled.primitives.clear();
    let (actual_pack, actual_scene, _) = admit(&compiled, pack)?;
    if encode(&actual_pack)? != encode(pack)? || encode(&actual_scene)? != encode(scene)? {
        return Err(error("Release values differ from their runtime document"));
    }
    // Rebuild primitive geometry independently so clearing its recipe cannot
    // hide a substituted compiled model.
    for (key, shape) in &document.primitives {
        let mut model = super::geometry::compile(key, shape, pack.textures.len())?;
        if let Some(edit) = document.models.get(key) {
            if let Some(states) = &edit.states {
                model.states = states.clone();
            }
            if let Some(graph) = &edit.graph {
                model.graph = Some(graph.clone());
            }
            for (index, material) in &edit.materials {
                model
                    .surfaces
                    .get_mut(*index)
                    .ok_or_else(|| error("Primitive material surface is absent"))?
                    .material = material.clone();
            }
        }
        let stored = pack
            .models
            .get(key)
            .ok_or_else(|| error("Compiled primitive is missing"))?;
        if encode(&model)? != encode(stored)? {
            return Err(error("Primitive differs from its public recipe"));
        }
    }
    Ok(super::Preview::new(&compiled, pack, root)?.content())
}
/// Export only public runtime data. Previews, host paths, journals, and secrets stay local.
pub fn build(workspace: &Workspace, destination: &Path) -> super::Result<Verified> {
    workspace::ancestors(destination)?;
    if destination.exists() {
        return Err(error("Release destination must not exist"));
    }
    let build = workspace.build()?;
    let pack: Pack = parse(
        "pack.json",
        &workspace::read(&build.path.join("pack.json"), 128 * 1024 * 1024)?,
        128 * 1024 * 1024,
    )?;
    let admitted = admission(&pack)?;
    let mut files = BTreeMap::new();
    let mut total = 0usize;
    for name in names(&pack)? {
        let bytes = workspace::read(&build.path.join(&name), file_limit(&name))?;
        total = total
            .checked_add(bytes.len())
            .ok_or_else(|| error("Release byte budget exceeded"))?;
        if total > LIMIT {
            return Err(error("Release byte budget exceeded"));
        }
        files.insert(name, bytes);
    }
    let manifest = Manifest {
        schema: SCHEMA.into(),
        generation: build.generation,
        content: build.content,
        profile: Profile::default(),
        admission: admitted,
        files: files
            .iter()
            .map(|(name, data)| {
                (
                    name.clone(),
                    File {
                        sha256: workspace::hash(data),
                        bytes: data.len() as u64,
                    },
                )
            })
            .collect(),
    };
    let encoded = encode(&manifest)?;
    if encoded.len() > MANIFEST_LIMIT {
        return Err(error("Release manifest budget exceeded"));
    }
    std::fs::create_dir(destination).map_err(|e| error(e.to_string()))?;
    for (name, data) in files {
        workspace::create(&destination.join(name), &data)?;
    }
    workspace::create(&destination.join("release.json"), &encoded)?;
    workspace::sync_dir(destination)?;
    verify(destination)
}
/// Verify the complete directory and return a sealed byte snapshot.
pub fn verify(root: &Path) -> super::Result<Verified> {
    workspace::ancestors(root)?;
    let encoded = workspace::read(&root.join("release.json"), MANIFEST_LIMIT)?;
    let manifest: Manifest = parse("release.json", &encoded, MANIFEST_LIMIT)?;
    if encoded != encode(&manifest)? {
        return Err(error("Release descriptor is not canonical JSON"));
    }
    if manifest.schema != SCHEMA
        || manifest.profile != Profile::default()
        || manifest.files.len() > 517
        || manifest.files.is_empty()
        || manifest.generation.len() != 64
        || !manifest
            .generation
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        || manifest.content == [0; 32]
    {
        return Err(error(
            "Unsupported release identity, compatibility profile, or file budget",
        ));
    }
    let actual: std::collections::BTreeSet<_> = std::fs::read_dir(root)
        .map_err(|e| error(e.to_string()))?
        .map(|entry| entry.map(|e| e.file_name()))
        .collect::<std::io::Result<_>>()
        .map_err(|e| error(e.to_string()))?;
    let expected: std::collections::BTreeSet<_> = manifest
        .files
        .keys()
        .map(std::ffi::OsString::from)
        .chain(std::iter::once("release.json".into()))
        .collect();
    if actual != expected {
        return Err(error("Release contains unsealed or missing files"));
    }
    let mut total = 0usize;
    let mut files = BTreeMap::new();
    for (name, file) in &manifest.files {
        if !public_name(name) || file.bytes == 0 || file.bytes > file_limit(name) as u64 {
            return Err(error("Invalid release file name or budget"));
        }
        total = total
            .checked_add(file.bytes as usize)
            .ok_or_else(|| error("Release byte budget exceeded"))?;
        if total > LIMIT {
            return Err(error("Release byte budget exceeded"));
        }
        let data = workspace::read(&root.join(name), file.bytes as usize)?;
        if data.len() as u64 != file.bytes || workspace::hash(&data) != file.sha256 {
            return Err(error("Release file digest or length differs"));
        }
        files.insert(name.clone(), data);
    }
    let required = |name: &str| {
        files
            .get(name)
            .ok_or_else(|| error(format!("Missing release file: {name}")))
    };
    let pack: Pack = parse("pack.json", required("pack.json")?, 128 * 1024 * 1024)?;
    let scene: Scene = parse("scene.json", required("scene.json")?, 1024 * 1024)?;
    let document: Document = parse("document.json", required("document.json")?, MANIFEST_LIMIT)?;
    for (name, canonical) in [
        ("pack.json", serde_json::to_vec_pretty(&pack)),
        ("scene.json", serde_json::to_vec_pretty(&scene)),
        ("document.json", serde_json::to_vec_pretty(&document)),
    ] {
        // The exporter defines canonical files. Exact encoding also refuses
        // duplicate map entries that a permissive decoder might discard.
        if required(name)? != &canonical.map_err(|e| error(e.to_string()))? {
            return Err(error(
                "Release data contains unknown, duplicate, or noncanonical fields",
            ));
        }
    }
    if manifest
        .files
        .keys()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>()
        != names(&pack)?
        || manifest.admission != admission(&pack)?
    {
        return Err(error(
            "Release dependency admission or public file set differs",
        ));
    }
    for asset in &pack.inventory.as_ref().unwrap().assets {
        if let verse_engine::inventory::Binding::Texture { slot } = asset.binding {
            if asset.bytes != required(&pack.textures[slot].file)?.len() as u64 {
                return Err(error("Inventory texture length differs"));
            }
        }
    }
    if validate_runtime(&pack, &scene, &document, root)? != manifest.content {
        return Err(error("Release runtime content identity differs"));
    }
    let id = workspace::hash(&encode(&manifest)?);
    Ok(Verified {
        id,
        manifest,
        files,
    })
}
