//! Original compiler inventory. Source declarations stay separate from file locations.
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
use verse_engine::{
    assets::Pack,
    inventory::{Asset, AssetId, Binding, Inventory, License, Origin, Purpose, fingerprint},
};
const CHARACTER_MANIFEST: &[u8] =
    include_bytes!("../../../../assets/verse/characters/quaternius/manifest.json");
const PROP_MANIFEST: &[u8] =
    include_bytes!("../../../../assets/verse/props/quaternius/manifest.json");
const PUGLIN: &str = "8d0a87d7165e5da0761418491511d2187e46de359e2f65110538f8b91a48e6af";
pub fn id(value: &str) -> Result<AssetId, String> {
    AssetId::new(value)
}
pub fn bundle(parts: &[&[u8]]) -> (String, u64) {
    let mut hash = Sha256::new();
    let mut bytes = 0;
    for part in parts {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part);
        bytes += 8 + part.len() as u64;
    }
    (format!("{:x}", hash.finalize()), bytes)
}
pub fn source(
    name: &str,
    creator: &str,
    license: License,
    format: &str,
    digest: String,
    bytes: u64,
) -> Result<Asset, String> {
    Ok(Asset {
        id: id(name)?,
        binding: Binding::Source {
            origin: Origin {
                creator: creator.into(),
                license,
                revision: digest.clone(),
                format: format.into(),
            },
        },
        sha256: digest,
        bytes,
        dependencies: vec![],
    })
}
/// Verify retained source data and licenses against the compiler's pinned manifest.
pub fn verify_characters(root: &Path) -> Result<(), String> {
    prepare_characters(root, None)
}
pub fn snapshot_characters(root: &Path, destination: &Path) -> Result<(), String> {
    prepare_characters(root, Some(destination))
}
fn prepare_characters(root: &Path, destination: Option<&Path>) -> Result<(), String> {
    prepare_sources(root, destination, CHARACTER_MANIFEST)
}
pub fn snapshot_props(root: &Path, destination: &Path) -> Result<(), String> {
    prepare_sources(root, Some(destination), PROP_MANIFEST)
}
fn prepare_sources(root: &Path, destination: Option<&Path>, pinned: &[u8]) -> Result<(), String> {
    let manifest: serde_json::Value = serde_json::from_slice(pinned).map_err(|e| e.to_string())?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut total = 0u64;
    for (name, expected) in manifest["files"]
        .as_object()
        .ok_or("Invalid character source manifest")?
    {
        let path = Path::new(name);
        let source = matches!(
            path.extension().and_then(|s| s.to_str()),
            Some("gltf" | "glb" | "bin" | "png")
        ) || name.ends_with("-license.txt")
            || name == "license.txt";
        if !source {
            continue;
        }
        if path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err("Character source path escapes its root".into());
        }
        use std::io::Read;
        let file_path = root.join(path);
        let metadata = std::fs::symlink_metadata(&file_path).map_err(|e| e.to_string())?;
        if !metadata.file_type().is_file()
            || !file_path
                .canonicalize()
                .map_err(|e| e.to_string())?
                .starts_with(&root)
        {
            return Err("Character source escapes its root or is not a regular file".into());
        }
        let available = (128 * 1024 * 1024u64).min(512 * 1024 * 1024u64 - total);
        if metadata.len() > available {
            return Err("Character source exceeds its byte budget".into());
        }
        let mut bytes = vec![];
        std::fs::File::open(&file_path)
            .map_err(|e| e.to_string())?
            .take(available + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > available {
            return Err("Character source exceeds its byte budget".into());
        }
        total += bytes.len() as u64;
        if format!("{:x}", Sha256::digest(&bytes))
            != expected.as_str().ok_or("Invalid character source digest")?
        {
            return Err(format!(
                "Character source or license digest mismatch: {name}"
            ));
        }
        if let Some(destination) = destination {
            let target = destination.join(path);
            std::fs::create_dir_all(target.parent().ok_or("Missing source snapshot parent")?)
                .map_err(|e| e.to_string())?;
            std::fs::write(target, bytes).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
/// Imports the exact admitted GLB bytes from a private compiler snapshot.
pub fn snapshot_bestiary(path: &Path, destination: &Path) -> Result<(), String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Bestiary source is not a regular file".into());
    }
    let mut bytes = vec![];
    file.take(128 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 128 * 1024 * 1024 || format!("{:x}", Sha256::digest(&bytes)) != PUGLIN {
        return Err("Bestiary source is not the admitted Puglin revision".into());
    }
    std::fs::create_dir_all(
        destination
            .parent()
            .ok_or("Missing Bestiary snapshot parent")?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(destination, bytes).map_err(|e| e.to_string())
}
pub fn validate_model_sources(pack: &Pack) -> Result<(), String> {
    let manifest: serde_json::Value =
        serde_json::from_slice(CHARACTER_MANIFEST).map_err(|e| e.to_string())?;
    let mut known: BTreeSet<_> = manifest["files"]
        .as_object()
        .ok_or("Invalid character source manifest")?
        .values()
        .filter_map(|v| v.as_str())
        .collect();
    let props: serde_json::Value =
        serde_json::from_slice(PROP_MANIFEST).map_err(|e| e.to_string())?;
    known.extend(
        props["files"]
            .as_object()
            .ok_or("Invalid prop manifest")?
            .values()
            .filter_map(|v| v.as_str()),
    );
    for (name, model) in &pack.models {
        let authored = [
            "verse/original/",
            "verse/particles/",
            "verse/procedural/",
            "verse/ground/",
            "verse/ribbon/",
        ]
        .iter()
        .any(|p| model.source.starts_with(p));
        if !authored
            && !known.contains(model.source_sha256.as_str())
            && model.source_sha256 != PUGLIN
        {
            return Err(format!(
                "Model source is not admitted by the original compiler: {name}"
            ));
        }
    }
    Ok(())
}
pub fn compile(pack: &mut Pack, dir: &Path, bestiary_path: Option<&Path>) -> Result<(), String> {
    let project = id("verse:source:project")?;
    let cc0 = id("verse:source:quaternius")?;
    let bestiary = id("verse:source:bestiary-puglin")?;
    let props_id = id("verse:source:fantasy-props")?;
    let props: serde_json::Value =
        serde_json::from_slice(PROP_MANIFEST).map_err(|e| e.to_string())?;
    let props_known: BTreeSet<_> = props["files"]
        .as_object()
        .ok_or("Invalid prop manifest")?
        .values()
        .filter_map(|v| v.as_str())
        .collect();
    let (revision, bytes) = bundle(&[
        include_bytes!("original.rs"),
        include_bytes!("characters.rs"),
        include_bytes!("props.rs"),
        include_bytes!("effects.rs"),
        include_bytes!("worlds.rs"),
        include_bytes!("inventory.rs"),
        include_bytes!("../collision.rs"),
        include_bytes!("../remote_content.rs"),
        include_bytes!("../../../verse-engine/src/assets.rs"),
        include_bytes!("../../../verse-engine/src/material.rs"),
        include_bytes!("../../../verse-engine/src/animation_graph.rs"),
        include_bytes!("../../../../assets/verse/original/ritual.json"),
    ]);
    let mut assets = vec![source(
        project.as_str(),
        "OpenAgents contributors",
        License::Apache2,
        "rust/json",
        revision.clone(),
        bytes,
    )?];
    let font = include_bytes!("../../../verse/assets/FiraMono-Medium.ttf");
    let (font_hash, font_bytes) = bundle(&[
        font,
        include_bytes!("../../../verse/assets/FiraMono-LICENSE"),
    ]);
    assets.push(source(
        "verse:source:fira-mono",
        "Mozilla Foundation and Telefonica S.A.",
        License::Ofl11,
        "ttf",
        font_hash,
        font_bytes,
    )?);
    let mut icons: Vec<&[u8]> = super::icons::all().map(|i| i.svg).collect();
    icons.extend([super::icons::LICENSE, super::icons::CREDITS]);
    let (icons_hash, icons_bytes) = bundle(&icons);
    assets.push(source(
        "verse:source:game-icons",
        "Lorc, Delapouite, and others, game-icons.net",
        License::CcBy30,
        "svg/credits",
        icons_hash,
        icons_bytes,
    )?);
    let manifest: serde_json::Value =
        serde_json::from_slice(CHARACTER_MANIFEST).map_err(|e| e.to_string())?;
    let known: BTreeSet<_> = manifest["files"]
        .as_object()
        .ok_or("Invalid character source manifest")?
        .values()
        .filter_map(|v| v.as_str())
        .collect();
    let mut origins = BTreeMap::new();
    for (name, model) in &pack.models {
        let origin = if [
            "verse/original/",
            "verse/particles/",
            "verse/procedural/",
            "verse/ground/",
            "verse/ribbon/",
        ]
        .iter()
        .any(|p| model.source.starts_with(p))
        {
            project.clone()
        } else if known.contains(model.source_sha256.as_str()) {
            cc0.clone()
        } else if props_known.contains(model.source_sha256.as_str()) {
            props_id.clone()
        } else if model.source_sha256 == PUGLIN {
            bestiary.clone()
        } else {
            return Err(format!(
                "Original compiler has no admitted source for model: {name}"
            ));
        };
        origins.insert(name.clone(), origin);
    }
    if origins.values().any(|s| s == &props_id) {
        let (hash, bytes) = bundle(&[
            PROP_MANIFEST,
            include_bytes!("../../../../assets/verse/props/quaternius/license.txt"),
        ]);
        assets.push(source(
            props_id.as_str(),
            "Quaternius",
            License::Cc0,
            "gltf/bin/png/source-manifest",
            hash,
            bytes,
        )?);
    }
    if origins.values().any(|s| s == &cc0 || s == &bestiary) {
        let (hash, bytes) = bundle(&[
            CHARACTER_MANIFEST,
            include_bytes!("../../../../assets/verse/characters/quaternius/base-license.txt"),
            include_bytes!("../../../../assets/verse/characters/quaternius/outfits-license.txt"),
            include_bytes!("../../../../assets/verse/characters/quaternius/animations-license.txt"),
        ]);
        assets.push(source(
            cc0.as_str(),
            "Quaternius",
            License::Cc0,
            "gltf/glb/png/source-manifest",
            hash,
            bytes,
        )?);
    }
    if origins.values().any(|s| s == &bestiary) {
        // This is the owner's local-use declaration, not a redistributable license grant.
        assets.push(source(
            bestiary.as_str(),
            "Owner-supplied Bestiary Standard package",
            License::OwnerSuppliedLocal,
            "glb",
            PUGLIN.into(),
            std::fs::metadata(bestiary_path.ok_or("Missing admitted Bestiary source path")?)
                .map_err(|e| e.to_string())?
                .len(),
        )?);
    }
    let mut texture_ids = BTreeMap::new();
    for (slot, texture) in pack.textures.iter().enumerate() {
        let users: Vec<_> = pack
            .models
            .iter()
            .filter(|(_, model)| {
                model
                    .surfaces
                    .iter()
                    .any(|s| s.texture_slots().any(|dependency| dependency == slot))
            })
            .collect();
        let authored = match texture.file.as_str() {
            "original-white.png" => Some("white"),
            "verse-effect-white.png" => Some("effect-white"),
            "particle-fire.png" => Some("particle-fire"),
            "particle-arcane.png" => Some("particle-arcane"),
            "particle-smoke.png" => Some("particle-smoke"),
            "particle-web.png" => Some("particle-web"),
            "particle-shadow.png" => Some("particle-shadow"),
            "particle-spark.png" => Some("particle-spark"),
            "particle-ribbon.png" => Some("particle-ribbon"),
            "particle-rune.png" => Some("particle-rune"),
            _ => None,
        };
        let name = if let Some(name) = authored {
            format!("verse:texture:original/{name}")
        } else if let Some((name, model)) = users.first() {
            let index = model
                .surfaces
                .iter()
                .position(|s| s.texture_slots().any(|dependency| dependency == slot))
                .unwrap();
            let surface = &model.surfaces[index];
            let channel = if surface.texture == slot {
                ""
            } else if surface.material.normal_texture == Some(slot) {
                "/normal"
            } else if surface.material.metallic_roughness_texture == Some(slot) {
                "/metallic-roughness"
            } else if surface.material.occlusion_texture == Some(slot) {
                "/occlusion"
            } else {
                "/emissive"
            };
            format!("verse:texture:material/{name}/{index}{channel}")
        } else {
            format!("verse:texture:unused-blob/{}", texture.sha256)
        };
        let mut dependencies = BTreeSet::from([project.clone()]);
        if authored.is_none() {
            let source_digest = texture
                .file
                .strip_prefix("universal-source-")
                .and_then(|s| s.strip_suffix(".png"));
            if source_digest.is_some_and(|s| known.contains(s)) {
                dependencies.insert(cc0.clone());
            } else if source_digest.is_some_and(|s| props_known.contains(s)) {
                dependencies.insert(props_id.clone());
            } else if users.iter().any(|(name, _)| origins[*name] == bestiary) {
                dependencies.insert(bestiary.clone());
            } else {
                return Err("Texture has no admitted source provenance".into());
            }
        }
        let identity = id(&name)?;
        texture_ids.insert(slot, identity.clone());
        assets.push(Asset {
            id: identity,
            binding: Binding::Texture { slot },
            sha256: texture.sha256.clone(),
            bytes: std::fs::metadata(dir.join(&texture.file))
                .map_err(|e| e.to_string())?
                .len(),
            dependencies: dependencies.into_iter().collect(),
        });
    }
    for (name, model) in &pack.models {
        let mut dependencies = BTreeSet::from([project.clone(), origins[name].clone()]);
        // Imported motion also uses the pinned CC0 library, including the Bestiary rig.
        if origins[name] == bestiary {
            dependencies.insert(cc0.clone());
        }
        dependencies.extend(
            model
                .surfaces
                .iter()
                .flat_map(|s| s.texture_slots())
                .map(|slot| texture_ids[&slot].clone()),
        );
        let (sha256, bytes) = fingerprint(model)?;
        assets.push(Asset {
            id: id(&format!("verse:model:{name}"))?,
            binding: Binding::Model { key: name.clone() },
            sha256,
            bytes,
            dependencies: dependencies.into_iter().collect(),
        });
    }
    let inventory = Inventory {
        version: 1,
        compiler: id("verse:compiler:original-chamber")?,
        compiler_revision: revision,
        assets,
    };
    inventory.verify(pack)?;
    inventory.admit(pack, Purpose::OriginalLocal, &inventory.roots())?;
    pack.inventory = Some(inventory);
    Ok(())
}
/// Refreshes content checksums after declared edits without changing identities or rights.
pub fn refresh(inventory: &mut Inventory, pack: &Pack, dir: &Path) -> Result<(), String> {
    for asset in &mut inventory.assets {
        match &asset.binding {
            Binding::Model { key } => {
                let (sha, bytes) = fingerprint(&pack.models[key])?;
                asset.sha256 = sha;
                asset.bytes = bytes;
            }
            Binding::Texture { slot } => {
                asset.sha256 = pack.textures[*slot].sha256.clone();
                asset.bytes = std::fs::metadata(dir.join(&pack.textures[*slot].file))
                    .map_err(|e| e.to_string())?
                    .len();
            }
            Binding::Source { .. } => {}
        }
    }
    inventory.verify(pack)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn procedural_inventory_is_complete_and_distributable() {
        let root =
            std::env::temp_dir().join(format!("verse-inventory-test-{}", std::process::id()));
        let mut pack = crate::compiler::original::generate(&root).unwrap();
        compile(&mut pack, &root, None).unwrap();
        let inventory = pack.inventory.as_ref().unwrap();
        inventory.verify(&pack).unwrap();
        inventory
            .admit(&pack, Purpose::Redistribute, &inventory.roots())
            .unwrap();
        let id = inventory
            .assets
            .iter()
            .find(|a| a.binding == Binding::Texture { slot: 0 })
            .unwrap()
            .id
            .clone();
        std::fs::rename(
            root.join(&pack.textures[0].file),
            root.join("relocated.png"),
        )
        .unwrap();
        pack.textures[0].file = "relocated.png".into();
        let prepared =
            verse_engine::loading::Prepared::load(pack, &root, Default::default()).unwrap();
        assert!(
            prepared
                .receipt()
                .inventory
                .as_ref()
                .unwrap()
                .assets
                .contains(&id)
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
