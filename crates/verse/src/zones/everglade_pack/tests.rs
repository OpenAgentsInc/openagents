use super::compile::{self, SETS};
use super::format::{self, Contents, EncodedTexture};
use super::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// An 8x8 RGBA leaf card: opaque green on the left half, transparent black on
/// the right.
fn leaf_png() -> Vec<u8> {
    let mut rgba = Vec::new();
    for _y in 0..8 {
        for x in 0..8 {
            let pixel: [u8; 4] = if x < 4 { [40, 160, 60, 255] } else { [0; 4] };
            rgba.extend_from_slice(&pixel);
        }
    }
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, 8, 8);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&rgba).unwrap();
    writer.finish().unwrap();
    bytes
}

/// A one-quad, alpha-masked card offset by one meter on x by its node, with a
/// normal map the compiler must ignore.
fn card() -> (Vec<u8>, Vec<u8>) {
    let mut bin = Vec::new();
    for p in [[0f32, 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]] {
        p.iter()
            .for_each(|n| bin.extend_from_slice(&n.to_le_bytes()));
    }
    for _ in 0..4 {
        [0f32, 0., 1.]
            .iter()
            .for_each(|n| bin.extend_from_slice(&n.to_le_bytes()));
    }
    for uv in [[0f32, 1.], [1., 1.], [1., 0.], [0., 0.]] {
        uv.iter()
            .for_each(|n| bin.extend_from_slice(&n.to_le_bytes()));
    }
    for index in [0u16, 1, 2, 0, 2, 3] {
        bin.extend_from_slice(&index.to_le_bytes());
    }
    let gltf = serde_json::json!({
        "asset": {"version": "2.0"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0, "name": "Card", "translation": [1.0, 0.0, 0.0]}],
        "materials": [{
            "name": "Leaf",
            "alphaMode": "MASK",
            "alphaCutoff": 0.5,
            "doubleSided": true,
            "normalTexture": {"index": 1},
            "pbrMetallicRoughness": {"baseColorTexture": {"index": 0}, "metallicFactor": 0.0}
        }],
        "meshes": [{"name": "Card", "primitives": [{
            "attributes": {"POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2},
            "indices": 3,
            "material": 0
        }]}],
        "textures": [{"source": 0}, {"source": 1}],
        "images": [{"uri": "Leaf.png"}, {"uri": "Leaf_Normal.png"}],
        "accessors": [
            {"bufferView": 0, "componentType": 5126, "count": 4, "type": "VEC3",
             "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 0.0]},
            {"bufferView": 1, "componentType": 5126, "count": 4, "type": "VEC3"},
            {"bufferView": 2, "componentType": 5126, "count": 4, "type": "VEC2"},
            {"bufferView": 3, "componentType": 5123, "count": 6, "type": "SCALAR"}
        ],
        "bufferViews": [
            {"buffer": 0, "byteOffset": 0, "byteLength": 48},
            {"buffer": 0, "byteOffset": 48, "byteLength": 48},
            {"buffer": 0, "byteOffset": 96, "byteLength": 32},
            {"buffer": 0, "byteOffset": 128, "byteLength": 12}
        ],
        "buffers": [{"byteLength": 140, "uri": "Card.bin"}]
    });
    (serde_json::to_vec_pretty(&gltf).unwrap(), bin)
}

/// Writes one admitted set, `glade`, and returns the root.
fn synthetic_root() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    let set = root.path().join("glade");
    std::fs::create_dir(&set).unwrap();
    let (gltf, bin) = card();
    let files: Vec<(&str, Vec<u8>)> = vec![
        ("Card.gltf", gltf),
        ("Card.bin", bin),
        ("Leaf.png", leaf_png()),
        ("license.txt", b"CC0 1.0 Universal".to_vec()),
    ];
    let mut digests = BTreeMap::new();
    for (name, bytes) in &files {
        std::fs::write(set.join(name), bytes).unwrap();
        digests.insert(name.to_string(), sha(bytes));
    }
    write_manifest(&set, &digests, &digests, &BTreeMap::new());
    root
}

fn write_manifest(
    set: &Path,
    files: &BTreeMap<String, String>,
    originals: &BTreeMap<String, String>,
    transforms: &BTreeMap<String, String>,
) {
    let manifest = serde_json::json!({
        "schema": compile::SCHEMA,
        "creator": "Quaternius",
        "license": "CC0-1.0",
        "package": "Synthetic Glade Kit",
        "files": files,
        "originals": originals,
        "transforms": transforms,
    });
    std::fs::write(
        set.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
}

#[test]
fn synthetic_sources_compile_deterministically_and_round_trip() {
    let root = synthetic_root();
    let first = compile::compile(root.path(), &["glade"], &Limits::EVERGLADE).unwrap();
    let second = compile::compile(root.path(), &["glade"], &Limits::EVERGLADE).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.sha256, sha(&first.bytes));
    assert_eq!((first.models, first.triangles), (1, 2));

    let pack = format::decode(&first.bytes, &Limits::EVERGLADE).unwrap();
    // The normal map is ignored, so only the base-color texture is packed,
    // and a texture within its edge keeps its admitted bytes.
    assert_eq!(pack.textures.len(), 1);
    assert_eq!(pack.textures[0].name, "glade/Leaf");
    assert_eq!((pack.textures[0].width, pack.textures[0].height), (8, 8));
    let contents = format::decode_contents(&first.bytes, &Limits::EVERGLADE).unwrap();
    assert_eq!(contents.textures[0].png, leaf_png());

    let material = &pack.materials[0];
    assert_eq!(material.name, "glade/Leaf");
    assert_eq!(material.texture, Some(0));
    assert_eq!(material.alpha, AlphaMode::Mask { cutoff: 0.5 });
    assert!(material.double_sided);

    let model = pack.model("glade/Card").expect("card model");
    assert_eq!(model.triangles(), 2);
    // The node's translation is applied to every position.
    let (min, max) = model.bounds();
    assert_eq!((min[0], max[0]), (1.0, 2.0));
    let vertex = model.primitives[0].vertices[0];
    assert_eq!(vertex.color, [255; 4]);
    assert!((vertex.normal[2] - 1.0).abs() < 1e-6);
}

#[test]
fn oversized_textures_are_downscaled_with_alpha_weighting() {
    let root = synthetic_root();
    let limits = Limits {
        texture_edge: 4,
        ..Limits::EVERGLADE
    };
    let compiled = compile::compile(root.path(), &["glade"], &limits).unwrap();
    let pack = format::decode(&compiled.bytes, &limits).unwrap();
    let texture = &pack.textures[0];
    assert_eq!((texture.width, texture.height), (4, 4));
    // Left half stays opaque green; the right half stays transparent.
    assert_eq!(&texture.rgba[0..4], &[40, 160, 60, 255]);
    assert_eq!(texture.rgba[3 * 4 + 3], 0);

    // One opaque red texel among three transparent black ones keeps its
    // color rather than averaging toward black.
    let source = [255, 0, 0, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let (width, height, out) = compile::downscale(&source, 2, 2, 1);
    assert_eq!((width, height), (1, 1));
    assert_eq!(out, vec![255, 0, 0, 64]);

    // Non-square images keep their aspect ratio.
    let wide = vec![255u8; 8 * 2 * 4];
    let (width, height, _) = compile::downscale(&wide, 8, 2, 4);
    assert_eq!((width, height), (4, 1));
}

#[test]
fn changed_sources_are_refused_by_digest() {
    let root = synthetic_root();
    let bin = root.path().join("glade/Card.bin");
    let mut bytes = std::fs::read(&bin).unwrap();
    bytes[0] ^= 1;
    std::fs::write(&bin, bytes).unwrap();
    let error = compile::compile(root.path(), &["glade"], &Limits::EVERGLADE).unwrap_err();
    assert!(error.contains("digest mismatch: glade/Card.bin"), "{error}");
}

#[test]
fn unadmitted_and_linked_files_are_refused() {
    let root = synthetic_root();
    std::fs::write(root.path().join("glade/Leaf_Normal.png"), leaf_png()).unwrap();
    let error = compile::compile(root.path(), &["glade"], &Limits::EVERGLADE).unwrap_err();
    assert!(error.contains("unadmitted file"), "{error}");

    #[cfg(unix)]
    {
        let root = synthetic_root();
        let set = root.path().join("glade");
        let outside = root.path().join("Leaf.png");
        std::fs::rename(set.join("Leaf.png"), &outside).unwrap();
        std::os::unix::fs::symlink(&outside, set.join("Leaf.png")).unwrap();
        assert!(compile::compile(root.path(), &["glade"], &Limits::EVERGLADE).is_err());
    }
}

#[test]
fn every_budget_is_enforced_by_the_compiler() {
    let root = synthetic_root();
    let refused = |limits: Limits| {
        compile::compile(root.path(), &["glade"], &limits).expect_err("budget must refuse")
    };
    let error = refused(Limits {
        triangles: 1,
        ..Limits::EVERGLADE
    });
    assert!(error.contains("triangle budget"), "{error}");
    let error = refused(Limits {
        model_triangles: 1,
        ..Limits::EVERGLADE
    });
    assert!(error.contains("per-model triangle budget"), "{error}");
    let error = refused(Limits {
        decoded_texture_bytes: 16,
        ..Limits::EVERGLADE
    });
    assert!(error.contains("decoded texture budget"), "{error}");
    let error = refused(Limits {
        pack_bytes: 64,
        ..Limits::EVERGLADE
    });
    assert!(error.contains("transfer budget"), "{error}");
    let error = refused(Limits {
        committed_bytes: 100,
        ..Limits::EVERGLADE
    });
    assert!(error.contains("committed budget"), "{error}");
}

#[test]
fn manifests_must_pin_license_digests_and_transforms() {
    let digest = sha(b"x");
    let other = sha(b"y");
    let manifest =
        |files: serde_json::Value, originals: serde_json::Value, transforms: serde_json::Value| {
            serde_json::to_vec(&serde_json::json!({
                "schema": compile::SCHEMA,
                "creator": "Quaternius",
                "license": "CC0-1.0",
                "package": "Kit",
                "files": files,
                "originals": originals,
                "transforms": transforms,
            }))
            .unwrap()
        };
    let base = serde_json::json!({"A.gltf": digest, "license.txt": digest});
    assert!(
        compile::parse_manifest(&manifest(base.clone(), base.clone(), serde_json::json!({})))
            .is_ok()
    );

    // No license text.
    let files = serde_json::json!({"A.gltf": digest});
    assert!(
        compile::parse_manifest(&manifest(files.clone(), files, serde_json::json!({}))).is_err()
    );
    // An unread normal map.
    let files =
        serde_json::json!({"A.gltf": digest, "license.txt": digest, "Bark_Normal.png": digest});
    assert!(
        compile::parse_manifest(&manifest(files.clone(), files, serde_json::json!({}))).is_err()
    );
    // A base-color texture whose name merely contains "Normal" is fine.
    let files =
        serde_json::json!({"A.gltf": digest, "license.txt": digest, "Bark_NormalTree.png": digest});
    assert!(
        compile::parse_manifest(&manifest(files.clone(), files, serde_json::json!({}))).is_ok()
    );
    // A missing original digest.
    assert!(
        compile::parse_manifest(&manifest(
            base.clone(),
            serde_json::json!({"A.gltf": digest}),
            serde_json::json!({})
        ))
        .is_err()
    );
    // A changed file without a transform, and a transform on an unchanged file.
    let files = serde_json::json!({"A.gltf": digest, "license.txt": digest, "T.png": digest});
    let originals = serde_json::json!({"A.gltf": digest, "license.txt": digest, "T.png": other});
    assert!(
        compile::parse_manifest(&manifest(
            files.clone(),
            originals.clone(),
            serde_json::json!({})
        ))
        .is_err()
    );
    assert!(
        compile::parse_manifest(&manifest(
            files.clone(),
            originals,
            serde_json::json!({"T.png": "sips -Z 512"})
        ))
        .is_ok()
    );
    assert!(
        compile::parse_manifest(&manifest(
            files.clone(),
            files,
            serde_json::json!({"T.png": "sips -Z 512"})
        ))
        .is_err()
    );
    // An escaping path and a malformed digest.
    let files = serde_json::json!({"../A.gltf": digest, "license.txt": digest});
    assert!(
        compile::parse_manifest(&manifest(files.clone(), files, serde_json::json!({}))).is_err()
    );
    let files = serde_json::json!({"A.gltf": "ABC", "license.txt": digest});
    assert!(
        compile::parse_manifest(&manifest(files.clone(), files, serde_json::json!({}))).is_err()
    );
    // An unknown field and the wrong license.
    let mut value: serde_json::Value =
        serde_json::from_slice(&manifest(base.clone(), base.clone(), serde_json::json!({})))
            .unwrap();
    value["extra"] = serde_json::json!(true);
    assert!(compile::parse_manifest(&serde_json::to_vec(&value).unwrap()).is_err());
    value.as_object_mut().unwrap().remove("extra");
    value["license"] = serde_json::json!("CC-BY-4.0");
    assert!(compile::parse_manifest(&serde_json::to_vec(&value).unwrap()).is_err());
}

#[test]
fn committed_manifests_admit_only_base_color_sources_with_licenses() {
    for set in SETS {
        let directory = repository().join(PACK_DIRECTORY).join(set);
        let bytes = std::fs::read(directory.join("manifest.json")).unwrap();
        let manifest = compile::parse_manifest(&bytes).unwrap();
        assert!(manifest.package.ends_with("MegaKit Standard"), "{set}");
        let license = std::fs::read_to_string(directory.join("license.txt")).unwrap();
        assert!(license.contains("CC0"), "{set} license text");
        assert!(directory.join("README.md").is_file(), "{set} README");
        for name in manifest.transforms.keys() {
            assert!(name.ends_with(".png"), "{set}/{name}");
        }
    }
}

/// Compiles the committed sources. Once the pack is published, the result
/// must be byte-identical to the committed pack and its pinned digest.
#[test]
fn committed_sources_compile_within_budgets_to_the_pinned_pack() {
    let root = repository().join(PACK_DIRECTORY);
    let compiled = compile::compile(&root, &SETS, &Limits::EVERGLADE).unwrap();
    assert!(compiled.triangles <= Limits::EVERGLADE.triangles);
    assert!(compiled.decoded_texture_bytes <= Limits::EVERGLADE.decoded_texture_bytes);
    assert!(
        compiled.source_bytes + compiled.bytes.len() as u64 <= Limits::EVERGLADE.committed_bytes
    );
    let pack = format::decode(&compiled.bytes, &Limits::EVERGLADE).unwrap();
    for name in [
        "nature/CommonTree_1",
        "nature/RockPath_Round_Wide",
        "village/Wall_Plaster_Straight",
        "village/Window_Wide_Flat1",
        "props/Workbench",
        "props/Chest_Wood",
    ] {
        assert!(pack.model(name).is_some(), "{name}");
    }
    // Leaves are masked and double-sided; window glass blends.
    assert!(
        pack.materials
            .iter()
            .any(|m| m.name == "nature/Leaves_NormalTree"
                && matches!(m.alpha, AlphaMode::Mask { .. })
                && m.double_sided)
    );
    assert!(
        pack.materials
            .iter()
            .any(|m| m.name == "village/MI_WindowGlass"
                && m.alpha == AlphaMode::Blend
                && m.texture.is_none())
    );
    assert!(
        pack.textures
            .iter()
            .all(|t| t.width <= 1024 && t.height <= 1024)
    );

    if PACK_BYTES != 0 {
        assert_eq!(compiled.sha256, PACK_SHA256, "rebuild and repin the pack");
        assert_eq!(compiled.bytes.len() as u64, PACK_BYTES);
        let path = root.join(format!("{PACK_SHA256}.{PACK_EXTENSION}"));
        let committed = ZonePack::load_local(&path).expect("committed pack loads");
        assert_eq!(committed, pack);
    }
}

fn tiny_contents() -> Contents {
    let root = synthetic_root();
    let compiled = compile::compile(root.path(), &["glade"], &Limits::EVERGLADE).unwrap();
    format::decode_contents(&compiled.bytes, &Limits::EVERGLADE).unwrap()
}

#[test]
fn malformed_packs_are_refused_before_allocation() {
    let contents = tiny_contents();
    let bytes = format::encode(&contents, &Limits::EVERGLADE).unwrap();
    assert!(format::decode(&bytes, &Limits::EVERGLADE).is_ok());

    // Truncation, trailing data, and a foreign format.
    assert!(format::decode_contents(&bytes[..bytes.len() - 1], &Limits::EVERGLADE).is_err());
    let mut longer = bytes.clone();
    longer.push(0);
    assert!(format::decode_contents(&longer, &Limits::EVERGLADE).is_err());
    let mut foreign = bytes.clone();
    foreign[0] = b'X';
    assert!(format::decode_contents(&foreign, &Limits::EVERGLADE).is_err());

    // A count far beyond the bytes present is refused, not allocated.
    let mut huge = bytes.clone();
    huge[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(format::decode_contents(&huge, &Limits::EVERGLADE).is_err());

    // An index past the vertices cannot be encoded.
    let mut broken = contents.clone();
    broken.models[0].primitives[0].indices[0] = 4;
    assert!(format::encode(&broken, &Limits::EVERGLADE).is_err());
    // A position outside the model bound cannot be encoded.
    let mut broken = contents.clone();
    broken.models[0].primitives[0].vertices[0].position[1] = 1000.0;
    assert!(format::encode(&broken, &Limits::EVERGLADE).is_err());
    // A texture no material uses cannot be encoded.
    let mut broken = contents.clone();
    broken.textures.push(EncodedTexture {
        name: "glade/Unused".into(),
        ..contents.textures[0].clone()
    });
    assert!(format::encode(&broken, &Limits::EVERGLADE).is_err());
    // A texture whose PNG disagrees with its declared size is refused.
    let mut lying = contents.textures[0].clone();
    lying.width = 4;
    assert!(format::decode_texture(&lying).is_err());
    // A smaller decoder budget refuses the same pack.
    let tight = Limits {
        decoded_texture_bytes: 8 * 8 * 4 - 1,
        ..Limits::EVERGLADE
    };
    assert!(format::decode_contents(&bytes, &tight).is_err());
}

#[test]
fn vertex_padding_must_be_zero() {
    let contents = tiny_contents();
    let bytes = format::encode(&contents, &Limits::EVERGLADE).unwrap();
    // The first vertex follows the model header: find the first vertex's
    // position by encoding its x coordinate and locating it after the PNG.
    let first = contents.models[0].primitives[0].vertices[0];
    let mut pattern = Vec::new();
    for n in first.position {
        pattern.extend_from_slice(&n.to_le_bytes());
    }
    let png_end = 8 + 4 + 1 + "glade/Leaf".len() + 12 + contents.textures[0].png.len();
    let at = png_end
        + bytes[png_end..]
            .windows(pattern.len())
            .position(|w| w == pattern.as_slice())
            .expect("first vertex");
    let mut padded = bytes.clone();
    padded[at + 15] = 1;
    assert!(format::decode_contents(&padded, &Limits::EVERGLADE).is_err());
}

fn test_pinned(bytes: &[u8]) -> PinnedFile {
    PinnedFile {
        label: "Test pack",
        // Leaked once per test so the struct keeps its static fields.
        sha256: Box::leak(sha(bytes).into_boxed_str()),
        bytes: bytes.len() as u64,
        url: "https://example.invalid/pack.vtp".into(),
        extension: "vtp",
        temp_prefix: ".test-",
        history: &[],
    }
}

#[test]
fn placeholder_pin_refuses_every_pack_until_published() {
    if PACK_BYTES == 0 {
        assert!(pinned().verify(&[]).is_err());
        assert!(ZonePack::decode_pinned(&tiny_contents_bytes()).is_err());
    } else {
        assert_eq!(PACK_SHA256.len(), 64);
        assert!(pinned().url.ends_with(&format!("{PACK_SHA256}.vtp")));
    }
    assert!(pinned().url.starts_with("https://"));
}

fn tiny_contents_bytes() -> Vec<u8> {
    format::encode(&tiny_contents(), &Limits::EVERGLADE).unwrap()
}

#[test]
fn cache_install_is_verified_atomic_and_served_without_network() {
    let bytes = tiny_contents_bytes();
    let file = test_pinned(&bytes);
    let cache = tempfile::tempdir().unwrap();
    let cache = cache.path().join("cache");
    let cancel = AtomicBool::new(false);

    // A loader records its cache without creating it.
    let loader = Loader::new(cache.clone());
    assert!(!cache.exists());
    drop(loader);

    let mut wrong = bytes.clone();
    wrong[0] ^= 1;
    assert!(file.install_cache(&cache, &wrong, &cancel).is_err());
    assert!(
        file.install_cache(&cache, &bytes, &AtomicBool::new(true))
            .is_err()
    );
    assert!(!cache.join(file.cache_name()).exists());

    file.install_cache(&cache, &bytes, &cancel).unwrap();
    assert_eq!(
        file.read_bounded(&cache.join(file.cache_name())).unwrap(),
        bytes
    );
    // A cached entry is served without touching the unreachable URL.
    let mut downloads = 0;
    let mut progress = |_: u64, _: u64| downloads += 1;
    let pack = file
        .fetch(&cache, &cancel, &mut progress, |b| {
            format::decode(b, &Limits::EVERGLADE)
        })
        .unwrap();
    assert_eq!(pack.models.len(), 1);
    assert_eq!(downloads, 0);

    #[cfg(unix)]
    {
        let alias = cache.join("alias.vtp");
        std::os::unix::fs::symlink(cache.join(file.cache_name()), &alias).unwrap();
        assert!(file.read_bounded(&alias).is_err());
    }
}

#[test]
fn only_https_sources_and_own_temp_names_are_used() {
    let bytes = tiny_contents_bytes();
    let mut file = test_pinned(&bytes);
    file.url = "http://example.invalid/pack.vtp".into();
    let cache = tempfile::tempdir().unwrap();
    let mut progress = |_: u64, _: u64| {};
    let error = file
        .fetch(cache.path(), &AtomicBool::new(false), &mut progress, |_| {
            Ok(())
        })
        .unwrap_err();
    assert!(error.contains("HTTPS"), "{error}");

    assert!(file.is_temp_name(".test-12-3.part"));
    assert!(!file.is_temp_name(".test-12-3.vtp"));
    assert!(!file.is_temp_name(".ruins-12-3.part"));
    assert!(!file.is_temp_name(".test-a-3.part"));
}

#[cfg(unix)]
#[test]
fn pruning_removes_only_named_history_and_stale_own_temps() {
    let bytes = tiny_contents_bytes();
    let old = sha(b"old pack");
    let mut file = test_pinned(&bytes);
    let old_static: &'static str = Box::leak(old.clone().into_boxed_str());
    file.history = Box::leak(vec![old_static].into_boxed_slice());
    let cache = tempfile::tempdir().unwrap();
    let cache = cache.path();
    file.install_cache(cache, &bytes, &AtomicBool::new(false))
        .unwrap();
    std::fs::write(cache.join(format!("{old}.vtp")), b"old").unwrap();
    std::fs::write(cache.join("unrelated.vtp"), b"keep").unwrap();
    std::fs::write(cache.join(".test-1-1.part"), b"fresh temp").unwrap();
    file.prune(cache);
    assert!(!cache.join(format!("{old}.vtp")).exists());
    assert!(cache.join("unrelated.vtp").exists());
    // A recent temp may belong to a live install.
    assert!(cache.join(".test-1-1.part").exists());
    assert!(cache.join(file.cache_name()).exists());
}
