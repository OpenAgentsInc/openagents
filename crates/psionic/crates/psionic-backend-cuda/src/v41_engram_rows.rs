//! Engram rows read from any host source, a step at a time (#12466).
//!
//! The Engram tables of layers 1 and 14 hold about 101 GB each. A stage
//! normally pins both in host memory and the device gathers the rows it
//! needs. A single machine can't pin 203 GB beside the routed experts, so
//! this module lets a stage read the rows from a [`V41EngramRows`] source
//! instead, such as the checkpoint's shards mapped from disk, which the page
//! cache keeps warm.
//!
//! Every row a step needs is known before the step runs: an Engram row id
//! depends only on the token and the three before it. Before a step,
//! [`EngramStaging::stage`] hashes the step's tokens on the host exactly as
//! `v41_engram_hash` does on the device, copies each token's 24 rows per
//! layer into a small page-locked staging table, and the step's graph
//! gathers from that table instead of the whole one. The rows' bytes are
//! the same, so the stage's outputs are bit-equal to pinned tables.
//!
//! Project Maya reads Engram rows from the SSD through a row cache
//! (`mw00/project-maya` at `451f9cb6a2`, `src/ds4/ds4_engram_io.hpp`, MIT);
//! this is a reimplementation of the idea for our stage.

use crate::v41_stage::{V41_ENGRAM_COLUMNS, V41EngramHash, V41TensorSource, V41TokenMap};
use crate::{CudaBackend, CudaBuffer, CudaHostBuffer};
use psionic_runtime::RuntimeError;
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::sync::Arc;

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::Backend(format!("v41 Engram rows: {}", message.into()))
}

/// Where Engram rows come from.
pub trait V41EngramRows: Send + Sync {
    /// Row `row` of model layer `layer`'s table: `width` FP8 codes and
    /// `width / 32` UE8M0 scales.
    ///
    /// # Errors
    ///
    /// Refuses a layer or row the source doesn't hold, or a failed read.
    fn row(&self, layer: usize, row: u64) -> Result<(&[u8], &[u8]), RuntimeError>;
}

/// Engram tables copied into ordinary memory, for small models and tests.
#[derive(Debug, Default)]
pub struct V41OwnedEngram {
    tables: BTreeMap<usize, (Vec<u8>, Vec<u8>, usize)>,
}

impl V41OwnedEngram {
    /// Reads `layers.L.engram.embed.{weight,scale}` for each layer.
    ///
    /// # Errors
    ///
    /// Refuses a missing tensor.
    pub fn from_source(
        source: &dyn V41TensorSource,
        layers: &[usize],
        width: usize,
    ) -> Result<Self, RuntimeError> {
        let mut tables = BTreeMap::new();
        for layer in layers {
            let codes = source
                .tensor(&format!("layers.{layer}.engram.embed.weight"))?
                .bytes;
            let scales = source
                .tensor(&format!("layers.{layer}.engram.embed.scale"))?
                .bytes;
            tables.insert(*layer, (codes, scales, width));
        }
        Ok(Self { tables })
    }
}

impl V41EngramRows for V41OwnedEngram {
    fn row(&self, layer: usize, row: u64) -> Result<(&[u8], &[u8]), RuntimeError> {
        let (codes, scales, width) = self
            .tables
            .get(&layer)
            .ok_or_else(|| invalid(format!("no table for layer {layer}")))?;
        let row = usize::try_from(row).map_err(|_| invalid("row"))?;
        let (w, s) = (*width, width / 32);
        if (row + 1) * w > codes.len() {
            return Err(invalid(format!("row {row} of layer {layer}")));
        }
        Ok((
            &codes[row * w..(row + 1) * w],
            &scales[row * s..(row + 1) * s],
        ))
    }
}

/// `v41_engram_hash` on the host: one token at a time, with the same
/// three-token history, which restarts at position 0.
#[derive(Clone, Debug)]
pub struct V41EngramHasher {
    map: Vec<u32>,
    pad: u32,
    /// Per Engram layer: 4 multipliers, 24 primes, 24 offsets.
    params: Vec<[u64; 52]>,
    heads: usize,
    history: [u32; 3],
    count: usize,
}

impl V41EngramHasher {
    /// A hasher for the stage's Engram layers, in the order the stage keeps
    /// them.
    ///
    /// # Errors
    ///
    /// Refuses constants of the wrong size.
    pub fn new(
        token_map: &V41TokenMap,
        hashes: &[V41EngramHash],
        heads: usize,
    ) -> Result<Self, RuntimeError> {
        let mut params = Vec::new();
        for h in hashes {
            if h.primes.len() != V41_ENGRAM_COLUMNS || h.offsets.len() != V41_ENGRAM_COLUMNS {
                return Err(invalid("hash constants"));
            }
            let mut p = [0_u64; 52];
            p[..4].copy_from_slice(&h.multipliers);
            p[4..28].copy_from_slice(&h.primes);
            p[28..52].copy_from_slice(&h.offsets);
            params.push(p);
        }
        if heads == 0 || 3 * heads != V41_ENGRAM_COLUMNS {
            return Err(invalid(format!("{heads} hash heads")));
        }
        Ok(Self {
            map: token_map.ids.clone(),
            pad: token_map.pad,
            params,
            heads,
            history: [0; 3],
            count: 0,
        })
    }

    /// The row ids of the token at `position`: 24 per Engram layer, layer
    /// major. A token outside the vocabulary gives `None`, as the device's
    /// `-1`.
    pub fn push(&mut self, position: usize, token: u32) -> Vec<Option<u64>> {
        if position == 0 {
            self.count = 0;
        }
        let valid = (token as usize) < self.map.len();
        let current = if valid {
            self.map[token as usize]
        } else {
            self.pad
        };
        let mut grams = [u64::from(current); 4];
        for (shift, gram) in grams.iter_mut().enumerate().skip(1) {
            *gram = if shift <= self.count {
                u64::from(self.history[self.count - shift])
            } else {
                u64::from(self.pad)
            };
        }
        if self.count < 3 {
            self.history[self.count] = current;
            self.count += 1;
        } else {
            self.history = [self.history[1], self.history[2], current];
        }
        let mut out = Vec::with_capacity(self.params.len() * V41_ENGRAM_COLUMNS);
        for p in &self.params {
            for col in 0..V41_ENGRAM_COLUMNS {
                let gram = col / self.heads + 1;
                let mut rolling = grams[0].wrapping_mul(p[0]);
                for g in 1..=gram {
                    rolling ^= grams[g].wrapping_mul(p[g]);
                }
                out.push(valid.then(|| rolling % p[4 + col] + p[28 + col]));
            }
        }
        out
    }
}

/// A page-locked table of the step's rows for one Engram layer.
#[derive(Debug)]
pub(crate) struct StagedTable {
    pub(crate) codes: CudaHostBuffer,
    pub(crate) scales: CudaHostBuffer,
    pub(crate) layer: usize,
}

/// A stage's Engram staging: the hasher, a staging table per Engram layer,
/// and the gather ids the graph copies per token.
pub(crate) struct EngramStaging {
    source: Arc<dyn V41EngramRows>,
    hasher: V41EngramHasher,
    pub(crate) tables: Vec<StagedTable>,
    /// `[max_step_tokens, 24]` `i64` ids: token `i`'s rows are staged at
    /// `24 i ..`.
    pub(crate) ids: CudaBuffer,
    width: usize,
    max_tokens: usize,
    /// Rows staged since load, and the time spent.
    pub(crate) rows: u64,
    pub(crate) seconds: f64,
}

impl std::fmt::Debug for EngramStaging {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngramStaging")
            .field(
                "layers",
                &self.tables.iter().map(|t| t.layer).collect::<Vec<_>>(),
            )
            .field("rows", &self.rows)
            .finish_non_exhaustive()
    }
}

/// A raw pointer the row copies share; each task writes its own rows.
#[derive(Clone, Copy)]
struct Out(*mut u8);
// SAFETY: tasks write disjoint rows of a buffer that outlives the loop.
unsafe impl Send for Out {}
unsafe impl Sync for Out {}

impl Out {
    /// The address `offset` bytes in.
    ///
    /// # Safety
    ///
    /// `offset` must lie inside the allocation.
    unsafe fn at(&self, offset: usize) -> *mut u8 {
        unsafe { self.0.add(offset) }
    }
}

impl EngramStaging {
    pub(crate) fn new(
        backend: &mut CudaBackend,
        source: Arc<dyn V41EngramRows>,
        hasher: V41EngramHasher,
        layers: &[usize],
        width: usize,
        max_tokens: usize,
    ) -> Result<Self, RuntimeError> {
        let rows = max_tokens * V41_ENGRAM_COLUMNS;
        let mut tables = Vec::new();
        for layer in layers {
            tables.push(StagedTable {
                codes: backend.host_buffer(rows * width)?,
                scales: backend.host_buffer(rows * (width / 32))?,
                layer: *layer,
            });
        }
        let ids: Vec<u8> = (0..rows as i64).flat_map(i64::to_le_bytes).collect();
        Ok(Self {
            source,
            hasher,
            tables,
            ids: backend.byte_buffer(&ids)?,
            width,
            max_tokens,
            rows: 0,
            seconds: 0.0,
        })
    }

    /// Page-locked bytes of the staging tables.
    pub(crate) fn host_bytes(&self) -> usize {
        self.tables
            .iter()
            .map(|t| t.codes.byte_len() + t.scales.byte_len())
            .sum()
    }

    /// Hashes the step's tokens and copies their rows into the staging
    /// tables, before the step's graph runs.
    pub(crate) fn stage(&mut self, position: usize, tokens: &[u32]) -> Result<(), RuntimeError> {
        if tokens.len() > self.max_tokens {
            return Err(invalid(format!("{} tokens", tokens.len())));
        }
        let started = std::time::Instant::now();
        let ids: Vec<Vec<Option<u64>>> = tokens
            .iter()
            .enumerate()
            .map(|(i, t)| self.hasher.push(position + i, *t))
            .collect();
        let (w, s) = (self.width, self.width / 32);
        let source = &*self.source;
        for (slot, table) in self.tables.iter().enumerate() {
            let (codes, scales) = (
                Out(table.codes.raw_host_ptr().cast()),
                Out(table.scales.raw_host_ptr().cast()),
            );
            if codes.0.is_null() || scales.0.is_null() {
                return Err(invalid("page-locked memory needs the Linux CUDA runtime"));
            }
            let layer = table.layer;
            let jobs: Vec<(usize, u64)> = ids
                .iter()
                .enumerate()
                .flat_map(|(i, row)| {
                    (0..V41_ENGRAM_COLUMNS).filter_map(move |c| {
                        row[slot * V41_ENGRAM_COLUMNS + c].map(|r| (i * V41_ENGRAM_COLUMNS + c, r))
                    })
                })
                .collect();
            let copy = |(at, row): &(usize, u64)| -> Result<(), RuntimeError> {
                let (c, sc) = source.row(layer, *row)?;
                if c.len() != w || sc.len() != s {
                    return Err(invalid(format!(
                        "row {row} of layer {layer} has the wrong size"
                    )));
                }
                // SAFETY: `at` is below `max_tokens × 24`, the staging
                // tables' row count, and each job writes its own row.
                unsafe {
                    std::ptr::copy_nonoverlapping(c.as_ptr(), codes.at(at * w), w);
                    std::ptr::copy_nonoverlapping(sc.as_ptr(), scales.at(at * s), s);
                }
                Ok(())
            };
            if jobs.len() > 256 {
                jobs.par_iter().try_for_each(copy)?;
            } else {
                jobs.iter().try_for_each(copy)?;
            }
            self.rows += jobs.len() as u64;
        }
        self.seconds += started.elapsed().as_secs_f64();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The host hash restarts at position 0 and fills the history with the
    /// pad id before the sequence starts, as the device kernel does.
    #[test]
    fn history_restarts_at_position_zero() {
        let map = V41TokenMap {
            ids: (0..16).map(|i| i * 3 + 1).collect(),
            pad: 2,
        };
        let hash = V41EngramHash {
            layer: 1,
            multipliers: [3, 5, 7, 11],
            primes: (0..24).map(|i| 101 + 2 * i).collect(),
            offsets: (0..24).map(|i| 1000 * i).collect(),
            rows: 30_000,
        };
        let mut a = V41EngramHasher::new(&map, std::slice::from_ref(&hash), 8).unwrap();
        let first: Vec<_> = [4_u32, 5, 6, 7, 8]
            .iter()
            .enumerate()
            .map(|(i, t)| a.push(i, *t))
            .collect();
        let again: Vec<_> = [4_u32, 5, 6, 7, 8]
            .iter()
            .enumerate()
            .map(|(i, t)| a.push(i, *t))
            .collect();
        assert_eq!(first, again);
        // The first token's bigram uses the pad id as its predecessor.
        let pad = u64::from(map.pad);
        let current = u64::from(map.ids[4]);
        let rolling = current * 3 ^ pad * 5;
        assert_eq!(first[0][0], Some(rolling % 101));
        // Out of vocabulary gives no row.
        assert!(a.push(5, 99).iter().all(Option::is_none));
    }
}
