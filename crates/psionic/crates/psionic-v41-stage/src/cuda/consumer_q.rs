//! The `consumer-q` artifact (#12465): routed experts requantized to 2 or 3
//! bits per value, for a single machine that can't hold the checkpoint's
//! FP4 experts in RAM and VRAM.
//!
//! **Lossy and unqualified** (owner, 2026-10-21): `consumer-q` is a separate
//! serving profile with its own quality card. It is never called qualified
//! against the B200 oracle and never replaces the exact path.
//!
//! ```text
//! psionic-v41-stage quantize --weights DIR --out DIR [--recipe q2q3|q3]
//!     [--importance FILE] [--layers A-B]
//! ```
//!
//! The artifact is a folder: `layer-LL.bin` per layer holds its 384 experts
//! one after another, each as `[w1 weights, w1 scales, w2 weights, w2
//! scales, w3 weights, w3 scales]` in the recipe's formats, and
//! `manifest.json` names the recipe, the formats, the source checkpoint,
//! the importance file's digest, and each layer file's SHA-256. A stage
//! reads it with `--experts file --consumer-q DIR` and maps the layer files
//! in place, as `--experts file` maps the checkpoint.
//!
//! Recipes: `q2q3` keeps gate and up (w1, w3) at 2 bits and down (w2) at
//! 3 bits, about 2.6 bits per weight (175 GB); `q3` keeps all three at 3
//! bits, about 3.3 bits per weight (221 GB).
//!
//! The quantizer is importance-weighted rounding after Project Maya's
//! calibration recipe (`mw00/project-maya` at `451f9cb6a2`, MIT),
//! reimplemented; see `psionic_backend_cuda::v41_quantize`.

use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read as _, Write as _},
    ops::Range,
    path::{Path, PathBuf},
};

use memmap2::Mmap;
use psionic_backend_cuda::{
    v41_expert_cache::V41ExpertBytes, v41_expert_lane::V41ExpertDims, v41_matvec::V41MatvecFormat,
    v41_quantize::v41_quantize_matrix,
};
use psionic_models::deepseek_v41_weights::{decode_e2m1, decode_f8_e8m0fnu};
use psionic_runtime::RuntimeError;
use rayon::prelude::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::tiers::CheckpointExperts;
use crate::Refused;

/// The artifact manifest's schema.
pub const CONSUMER_Q_SCHEMA: &str = "openagents.deepseek_v41.consumer_q.v1";
/// The importance file's schema.
pub const IMPORTANCE_MAGIC: &[u8; 8] = b"V41IMP01";

/// Which formats a recipe gives `[w1, w2, w3]`.
///
/// # Errors
///
/// Refuses an unknown recipe.
pub fn recipe_formats(recipe: &str) -> Result<[V41MatvecFormat; 3], Refused> {
    let (q2, q3) = (V41MatvecFormat::Q2Row32, V41MatvecFormat::Q3Row32);
    match recipe {
        "q2q3" => Ok([q2, q3, q2]),
        "q3" => Ok([q3, q3, q3]),
        other => Err(Refused(format!("recipe {other} is not q2q3 or q3"))),
    }
}

fn format_name(f: V41MatvecFormat) -> &'static str {
    match f {
        V41MatvecFormat::Q2Row32 => "q2-row32",
        V41MatvecFormat::Q3Row32 => "q3-row32",
        V41MatvecFormat::Fp4E2M1Row32 => "fp4-e2m1-row32",
        V41MatvecFormat::Fp8Block32x32 => "fp8-block32x32",
    }
}

fn parse_format(name: &str) -> Result<V41MatvecFormat, Refused> {
    match name {
        "q2-row32" => Ok(V41MatvecFormat::Q2Row32),
        "q3-row32" => Ok(V41MatvecFormat::Q3Row32),
        other => Err(Refused(format!("the artifact names format {other}"))),
    }
}

/// Per-expert, per-column mean squares of the activations each routed
/// projection saw during calibration: `inputs[layer][expert]` (hidden
/// columns, for w1 and w3) and `hidden[layer][expert]` (expert-width
/// columns, for w2), with the rows counted.
#[derive(Debug, Default)]
pub struct Importance {
    pub inputs: BTreeMap<usize, Vec<Vec<f32>>>,
    pub hidden: BTreeMap<usize, Vec<Vec<f32>>>,
    pub digest: Option<String>,
}

impl Importance {
    /// Reads an importance file: the magic, then little-endian `u32`
    /// layers, experts, hidden, and width, then per layer its number and
    /// `experts × (hidden + width)` `f32` mean squares.
    ///
    /// # Errors
    ///
    /// Refuses a malformed file.
    pub fn read(path: &Path) -> Result<Self, Refused> {
        let mut bytes = Vec::new();
        File::open(path)
            .and_then(|mut f| f.read_to_end(&mut bytes))
            .map_err(|e| Refused(format!("can't read {}: {e}", path.display())))?;
        let bad = || Refused(format!("{} is not an importance file", path.display()));
        if bytes.len() < 24 || &bytes[..8] != IMPORTANCE_MAGIC {
            return Err(bad());
        }
        let word =
            |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap_or([0; 4])) as usize;
        let (layers, experts, hidden, width) = (word(8), word(12), word(16), word(20));
        let mut at = 24;
        let mut this = Self {
            digest: Some(hex::encode(Sha256::digest(&bytes))),
            ..Self::default()
        };
        for _ in 0..layers {
            if at + 4 > bytes.len() {
                return Err(bad());
            }
            let layer = word(at);
            at += 4;
            let need = 4 * experts * (hidden + width);
            if at + need > bytes.len() {
                return Err(bad());
            }
            let values: Vec<f32> = bytes[at..at + need]
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            at += need;
            let (inputs, hidden_part) = values.split_at(experts * hidden);
            this.inputs
                .insert(layer, inputs.chunks(hidden).map(<[f32]>::to_vec).collect());
            this.hidden.insert(
                layer,
                hidden_part.chunks(width).map(<[f32]>::to_vec).collect(),
            );
        }
        Ok(this)
    }

    /// Reads several importance files (one per stage) as one.
    ///
    /// # Errors
    ///
    /// Refuses a malformed file or a layer two files both hold.
    pub fn read_many(paths: &[&Path]) -> Result<Self, Refused> {
        let mut all = Self::default();
        let mut digests = Vec::new();
        for path in paths {
            let one = Self::read(path)?;
            for (layer, v) in one.inputs {
                if all.inputs.insert(layer, v).is_some() {
                    return Err(Refused(format!("layer {layer} is in two importance files")));
                }
            }
            all.hidden.extend(one.hidden);
            digests.extend(one.digest);
        }
        all.digest = Some(hex::encode(Sha256::digest(digests.join(",").as_bytes())));
        Ok(all)
    }

    /// Writes the file [`Self::read`] reads.
    ///
    /// # Errors
    ///
    /// Refuses a file that can't be written.
    pub fn write(
        path: &Path,
        layers: &BTreeMap<usize, (Vec<f32>, Vec<f32>)>,
        experts: usize,
        hidden: usize,
        width: usize,
    ) -> Result<(), Refused> {
        let mut out = IMPORTANCE_MAGIC.to_vec();
        for n in [layers.len(), experts, hidden, width] {
            out.extend_from_slice(&(n as u32).to_le_bytes());
        }
        for (layer, (inputs, hidden_part)) in layers {
            out.extend_from_slice(&(*layer as u32).to_le_bytes());
            for v in inputs.iter().chain(hidden_part) {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        File::create(path)
            .and_then(|mut f| f.write_all(&out))
            .map_err(|e| Refused(format!("can't write {}: {e}", path.display())))
    }
}

/// One FP4 checkpoint matrix as `f32` values, row-major.
fn dequantize_fp4(weights: &[u8], scales: &[u8], rows: usize, cols: usize, out: &mut Vec<f32>) {
    let blocks = cols / 32;
    out.clear();
    for r in 0..rows {
        for b in 0..blocks {
            let s = decode_f8_e8m0fnu(scales[r * blocks + b]);
            for j in 0..32 {
                let byte = weights[r * cols / 2 + b * 16 + j / 2];
                let code = if j % 2 == 0 { byte & 15 } else { byte >> 4 };
                out.push(decode_e2m1(code) * s);
            }
        }
    }
}

/// Quantizes the routed experts of `layers` from the checkpoint into an
/// artifact folder. Returns the manifest.
///
/// # Errors
///
/// Refuses unreadable sources and unwritable outputs.
#[allow(clippy::too_many_arguments)]
pub fn quantize(
    checkpoint: &psionic_models::deepseek_v41_weights::DeepSeekV41Checkpoint,
    out: &Path,
    recipe: &str,
    importance: Option<&Importance>,
    layers: Range<usize>,
    experts: usize,
    hidden: usize,
    width: usize,
) -> Result<Value, Refused> {
    let formats = recipe_formats(recipe)?;
    let dims = V41ExpertDims {
        formats,
        ..V41ExpertDims::fp4(hidden, width, 10.0)
    };
    std::fs::create_dir_all(out)
        .map_err(|e| Refused(format!("can't make {}: {e}", out.display())))?;
    let source = CheckpointExperts::open(checkpoint, layers.clone(), experts)?;
    let expert_bytes = dims.expert_bytes();
    let mut files = serde_json::Map::new();
    let started = std::time::Instant::now();
    for layer in layers.clone() {
        let parts: Vec<Vec<u8>> = (0..experts)
            .into_par_iter()
            .map_init(
                || (Vec::new(), Vec::new(), Vec::new()),
                |(w, s, values), e| -> Result<Vec<u8>, RuntimeError> {
                    let mut bytes = Vec::with_capacity(expert_bytes);
                    for p in 0..3 {
                        let (rows, cols) = dims.shape(p);
                        source.read_projection(layer, e, p, w, s)?;
                        dequantize_fp4(w, s, rows, cols, values);
                        let imp = importance.and_then(|i| {
                            let table = if p == 1 { &i.hidden } else { &i.inputs };
                            table
                                .get(&layer)
                                .and_then(|v| v.get(e))
                                .filter(|v| v.len() == cols)
                        });
                        // Columns no calibration row reached keep a small floor.
                        let floored: Option<Vec<f32>> = imp.map(|v| {
                            let mean = v.iter().sum::<f32>() / v.len().max(1) as f32;
                            v.iter().map(|x| x.max(mean * 1e-3).max(1e-12)).collect()
                        });
                        let (qw, qs) =
                            v41_quantize_matrix(formats[p], values, rows, cols, floored.as_deref());
                        bytes.extend_from_slice(&qw);
                        bytes.extend_from_slice(&qs);
                    }
                    Ok(bytes)
                },
            )
            .collect::<Result<_, _>>()
            .map_err(|e| Refused(format!("layer {layer}: {e}")))?;
        let path = out.join(format!("layer-{layer:02}.bin"));
        let mut hasher = Sha256::new();
        let mut file = File::create(&path)
            .map_err(|e| Refused(format!("can't write {}: {e}", path.display())))?;
        for part in &parts {
            hasher.update(part);
            file.write_all(part)
                .map_err(|e| Refused(format!("can't write {}: {e}", path.display())))?;
        }
        files.insert(layer.to_string(), json!(hex::encode(hasher.finalize())));
        eprintln!(
            "quantize: layer {layer} done after {:.0} s",
            started.elapsed().as_secs_f64()
        );
    }
    let manifest = json!({
        "schema": CONSUMER_Q_SCHEMA,
        "profile": "consumer-q",
        "lossy": true,
        "qualified": false,
        "recipe": recipe,
        "formats": {"w1": format_name(formats[0]), "w2": format_name(formats[1]), "w3": format_name(formats[2])},
        "bits_per_weight": (expert_bytes * 8) as f64 / (3 * hidden * width) as f64,
        "experts": experts,
        "hidden": hidden,
        "width": width,
        "layers": files,
        "importance_sha256": importance.and_then(|i| i.digest.clone()),
        "source": {
            "repo": psionic_models::deepseek_v41_weights::DEEPSEEK_V41_FLASH_REPO,
            "revision": psionic_models::deepseek_v41_weights::DEEPSEEK_V41_FLASH_REVISION,
        },
        "method": "importance-weighted rounding to Lloyd-Max levels with a log-scale byte per 32 values and an f32 scale per row; after Project Maya (mw00/project-maya 451f9cb6a2, MIT), reimplemented",
    });
    let path = out.join("manifest.json");
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&manifest).map_err(|e| Refused(e.to_string()))?,
    )
    .map_err(|e| Refused(format!("can't write {}: {e}", path.display())))?;
    Ok(manifest)
}

/// A `consumer-q` artifact, mapped read-only, as a host-lane source.
pub struct ConsumerQExperts {
    maps: BTreeMap<usize, Mmap>,
    dims: V41ExpertDims,
    experts: usize,
    pub manifest: Value,
    pub dir: PathBuf,
}

impl ConsumerQExperts {
    /// Opens the artifact in `dir` for `layers`, checking each layer file's
    /// size (its digest is checked by `verify`).
    ///
    /// # Errors
    ///
    /// Refuses a missing or malformed artifact.
    pub fn open(
        dir: &Path,
        layers: Range<usize>,
        hidden: usize,
        width: usize,
    ) -> Result<Self, Refused> {
        let path = dir.join("manifest.json");
        let manifest: Value = serde_json::from_slice(
            &std::fs::read(&path)
                .map_err(|e| Refused(format!("can't read {}: {e}", path.display())))?,
        )
        .map_err(|e| Refused(format!("{} is not JSON: {e}", path.display())))?;
        if manifest["schema"] != CONSUMER_Q_SCHEMA {
            return Err(Refused(format!(
                "{} is not a {CONSUMER_Q_SCHEMA} manifest",
                path.display()
            )));
        }
        let f = |name: &str| parse_format(manifest["formats"][name].as_str().unwrap_or_default());
        let formats = [f("w1")?, f("w2")?, f("w3")?];
        let dims = V41ExpertDims {
            formats,
            ..V41ExpertDims::fp4(hidden, width, 10.0)
        };
        let experts = manifest["experts"].as_u64().unwrap_or(0) as usize;
        let mut maps = BTreeMap::new();
        for layer in layers {
            let path = dir.join(format!("layer-{layer:02}.bin"));
            let file = File::open(&path)
                .map_err(|e| Refused(format!("can't open {}: {e}", path.display())))?;
            // SAFETY: artifact files are read-only and nothing writes them
            // while a stage runs.
            let map = unsafe { Mmap::map(&file) }
                .map_err(|e| Refused(format!("can't map {}: {e}", path.display())))?;
            if map.len() != experts * dims.expert_bytes() {
                return Err(Refused(format!(
                    "{} has {} bytes, not {}",
                    path.display(),
                    map.len(),
                    experts * dims.expert_bytes()
                )));
            }
            maps.insert(layer, map);
        }
        Ok(Self {
            maps,
            dims,
            experts,
            manifest,
            dir: dir.to_path_buf(),
        })
    }

    /// The formats of `[w1, w2, w3]`.
    #[must_use]
    pub fn formats(&self) -> [V41MatvecFormat; 3] {
        self.dims.formats
    }

    /// Hashes every mapped layer file against the manifest.
    ///
    /// # Errors
    ///
    /// Refuses a file whose digest differs.
    pub fn verify(&self) -> Result<(), Refused> {
        let bad: Vec<usize> = self
            .maps
            .par_iter()
            .filter(|(layer, map)| {
                self.manifest["layers"][layer.to_string()].as_str()
                    != Some(&hex::encode(Sha256::digest(&map[..])))
            })
            .map(|(layer, _)| *layer)
            .collect();
        if bad.is_empty() {
            Ok(())
        } else {
            Err(Refused(format!(
                "the consumer-q layer files {bad:?} don't match the manifest"
            )))
        }
    }
}

impl V41ExpertBytes for ConsumerQExperts {
    fn projection(
        &self,
        layer: usize,
        expert: usize,
        projection: usize,
    ) -> Result<(&[u8], &[u8]), RuntimeError> {
        let map = self
            .maps
            .get(&layer)
            .ok_or_else(|| RuntimeError::Backend(format!("the artifact holds no layer {layer}")))?;
        if expert >= self.experts || projection > 2 {
            return Err(RuntimeError::Backend(format!(
                "expert {expert} w{}",
                projection + 1
            )));
        }
        let mut at = expert * self.dims.expert_bytes();
        for p in 0..projection {
            let (w, s) = self.dims.part_bytes(p);
            at += w + s;
        }
        let (w, s) = self.dims.part_bytes(projection);
        Ok((&map[at..at + w], &map[at + w..at + w + s]))
    }
}
