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
    let first = compile::compile(root.path(), &["glade"], None, &Limits::EVERGLADE).unwrap();
    let second = compile::compile(root.path(), &["glade"], None, &Limits::EVERGLADE).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.sha256, sha(&first.bytes));
    assert_eq!((first.models, first.triangles), (1, 2));

    let pack = format::decode(&first.bytes, &Limits::EVERGLADE).unwrap();
    // The normal map is ignored, so only the base-color texture is packed,
    // and a texture within its edge keeps its pixels exactly, in the
    // smaller of its admitted bytes and the compiler's encoding.
    assert_eq!(pack.textures.len(), 1);
    assert_eq!(pack.textures[0].name, "glade/Leaf");
    assert_eq!((pack.textures[0].width, pack.textures[0].height), (8, 8));
    let contents = format::decode_contents(&first.bytes, &Limits::EVERGLADE).unwrap();
    assert!(contents.textures[0].png.len() <= leaf_png().len());
    let admitted = format::decode_texture(&EncodedTexture {
        png: leaf_png(),
        ..contents.textures[0].clone()
    })
    .unwrap();
    assert_eq!(pack.textures[0].rgba, admitted.rgba);

    let material = &pack.materials[0];
    assert_eq!(material.name, "glade/Leaf");
    assert_eq!(material.texture, Some(0));
    assert_eq!(material.alpha, AlphaMode::Mask { cutoff: 0.5 });
    assert!(material.double_sided);

    let model = pack.model("glade/Card").expect("card model");
    assert_eq!(model.triangles(), 2);
    // The node's translation is applied to every position, within the
    // quantization's half step.
    let (min, max) = model.bounds();
    assert_eq!(min[0], 1.0);
    assert!((max[0] - 2.0).abs() < 1e-5, "{}", max[0]);
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
    let compiled = compile::compile(root.path(), &["glade"], None, &limits).unwrap();
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
    let error = compile::compile(root.path(), &["glade"], None, &Limits::EVERGLADE).unwrap_err();
    assert!(error.contains("digest mismatch: glade/Card.bin"), "{error}");
}

#[test]
fn unadmitted_and_linked_files_are_refused() {
    let root = synthetic_root();
    std::fs::write(root.path().join("glade/Leaf_Normal.png"), leaf_png()).unwrap();
    let error = compile::compile(root.path(), &["glade"], None, &Limits::EVERGLADE).unwrap_err();
    assert!(error.contains("unadmitted file"), "{error}");

    #[cfg(unix)]
    {
        let root = synthetic_root();
        let set = root.path().join("glade");
        let outside = root.path().join("Leaf.png");
        std::fs::rename(set.join("Leaf.png"), &outside).unwrap();
        std::os::unix::fs::symlink(&outside, set.join("Leaf.png")).unwrap();
        assert!(compile::compile(root.path(), &["glade"], None, &Limits::EVERGLADE).is_err());
    }
}

#[test]
fn every_budget_is_enforced_by_the_compiler() {
    let root = synthetic_root();
    let refused = |limits: Limits| {
        compile::compile(root.path(), &["glade"], None, &limits).expect_err("budget must refuse")
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
        if compile::FORM_SETS.contains(&set) {
            assert!(manifest.package.contains("Quaternius"), "{set}");
        } else {
            assert!(manifest.package.ends_with("MegaKit Standard"), "{set}");
        }
        let license = std::fs::read_to_string(directory.join("license.txt")).unwrap();
        assert!(license.contains("CC0"), "{set} license text");
        assert!(directory.join("README.md").is_file(), "{set} README");
        // Only images change in the kits' sets; the generated set records
        // each model's conversion from its committed glb, and the lod set
        // each far level's recipe, or the script that built it. Any of
        // them may also be losslessly compacted (`everglade_compact.py`).
        for (name, transform) in &manifest.transforms {
            let compacted = transform.contains("everglade_compact.py");
            if set == "generated" {
                assert!(
                    transform.contains("everglade_admit.py") || compacted,
                    "{set}/{name}"
                );
            } else if set == "foliage" {
                assert!(transform.contains("foliage_admit.py"), "{set}/{name}");
            } else if set == "lod" {
                assert!(
                    transform.contains("everglade_lod.py")
                        || transform.contains("town_houses.py")
                        || transform.contains("greco_futurism.py"),
                    "{set}/{name}"
                );
            } else if compile::FORM_SETS.contains(&set) {
                assert!(transform.contains("beasts_admit.py"), "{set}/{name}");
            } else {
                assert!(name.ends_with(".png"), "{set}/{name}");
            }
        }
    }
}

/// Compiles the committed sources. Once the pack is published, the result
/// must be byte-identical to the committed pack and its pinned digest.
#[test]
fn committed_sources_compile_within_budgets_to_the_pinned_pack() {
    let root = repository().join(PACK_DIRECTORY);
    let compiled = compile::compile(
        &root,
        &SETS,
        Some(&root.join(compile::PLAYER_SOURCES)),
        &Limits::EVERGLADE,
    )
    .unwrap();
    assert!(compiled.triangles <= Limits::EVERGLADE.triangles);
    assert!(compiled.decoded_texture_bytes <= Limits::EVERGLADE.decoded_texture_bytes);
    assert!(
        compiled.source_bytes + compiled.bytes.len() as u64 <= Limits::EVERGLADE.committed_bytes
    );
    let pack = format::decode(&compiled.bytes, &Limits::EVERGLADE).unwrap();
    for name in [
        "nature/CommonTree_3",
        "nature/RockPath_Round_Wide",
        "village/Wall_Plaster_Straight",
        "village/Window_Wide_Flat1",
        "props/Workbench",
        "props/Crate_Metal",
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
    // The ritual chamber's player, with the clips the movement states play,
    // within its own triangle budget and with bounded images.
    let player = pack.character.as_ref().expect("the player's character");
    assert_eq!(player.name, "player/male-ranger");
    for clip in ["idle", "walk", "run", "jump"] {
        assert!(player.clip(clip).is_some(), "{clip}");
    }
    assert!(player.clip("walk").unwrap().distance > 1.0);
    assert!(player.clip("run").unwrap().distance > player.clip("walk").unwrap().distance);
    assert_eq!(player.clip("idle").unwrap().distance, 0.0);
    assert!(player.triangles() <= Limits::EVERGLADE.character_triangles);
    // The Wild Shape beasts: the Giant Spider with its five clips, a walk
    // that carries it forward, and the stylized bear, wolf, and eagle.
    let spider = pack.form("beasts/giant_spider").expect("the Giant Spider");
    for clip in ["idle", "walk", "attack", "jump", "death"] {
        assert!(spider.clip(clip).is_some(), "spider {clip}");
    }
    assert!(spider.clip("walk").unwrap().distance > 0.2);
    assert_eq!(spider.clip("idle").unwrap().distance, 0.0);
    for name in ["beasts/bear", "beasts/wolf", "beasts/eagle"] {
        assert!(pack.form(name).is_some(), "{name}");
    }
    // Alice, the placed character: the Universal rig, the player's eight
    // clips, her high-definition level within the character budget, and
    // one 1,024-pixel atlas that an opaque primitive and the hair cards'
    // alpha-masked one share.
    let alice = pack.form(compile::ALICE_FORM).expect("Alice");
    // The Universal 65 joints and the mesh's and armature's own nodes.
    assert_eq!(alice.joints.len(), 67);
    for (name, _) in compile::PLAYER_CLIPS {
        assert!(alice.clip(name).is_some(), "Alice {name}");
    }
    assert!(alice.triangles() > 30_000, "{}", alice.triangles());
    assert!(alice.triangles() <= Limits::EVERGLADE.character_triangles);
    assert_eq!(alice.primitives.len(), 2);
    let looks: Vec<_> = alice
        .primitives
        .iter()
        .map(|p| &pack.materials[p.material as usize])
        .collect();
    assert_eq!(looks[0].texture, looks[1].texture);
    assert!(looks.iter().any(|m| m.alpha == AlphaMode::Opaque));
    assert!(
        looks
            .iter()
            .any(|m| matches!(m.alpha, AlphaMode::Mask { .. }) && m.double_sided)
    );
    // Her workstation in the owner's house is a standing desk with no
    // chair: nothing stands at seat height where she works behind it.
    let house = pack.model("generated/greco_house").expect("the house");
    let floor = 1.6;
    let seat = house
        .primitives
        .iter()
        .flat_map(|p| &p.vertices)
        .filter(|v| {
            let [x, y, z] = v.position;
            (x + 4.6).abs() < 1.5
                && (-24.4..-23.15).contains(&z)
                && (0.25..0.9).contains(&(y - floor))
        })
        .count();
    assert_eq!(seat, 0, "something stands at seat height behind her desk");
    let atlas = looks[0].texture.unwrap();
    assert_eq!(
        pack.textures[atlas as usize].width,
        compile::ALICE_TEXTURE_EDGE
    );
    assert!(compiled.triangles >= player.triangles());
    for primitive in &player.primitives {
        let texture = pack.materials[primitive.material as usize].texture.unwrap();
        let texture = &pack.textures[texture as usize];
        assert!(texture.name.starts_with("player/"));
        assert!(texture.width <= compile::PLAYER_TEXTURE_EDGE);
    }

    if PACK_BYTES != 0 {
        // The character compilers' arithmetic is portable (f64 on glam's
        // scalar types, sines from libm), so every machine compiles the
        // same sources to the same bytes.
        assert_eq!(compiled.sha256, PACK_SHA256, "rebuild and repin the pack");
        assert_eq!(compiled.bytes.len() as u64, PACK_BYTES);
        let path = root.join(format!("{}.{PACK_EXTENSION}", compiled.sha256));
        let committed = std::fs::read(&path).expect("the matching pack is committed");
        assert!(
            committed == compiled.bytes,
            "{} differs from its sources",
            path.display()
        );
        // The pinned pack, which the loader admits, decodes to the same
        // models, materials, and textures.
        let pinned = root.join(format!("{PACK_SHA256}.{PACK_EXTENSION}"));
        let pinned = ZonePack::load_local(&pinned).expect("the pinned pack loads");
        assert_eq!(pinned.models.len(), pack.models.len());
        assert_eq!(pinned.materials, pack.materials);
        assert_eq!(pinned.textures, pack.textures);
    }
}

fn tiny_contents() -> Contents {
    let root = synthetic_root();
    let compiled = compile::compile(root.path(), &["glade"], None, &Limits::EVERGLADE).unwrap();
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
fn quantized_vertices_stay_within_half_a_step_and_encode_again_identically() {
    let mut contents = tiny_contents();
    // An awkward spread: a building's height, a tiled coordinate, colors on
    // some vertices only.
    let primitive = &mut contents.models[0].primitives[0];
    let wanted = [
        ([-3.217f32, 0.0, 0.125], [-2.5f32, 0.3]),
        ([4.9, 18.578, -7.31], [24.2, -1.0]),
        ([0.333, 9.001, 2.0], [0.5, 7.77]),
        ([1.0, 1.0, 1.0], [3.0, 3.0]),
    ];
    for (vertex, (position, uv)) in primitive.vertices.iter_mut().zip(wanted) {
        vertex.position = position;
        vertex.uv = uv;
    }
    primitive.vertices[1].color = [200, 10, 30, 255];
    let bytes = format::encode(&contents, &Limits::EVERGLADE).unwrap();
    let decoded = format::decode_contents(&bytes, &Limits::EVERGLADE).unwrap();
    let got = &decoded.models[0].primitives[0].vertices;
    for (vertex, (position, uv)) in got.iter().zip(wanted) {
        for axis in 0..3 {
            let error = (vertex.position[axis] - position[axis]).abs();
            assert!(error <= 30.0 / 65535.0 / 2.0 + 1e-6, "{error}");
        }
        for axis in 0..2 {
            let error = (vertex.uv[axis] - uv[axis]).abs();
            assert!(error <= 30.0 / 65535.0 / 2.0 + 1e-6, "{error}");
        }
    }
    assert_eq!(got[1].color, [200, 10, 30, 255]);
    assert_eq!(got[0].color, [255; 4]);
    assert_eq!(format::encode(&decoded, &Limits::EVERGLADE).unwrap(), bytes);
}

#[test]
fn a_body_longer_than_declared_or_its_budget_is_refused() {
    let bytes = tiny_contents_bytes();
    // The body length follows the one texture.
    let contents = tiny_contents();
    let at = 8 + 4 + 1 + "glade/Leaf".len() + 12 + contents.textures[0].png.len();
    let declared = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    let mut short = bytes.clone();
    short[at..at + 4].copy_from_slice(&(declared - 1).to_le_bytes());
    assert!(format::decode_contents(&short, &Limits::EVERGLADE).is_err());
    let tight = Limits {
        body_bytes: u64::from(declared) - 1,
        ..Limits::EVERGLADE
    };
    assert!(format::decode_contents(&bytes, &tight).is_err());
    assert!(format::encode(&contents, &tight).is_err());
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

/// A one-triangle character on the tiny pack's material.
fn tiny_character() -> format::Character {
    use format::{Character, Clip, Joint, SkinnedPrimitive, SkinnedVertex, Track, Vertex};
    let identity = glam::Mat4::IDENTITY.to_cols_array();
    let joint = |parent| Joint {
        parent,
        translation: [0.0, 1.0, 0.0],
        rotation: [0.0, 0.0, 0.0, 1.0],
        scale: [1.0; 3],
        inverse_bind: identity,
    };
    let vertex = |x: f32| SkinnedVertex {
        vertex: Vertex {
            position: [x, 1.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            uv: [x, 0.5],
            color: [255; 4],
        },
        joints: [1, 0, 0, 0],
        weights: [200, 55, 0, 0],
    };
    Character {
        name: "player/test".into(),
        joints: vec![joint(-1), joint(0)],
        primitives: vec![SkinnedPrimitive {
            material: 0,
            vertices: vec![vertex(0.0), vertex(1.0), vertex(0.5)],
            indices: vec![0, 1, 2],
        }],
        clips: vec![Clip {
            name: "walk".into(),
            duration: 1.0,
            distance: 1.5,
            tracks: vec![Track {
                joint: 1,
                translation: vec![],
                rotation: vec![
                    (0.0, [0.0, 0.0, 0.0, 1.0]),
                    (1.0, [0.0, 0.7071, 0.0, 0.7071]),
                ],
                scale: vec![(0.0, [1.0; 3])],
            }],
        }],
    }
}

#[test]
fn a_character_round_trips_and_is_bounded() {
    let mut contents = tiny_contents();
    contents.character = Some(tiny_character());
    let bytes = format::encode(&contents, &Limits::EVERGLADE).unwrap();
    let decoded = format::decode_contents(&bytes, &Limits::EVERGLADE).unwrap();
    // Everything but the quantized vertices round-trips exactly, and they
    // within half a step.
    let (got, sent) = (
        decoded.character.as_ref().unwrap(),
        contents.character.as_ref().unwrap(),
    );
    assert_eq!((&got.joints, &got.clips), (&sent.joints, &sent.clips));
    for (a, b) in got.primitives[0]
        .vertices
        .iter()
        .zip(&sent.primitives[0].vertices)
    {
        assert_eq!((a.joints, a.weights), (b.joints, b.weights));
        assert!((a.vertex.position[0] - b.vertex.position[0]).abs() < 1e-5);
        assert!((a.vertex.uv[0] - b.vertex.uv[0]).abs() < 1e-5);
    }
    assert_eq!(format::encode(&decoded, &Limits::EVERGLADE).unwrap(), bytes);
    assert!(format::decode_contents(&bytes[..bytes.len() - 1], &Limits::EVERGLADE).is_err());

    // A character's triangles count against its own budget.
    let tight = Limits {
        character_triangles: 0,
        ..Limits::EVERGLADE
    };
    assert!(format::encode(&contents, &tight).is_err());
    assert!(format::decode_contents(&bytes, &tight).is_err());

    let refused = |change: &dyn Fn(&mut format::Character)| {
        let mut broken = contents.clone();
        change(broken.character.as_mut().unwrap());
        format::encode(&broken, &Limits::EVERGLADE).is_err()
    };
    // A parent after its child, an influence past the skeleton, a vertex
    // with no weight, a colored vertex, a key past its clip, a repeated
    // clip, a nonfinite pose, and a missing material are refused.
    assert!(refused(&|c| c.joints[0].parent = 1));
    assert!(refused(&|c| c.primitives[0].vertices[0].joints[0] = 2));
    assert!(refused(&|c| c.primitives[0].vertices[0].weights = [0; 4]));
    assert!(refused(
        &|c| c.primitives[0].vertices[0].vertex.color = [0; 4]
    ));
    assert!(refused(&|c| c.clips[0].tracks[0].rotation[1].0 = 2.0));
    assert!(refused(&|c| c.clips.push(c.clips[0].clone())));
    assert!(refused(&|c| c.joints[1].translation[0] = f32::NAN));
    assert!(refused(&|c| c.primitives[0].material = 99));
}
