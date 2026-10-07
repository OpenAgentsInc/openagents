//! The licensed medieval kit compiled into a kit pack
//! (`docs/verse/everglade-medieval-refactor.md`).
//!
//! The input is the build directory `scripts/unreal/medieval_kit_build.py`
//! writes outside the repository: one `<id>.gltf` and `<id>.bin` per kit
//! piece, a `<id>.far.gltf` far level where a piece has one, and the
//! base-color PNGs they share. Every static model compiles through the same
//! importer as the Everglade pack's into a pack of models named `kit/<id>`
//! and far levels named `lod/kit.<id>`, under [`Limits::KIT`]. No source
//! manifest is read: the files are our own build of a licensed kit, and the
//! pack's digest, pinned in `everglade_pack::kit`, is what a reader checks.

use std::collections::BTreeMap;
use std::path::Path;

use sha2::{Digest, Sha256};

use super::{
    Builder, Character, Compiled, Contents, Limits, Model, SourceSet, format, is_admissible_name,
    read_regular,
};

/// The set name a kit pack's models and images take.
pub const SET: &str = "kit";
/// The suffix of a far level's file stem: `fountain.far.gltf` is the far
/// level of `kit/fountain`.
pub const FAR_SUFFIX: &str = ".far";
/// The prefix of a far level's model name, after `zones::everglade::detail`.
pub const FAR_PREFIX: &str = "lod/kit.";
const MAX_FILES: usize = 1024;
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
/// Files a build writes beside its models that the compiler ignores.
const REPORTS: [&str; 1] = ["build.json"];

/// The far level's model name for a kit model, such as `lod/kit.fountain`
/// for `kit/fountain`.
#[must_use]
pub fn far_name(model: &str) -> String {
    format!("{FAR_PREFIX}{}", model.trim_start_matches("kit/"))
}

/// Whether `name` is a kit model or a kit far level.
#[must_use]
pub fn is_kit_model(name: &str) -> bool {
    name.strip_prefix("kit/")
        .or_else(|| name.strip_prefix(FAR_PREFIX))
        .is_some_and(|id| {
            !id.is_empty()
                && id
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

/// Compiles the build in `directory` into a kit pack. `grade` maps each
/// texture's sRGB pixels before they are stored, so the kit's base colors
/// meet Everglade's painted look (`everglade_pack::kit::grade`).
///
/// # Errors
///
/// Returns a message when a file is unreadable or too large, a model fails
/// the importer, a model name is not a kit name, or the pack exceeds
/// [`Limits::KIT`].
pub fn compile(directory: &Path, grade: fn(&mut [u8])) -> Result<Compiled, String> {
    let limits = &Limits::KIT;
    let mut files = BTreeMap::new();
    let mut source_bytes = 0;
    let entries = std::fs::read_dir(directory)
        .map_err(|_| format!("Kit build is missing: {}", directory.display()))?;
    for (count, entry) in entries.enumerate() {
        if count >= MAX_FILES {
            return Err("Kit build has too many files".into());
        }
        let entry = entry.map_err(|_| "Kit build could not be listed")?;
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or("Kit build has a file name that is not UTF-8")?
            .to_owned();
        if name == ".DS_Store" || REPORTS.contains(&name.as_str()) {
            continue;
        }
        if !is_admissible_name(&name) || name == "LICENSE.txt" {
            return Err(format!("Kit build has an unexpected file: {name}"));
        }
        let bytes = read_regular(&entry.path(), MAX_FILE_BYTES)?;
        source_bytes += bytes.len() as u64;
        files.insert(name, bytes);
    }
    let set = [SourceSet {
        name: SET.to_owned(),
        files,
    }];
    let mut builder = Builder {
        limits,
        sets: &set,
        contents: Contents::default(),
        textures: BTreeMap::new(),
    };
    let mut models = Vec::new();
    for file in set[0].files.keys().filter(|name| name.ends_with(".gltf")) {
        let mut model = builder.model(&set[0], file)?;
        if let Some(id) = model.name.strip_suffix(FAR_SUFFIX) {
            model.name = far_name(id);
        }
        if !is_kit_model(&model.name) {
            return Err(format!("Kit build has a model with a non-kit name: {file}"));
        }
        models.push(model);
    }
    models.sort_by(|a, b| a.name.cmp(&b.name));
    builder.contents.models = models;
    let mut contents = builder.contents;
    for texture in &mut contents.textures {
        let decoded = format::decode_texture(texture)?;
        let mut rgba = decoded.rgba;
        grade(&mut rgba);
        texture.png = super::encode_png(decoded.width, decoded.height, &rgba)?;
    }
    let bytes = format::encode(&contents, limits)?;
    let decoded = decode(&bytes)?;
    Ok(Compiled {
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        models: decoded.models.len(),
        triangles: decoded.models.iter().map(Model::triangles).sum::<u64>()
            + decoded.forms.iter().map(Character::triangles).sum::<u64>(),
        decoded_texture_bytes: decoded.decoded_texture_bytes(),
        source_bytes,
        bytes,
    })
}

/// Decodes a kit pack and checks it holds only kit models.
///
/// # Errors
///
/// Returns a message when the pack fails its bounds, carries a character or
/// forms, or names a model outside the kit.
pub fn decode(bytes: &[u8]) -> Result<format::ZonePack, String> {
    let pack = format::decode(bytes, &Limits::KIT)?;
    if pack.character.is_some() || !pack.forms.is_empty() {
        return Err("A kit pack holds no characters".into());
    }
    if let Some(model) = pack.models.iter().find(|m| !is_kit_model(&m.name)) {
        return Err(format!(
            "Kit pack has a model outside the kit: {}",
            model.name
        ));
    }
    Ok(pack)
}

/// A tiny stand-in kit pack for tests: each of `ids` a 1 m box on one
/// one-pixel image, and a far level of the first. No licensed content.
#[doc(hidden)]
#[must_use]
pub fn sample(ids: &[&str]) -> Vec<u8> {
    use super::super::format::{AlphaMode, EncodedTexture, Material, Primitive, Vertex};
    let cube = |name: String, size: f32| {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        for (normal, u, v) in [
            ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
            ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
            ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
            ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
        ] {
            let base = vertices.len() as u32;
            for (a, b) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                // A cube standing on y = 0, centered on x and z; u x v is
                // the face's normal, so each face winds counter-clockwise.
                let lift = [0.0, 0.5, 0.0];
                let at = |i: usize| {
                    size * (lift[i] + 0.5 * normal[i] + (a - 0.5) * u[i] + (b - 0.5) * v[i])
                };
                vertices.push(Vertex {
                    position: [at(0), at(1), at(2)],
                    normal,
                    uv: [a, b],
                    color: [255; 4],
                });
            }
            indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        Model {
            name,
            primitives: vec![Primitive {
                material: 0,
                vertices,
                indices,
            }],
        }
    };
    let mut models: Vec<Model> = ids
        .iter()
        .map(|id| cube(format!("kit/{id}"), 1.0))
        .collect();
    if let Some(first) = ids.first() {
        models.push(cube(far_name(&format!("kit/{first}")), 1.0));
    }
    models.sort_by(|a, b| a.name.cmp(&b.name));
    let png = super::encode_png(1, 1, &[180, 140, 100, 255]).expect("one pixel encodes");
    let contents = Contents {
        textures: vec![EncodedTexture {
            name: "kit/T_sample".into(),
            width: 1,
            height: 1,
            png,
        }],
        materials: vec![Material {
            name: "kit/T_sample".into(),
            texture: Some(0),
            base_color: [1.0; 4],
            alpha: AlphaMode::Opaque,
            double_sided: false,
        }],
        models,
        character: None,
        forms: Vec::new(),
    };
    format::encode(&contents, &Limits::KIT).expect("the sample kit pack encodes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sample_kit_pack_decodes_under_the_kit_limits() {
        let bytes = sample(&["wall-4", "fountain"]);
        let pack = decode(&bytes).unwrap();
        let names: Vec<&str> = pack.models.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["kit/fountain", "kit/wall-4", "lod/kit.wall-4"]);
        assert!(pack.character.is_none() && pack.forms.is_empty());
    }

    #[test]
    fn a_kit_pack_holds_only_kit_names() {
        let mut contents = format::decode_contents(&sample(&["wall-4"]), &Limits::KIT).unwrap();
        contents.models[0].name = "village/Wall".into();
        contents.models.sort_by(|a, b| a.name.cmp(&b.name));
        let bytes = format::encode(&contents, &Limits::KIT).unwrap();
        let error = decode(&bytes).unwrap_err();
        assert!(error.contains("outside the kit"), "{error}");
        assert!(is_kit_model("kit/roof-end-mirror"));
        assert!(is_kit_model("lod/kit.fountain"));
        assert!(!is_kit_model("kit/Wall_A"));
        assert!(!is_kit_model("lod/village.Wall"));
    }

    #[test]
    fn the_kit_limits_refuse_a_larger_texture() {
        let mut contents = format::decode_contents(&sample(&["wall-4"]), &Limits::KIT).unwrap();
        contents.textures[0].width = Limits::KIT.texture_edge + 1;
        assert!(format::encode(&contents, &Limits::KIT).is_err());
    }

    #[test]
    fn compiling_a_build_grades_its_images_and_names_far_levels() {
        use crate::zones::everglade_pack::tests::{card, leaf_png};
        let dir = tempfile::tempdir().unwrap();
        let (gltf, bin) = card();
        std::fs::write(dir.path().join("wall-4.gltf"), &gltf).unwrap();
        std::fs::write(dir.path().join("wall-4.far.gltf"), &gltf).unwrap();
        std::fs::write(dir.path().join("Card.bin"), &bin).unwrap();
        std::fs::write(dir.path().join("Leaf.png"), leaf_png()).unwrap();
        std::fs::write(dir.path().join("build.json"), "{}").unwrap();
        fn redden(rgba: &mut [u8]) {
            for p in rgba.chunks_exact_mut(4) {
                p[0] = 200;
            }
        }
        let compiled = compile(dir.path(), redden).unwrap();
        let pack = decode(&compiled.bytes).unwrap();
        let names: Vec<&str> = pack.models.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["kit/wall-4", "lod/kit.wall-4"]);
        assert_eq!(pack.textures[0].name, "kit/Leaf");
        assert!(pack.textures[0].rgba.chunks_exact(4).all(|p| p[0] == 200));
        // The same build compiles to the same bytes.
        assert_eq!(compile(dir.path(), redden).unwrap().sha256, compiled.sha256);
        // A model whose file is not a kit ID is refused.
        std::fs::write(dir.path().join("Wall_A.gltf"), &gltf).unwrap();
        assert!(compile(dir.path(), |_| ()).is_err());
    }
}
