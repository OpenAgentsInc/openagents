//! A licensed character compiled into a private pack
//! (`docs/verse/private-assets.md`).
//!
//! The input is the build directory `scripts/blender/private_character.py`
//! writes, outside the repository: `guest.gltf` and `guest_far.gltf`, each
//! with its `.bin` buffer and one base-color PNG. Both levels compile through
//! the forms importer into a pack that holds only forms, [`NEAR`] and
//! [`FAR`], under [`Limits::PRIVATE`]. No source manifest is read: the files
//! are our own build of a licensed asset, and the pack's digest, recorded in
//! the asset's private manifest, is what a reader checks.

use std::collections::BTreeMap;
use std::path::Path;

use sha2::{Digest, Sha256};

use super::{
    Builder, Character, Compiled, Contents, Limits, Model, SourceSet, format, read_regular,
};

/// The set name a private pack's forms and images take.
pub const SET: &str = "private";
/// The near level's form.
pub const NEAR: &str = "private/guest";
/// The far level's form.
pub const FAR: &str = "private/guest_far";
/// The build files the compiler reads, and nothing else.
pub const FILES: [&str; 6] = [
    "guest.gltf",
    "guest.bin",
    "guest.png",
    "guest_far.gltf",
    "guest_far.bin",
    "guest_far.png",
];
const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;

/// Compiles the build in `directory` into a private pack.
///
/// # Errors
///
/// Returns a message when a file is missing or too large, a level fails the
/// forms importer, or the pack exceeds [`Limits::PRIVATE`].
pub fn compile(directory: &Path) -> Result<Compiled, String> {
    let limits = &Limits::PRIVATE;
    let mut files = BTreeMap::new();
    let mut source_bytes = 0;
    for name in FILES {
        let bytes = read_regular(&directory.join(name), MAX_FILE_BYTES)?;
        source_bytes += bytes.len() as u64;
        files.insert(name.to_owned(), bytes);
    }
    let sets = [SourceSet {
        name: SET.to_owned(),
        files,
    }];
    let mut builder = Builder {
        limits,
        sets: &sets,
        contents: Contents::default(),
        textures: BTreeMap::new(),
    };
    let near = builder.form(&sets[0], "guest.gltf")?;
    let far = builder.form(&sets[0], "guest_far.gltf")?;
    if near.name != NEAR || far.name != FAR {
        return Err("A private pack's levels must be guest and guest_far".into());
    }
    if near.clip("idle").is_none() {
        return Err("A private character needs an idle clip".into());
    }
    builder.contents.forms = vec![near, far];
    let contents = builder.contents;
    let bytes = format::encode(&contents, limits)?;
    let decoded = format::decode(&bytes, limits)?;
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

/// Decodes a private pack and checks it holds the near form.
///
/// # Errors
///
/// Returns a message when the pack fails its bounds or lacks [`NEAR`].
pub fn decode(bytes: &[u8]) -> Result<format::ZonePack, String> {
    let pack = format::decode(bytes, &Limits::PRIVATE)?;
    if pack.form(NEAR).is_none() {
        return Err("Private pack has no guest".into());
    }
    Ok(pack)
}

/// A tiny stand-in private pack for tests: a two-joint stick figure, with a
/// far level of the same, a one-pixel image, and an idle clip. No licensed
/// content.
#[doc(hidden)]
#[must_use]
pub fn sample() -> Vec<u8> {
    use super::super::format::{
        AlphaMode, EncodedTexture, Joint, Material, SkinnedPrimitive, SkinnedVertex, Track, Vertex,
    };
    let joint = |parent: i16, y: f32| Joint {
        parent,
        translation: [0.0, y, 0.0],
        rotation: [0.0, 0.0, 0.0, 1.0],
        scale: [1.0; 3],
        inverse_bind: [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, -y, 0.0, 1.0,
        ],
    };
    let vertex = |x: f32, y: f32, j: u8| SkinnedVertex {
        vertex: Vertex {
            position: [x, y, 0.0],
            normal: [0.0, 0.0, 1.0],
            uv: [0.5, 0.5],
            color: [255; 4],
        },
        joints: [j, 0, 0, 0],
        weights: [255, 0, 0, 0],
    };
    let character = |name: &str| Character {
        name: name.into(),
        joints: vec![joint(-1, 0.0), joint(0, 1.0)],
        primitives: vec![SkinnedPrimitive {
            material: 0,
            vertices: vec![
                vertex(-0.2, 0.0, 0),
                vertex(0.2, 0.0, 0),
                vertex(0.0, 1.6, 1),
            ],
            indices: vec![0, 1, 2],
        }],
        clips: vec![super::super::format::Clip {
            name: "idle".into(),
            duration: 4.0,
            distance: 0.0,
            tracks: vec![Track {
                joint: 1,
                translation: Vec::new(),
                rotation: vec![(0.0, [0.0, 0.0, 0.0, 1.0]), (4.0, [0.0, 0.0, 0.0, 1.0])],
                scale: Vec::new(),
            }],
        }],
    };
    let png = super::encode_png(1, 1, &[200, 150, 120, 255]).expect("one pixel encodes");
    let contents = Contents {
        textures: vec![EncodedTexture {
            name: "private/guest".into(),
            width: 1,
            height: 1,
            png,
        }],
        materials: vec![Material {
            name: "private/guest".into(),
            texture: Some(0),
            base_color: [1.0; 4],
            alpha: AlphaMode::Opaque,
            double_sided: false,
        }],
        models: Vec::new(),
        character: None,
        forms: vec![character(NEAR), character(FAR)],
    };
    format::encode(&contents, &Limits::PRIVATE).expect("the sample pack encodes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_form_only_pack_decodes_under_the_private_limits_only() {
        let bytes = sample();
        let pack = decode(&bytes).unwrap();
        assert!(pack.models.is_empty());
        assert_eq!(pack.forms.len(), 2);
        assert!(pack.form(NEAR).unwrap().clip("idle").is_some());
        // Everglade's own limits refuse nothing here, but a pack without the
        // guest is no private pack.
        let mut contents = format::decode_contents(&bytes, &Limits::PRIVATE).unwrap();
        contents.forms.retain(|f| f.name == FAR);
        let without = format::encode(&contents, &Limits::PRIVATE).unwrap();
        assert!(decode(&without).is_err());
        // A pack with nothing in it is still refused.
        contents.forms.clear();
        assert!(format::encode(&contents, &Limits::PRIVATE).is_err());
    }

    #[test]
    fn compiling_needs_every_build_file() {
        let empty = tempfile::tempdir().unwrap();
        let error = compile(empty.path()).unwrap_err();
        assert!(error.contains("missing"), "{error}");
    }
}
