//! Content identity for the scene, compiled geometry, and runtime textures.
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path};
use verse_engine::{assets::Pack, director::Scene};
/// Computes once before login. The supplied asset directory is not part of the digest.
pub fn identity(pack: &Pack, scene: &Scene, dir: &Path) -> Result<[u8; 32], String> {
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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_binds_scene_geometry_and_verified_texture_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let mut pack = super::super::original::generate(dir.path()).unwrap();
        let mut scene = Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let original = identity(&pack, &scene, dir.path()).unwrap();
        assert_eq!(original, identity(&pack, &scene, dir.path()).unwrap());
        let relocated = tempfile::tempdir().unwrap();
        for texture in &pack.textures {
            std::fs::copy(
                dir.path().join(&texture.file),
                relocated.path().join(&texture.file),
            )
            .unwrap();
        }
        assert_eq!(original, identity(&pack, &scene, relocated.path()).unwrap());
        scene.actors[0].position.x += 1.;
        assert_ne!(original, identity(&pack, &scene, dir.path()).unwrap());
        scene.actors[0].position.x -= 1.;
        pack.source_revision.push_str("-changed");
        assert_ne!(original, identity(&pack, &scene, dir.path()).unwrap());
        pack.source_revision
            .truncate(pack.source_revision.len() - 8);
        assert!(!pack.textures.is_empty());
        let path = dir.path().join(&pack.textures[0].file);
        let before = std::fs::read(&path).unwrap();
        let mut modified = before.clone();
        modified[0] ^= 1;
        std::fs::write(&path, &modified).unwrap();
        assert!(identity(&pack, &scene, dir.path()).is_err());
        pack.textures[0].sha256 = format!("{:x}", Sha256::digest(&modified));
        assert_ne!(original, identity(&pack, &scene, dir.path()).unwrap());
        std::fs::remove_file(path).unwrap();
        assert!(identity(&pack, &scene, dir.path()).is_err());
    }
}
