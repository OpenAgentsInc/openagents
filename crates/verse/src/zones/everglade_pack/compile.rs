//! The Everglade pack compiler: admitted Quaternius sources in, one pinned
//! `VTP2` pack out.
//!
//! The compiler reads each source set under `assets/verse/everglade/`, checks
//! every file against the set's `openagents.verse.source-manifest.v1`
//! manifest, imports every admitted glTF model with its node transforms
//! applied, and keeps only base-color textures. Normal, occlusion, roughness,
//! and emissive maps are ignored, so they are not admitted. Textures larger
//! than their edge are downscaled with an alpha-weighted box filter; the rest
//! are stored as their admitted bytes. The same sources always give the same
//! pack bytes.
//!
//! The player's character is the ritual chamber's: the Universal
//! [`PLAYER_APPEARANCE`] composed by `imported::characters` from the retained
//! character sources, which are checked against their own pinned manifest
//! (`assets/verse/characters/quaternius/manifest.json`) and counted under it
//! rather than under the Everglade sets. Only its base-color images, bounded
//! to [`PLAYER_TEXTURE_EDGE`], its skeleton, and the clips in
//! [`PLAYER_CLIPS`] are packed.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use glam::{Mat3, Mat4, Vec3};
use sha2::{Digest, Sha256};

use super::format::{
    self, AlphaMode, Character, Clip, Contents, EncodedTexture, Joint, Limits, Material, Model,
    Primitive, SkinnedPrimitive, SkinnedVertex, Track, Vertex,
};

/// The manifest schema every admitted set declares.
pub const SCHEMA: &str = "openagents.verse.source-manifest.v1";
/// The admitted source sets, in pack order.
pub const SETS: [&str; 3] = ["nature", "village", "props"];
/// The longest edge of a texture that covers small or distant geometry.
pub const SMALL_TEXTURE_EDGE: u32 = 512;
/// The longest edge of a texture that covers large or near surfaces.
pub const LARGE_TEXTURE_EDGE: u32 = 1024;
/// Textures that keep [`LARGE_TEXTURE_EDGE`]; every other texture gets
/// [`SMALL_TEXTURE_EDGE`].
pub const LARGE_TEXTURES: &[&str] = &[
    "nature/Bark_NormalTree.png",
    "nature/Leaves_NormalTree_C.png",
    "village/T_Plaster_BaseColor.png",
    "village/T_WoodTrim_BaseColor.png",
    "village/T_RoundTiles_BaseColor.png",
    "props/T_Trim_Furniture_BaseColor.png",
    "props/T_Trim_Props_BaseColor.png",
];
/// The retained Universal character sources, relative to the pack
/// directory.
pub const PLAYER_SOURCES: &str = "../characters/quaternius";
/// The appearance the ritual chamber's player wears by default
/// (`verse_play`).
pub const PLAYER_APPEARANCE: &str = "male-ranger";
/// The longest edge of the player's outfit image.
pub const PLAYER_TEXTURE_EDGE: u32 = 512;
/// The longest edge of the player's other base-color images that cover at
/// least [`PLAYER_DETAIL_TRIANGLES`]; the rest get half of it.
pub const PLAYER_DETAIL_EDGE: u32 = 256;
/// The triangles an image must cover to get [`PLAYER_DETAIL_EDGE`].
pub const PLAYER_DETAIL_TRIANGLES: usize = 1_000;
/// How far a dropped key may differ from the interpolation of its
/// neighbors, in meters, quaternion units, or scale.
const KEY_TOLERANCE: f32 = 1e-4;
/// The player's clips: the pack's name and the chamber's clip ID for idle,
/// walk, run, the airborne pose, backpedal, and the two strafes.
pub const PLAYER_CLIPS: [(&str, u16); 7] = [
    ("idle", 0),
    ("walk", 4),
    ("run", 5),
    ("jump", 37),
    ("backpedal", 13),
    ("strafe_left", 14),
    ("strafe_right", 15),
];
const CREATOR: &str = "Quaternius";
const LICENSE: &str = "CC0-1.0";
const LICENSE_FILE: &str = "license.txt";
const MANIFEST_FILE: &str = "manifest.json";
const README_FILE: &str = "README.md";
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_SOURCE_FILE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_SOURCE_IMAGE_EDGE: u32 = 4096;
const MAX_SET_ENTRIES: usize = 512;
const MAX_NODE_DEPTH: usize = 32;
const MAX_NODES: usize = 256;
const MAX_FILE_NAME_BYTES: usize = 96;

/// A parsed and checked source manifest.
#[derive(Clone, Debug, PartialEq)]
pub struct Manifest {
    /// The kit's name.
    pub package: String,
    /// SHA-256 of each admitted file, as committed.
    pub files: BTreeMap<String, String>,
    /// SHA-256 of each file as it shipped in the kit.
    pub originals: BTreeMap<String, String>,
    /// How each changed file was derived from the kit's file.
    pub transforms: BTreeMap<String, String>,
}

/// A compiled pack and what it holds.
#[derive(Clone, Debug, PartialEq)]
pub struct Compiled {
    /// The pack bytes.
    pub bytes: Vec<u8>,
    /// Lowercase hexadecimal SHA-256 of `bytes`.
    pub sha256: String,
    /// Models in the pack.
    pub models: usize,
    /// Triangles over every model, once each, and the character.
    pub triangles: u64,
    /// Decoded RGBA bytes over every texture.
    pub decoded_texture_bytes: u64,
    /// Bytes of every committed source file, including manifests and READMEs.
    pub source_bytes: u64,
}

fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn is_admissible_name(name: &str) -> bool {
    let extension_ok = name == LICENSE_FILE
        || [".gltf", ".bin", ".png"]
            .iter()
            .any(|extension| name.ends_with(extension));
    extension_ok
        && name.len() <= MAX_FILE_NAME_BYTES
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
}

/// True for images the current material path cannot read. They are not
/// admitted until a shader samples them.
fn is_unread_map(name: &str) -> bool {
    name.strip_suffix(".png").is_some_and(|stem| {
        ["_Normal", "_ORM", "_Roughness", "_Metallic", "_Emissive"]
            .iter()
            .any(|marker| stem.ends_with(marker))
    })
}

fn string_map(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<BTreeMap<String, String>, String> {
    let entries = object
        .get(key)
        .and_then(|v| v.as_object())
        .ok_or(format!("Source manifest is missing {key}"))?;
    let mut map = BTreeMap::new();
    for (name, value) in entries {
        let value = value
            .as_str()
            .ok_or(format!("Source manifest {key} has a non-string value"))?;
        map.insert(name.clone(), value.to_owned());
    }
    Ok(map)
}

/// Parses and checks a source manifest: schema, creator, license, package,
/// file names, both digests of every file, and each recorded transform.
pub fn parse_manifest(bytes: &[u8]) -> Result<Manifest, String> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| "Source manifest is not valid JSON")?;
    let object = value
        .as_object()
        .ok_or("Source manifest is not an object")?;
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "schema" | "creator" | "license" | "package" | "files" | "originals" | "transforms"
        ) {
            return Err(format!("Source manifest has an unknown field: {key}"));
        }
    }
    fn field<'a>(object: &'a serde_json::Map<String, serde_json::Value>, key: &str) -> &'a str {
        object.get(key).and_then(|v| v.as_str()).unwrap_or_default()
    }
    let text = |key| field(object, key);
    if text("schema") != SCHEMA {
        return Err("Source manifest has an unsupported schema".into());
    }
    if text("creator") != CREATOR || text("license") != LICENSE {
        return Err("Source manifest must declare Quaternius and CC0-1.0".into());
    }
    let package = text("package").to_owned();
    if package.trim().is_empty() {
        return Err("Source manifest must name its package".into());
    }
    let files = string_map(object, "files")?;
    let originals = string_map(object, "originals")?;
    let transforms = string_map(object, "transforms")?;
    if !files.contains_key(LICENSE_FILE) {
        return Err("Source manifest must admit the kit's license text".into());
    }
    if !files.keys().any(|name| name.ends_with(".gltf")) {
        return Err("Source manifest admits no models".into());
    }
    for (name, digest) in &files {
        if !is_admissible_name(name) {
            return Err(format!("Source manifest has an invalid file name: {name}"));
        }
        if is_unread_map(name) {
            return Err(format!(
                "Source manifest admits an unread texture map: {name}"
            ));
        }
        if !is_digest(digest) {
            return Err(format!("Source manifest has an invalid digest for {name}"));
        }
        let original = originals
            .get(name)
            .ok_or(format!("Source manifest has no original digest for {name}"))?;
        if !is_digest(original) {
            return Err(format!(
                "Source manifest has an invalid original digest for {name}"
            ));
        }
        match transforms.get(name) {
            Some(transform) => {
                if !name.ends_with(".png") || transform.trim().is_empty() || original == digest {
                    return Err(format!(
                        "Source manifest has an invalid transform for {name}"
                    ));
                }
            }
            None if original != digest => {
                return Err(format!(
                    "Source manifest changes {name} without a transform"
                ));
            }
            None => {}
        }
    }
    if originals.keys().any(|name| !files.contains_key(name))
        || transforms.keys().any(|name| !files.contains_key(name))
    {
        return Err("Source manifest describes a file it does not admit".into());
    }
    Ok(Manifest {
        package,
        files,
        originals,
        transforms,
    })
}

/// Reads a regular file that is not a symbolic link, within `max` bytes.
fn read_regular(path: &Path, max: u64) -> Result<Vec<u8>, String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|_| format!("Source file is missing: {}", path.display()))?;
    if !metadata.file_type().is_file() {
        return Err(format!("Source is not a regular file: {}", path.display()));
    }
    if metadata.len() > max {
        return Err(format!(
            "Source file exceeds its size limit: {}",
            path.display()
        ));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(max + 1).read_to_end(&mut bytes))
        .map_err(|_| format!("Source file could not be read: {}", path.display()))?;
    if bytes.len() as u64 > max {
        return Err(format!(
            "Source file exceeds its size limit: {}",
            path.display()
        ));
    }
    Ok(bytes)
}

/// One admitted set with every file verified against its manifest.
struct SourceSet {
    name: String,
    files: BTreeMap<String, Vec<u8>>,
}

/// Verifies one set and returns it with its committed size.
fn read_set(root: &Path, set: &str) -> Result<(SourceSet, u64), String> {
    let directory = root.join(set);
    let manifest_bytes = read_regular(&directory.join(MANIFEST_FILE), MAX_MANIFEST_BYTES)?;
    let manifest = parse_manifest(&manifest_bytes)
        .map_err(|error| format!("{set}/{MANIFEST_FILE}: {error}"))?;
    // Only admitted files, the manifest, and the README may be committed.
    let mut committed = 0u64;
    let entries = std::fs::read_dir(&directory)
        .map_err(|_| format!("Source set is missing: {}", directory.display()))?;
    for (count, entry) in entries.enumerate() {
        if count >= MAX_SET_ENTRIES {
            return Err(format!("Source set {set} has too many files"));
        }
        let entry = entry.map_err(|_| format!("Source set {set} could not be listed"))?;
        let name = entry.file_name();
        let name = name.to_str().ok_or(format!(
            "Source set {set} has a file name that is not UTF-8"
        ))?;
        if name == ".DS_Store" {
            continue;
        }
        if name != MANIFEST_FILE && name != README_FILE && !manifest.files.contains_key(name) {
            return Err(format!("Source set {set} has an unadmitted file: {name}"));
        }
        let metadata = std::fs::symlink_metadata(entry.path())
            .map_err(|_| format!("Source set {set} could not be listed"))?;
        if !metadata.file_type().is_file() {
            return Err(format!("Source set {set} has a non-regular entry: {name}"));
        }
        committed += metadata.len();
    }
    let mut files = BTreeMap::new();
    for (name, digest) in &manifest.files {
        let bytes = read_regular(&directory.join(name), MAX_SOURCE_FILE_BYTES)?;
        if format!("{:x}", Sha256::digest(&bytes)) != *digest {
            return Err(format!("Source digest mismatch: {set}/{name}"));
        }
        files.insert(name.clone(), bytes);
    }
    Ok((
        SourceSet {
            name: set.to_owned(),
            files,
        },
        committed,
    ))
}

/// The edge a texture is compiled to.
pub fn texture_edge(set: &str, file: &str, limits: &Limits) -> u32 {
    let path = format!("{set}/{file}");
    let wanted = if LARGE_TEXTURES.contains(&path.as_str()) {
        LARGE_TEXTURE_EDGE
    } else {
        SMALL_TEXTURE_EDGE
    };
    wanted.min(limits.texture_edge)
}

/// Downscales straight-alpha RGBA so the longer edge is `edge`. Each output
/// pixel averages its source box, weighting color by alpha so transparent
/// texels do not darken leaf and flower edges.
pub fn downscale(rgba: &[u8], width: u32, height: u32, edge: u32) -> (u32, u32, Vec<u8>) {
    let longest = width.max(height) as u64;
    let scaled =
        |n: u32| ((n as u64 * edge as u64 + longest / 2) / longest).clamp(1, edge as u64) as u32;
    let (out_width, out_height) = (scaled(width), scaled(height));
    let mut out = Vec::with_capacity(out_width as usize * out_height as usize * 4);
    let span = |i: u32, from: u32, to: u32| {
        let start = (i as u64 * from as u64 / to as u64) as u32;
        let end = (((i as u64 + 1) * from as u64 / to as u64) as u32).max(start + 1);
        start..end
    };
    for y in 0..out_height {
        let rows = span(y, height, out_height);
        for x in 0..out_width {
            let columns = span(x, width, out_width);
            let (mut count, mut alpha) = (0u64, 0u64);
            let mut weighted = [0u64; 3];
            let mut plain = [0u64; 3];
            for sy in rows.clone() {
                for sx in columns.clone() {
                    let at = (sy as usize * width as usize + sx as usize) * 4;
                    let p = &rgba[at..at + 4];
                    let a = p[3] as u64;
                    for c in 0..3 {
                        weighted[c] += p[c] as u64 * a;
                        plain[c] += p[c] as u64;
                    }
                    alpha += a;
                    count += 1;
                }
            }
            for c in 0..3 {
                out.push(if alpha > 0 {
                    ((weighted[c] + alpha / 2) / alpha) as u8
                } else {
                    ((plain[c] + count / 2) / count) as u8
                });
            }
            out.push(((alpha + count / 2) / count) as u8);
        }
    }
    (out_width, out_height, out)
}

fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::High);
    let mut writer = encoder
        .write_header()
        .map_err(|_| "Texture could not be encoded")?;
    writer
        .write_image_data(rgba)
        .map_err(|_| "Texture could not be encoded")?;
    writer
        .finish()
        .map_err(|_| "Texture could not be encoded")?;
    Ok(bytes)
}

/// Bounds a source PNG to `edge`. A PNG that already fits and is 8-bit RGB or
/// RGBA keeps its admitted bytes; any other is decoded, downscaled if needed,
/// and stored as 8-bit RGBA.
pub fn prepare_texture(name: String, bytes: &[u8], edge: u32) -> Result<EncodedTexture, String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    decoder.set_limits(png::Limits {
        bytes: (MAX_SOURCE_IMAGE_EDGE as usize).pow(2) * 4 + 1024 * 1024,
    });
    let mut reader = decoder
        .read_info()
        .map_err(|_| format!("Source texture is not a readable PNG: {name}"))?;
    let info = reader.info();
    let (width, height) = (info.width, info.height);
    let keeps_bytes = info.bit_depth == png::BitDepth::Eight
        && matches!(info.color_type, png::ColorType::Rgb | png::ColorType::Rgba)
        && !info.interlaced;
    if info.animation_control.is_some()
        || width == 0
        || height == 0
        || width > MAX_SOURCE_IMAGE_EDGE
        || height > MAX_SOURCE_IMAGE_EDGE
    {
        return Err(format!(
            "Source texture has an unsupported size or format: {name}"
        ));
    }
    if keeps_bytes && width <= edge && height <= edge {
        return Ok(EncodedTexture {
            name,
            width,
            height,
            png: bytes.to_vec(),
        });
    }
    let size = reader
        .output_buffer_size()
        .ok_or(format!("Source texture is too large: {name}"))?;
    let mut pixels = vec![0; size];
    let output = reader
        .next_frame(&mut pixels)
        .map_err(|_| format!("Source texture could not be decoded: {name}"))?;
    let pixels = &pixels[..output.buffer_size()];
    let (color, _) = reader.output_color_type();
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    match color {
        png::ColorType::Grayscale => pixels.iter().for_each(|&g| rgba.extend([g, g, g, 255])),
        png::ColorType::GrayscaleAlpha => pixels
            .chunks_exact(2)
            .for_each(|p| rgba.extend([p[0], p[0], p[0], p[1]])),
        png::ColorType::Rgb => pixels
            .chunks_exact(3)
            .for_each(|p| rgba.extend([p[0], p[1], p[2], 255])),
        png::ColorType::Rgba => rgba.extend_from_slice(pixels),
        png::ColorType::Indexed => {
            return Err(format!("Source texture was not expanded: {name}"));
        }
    }
    if rgba.len() != width as usize * height as usize * 4 {
        return Err(format!("Source texture is incomplete: {name}"));
    }
    let (width, height, rgba) = if width > edge || height > edge {
        downscale(&rgba, width, height, edge)
    } else {
        (width, height, rgba)
    };
    Ok(EncodedTexture {
        name,
        width,
        height,
        png: encode_png(width, height, &rgba)?,
    })
}

fn pack_name(raw: &str) -> String {
    raw.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Accumulates deduplicated textures and materials while models import.
struct Builder<'l> {
    limits: &'l Limits,
    contents: Contents,
    textures: BTreeMap<(String, String), u16>,
}

impl Builder<'_> {
    fn texture(&mut self, set: &SourceSet, uri: &str) -> Result<u16, String> {
        let key = (set.name.clone(), uri.to_owned());
        if let Some(&index) = self.textures.get(&key) {
            return Ok(index);
        }
        if !uri.ends_with(".png") || is_unread_map(uri) {
            return Err(format!(
                "Base-color image is not an admitted PNG: {}/{uri}",
                set.name
            ));
        }
        let bytes = set.files.get(uri).ok_or(format!(
            "Base-color image is not admitted: {}/{uri}",
            set.name
        ))?;
        let name = format!("{}/{}", set.name, pack_name(uri.trim_end_matches(".png")));
        let edge = texture_edge(&set.name, uri, self.limits);
        let texture = prepare_texture(name, bytes, edge)?;
        let index = u16::try_from(self.contents.textures.len()).map_err(|_| "Too many textures")?;
        self.contents.textures.push(texture);
        self.textures.insert(key, index);
        Ok(index)
    }

    fn material(&mut self, set: &SourceSet, material: gltf::Material<'_>) -> Result<u16, String> {
        let source_name = material
            .name()
            .ok_or(format!("Source material has no name in set {}", set.name))?;
        let pbr = material.pbr_metallic_roughness();
        let texture = match pbr.base_color_texture() {
            Some(info) => {
                if info.tex_coord() != 0 {
                    return Err(format!(
                        "Source material {source_name} uses a second texture coordinate set"
                    ));
                }
                match info.texture().source().source() {
                    gltf::image::Source::Uri { uri, .. } => Some(self.texture(set, uri)?),
                    gltf::image::Source::View { .. } => {
                        return Err(format!(
                            "Source material {source_name} embeds its image in a buffer"
                        ));
                    }
                }
            }
            None => None,
        };
        let alpha = match material.alpha_mode() {
            gltf::material::AlphaMode::Opaque => AlphaMode::Opaque,
            gltf::material::AlphaMode::Mask => AlphaMode::Mask {
                cutoff: material.alpha_cutoff().unwrap_or(0.5),
            },
            gltf::material::AlphaMode::Blend => AlphaMode::Blend,
        };
        let candidate = Material {
            name: format!("{}/{}", set.name, pack_name(source_name)),
            texture,
            base_color: pbr.base_color_factor(),
            alpha,
            double_sided: material.double_sided(),
        };
        // Reuse an identical material; give a same-named variant its own name.
        let same = |m: &Material| {
            m.texture == candidate.texture
                && m.base_color == candidate.base_color
                && m.alpha == candidate.alpha
                && m.double_sided == candidate.double_sided
        };
        let mut variant = 1;
        loop {
            let name = if variant == 1 {
                candidate.name.clone()
            } else {
                format!("{}~{variant}", candidate.name)
            };
            match self.contents.materials.iter().position(|m| m.name == name) {
                Some(index) if same(&self.contents.materials[index]) => return Ok(index as u16),
                Some(_) => variant += 1,
                None => {
                    let index = u16::try_from(self.contents.materials.len())
                        .map_err(|_| "Too many materials")?;
                    self.contents.materials.push(Material {
                        name,
                        ..candidate.clone()
                    });
                    return Ok(index);
                }
            }
        }
    }

    /// Composes the chamber's player from the verified character sources and
    /// converts it from the chamber's basis back to the sources' space.
    fn player(&mut self, sources: &Path) -> Result<Character, String> {
        crate::imported::inventory::verify_characters(sources)?;
        // The importer writes its bounded images beside its pack; they are
        // read back, bounded again, and removed.
        let scratch =
            std::env::temp_dir().join(format!("verse-everglade-player-{}", std::process::id()));
        std::fs::create_dir_all(&scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
        let result = self.player_in(sources, &scratch);
        let _ = std::fs::remove_dir_all(&scratch);
        result
    }

    fn player_in(&mut self, sources: &Path, scratch: &Path) -> Result<Character, String> {
        use crate::imported::characters;
        let mut engine = verse_engine::assets::Pack {
            inventory: None,
            version: 1,
            source_revision: String::new(),
            models: BTreeMap::new(),
            textures: Vec::new(),
            placements: Vec::new(),
        };
        let model = characters::appearance(&mut engine, scratch, sources, PLAYER_APPEARANCE)?;
        let skin = model.skin.as_ref().ok_or("The player has no skin")?;
        let inverse = Mat4::from_cols_array(&skin.basis).inverse();
        if model.bones.len() > format::MAX_JOINTS {
            return Err("The player has too many joints".into());
        }
        let joints = model
            .bones
            .iter()
            .enumerate()
            .map(|(i, bone)| Joint {
                parent: bone.parent,
                translation: skin.rest[i].translation,
                rotation: skin.rest[i].rotation,
                scale: skin.rest[i].scale,
                inverse_bind: skin.inverse_bind[i],
            })
            .collect();
        // The image that covers the most triangles, the outfit, keeps the
        // largest edge; the head and skin are small on screen, and images
        // under few triangles, such as the eyes and brows, smaller still.
        let mut covered: BTreeMap<usize, usize> = BTreeMap::new();
        for surface in &model.surfaces {
            *covered.entry(surface.texture).or_default() += surface.indices.len() / 3;
        }
        let outfit = covered.iter().max_by_key(|c| *c.1).map(|c| *c.0);
        // One primitive per material, in first-use order.
        let mut primitives: Vec<SkinnedPrimitive> = Vec::new();
        for surface in model.surfaces.iter().filter(|s| !s.indices.is_empty()) {
            let edge = if Some(surface.texture) == outfit {
                PLAYER_TEXTURE_EDGE
            } else if covered[&surface.texture] >= PLAYER_DETAIL_TRIANGLES {
                PLAYER_DETAIL_EDGE
            } else {
                PLAYER_DETAIL_EDGE / 2
            };
            let material = self.player_material(&engine, scratch, surface, edge)?;
            let vertices: Vec<SkinnedVertex> = surface
                .vertices
                .iter()
                .map(|v| SkinnedVertex {
                    vertex: Vertex {
                        position: inverse.transform_point3(v.position.into()).to_array(),
                        normal: inverse
                            .transform_vector3(v.normal.into())
                            .normalize_or_zero()
                            .to_array(),
                        uv: v.uv,
                        color: [255; 4],
                    },
                    joints: v.joints.map(|j| j as u8),
                    weights: unit_weights(v.weights),
                })
                .collect();
            let limit = u16::MAX as usize + 1;
            if let Some(existing) = primitives
                .iter_mut()
                .find(|p| p.material == material && p.vertices.len() + vertices.len() <= limit)
            {
                let base = existing.vertices.len() as u32;
                existing.vertices.extend(vertices);
                existing
                    .indices
                    .extend(surface.indices.iter().map(|i| i + base));
            } else if vertices.len() <= limit {
                primitives.push(SkinnedPrimitive {
                    material,
                    vertices,
                    indices: surface.indices.clone(),
                });
            } else {
                return Err("A player surface exceeds 65,536 vertices".into());
            }
        }
        let mut clips = Vec::new();
        for (name, id) in PLAYER_CLIPS {
            let clip = model
                .clips
                .iter()
                .find(|c| c.id == id)
                .ok_or(format!("The player has no {name} clip"))?;
            clips.push(Clip {
                name: name.into(),
                duration: clip.duration,
                distance: characters::loop_distance(id),
                tracks: clip
                    .bones
                    .iter()
                    .map(|keys| Track {
                        joint: keys.bone as u16,
                        translation: reduce_keys(&keys.translation),
                        rotation: reduce_keys(&keys.rotation),
                        scale: reduce_keys(&keys.scale),
                    })
                    .collect(),
            });
        }
        for primitive in &mut primitives {
            weld(primitive);
        }
        Ok(Character {
            name: format!("player/{PLAYER_APPEARANCE}"),
            joints,
            primitives,
            clips,
        })
    }

    /// The pack material for one of the player's surfaces: its base-color
    /// image, tint and opacity, and alpha mode. The sources' materials are
    /// all double-sided.
    fn player_material(
        &mut self,
        engine: &verse_engine::assets::Pack,
        scratch: &Path,
        surface: &verse_engine::assets::Surface,
        edge: u32,
    ) -> Result<u16, String> {
        let image = engine
            .textures
            .get(surface.texture)
            .ok_or("A player surface names a missing image")?;
        let key = ("player".to_owned(), image.file.clone());
        let texture = match self.textures.get(&key) {
            Some(&index) => index,
            None => {
                let bytes = read_regular(&scratch.join(&image.file), MAX_SOURCE_FILE_BYTES)?;
                let digest = image
                    .file
                    .trim_start_matches("universal-source-")
                    .get(..16)
                    .ok_or("A player image has an unexpected name")?;
                let edge = edge.min(self.limits.texture_edge);
                let texture = prepare_texture(format!("player/{digest}"), &bytes, edge)?;
                let index =
                    u16::try_from(self.contents.textures.len()).map_err(|_| "Too many textures")?;
                self.contents.textures.push(texture);
                self.textures.insert(key, index);
                index
            }
        };
        let alpha = match surface.blend {
            0 => AlphaMode::Opaque,
            1 => AlphaMode::Mask {
                cutoff: surface.material.alpha_cutoff,
            },
            2 => AlphaMode::Blend,
            _ => return Err("A player surface is additive".into()),
        };
        let candidate = Material {
            name: format!("player/{}", self.contents.materials.len()),
            texture: Some(texture),
            base_color: [
                surface.tint[0],
                surface.tint[1],
                surface.tint[2],
                surface.material.opacity,
            ],
            alpha,
            double_sided: true,
        };
        if let Some(index) = self.contents.materials.iter().position(|m| {
            m.name.starts_with("player/")
                && m.texture == candidate.texture
                && m.base_color == candidate.base_color
                && m.alpha == candidate.alpha
        }) {
            return Ok(index as u16);
        }
        let index =
            u16::try_from(self.contents.materials.len()).map_err(|_| "Too many materials")?;
        self.contents.materials.push(candidate);
        Ok(index)
    }

    fn model(&mut self, set: &SourceSet, file: &str) -> Result<Model, String> {
        let label = format!("{}/{file}", set.name);
        let gltf = gltf::Gltf::from_slice(&set.files[file])
            .map_err(|error| format!("{label}: {error}"))?;
        let document = &gltf.document;
        if document.extensions_used().next().is_some()
            || document.extensions_required().next().is_some()
            || document.skins().next().is_some()
            || document.animations().next().is_some()
            || document.nodes().count() > MAX_NODES
        {
            return Err(format!(
                "{label}: models must be static, without extensions, and within the node limit"
            ));
        }
        let mut buffers = Vec::new();
        for buffer in document.buffers() {
            let gltf::buffer::Source::Uri(uri) = buffer.source() else {
                return Err(format!("{label}: binary glTF buffers are not admitted"));
            };
            let bytes = set
                .files
                .get(uri)
                .filter(|_| uri.ends_with(".bin"))
                .ok_or(format!("{label}: buffer {uri} is not admitted"))?;
            if bytes.len() < buffer.length() {
                return Err(format!("{label}: buffer {uri} is shorter than declared"));
            }
            buffers.push(bytes.as_slice());
        }
        let scene = document
            .default_scene()
            .or_else(|| document.scenes().next())
            .ok_or(format!("{label}: no scene"))?;
        let mut primitives: Vec<Primitive> = Vec::new();
        let mut stack: Vec<(gltf::Node<'_>, Mat4, usize)> = scene
            .nodes()
            .map(|node| (node, Mat4::IDENTITY, 0))
            .collect();
        while let Some((node, parent, depth)) = stack.pop() {
            if depth > MAX_NODE_DEPTH {
                return Err(format!("{label}: node hierarchy is too deep"));
            }
            let world = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
            for child in node.children() {
                stack.push((child, world, depth + 1));
            }
            let Some(mesh) = node.mesh() else { continue };
            if node.skin().is_some() || mesh.weights().is_some() {
                return Err(format!(
                    "{label}: skinned or morphed meshes are not admitted"
                ));
            }
            for primitive in mesh.primitives() {
                self.primitive(set, &label, &buffers, world, primitive, &mut primitives)?;
            }
        }
        if primitives.is_empty() {
            return Err(format!("{label}: model has no triangles"));
        }
        Ok(Model {
            name: format!("{}/{}", set.name, pack_name(file.trim_end_matches(".gltf"))),
            primitives,
        })
    }

    fn primitive(
        &mut self,
        set: &SourceSet,
        label: &str,
        buffers: &[&[u8]],
        world: Mat4,
        primitive: gltf::Primitive<'_>,
        out: &mut Vec<Primitive>,
    ) -> Result<(), String> {
        if primitive.mode() != gltf::mesh::Mode::Triangles
            || primitive.morph_targets().next().is_some()
        {
            return Err(format!("{label}: only static triangle lists are admitted"));
        }
        let material = primitive.material();
        if material.index().is_none() {
            return Err(format!("{label}: a primitive has no material"));
        }
        let material = self.material(set, material)?;
        let reader = primitive.reader(|buffer| buffers.get(buffer.index()).copied());
        let positions: Vec<[f32; 3]> = reader
            .read_positions()
            .ok_or(format!("{label}: a primitive has no positions"))?
            .collect();
        let normals: Vec<[f32; 3]> = reader
            .read_normals()
            .ok_or(format!("{label}: a primitive has no normals"))?
            .collect();
        let uvs: Vec<[f32; 2]> = match reader.read_tex_coords(0) {
            Some(coords) => coords.into_f32().collect(),
            None if self.contents.materials[material as usize].texture.is_none() => {
                vec![[0.0; 2]; positions.len()]
            }
            None => return Err(format!("{label}: a textured primitive has no coordinates")),
        };
        let colors: Vec<[u8; 4]> = match reader.read_colors(0) {
            Some(colors) => colors.into_rgba_u8().collect(),
            None => vec![[255; 4]; positions.len()],
        };
        let count = positions.len();
        if count == 0 || normals.len() != count || uvs.len() != count || colors.len() != count {
            return Err(format!("{label}: a primitive has mismatched attributes"));
        }
        let mut indices: Vec<u32> = match reader.read_indices() {
            Some(indices) => indices.into_u32().collect(),
            None => (0..count as u32).collect(),
        };
        if indices.is_empty()
            || !indices.len().is_multiple_of(3)
            || indices.iter().any(|&i| i as usize >= count)
        {
            return Err(format!("{label}: a primitive has invalid indices"));
        }
        let normal_matrix = Mat3::from_mat4(world).inverse().transpose();
        if world.determinant() < 0.0 {
            for triangle in indices.chunks_exact_mut(3) {
                triangle.swap(1, 2);
            }
        }
        let vertices: Vec<Vertex> = (0..count)
            .map(|i| {
                let normal = (normal_matrix * Vec3::from(normals[i])).normalize_or_zero();
                Vertex {
                    position: world.transform_point3(positions[i].into()).to_array(),
                    normal: normal.to_array(),
                    uv: uvs[i],
                    color: colors[i],
                }
            })
            .collect();
        // Merge into this model's primitive for the same material while the
        // 16-bit index range allows it.
        let limit = u16::MAX as usize + 1;
        if let Some(existing) = out
            .iter_mut()
            .find(|p| p.material == material && p.vertices.len() + count <= limit)
        {
            let base = existing.vertices.len() as u32;
            existing.vertices.extend(vertices);
            existing.indices.extend(indices.iter().map(|i| i + base));
        } else if count <= limit {
            out.push(Primitive {
                material,
                vertices,
                indices,
            });
        } else {
            return Err(format!("{label}: a primitive exceeds 65,536 vertices"));
        }
        Ok(())
    }
}

/// Drops each key that linear interpolation between the kept keys around it
/// reproduces within [`KEY_TOLERANCE`], keeping the first and last keys.
/// A channel that never changes keeps one key.
fn reduce_keys<const N: usize>(keys: &[(f32, [f32; N])]) -> Vec<(f32, [f32; N])> {
    let Some((&first, rest)) = keys.split_first() else {
        return Vec::new();
    };
    if rest.iter().all(|k| {
        k.1.iter()
            .zip(first.1)
            .all(|(a, b)| (a - b).abs() <= KEY_TOLERANCE)
    }) {
        return vec![first];
    }
    let mut kept = vec![first];
    for i in 1..keys.len() {
        let Some(&next) = keys.get(i + 1) else {
            kept.push(keys[i]);
            break;
        };
        let previous = *kept.last().unwrap_or(&first);
        let span = next.0 - previous.0;
        let t = if span > 0.0 {
            (keys[i].0 - previous.0) / span
        } else {
            0.0
        };
        let redundant = (0..N).all(|c| {
            let between = previous.1[c] + (next.1[c] - previous.1[c]) * t;
            (between - keys[i].1[c]).abs() <= KEY_TOLERANCE
        });
        if !redundant {
            kept.push(keys[i]);
        }
    }
    kept
}

/// Merges identical vertices of one primitive and renumbers its indices.
fn weld(primitive: &mut SkinnedPrimitive) {
    type Key = ([u32; 3], [u8; 3], [u32; 2], [u8; 4], [u8; 4]);
    let key = |v: &SkinnedVertex| -> Key {
        (
            v.vertex.position.map(f32::to_bits),
            v.vertex.normal.map(|n| format::snorm8(n) as u8),
            v.vertex.uv.map(f32::to_bits),
            v.joints,
            v.weights,
        )
    };
    let mut seen: BTreeMap<Key, u32> = BTreeMap::new();
    let mut vertices = Vec::new();
    let mut remap = Vec::with_capacity(primitive.vertices.len());
    for vertex in &primitive.vertices {
        let index = *seen.entry(key(vertex)).or_insert_with(|| {
            vertices.push(*vertex);
            vertices.len() as u32 - 1
        });
        remap.push(index);
    }
    for index in &mut primitive.indices {
        *index = remap[*index as usize];
    }
    primitive.vertices = vertices;
}

/// Quantizes four skin weights to 255ths that sum to 255, giving any
/// rounding remainder to the heaviest.
fn unit_weights(weights: [f32; 4]) -> [u8; 4] {
    let clean = weights.map(|w| if w.is_finite() { w.max(0.0) } else { 0.0 });
    let total: f32 = clean.iter().sum();
    if total <= 0.0 {
        return [255, 0, 0, 0];
    }
    let mut out = clean.map(|w| (w / total * 255.0).round() as u8);
    let sum: i32 = out.iter().map(|&w| i32::from(w)).sum();
    let heaviest = (0..4)
        .max_by(|&a, &b| clean[a].total_cmp(&clean[b]))
        .unwrap_or(0);
    out[heaviest] = (i32::from(out[heaviest]) + 255 - sum).clamp(0, 255) as u8;
    out
}

/// Compiles the admitted sets under `root`, and the player from the
/// character sources at `player` when given, into a pack within `limits`.
///
/// Refuses a file whose digest differs from its manifest, an unadmitted file
/// in a set, and any budget the pack or the committed sources exceed. The
/// result has been decoded by the same decoder the loader runs.
pub fn compile(
    root: &Path,
    sets: &[&str],
    player: Option<&Path>,
    limits: &Limits,
) -> Result<Compiled, String> {
    let mut source_bytes = 0u64;
    let mut loaded = Vec::new();
    for set in sets {
        let (source, committed) = read_set(root, set)?;
        source_bytes += committed;
        loaded.push(source);
    }
    let mut builder = Builder {
        limits,
        contents: Contents::default(),
        textures: BTreeMap::new(),
    };
    let mut models = Vec::new();
    for set in &loaded {
        for file in set.files.keys().filter(|name| name.ends_with(".gltf")) {
            models.push(builder.model(set, file)?);
        }
    }
    models.sort_by(|a, b| a.name.cmp(&b.name));
    builder.contents.models = models;
    if let Some(sources) = player {
        let character = builder.player(sources)?;
        builder.contents.character = Some(character);
    }
    let contents = builder.contents;
    let bytes = format::encode(&contents, limits)?;
    let decoded = format::decode(&bytes, limits)?;
    if source_bytes + bytes.len() as u64 > limits.committed_bytes {
        return Err(format!(
            "Sources ({source_bytes} bytes) and pack ({} bytes) exceed the committed budget",
            bytes.len()
        ));
    }
    Ok(Compiled {
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        models: decoded.models.len(),
        triangles: decoded.models.iter().map(Model::triangles).sum::<u64>()
            + decoded.character.as_ref().map_or(0, Character::triangles),
        decoded_texture_bytes: decoded.decoded_texture_bytes(),
        source_bytes,
        bytes,
    })
}
