//! Content identity for the scene, compiled geometry, and runtime textures.
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path};
use verse_engine::{assets::Pack, director::Scene};
/// Requires outfit render models to support every chamber animation state.
pub fn outfit_models(
    pack: &Pack,
    catalog: &verse_world::service::outfits::Catalog,
) -> Result<(), String> {
    catalog.validate()?;
    for outfit in &catalog.outfits {
        outfit_model(pack, &outfit.model)?;
    }
    Ok(())
}
pub fn outfit_model(pack: &Pack, name: &str) -> Result<(), String> {
    let model = pack
        .models
        .get(name)
        .ok_or("Equipped outfit model is missing from admitted pack")?;
    if verse_engine::motion::State::ALL
        .iter()
        .any(|state| !model.states.contains_key(state))
    {
        return Err("Outfit model is missing a chamber animation state".into());
    }
    model.validate_animation()
}
/// Admits static equipment models and required sockets on every possible player rig.
pub fn equipment_models(
    pack: &Pack,
    scene: &Scene,
    outfits: &verse_world::service::outfits::Catalog,
    catalog: &verse_world::service::equipment::Catalog,
) -> Result<(), String> {
    catalog.validate()?;
    if catalog.gear.is_empty() {
        return Ok(());
    }
    for gear in &catalog.gear {
        equipment_model(pack, gear)?;
    }
    let names = scene
        .actors
        .iter()
        .filter(|a| a.model == "adventurer")
        .map(|a| a.model.as_str())
        .chain(outfits.outfits.iter().map(|o| o.model.as_str()));
    for name in names {
        let model = pack
            .models
            .get(name)
            .ok_or("Equipment parent rig is missing")?;
        verse_engine::sockets::Sockets::admit(model)?;
        if catalog
            .gear
            .iter()
            .any(|g| !model.attachments.iter().any(|a| a.id == g.slot.socket()))
        {
            return Err("Equipment parent rig is missing a required socket".into());
        }
    }
    Ok(())
}
pub fn equipment_model(
    pack: &Pack,
    gear: &verse_world::service::equipment::Gear,
) -> Result<(), String> {
    gear.validate()?;
    let model = pack
        .models
        .get(&gear.model)
        .ok_or("Equipped gear model is missing from admitted pack")?;
    if !model.clips.is_empty() {
        return Err("Equipment model must be static".into());
    }
    model.validate_animation()
}
/// Admits a hosted scene and its visible equipment against one compiled pack.
pub fn admit(
    pack: &Pack,
    scene: &Scene,
    dir: &Path,
    outfits: &verse_world::service::outfits::Catalog,
    equipment: &verse_world::service::equipment::Catalog,
) -> Result<[u8; 32], String> {
    let source = admit_source(pack, scene, dir, outfits, equipment)?;
    match verse_engine::mips::archive::Archive::read(pack, dir)? {
        Some(archive) => bind_mips(source, archive.identity()?),
        None => Ok(source),
    }
}
/// Admits verified source data while a content tool compiles material variants.
pub fn admit_source(
    pack: &Pack,
    scene: &Scene,
    dir: &Path,
    outfits: &verse_world::service::outfits::Catalog,
    equipment: &verse_world::service::equipment::Catalog,
) -> Result<[u8; 32], String> {
    pack.validate()?;
    scene.validate()?;
    outfit_models(pack, outfits)?;
    equipment_models(pack, scene, outfits, equipment)?;
    for actor in &scene.actors {
        if !pack.models.contains_key(&actor.model) {
            return Err("Scene actor model is missing from admitted pack".into());
        }
    }
    let catalog = verse_engine::residency::Catalog::new(pack)?;
    for cue in &scene.cues {
        if let verse_engine::director::Action::Yell { animation, .. } = cue.action {
            let actor = scene
                .actors
                .iter()
                .find(|actor| actor.id == cue.actor)
                .ok_or("Scene cue actor is missing")?;
            catalog.check_animation(catalog.model(&actor.model)?, animation)?;
        }
    }
    identity_source(pack, scene, dir)
}
/// Computes once before login. The supplied asset directory is not part of the digest.
pub fn identity(pack: &Pack, scene: &Scene, dir: &Path) -> Result<[u8; 32], String> {
    let content = identity_source(pack, scene, dir)?;
    match verse_engine::mips::archive::Archive::read(pack, dir)? {
        Some(archive) => bind_mips(content, archive.identity()?),
        None => Ok(content),
    }
}
fn identity_source(pack: &Pack, scene: &Scene, dir: &Path) -> Result<[u8; 32], String> {
    pack.validate()?;
    scene.validate()?;
    let mut digest = Sha256::new();
    digest.update(b"verse.remote.content.v1\0");
    for bytes in [
        serde_json::to_vec(scene).map_err(|_| "Cannot encode scene content")?,
        serde_json::to_vec(pack).map_err(|_| "Cannot encode compiled asset content")?,
    ] {
        if bytes.len() > 128 * 1024 * 1024 {
            return Err("Remote content manifest exceeds its byte budget".into());
        }
        digest.update((bytes.len() as u64).to_be_bytes());
        digest.update(bytes);
    }
    let mut total = 0u64;
    for texture in &pack.textures {
        let file = std::fs::File::open(dir.join(&texture.file))
            .map_err(|_| "Cannot open admitted runtime texture")?;
        let metadata = file
            .metadata()
            .map_err(|_| "Cannot inspect admitted runtime texture")?;
        let length = metadata.len();
        total = total
            .checked_add(length)
            .ok_or("Runtime texture byte budget exceeded")?;
        if !metadata.is_file() || length > 64 * 1024 * 1024 || total > 512 * 1024 * 1024 {
            return Err("Runtime texture byte budget exceeded".into());
        }
        let mut file = file.take(64 * 1024 * 1024 + 1);
        let mut content = Sha256::new();
        let mut read = 0u64;
        let mut buffer = [0u8; 65536];
        loop {
            let count = file
                .read(&mut buffer)
                .map_err(|_| "Cannot read admitted runtime texture")?;
            if count == 0 {
                break;
            }
            read += count as u64;
            content.update(&buffer[..count]);
        }
        if read != length {
            return Err("Runtime texture changed during admission".into());
        }
        let content = content.finalize();
        if format!("{:x}", content) != texture.sha256 {
            return Err("Runtime texture digest differs from asset manifest".into());
        }
        digest.update((texture.file.len() as u64).to_be_bytes());
        digest.update(texture.file.as_bytes());
        digest.update(content);
    }
    Ok(digest.finalize().into())
}
/// Binds admitted authored mip bytes without depending on their file location.
pub fn bind_mips(content: [u8; 32], mips: [u8; 32]) -> Result<[u8; 32], String> {
    let mut digest = Sha256::new();
    digest.update(b"verse.remote.content.mips.v1\0");
    digest.update(content);
    digest.update(mips);
    Ok(digest.finalize().into())
}

/// Computes the host's content identity from bytes already admitted by the loader.
/// This performs no filesystem reads and includes any admitted mip archive.
pub fn identity_prepared(
    prepared: &verse_engine::loading::Prepared,
    scene: &Scene,
) -> Result<[u8; 32], String> {
    let pack = prepared.pack();
    pack.validate()?;
    scene.validate()?;
    let mut digest = Sha256::new();
    digest.update(b"verse.remote.content.v1\0");
    for bytes in [
        serde_json::to_vec(scene).map_err(|_| "Cannot encode scene content")?,
        serde_json::to_vec(pack).map_err(|_| "Cannot encode compiled asset content")?,
    ] {
        if bytes.len() > 128 * 1024 * 1024 {
            return Err("Remote content manifest exceeds its byte budget".into());
        }
        digest.update((bytes.len() as u64).to_be_bytes());
        digest.update(bytes);
    }
    if prepared.receipt().textures.len() != pack.textures.len() {
        return Err("Admitted texture closure differs from pack".into());
    }
    for (texture, receipt) in pack.textures.iter().zip(&prepared.receipt().textures) {
        if texture.file != receipt.file || texture.sha256 != receipt.sha256 {
            return Err("Admitted texture differs from pack".into());
        }
        let mut hash = [0; 32];
        if receipt.sha256.len() != 64 || !receipt.sha256.is_ascii() {
            return Err("Admitted texture digest is malformed".into());
        }
        for (i, out) in hash.iter_mut().enumerate() {
            *out = u8::from_str_radix(&receipt.sha256[i * 2..i * 2 + 2], 16)
                .map_err(|_| "Admitted texture digest is malformed")?;
        }
        digest.update((texture.file.len() as u64).to_be_bytes());
        digest.update(texture.file.as_bytes());
        digest.update(hash);
    }
    let content = digest.finalize().into();
    match prepared.mips() {
        Some(mips) => bind_mips(content, mips.identity()?),
        None => Ok(content),
    }
}
