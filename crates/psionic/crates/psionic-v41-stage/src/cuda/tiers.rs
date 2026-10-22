//! Host-memory tiers for a single-machine CUDA stage: the routed expert
//! cache (#12463) and Engram rows read from the shards (#12466).
//!
//! `--experts file` reads routed experts in place from the checkpoint's
//! shards, which this module maps read-only, so the page cache holds the
//! warm ones and the rest come from disk. `--engram file` does the same for
//! the Engram tables. Both hand the stage plain byte slices, so the stage's
//! arithmetic is the same as with pinned memory.

use std::{collections::BTreeMap, fs::File, ops::Range, path::Path, sync::Arc};

use memmap2::Mmap;
use psionic_backend_cuda::{
    v41_engram_rows::{V41EngramRows, V41OwnedEngram},
    v41_expert_cache::{
        V41ExpertBytes, V41ExpertCacheConfig, V41ExpertCacheSetup, V41MissLane, V41OwnedExperts,
    },
    v41_expert_lane::V41ExpertDims,
    v41_stage::{V41StageOptions, V41StageShape, V41TensorSource},
};
use psionic_models::deepseek_v41_weights::DeepSeekV41Checkpoint;
use psionic_runtime::RuntimeError;
use serde_json::{Value, json};

use crate::{CacheSize, Experts, HostTiers, Refused};

/// The schema of a routing profile file.
pub const PROFILE_SCHEMA: &str = "openagents.deepseek_v41.expert_profile.v1";

fn backend_error(message: impl Into<String>) -> RuntimeError {
    RuntimeError::Backend(message.into())
}

/// The checkpoint's shards, mapped read-only.
struct MappedShards {
    maps: Vec<Mmap>,
    names: BTreeMap<String, usize>,
}

impl MappedShards {
    fn map(&mut self, checkpoint: &DeepSeekV41Checkpoint, shard: &str) -> Result<usize, Refused> {
        if let Some(i) = self.names.get(shard) {
            return Ok(*i);
        }
        let path = checkpoint.root().join(shard);
        let file = File::open(&path)
            .map_err(|e| Refused(format!("can't open {}: {e}", path.display())))?;
        // SAFETY: the shards are read-only checkpoint files that nothing
        // writes while a stage runs; their hashes were checked at open.
        let map = unsafe { Mmap::map(&file) }
            .map_err(|e| Refused(format!("can't map {}: {e}", path.display())))?;
        self.maps.push(map);
        let i = self.maps.len() - 1;
        self.names.insert(shard.to_owned(), i);
        Ok(i)
    }
}

/// A tensor's place in a mapped shard: (shard, offset, length).
type Place = (usize, usize, usize);

fn place(
    checkpoint: &DeepSeekV41Checkpoint,
    shards: &mut MappedShards,
    name: &str,
) -> Result<Place, Refused> {
    let entry = checkpoint
        .entry(name)
        .map_err(|e| Refused(format!("{name}: {e}")))?;
    let shard = checkpoint
        .shard_of(name)
        .ok_or_else(|| Refused(format!("{name} has no shard")))?;
    let i = shards.map(checkpoint, shard)?;
    let (offset, len) = (
        usize::try_from(entry.file_offset).map_err(|e| Refused(e.to_string()))?,
        usize::try_from(entry.byte_len).map_err(|e| Refused(e.to_string()))?,
    );
    if offset + len > shards.maps[i].len() {
        return Err(Refused(format!("{name} runs past its shard")));
    }
    Ok((i, offset, len))
}

/// Routed experts read in place from the mapped shards.
pub struct CheckpointExperts {
    shards: MappedShards,
    /// (layer, expert, projection) to (weights, scales).
    parts: BTreeMap<(usize, usize, usize), (Place, Place)>,
}

impl CheckpointExperts {
    /// Maps every routed expert of `layers`.
    ///
    /// # Errors
    ///
    /// Refuses missing tensors and shards that can't be mapped.
    pub fn open(
        checkpoint: &DeepSeekV41Checkpoint,
        layers: Range<usize>,
        experts: usize,
    ) -> Result<Self, Refused> {
        let mut shards = MappedShards {
            maps: Vec::new(),
            names: BTreeMap::new(),
        };
        let mut parts = BTreeMap::new();
        for layer in layers {
            for expert in 0..experts {
                for (p, w) in ["w1", "w2", "w3"].iter().enumerate() {
                    let stem = format!("layers.{layer}.ffn.experts.{expert}.{w}");
                    let weights = place(checkpoint, &mut shards, &format!("{stem}.weight"))?;
                    let scales = place(checkpoint, &mut shards, &format!("{stem}.scale"))?;
                    parts.insert((layer, expert, p), (weights, scales));
                }
            }
        }
        Ok(Self { shards, parts })
    }

    /// Bytes of every mapped expert.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.parts.values().map(|(w, s)| w.2 + s.2).sum()
    }
}

impl V41ExpertBytes for CheckpointExperts {
    fn projection(
        &self,
        layer: usize,
        expert: usize,
        projection: usize,
    ) -> Result<(&[u8], &[u8]), RuntimeError> {
        let ((wi, wo, wl), (si, so, sl)) = *self
            .parts
            .get(&(layer, expert, projection))
            .ok_or_else(|| backend_error(format!("no expert {expert} in layer {layer}")))?;
        Ok((
            &self.shards.maps[wi][wo..wo + wl],
            &self.shards.maps[si][so..so + sl],
        ))
    }
}

/// Engram tables read in place from the mapped shards.
pub struct CheckpointEngram {
    shards: MappedShards,
    /// Layer to (codes, scales, width).
    tables: BTreeMap<usize, (Place, Place, usize)>,
}

impl CheckpointEngram {
    /// Maps the Engram tables of `layers`.
    ///
    /// # Errors
    ///
    /// Refuses missing tensors and shards that can't be mapped.
    pub fn open(
        checkpoint: &DeepSeekV41Checkpoint,
        layers: &[usize],
        width: usize,
    ) -> Result<Self, Refused> {
        let mut shards = MappedShards {
            maps: Vec::new(),
            names: BTreeMap::new(),
        };
        let mut tables = BTreeMap::new();
        for layer in layers {
            let codes = place(
                checkpoint,
                &mut shards,
                &format!("layers.{layer}.engram.embed.weight"),
            )?;
            let scales = place(
                checkpoint,
                &mut shards,
                &format!("layers.{layer}.engram.embed.scale"),
            )?;
            // A step reads a few hundred-byte rows at random: readahead
            // would only fill the page cache with neighbours no step needs.
            for (i, offset, len) in [codes, scales] {
                shards.maps[i]
                    .advise_range(memmap2::Advice::Random, offset, len)
                    .map_err(|e| Refused(format!("can't advise the Engram table's pages: {e}")))?;
            }
            tables.insert(*layer, (codes, scales, width));
        }
        Ok(Self { shards, tables })
    }
}

impl V41EngramRows for CheckpointEngram {
    fn row(&self, layer: usize, row: u64) -> Result<(&[u8], &[u8]), RuntimeError> {
        let ((ci, co, cl), (si, so, sl), width) = *self
            .tables
            .get(&layer)
            .ok_or_else(|| backend_error(format!("no Engram table for layer {layer}")))?;
        let row = usize::try_from(row).map_err(|_| backend_error("row"))?;
        let (w, s) = (width, width / 32);
        if (row + 1) * w > cl || (row + 1) * s > sl {
            return Err(backend_error(format!("Engram row {row} of layer {layer}")));
        }
        Ok((
            &self.shards.maps[ci][co + row * w..co + (row + 1) * w],
            &self.shards.maps[si][so + row * s..so + (row + 1) * s],
        ))
    }
}

/// Reads a routing profile written by [`write_profile`].
///
/// # Errors
///
/// Refuses an unreadable file or another schema.
pub fn read_profile(path: &Path) -> Result<BTreeMap<usize, Vec<f32>>, Refused> {
    let bytes =
        std::fs::read(path).map_err(|e| Refused(format!("can't read {}: {e}", path.display())))?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|e| Refused(format!("{} is not JSON: {e}", path.display())))?;
    if value["schema"] != PROFILE_SCHEMA {
        return Err(Refused(format!(
            "{} is not a {PROFILE_SCHEMA} file",
            path.display()
        )));
    }
    let mut out = BTreeMap::new();
    for (layer, heat) in value["layers"].as_object().into_iter().flatten() {
        let layer: usize = layer
            .parse()
            .map_err(|_| Refused(format!("profile layer {layer}")))?;
        let heat = heat
            .as_array()
            .ok_or_else(|| Refused(format!("profile layer {layer} has no list")))?
            .iter()
            .map(|v| v.as_f64().map(|v| v as f32))
            .collect::<Option<Vec<f32>>>()
            .ok_or_else(|| {
                Refused(format!(
                    "profile layer {layer} has a value that isn't a number"
                ))
            })?;
        out.insert(layer, heat);
    }
    Ok(out)
}

/// A routing profile as JSON: each layer's heat per expert.
#[must_use]
pub fn profile_json(profile: &BTreeMap<usize, Vec<f32>>, note: &str) -> Value {
    json!({
        "schema": PROFILE_SCHEMA,
        "note": note,
        "layers": profile
            .iter()
            .map(|(l, h)| (l.to_string(), json!(h)))
            .collect::<serde_json::Map<_, _>>(),
    })
}

/// Writes a routing profile.
///
/// # Errors
///
/// Refuses a file that can't be written.
pub fn write_profile(
    path: &Path,
    profile: &BTreeMap<usize, Vec<f32>>,
    note: &str,
) -> Result<(), Refused> {
    let json =
        serde_json::to_vec(&profile_json(profile, note)).map_err(|e| Refused(e.to_string()))?;
    std::fs::write(path, json).map_err(|e| Refused(format!("can't write {}: {e}", path.display())))
}

/// Where a model's tensors come from, for [`stage_options`].
pub enum TierSource<'a> {
    Checkpoint(&'a DeepSeekV41Checkpoint),
    Synthetic(&'a dyn V41TensorSource),
}

/// The stage options the tiers ask for, and whether the routed stacks are
/// page-locked (`experts_on_host`).
///
/// # Errors
///
/// Refuses tiers that don't fit the expert placement and sources that
/// can't be opened.
pub fn stage_options(
    source: TierSource<'_>,
    shape: &V41StageShape,
    layers: &Range<usize>,
    experts: Experts,
    tiers: &HostTiers,
) -> Result<(V41StageOptions, bool), Refused> {
    tiers.check(experts)?;
    let mut options = V41StageOptions {
        batched_prefill: tiers.batched_prefill,
        ..V41StageOptions::default()
    };
    let engram_layers: Vec<usize> = [1_usize, 14]
        .into_iter()
        .filter(|l| layers.contains(l))
        .collect();
    if tiers.engram_file && !engram_layers.is_empty() {
        let rows: Arc<dyn V41EngramRows> = match &source {
            TierSource::Checkpoint(c) => Arc::new(CheckpointEngram::open(
                c,
                &engram_layers,
                shape.engram_width,
            )?),
            TierSource::Synthetic(s) => Arc::new(
                V41OwnedEngram::from_source(*s, &engram_layers, shape.engram_width)
                    .map_err(|e| Refused(e.to_string()))?,
            ),
        };
        options.engram_rows = Some(rows);
    }
    if let Some(size) = tiers.expert_cache {
        let dims = V41ExpertDims {
            hidden: shape.hidden,
            width: shape.expert_width,
            swiglu_limit: shape.swiglu_limit,
        };
        let slots = match size {
            CacheSize::Auto => 0,
            CacheSize::Slots(n) => n.max(1),
            CacheSize::Gigabytes(gb) => {
                ((gb * 1e9) as usize / (dims.expert_bytes() * layers.len().max(1))).max(1)
            }
        };
        let mut config = V41ExpertCacheConfig::new(slots);
        config.miss_lane = match tiers.miss_lane.as_deref() {
            Some(lane) => V41MissLane::parse(lane).map_err(|e| Refused(e.to_string()))?,
            None => V41MissLane::Auto,
        };
        config.host_threads = tiers.host_threads;
        config.frozen = tiers.frozen;
        if let Some(path) = &tiers.expert_profile {
            config.profile = read_profile(path)?;
        }
        let host: Option<Arc<dyn V41ExpertBytes>> = match (experts, &source) {
            (Experts::Host, _) => None,
            (_, TierSource::Checkpoint(c)) => Some(Arc::new(CheckpointExperts::open(
                c,
                layers.clone(),
                shape.router.experts,
            )?)),
            (_, TierSource::Synthetic(s)) => Some(Arc::new(
                V41OwnedExperts::from_source(*s, layers.clone(), shape.router.experts)
                    .map_err(|e| Refused(e.to_string()))?,
            )),
        };
        options.expert_cache = Some(V41ExpertCacheSetup { config, host });
    }
    Ok((options, experts == Experts::Host))
}
