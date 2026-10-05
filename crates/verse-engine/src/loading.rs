//! Explicit-root, bounded preparation of immutable packs before GPU allocation.
use crate::assets::Pack;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::File,
    io::{Read, Write},
    path::Path,
};

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Budget {
    pub manifest_bytes: u64,
    pub encoded_file_bytes: u64,
    pub encoded_total_bytes: u64,
    pub rgba_total_bytes: u64,
    pub decoder_workspace_bytes: u64,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            manifest_bytes: 128 * 1024 * 1024,
            encoded_file_bytes: 64 * 1024 * 1024,
            encoded_total_bytes: 128 * 1024 * 1024,
            rgba_total_bytes: 256 * 1024 * 1024,
            decoder_workspace_bytes: 64 * 1024 * 1024,
        }
    }
}
impl Budget {
    fn validate(self) -> Result<(), String> {
        if [
            self.manifest_bytes,
            self.encoded_file_bytes,
            self.encoded_total_bytes,
            self.rgba_total_bytes,
            self.decoder_workspace_bytes,
        ]
        .iter()
        .any(|v| !(1..=1024 * 1024 * 1024).contains(v))
        {
            return Err("Invalid asset preparation budget".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct TextureReceipt {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset: Option<crate::inventory::AssetId>,
    pub file: String,
    pub sha256: String,
    pub encoded_bytes: u64,
    pub rgba_bytes: u64,
    pub width: u32,
    pub height: u32,
}
#[derive(Clone, Debug, Serialize)]
pub struct Receipt {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inventory: Option<crate::inventory::Admission>,
    pub schema: &'static str,
    pub manifest_sha256: String,
    pub manifest_bytes: u64,
    pub vertices: u64,
    pub indices: u64,
    pub encoded_bytes: u64,
    pub rgba_bytes: u64,
    pub budget: Budget,
    pub textures: Vec<TextureReceipt>,
}
/// Validated, eight-bit sRGB RGBA bytes in pack texture order.
#[derive(Clone)]
pub struct Texture {
    pixels: std::sync::Arc<[u8]>,
    width: u32,
    height: u32,
}
impl Texture {
    pub fn rgba(&self) -> &[u8] {
        &self.pixels
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
}
/// No partial prepared pack escapes if any declared dependency is refused.
#[derive(Clone)]
pub struct Prepared {
    pack: std::sync::Arc<Pack>,
    textures: Vec<Texture>,
    receipt: Receipt,
}
struct ManifestDigest {
    hash: Sha256,
    bytes: u64,
    limit: u64,
}
impl Write for ManifestDigest {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let next = self
            .bytes
            .checked_add(bytes.len() as u64)
            .filter(|n| *n <= self.limit)
            .ok_or_else(|| std::io::Error::other("Asset manifest exceeds its byte budget"))?;
        self.hash.update(bytes);
        self.bytes = next;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
#[cfg(unix)]
fn open_texture(root: &File, name: &str) -> Result<File, String> {
    use std::os::fd::{AsRawFd, FromRawFd};
    let name = std::ffi::CString::new(name).map_err(|e| e.to_string())?;
    // The root descriptor stays live. A flat validated name cannot traverse parents;
    // no-follow and nonblocking flags refuse swapped symlinks and special files.
    let descriptor = unsafe {
        libc::openat(
            root.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK,
        )
    };
    if descriptor < 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    // openat returned a new owned descriptor; File closes it on every exit path.
    Ok(unsafe { File::from_raw_fd(descriptor) })
}
impl Prepared {
    pub fn load(pack: Pack, root: &Path, budget: Budget) -> Result<Self, String> {
        budget.validate()?;
        pack.validate()?;
        let admission = if let Some(inventory) = &pack.inventory {
            inventory.verify(&pack)?;
            Some(inventory.admit(
                &pack,
                crate::inventory::Purpose::OriginalLocal,
                &inventory.roots(),
            )?)
        } else {
            None
        };
        let mut digest = ManifestDigest {
            hash: Sha256::new(),
            bytes: 0,
            limit: budget.manifest_bytes,
        };
        serde_json::to_writer(&mut digest, &pack).map_err(|e| e.to_string())?;
        let mut receipt = Receipt {
            inventory: admission,
            schema: "openagents.verse.prepared-pack.v1",
            manifest_sha256: format!("{:x}", digest.hash.finalize()),
            manifest_bytes: digest.bytes,
            vertices: 0,
            indices: 0,
            encoded_bytes: 0,
            rgba_bytes: 0,
            budget,
            textures: vec![],
        };
        for model in pack.models.values() {
            for surface in &model.surfaces {
                receipt.vertices += surface.vertices.len() as u64;
                receipt.indices += surface.indices.len() as u64;
            }
        }
        // Admit the entire declared output footprint before reading or decoding any file.
        let mut files = BTreeSet::new();
        for texture in &pack.textures {
            if !files.insert(&texture.file) {
                return Err("Duplicate texture file declaration".into());
            }
            if texture.sha256.len() != 64
                || !texture
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err("Invalid texture digest".into());
            }
            receipt.rgba_bytes = receipt
                .rgba_bytes
                .checked_add(u64::from(texture.width) * u64::from(texture.height) * 4)
                .filter(|n| *n <= budget.rgba_total_bytes)
                .ok_or("Decoded textures exceed their aggregate budget")?;
        }
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        if !root.is_dir() {
            return Err("Asset root is not a directory".into());
        }
        #[cfg(unix)]
        let directory = File::open(&root).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        if !directory.metadata().map_err(|e| e.to_string())?.is_dir() {
            return Err("Asset root is not a directory".into());
        }
        let mut textures = Vec::with_capacity(pack.textures.len());
        for (slot, texture) in pack.textures.iter().enumerate() {
            let path = root.join(&texture.file);
            let metadata = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            if !metadata.file_type().is_file()
                || path.canonicalize().map_err(|e| e.to_string())?.parent() != Some(root.as_path())
            {
                return Err(
                    "Texture file escapes the admitted root or is not a regular file".into(),
                );
            }
            let available = budget
                .encoded_file_bytes
                .min(budget.encoded_total_bytes - receipt.encoded_bytes);
            if metadata.len() > available {
                return Err("Encoded texture exceeds its byte budget".into());
            }
            #[cfg(unix)]
            let file = open_texture(&directory, &texture.file)?;
            #[cfg(not(unix))]
            let file = File::open(&path).map_err(|e| e.to_string())?;
            let opened = file.metadata().map_err(|e| e.to_string())?;
            if !opened.is_file() {
                return Err("Texture is not a regular file".into());
            }
            if opened.len() > available {
                return Err("Encoded texture exceeds its byte budget".into());
            }
            let mut bytes = Vec::with_capacity(opened.len() as usize);
            file.take(available + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() as u64 > available {
                return Err("Encoded texture exceeds its byte budget".into());
            }
            if let Some(inventory) = &pack.inventory {
                let asset = inventory
                    .assets
                    .iter()
                    .find(|a| a.binding == crate::inventory::Binding::Texture { slot })
                    .ok_or("Missing inventory texture")?;
                if asset.bytes != bytes.len() as u64 {
                    return Err("Inventory encoded texture length mismatch".into());
                }
            }
            let actual_digest = format!("{:x}", Sha256::digest(&bytes));
            if actual_digest != texture.sha256 {
                return Err("Texture digest mismatch".into());
            }
            let mut decoder = png::Decoder::new(std::io::Cursor::new(&bytes));
            decoder.set_limits(png::Limits {
                bytes: budget.decoder_workspace_bytes as usize,
            });
            let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
            let info = reader.info();
            if info.width != texture.width
                || info.height != texture.height
                || info.color_type != png::ColorType::Rgba
                || info.bit_depth != png::BitDepth::Eight
                || info.animation_control.is_some()
            {
                return Err(
                    "Texture does not match its declared static eight-bit RGBA format".into(),
                );
            }
            let expected = texture.width as usize * texture.height as usize * 4;
            if reader.output_buffer_size() != Some(expected) {
                return Err("Invalid decoded texture size".into());
            }
            let mut pixels = vec![0; expected];
            let output = reader.next_frame(&mut pixels).map_err(|e| e.to_string())?;
            if output.buffer_size() != expected {
                return Err("Incomplete decoded texture".into());
            }
            reader.finish().map_err(|e| e.to_string())?;
            receipt.encoded_bytes += bytes.len() as u64;
            receipt.textures.push(TextureReceipt {
                asset: pack
                    .inventory
                    .as_ref()
                    .and_then(|i| {
                        i.assets
                            .iter()
                            .find(|a| a.binding == crate::inventory::Binding::Texture { slot })
                    })
                    .map(|a| a.id.clone()),
                file: texture.file.clone(),
                sha256: actual_digest,
                encoded_bytes: bytes.len() as u64,
                rgba_bytes: expected as u64,
                width: texture.width,
                height: texture.height,
            });
            textures.push(Texture {
                pixels: pixels.into(),
                width: texture.width,
                height: texture.height,
            });
        }
        Ok(Self {
            pack: std::sync::Arc::new(pack),
            textures,
            receipt,
        })
    }
    pub fn pack(&self) -> &Pack {
        &self.pack
    }
    pub fn textures(&self) -> &[Texture] {
        &self.textures
    }
    pub fn receipt(&self) -> &Receipt {
        &self.receipt
    }
    pub fn into_parts(self) -> (Pack, Vec<Texture>, Receipt) {
        (
            std::sync::Arc::unwrap_or_clone(self.pack),
            self.textures,
            self.receipt,
        )
    }
    /// Retain verified source for resource recreation without reopening mutable files.
    pub fn into_shared_parts(self) -> (std::sync::Arc<Pack>, Vec<Texture>, Receipt) {
        (self.pack, self.textures, self.receipt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::{Model, Texture as Reference};
    use std::{
        collections::BTreeMap,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };
    static ID: AtomicU64 = AtomicU64::new(0);
    struct Fixture {
        root: PathBuf,
        pack: Pack,
    }
    fn png_bytes(color: png::ColorType, depth: png::BitDepth, pixels: &[u8]) -> Vec<u8> {
        let mut bytes = vec![];
        let mut encoder = png::Encoder::new(&mut bytes, 2, 2);
        encoder.set_color(color);
        encoder.set_depth(depth);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(pixels)
            .unwrap();
        bytes
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "verse-engine-assets-{}-{}",
                std::process::id(),
                ID.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&root).unwrap();
            let mut textures = vec![];
            for i in 0..2 {
                let file = format!("{i}.png");
                let bytes = png_bytes(png::ColorType::Rgba, png::BitDepth::Eight, &[i; 16]);
                std::fs::write(root.join(&file), &bytes).unwrap();
                textures.push(Reference {
                    file,
                    sha256: format!("{:x}", Sha256::digest(&bytes)),
                    width: 2,
                    height: 2,
                });
            }
            let pack = Pack {
                inventory: None,
                version: 1,
                source_revision: "test/assets".into(),
                textures,
                placements: vec![],
                models: BTreeMap::from([(
                    "fixture".into(),
                    Model {
                        graph: None,
                        markers: Vec::new(),
                        states: Default::default(),
                        skin: None,
                        source: "test/assets".into(),
                        source_sha256: String::new(),
                        surfaces: vec![],
                        bones: vec![],
                        clips: vec![],
                        height: 1.,
                        attachments: vec![],
                    },
                )]),
            };
            Self { root, pack }
        }
        fn load(&self, budget: Budget) -> Result<Prepared, String> {
            Prepared::load(self.pack.clone(), &self.root, budget)
        }
        fn replace(&mut self, index: usize, bytes: Vec<u8>) {
            self.pack.textures[index].sha256 = format!("{:x}", Sha256::digest(&bytes));
            std::fs::write(self.root.join(&self.pack.textures[index].file), bytes).unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
    #[test]
    fn complete_preparation_is_immutable_ordered_and_bound_to_the_manifest() {
        let f = Fixture::new();
        let prepared = f.load(Budget::default()).unwrap();
        assert_eq!(prepared.textures().len(), 2);
        assert_eq!(prepared.textures()[0].rgba(), &[0; 16]);
        assert_eq!(prepared.textures()[1].rgba(), &[1; 16]);
        assert_eq!(prepared.textures()[0].width(), 2);
        assert_eq!(prepared.textures()[0].height(), 2);
        assert_eq!(prepared.receipt().rgba_bytes, 32);
        assert_eq!(
            prepared.receipt().encoded_bytes,
            prepared
                .receipt()
                .textures
                .iter()
                .map(|t| t.encoded_bytes)
                .sum::<u64>()
        );
        assert_eq!(
            prepared.receipt().manifest_sha256,
            format!("{:x}", Sha256::digest(serde_json::to_vec(&f.pack).unwrap()))
        );
        std::fs::remove_file(f.root.join("0.png")).unwrap();
        assert_eq!(prepared.textures()[0].rgba(), &[0; 16]);
        assert!(f.load(Budget::default()).is_err());
    }
    #[test]
    fn manifest_file_reads_are_bounded_and_validated() {
        let f = Fixture::new();
        let path = f.root.join("pack.json");
        std::fs::write(&path, serde_json::to_vec(&f.pack).unwrap()).unwrap();
        assert_eq!(Pack::read(&path).unwrap().source_revision, "test/assets");
        let oversized = File::create(&path).unwrap();
        oversized.set_len(128 * 1024 * 1024 + 1).unwrap();
        assert!(Pack::read(&path).is_err());
        drop(oversized);
        std::fs::write(path, b"invalid manifest").unwrap();
        assert!(Pack::read(&f.root.join("pack.json")).is_err());
    }
    #[test]
    fn all_budget_classes_are_enforced_without_a_partial_result() {
        let f = Fixture::new();
        let admitted = f.load(Budget::default()).unwrap();
        let receipt = admitted.receipt();
        for budget in [
            Budget {
                manifest_bytes: 1,
                ..Default::default()
            },
            Budget {
                encoded_file_bytes: 1,
                ..Default::default()
            },
            Budget {
                encoded_total_bytes: receipt.encoded_bytes - 1,
                ..Default::default()
            },
            Budget {
                rgba_total_bytes: 31,
                ..Default::default()
            },
            Budget {
                decoder_workspace_bytes: 1,
                ..Default::default()
            },
            Budget {
                rgba_total_bytes: 0,
                ..Default::default()
            },
        ] {
            assert!(f.load(budget).is_err(), "{budget:?}");
        }
        let boundary = Budget {
            manifest_bytes: receipt.manifest_bytes,
            encoded_file_bytes: receipt
                .textures
                .iter()
                .map(|t| t.encoded_bytes)
                .max()
                .unwrap(),
            encoded_total_bytes: receipt.encoded_bytes,
            rgba_total_bytes: 32,
            ..Default::default()
        };
        f.load(boundary).unwrap();
    }
    #[test]
    fn mutated_missing_duplicate_and_invalid_dependencies_refuse_the_whole_pack() {
        let mut f = Fixture::new();
        let original = std::fs::read(f.root.join("1.png")).unwrap();
        std::fs::write(f.root.join("1.png"), b"changed").unwrap();
        assert!(
            f.load(Default::default())
                .err()
                .unwrap()
                .contains("digest mismatch")
        );
        f.replace(1, original);
        f.pack.textures[1].width = 3;
        assert!(
            f.load(Default::default())
                .err()
                .unwrap()
                .contains("declared")
        );
        f.pack.textures[1].width = 2;
        f.pack.textures[1] = f.pack.textures[0].clone();
        assert!(
            f.load(Default::default())
                .err()
                .unwrap()
                .contains("Duplicate")
        );
        f.pack.textures[1].file = "../other.png".into();
        assert!(f.load(Default::default()).is_err());
        f.pack.textures[1].file = "missing.png".into();
        assert!(f.load(Default::default()).is_err());
        f.pack.textures[1].sha256 = "xyz".into();
        assert!(f.load(Default::default()).err().unwrap().contains("digest"));
    }
    #[test]
    fn unsupported_channels_depth_and_malformed_png_are_refused_before_upload() {
        let mut f = Fixture::new();
        f.replace(
            0,
            png_bytes(png::ColorType::Rgb, png::BitDepth::Eight, &[0; 12]),
        );
        assert!(f.load(Default::default()).err().unwrap().contains("format"));
        f.replace(
            0,
            png_bytes(png::ColorType::Rgba, png::BitDepth::Sixteen, &[0; 32]),
        );
        assert!(f.load(Default::default()).err().unwrap().contains("format"));
        let mut truncated = png_bytes(png::ColorType::Rgba, png::BitDepth::Eight, &[0; 16]);
        truncated.truncate(truncated.len() - 10);
        f.replace(0, truncated);
        assert!(f.load(Default::default()).is_err());
        f.replace(0, b"not a PNG".to_vec());
        assert!(f.load(Default::default()).is_err());
    }
    #[test]
    fn placement_dependencies_and_material_values_are_admitted_before_io() {
        let mut f = Fixture::new();
        f.pack.placements.push(crate::assets::Placement {
            model: "missing".into(),
            position: [0.; 3],
            rotation: [0., 0., 0., 1.],
            scale: 1.,
        });
        assert!(f.load(Default::default()).is_err());
        f.pack.placements[0].model = "fixture".into();
        f.load(Default::default()).unwrap();
        f.pack.placements[0].rotation = [0.; 4];
        assert!(f.load(Default::default()).is_err());
        f.pack.placements.clear();
        f.pack
            .models
            .get_mut("fixture")
            .unwrap()
            .surfaces
            .push(crate::assets::Surface {
                material: Default::default(),
                vertices: vec![],
                indices: vec![],
                texture: 0,
                blend: 0,
                emissive: false,
                topology: Default::default(),
                unlit: false,
                tint: [f32::NAN; 3],
            });
        assert!(f.load(Default::default()).is_err());
    }
    #[test]
    fn animated_png_requires_a_separate_declared_asset_contract() {
        let mut f = Fixture::new();
        let mut bytes = vec![];
        {
            let mut encoder = png::Encoder::new(&mut bytes, 2, 2);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_animated(2, 0).unwrap();
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[0; 16]).unwrap();
            writer.write_image_data(&[1; 16]).unwrap();
        }
        f.replace(0, bytes);
        assert!(f.load(Default::default()).err().unwrap().contains("static"));
    }
    #[cfg(unix)]
    #[test]
    fn pinned_directory_and_no_follow_open_survive_namespace_changes() {
        let f = Fixture::new();
        let directory = File::open(&f.root).unwrap();
        let old = f.root.with_extension("moved");
        std::fs::rename(&f.root, &old).unwrap();
        std::fs::create_dir(&f.root).unwrap();
        std::fs::write(f.root.join("0.png"), b"replacement namespace").unwrap();
        let mut bytes = vec![];
        open_texture(&directory, "0.png")
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            f.pack.textures[0].sha256
        );
        std::fs::remove_file(old.join("0.png")).unwrap();
        std::os::unix::fs::symlink(old.join("1.png"), old.join("0.png")).unwrap();
        assert!(open_texture(&directory, "0.png").is_err());
        std::fs::remove_dir_all(old).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn symlink_files_and_nonregular_dependencies_are_refused() {
        let f = Fixture::new();
        let outside = Fixture::new();
        std::fs::remove_file(f.root.join("0.png")).unwrap();
        std::os::unix::fs::symlink(outside.root.join("0.png"), f.root.join("0.png")).unwrap();
        assert!(
            f.load(Default::default())
                .err()
                .unwrap()
                .contains("regular file")
        );
        std::fs::remove_file(f.root.join("0.png")).unwrap();
        std::os::unix::fs::symlink(f.root.join("1.png"), f.root.join("0.png")).unwrap();
        assert!(f.load(Default::default()).is_err());
        std::fs::remove_file(f.root.join("0.png")).unwrap();
        std::fs::create_dir(f.root.join("0.png")).unwrap();
        assert!(f.load(Default::default()).is_err());
    }
}
