//! Validation for portable scene records and unsigned asset manifests.
use crate::*;
use prost::Message;
use std::collections::{BTreeMap, BTreeSet};

fn text(s: &str, required: bool) -> bool {
    (!required || !s.is_empty()) && s.len() <= 192 && s.bytes().all(|b| b >= 32 && b != 127)
}
fn token(s: &str, maximum: usize) -> bool {
    !s.is_empty()
        && s.len() <= maximum
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}
pub fn safe_path(s: &str) -> bool {
    text(s, true) && !s.contains(['\\', ':']) && s.split('/').all(|p| !matches!(p, "" | "." | ".."))
}
fn camera(c: &SceneCamera) -> bool {
    id(&c.id)
        && text(&c.name, false)
        && c.position.as_ref().is_some_and(point)
        && c.target.as_ref().is_some_and(point)
        && c.position != c.target
        && c.vertical_fov_degrees.is_finite()
        && (1.0..=179.0).contains(&c.vertical_fov_degrees)
}
pub fn validate_scene(scene: &CompiledScene) -> Result<(), &'static str> {
    if scene.schema_version != 1
        || !id(&scene.scene_id)
        || scene.coordinates != 1
        || scene.placements.len() > 32768
        || scene.lights.len() > 2048
        || scene.cameras.len() > 64
        || !scene.spawn.as_ref().is_some_and(camera)
    {
        return Err("invalid compiled scene header, limits, or spawn");
    }
    for ids in [
        scene.placements.iter().map(|p| &p.id).collect::<Vec<_>>(),
        scene.lights.iter().map(|p| &p.id).collect(),
        scene.cameras.iter().map(|p| &p.id).collect(),
    ] {
        let unique: BTreeSet<_> = ids.iter().collect();
        if unique.len() != ids.len() || ids.iter().any(|s| !id(s)) {
            return Err("invalid or duplicate scene ID");
        }
    }
    for placement in &scene.placements {
        let m = &placement.transform;
        if !text(&placement.asset_id, true)
            || !text(&placement.actor, false)
            || !text(&placement.source_class, false)
            || m.len() != 16
            || m.iter().any(|n| !n.is_finite() || n.abs() > 1_000_000.)
        {
            return Err("invalid compiled placement");
        }
        // Evaluate the same float32 wire matrix in float64 in every consumer.
        let d: [f64; 16] = std::array::from_fn(|i| f64::from(m[i]));
        let determinant = d[0] * (d[5] * d[10] - d[9] * d[6]) - d[4] * (d[1] * d[10] - d[9] * d[2])
            + d[8] * (d[1] * d[6] - d[5] * d[2]);
        if determinant.abs() <= 1e-12
            || [3, 7, 11].iter().any(|i| m[*i].abs() > 1e-5)
            || (m[15] - 1.).abs() > 1e-5
        {
            return Err("invalid affine transform");
        }
    }
    for light in &scene.lights {
        if !light.position.as_ref().is_some_and(point)
            || !light.linear_color.as_ref().is_some_and(|c| {
                [c.x, c.y, c.z]
                    .iter()
                    .all(|n| n.is_finite() && (0.0..=32.0).contains(n))
            })
            || !light.radius_m.is_finite()
            || light.radius_m <= 0.
            || light.radius_m > 1_000_000.
        {
            return Err("invalid compiled light");
        }
    }
    if scene.cameras.iter().any(|c| !camera(c)) {
        return Err("invalid compiled camera");
    }
    Ok(())
}
pub fn decode_scene(bytes: &[u8]) -> Result<CompiledScene, &'static str> {
    use prost::Message;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("compiled scene exceeds byte limit");
    }
    let scene = CompiledScene::decode(bytes).map_err(|_| "invalid compiled scene encoding")?;
    validate_scene(&scene)?;
    Ok(scene)
}
pub fn validate_manifest(manifest: &AssetManifest) -> Result<(), &'static str> {
    if manifest.schema_version != 1
        || !id(&manifest.scene_id)
        || !token(&manifest.profile, 64)
        || !(1..=32).contains(&manifest.artifacts.len())
    {
        return Err("invalid asset manifest header");
    }
    let (mut ids, mut paths, mut total) = (BTreeSet::new(), BTreeSet::new(), 0u64);
    for asset in &manifest.artifacts {
        if !id(&asset.id)
            || !ids.insert(&asset.id)
            || !safe_path(&asset.path)
            || !paths.insert(&asset.path)
            || asset.sha256.len() != 32
            || !(1..=512 * 1024 * 1024).contains(&asset.byte_length)
            || !token(&asset.format, 64)
            || asset.dependencies.len() > 8
            || asset.buffers.len() > 8
            || asset.dependencies.iter().collect::<BTreeSet<_>>().len() != asset.dependencies.len()
        {
            return Err("invalid asset artifact");
        }
        total += asset.byte_length;
        let mut semantics = BTreeSet::new();
        for buffer in &asset.buffers {
            if !id(&buffer.semantic)
                || !semantics.insert(&buffer.semantic)
                || buffer.byte_length == 0
                || buffer.byte_stride == 0
                || buffer
                    .byte_offset
                    .checked_add(buffer.byte_length)
                    .is_none_or(|end| end > asset.byte_length)
                || buffer
                    .element_count
                    .checked_mul(u64::from(buffer.byte_stride))
                    != Some(buffer.byte_length)
            {
                return Err("invalid buffer descriptor");
            }
        }
    }
    if total > 1024 * 1024 * 1024 {
        return Err("manifest total exceeds byte limit");
    }
    let graph: BTreeMap<_, _> = manifest
        .artifacts
        .iter()
        .map(|a| (a.id.as_str(), &a.dependencies))
        .collect();
    fn visit<'a>(
        key: &'a str,
        graph: &BTreeMap<&'a str, &'a Vec<String>>,
        active: &mut BTreeSet<&'a str>,
        done: &mut BTreeSet<&'a str>,
    ) -> Result<(), &'static str> {
        let dependencies = graph.get(key).ok_or("missing asset dependency")?;
        if done.contains(key) {
            return Ok(());
        }
        if !active.insert(key) {
            return Err("cyclic asset dependency");
        }
        for dependency in *dependencies {
            visit(dependency, graph, active, done)?;
        }
        active.remove(key);
        done.insert(key);
        Ok(())
    }
    let (mut active, mut done) = (BTreeSet::new(), BTreeSet::new());
    for key in graph.keys() {
        visit(key, &graph, &mut active, &mut done)?;
    }
    Ok(())
}
pub fn decode_manifest(bytes: &[u8]) -> Result<AssetManifest, &'static str> {
    use prost::Message;
    if bytes.len() > 64 * 1024 {
        return Err("manifest exceeds byte limit");
    }
    let manifest = AssetManifest::decode(bytes).map_err(|_| "invalid manifest encoding")?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

/// Bind exported files without changing their encoding or publication policy.
pub fn manifest_files(
    directory: &std::path::Path,
    scene: &str,
    profile: &str,
    files: &[(&str, &str, &str, &[&str])],
) -> Result<AssetManifest, String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut manifest = AssetManifest {
        schema_version: 1,
        scene_id: scene.into(),
        profile: profile.into(),
        artifacts: vec![],
    };
    for (id, path, format, dependencies) in files {
        if !safe_path(path) {
            return Err("unsafe asset path".into());
        }
        let mut current = directory.to_path_buf();
        for part in path.split('/') {
            current.push(part);
            if std::fs::symlink_metadata(&current)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_symlink()
            {
                return Err("symlink asset path".into());
            }
        }
        let mut file = std::fs::File::open(current).map_err(|e| e.to_string())?;
        let length = file.metadata().map_err(|e| e.to_string())?.len();
        if !(1..=512 * 1024 * 1024).contains(&length) {
            return Err("asset exceeds byte limit".into());
        }
        let (mut hash, mut total, mut buffer) = (Sha256::new(), 0u64, [0u8; 65536]);
        loop {
            let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
            if count == 0 {
                break;
            }
            total += count as u64;
            if total > length {
                return Err("asset changed while hashing".into());
            }
            hash.update(&buffer[..count]);
        }
        if total != length {
            return Err("asset changed while hashing".into());
        }
        manifest.artifacts.push(Artifact {
            id: (*id).into(),
            path: (*path).into(),
            format: (*format).into(),
            sha256: hash.finalize().to_vec(),
            byte_length: length,
            dependencies: dependencies.iter().map(|s| (*s).into()).collect(),
            buffers: vec![],
        });
    }
    validate_manifest(&manifest).map_err(str::to_owned)?;
    Ok(manifest)
}

/// Consumer profiles are generated from top-level proto/profiles.json.
/// Requirements are compiled into the consumer, never trusted from a manifest.
pub fn consumer_profile(id: &str) -> Result<&'static ConsumerAssetProfile, String> {
    static CATALOG: std::sync::OnceLock<AssetProfileCatalog> = std::sync::OnceLock::new();
    let catalog = CATALOG.get_or_init(|| {
        AssetProfileCatalog::decode(&include_bytes!("../data/profiles.pb")[..])
            .expect("checked consumer profiles")
    });
    catalog
        .profiles
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| "unknown asset consumer profile".into())
}

pub fn manifest_for_profile(
    directory: &std::path::Path,
    id: &str,
) -> Result<AssetManifest, String> {
    let profile = consumer_profile(id)?;
    let dependencies: Vec<Vec<&str>> = profile
        .files
        .iter()
        .map(|f| f.dependencies.iter().map(String::as_str).collect())
        .collect();
    let files: Vec<_> = profile
        .files
        .iter()
        .zip(&dependencies)
        .map(|(f, d)| {
            (
                f.id.as_str(),
                f.path.as_str(),
                f.format.as_str(),
                d.as_slice(),
            )
        })
        .collect();
    manifest_files(directory, &profile.scene_id, id, &files)
}

pub fn validate_profile(manifest: &AssetManifest) -> Result<(), String> {
    validate_manifest(manifest).map_err(str::to_owned)?;
    let profile = consumer_profile(&manifest.profile)?;
    if profile.scene_id != manifest.scene_id || profile.files.len() != manifest.artifacts.len() {
        return Err("asset profile required files differ".into());
    }
    for file in &profile.files {
        let asset = manifest
            .artifacts
            .iter()
            .find(|a| a.id == file.id)
            .ok_or("asset profile required file missing")?;
        if asset.path != file.path
            || asset.format != file.format
            || asset.dependencies.iter().collect::<BTreeSet<_>>()
                != file.dependencies.iter().collect::<BTreeSet<_>>()
        {
            return Err("asset profile required file differs".into());
        }
        for buffer in &file.buffers {
            if !asset
                .buffers
                .iter()
                .any(|b| b.semantic == buffer.semantic && b.byte_stride == buffer.byte_stride)
            {
                return Err("asset profile required buffer missing".into());
            }
        }
    }
    Ok(())
}

pub fn write_manifest(path: &std::path::Path, manifest: &AssetManifest) -> Result<(), String> {
    validate_manifest(manifest).map_err(str::to_owned)?;
    if consumer_profile(&manifest.profile).is_ok() {
        validate_profile(manifest)?;
    }
    std::fs::write(path, manifest.encode_to_vec()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_portable_corpus() {
        let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/portable");
        for entry in std::fs::read_dir(folder).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|ext| ext != "pb") {
                continue;
            }
            let name = path.file_name().unwrap().to_str().unwrap();
            let data = std::fs::read(&path).unwrap();
            let result = if name.starts_with("manifest") {
                decode_manifest(&data).map(|_| ())
            } else {
                decode_scene(&data).map(|_| ())
            };
            assert_eq!(
                result.is_ok(),
                !name.contains("invalid"),
                "{name}: {result:?}"
            );
        }
    }
    #[test]
    fn grid_contains_the_expected_gym_profile() {
        let scene = grid();
        assert_eq!(scene.scene_id, "grid");
        assert_eq!(
            scene
                .colliders
                .iter()
                .filter(|c| c.id.starts_with("gym-"))
                .count(),
            5
        );
        assert_eq!(scene.landmarks.len(), 7);
    }
}

#[cfg(test)]
mod profile_tests {
    use super::*;
    #[test]
    fn compiled_profiles_bind_files_and_buffer_semantics() {
        for identity in [
            "aethyr-grid-v1",
            "aethyr-town-v1",
            "openagents-grid-pack-v1",
        ] {
            let profile = consumer_profile(identity).unwrap();
            let mut manifest = AssetManifest {
                schema_version: 1,
                scene_id: profile.scene_id.clone(),
                profile: identity.into(),
                artifacts: vec![],
            };
            for file in &profile.files {
                manifest.artifacts.push(Artifact {
                    id: file.id.clone(),
                    path: file.path.clone(),
                    format: file.format.clone(),
                    dependencies: file.dependencies.clone(),
                    byte_length: 128,
                    sha256: vec![0; 32],
                    buffers: file
                        .buffers
                        .iter()
                        .map(|b| BufferDescriptor {
                            semantic: b.semantic.clone(),
                            byte_stride: b.byte_stride,
                            byte_length: u64::from(b.byte_stride),
                            element_count: 1,
                            byte_offset: 0,
                        })
                        .collect(),
                });
            }
            validate_profile(&manifest).unwrap();
            manifest.artifacts[0].path = "wrong-file.bin".into();
            assert!(validate_profile(&manifest).is_err());
        }
        assert!(consumer_profile("unknown-profile").is_err());
    }
}
