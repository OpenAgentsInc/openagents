//! Static embeddings for local snippet and note search.

use super::{ModelFamily, floats, identity, load_engine, pool};
use crate::{
    EngineKind, Error, ExecutionIdentity, ModelEngine, Result, Tensor, TensorData, Tensors,
};
use rayon::prelude::*;
use std::{collections::BTreeMap, path::Path};

pub const WIDTH: usize = 64;
const VOCAB: usize = 29528;

pub struct PotionEngine {
    weights: Vec<f32>,
    pool: rayon::ThreadPool,
    identity: ExecutionIdentity,
}

impl PotionEngine {
    pub(super) fn load_verified(root: &Path, threads: usize) -> Result<Self> {
        let pool = pool(threads)?;
        let bytes = std::fs::read(root.join("model.safetensors"))?;
        let tensors =
            safetensors::SafeTensors::deserialize(&bytes).map_err(|e| Error(e.to_string()))?;
        Ok(Self {
            weights: floats(&tensors, "embeddings", &[VOCAB, WIDTH])?,
            pool,
            identity: identity("psionic-potion-cpu", threads),
        })
    }

    fn forward(&self, inputs: &Tensors) -> Result<Tensors> {
        if inputs.len() != 2 {
            return Err(Error(
                "The embedding model requires input_ids and offsets".into(),
            ));
        }
        let ids = inputs
            .get("input_ids")
            .ok_or_else(|| Error("Missing input_ids".into()))?;
        let offsets = inputs
            .get("offsets")
            .ok_or_else(|| Error("Missing offsets".into()))?;
        if ids.shape.len() != 1 || offsets.shape.len() != 1 {
            return Err(Error(
                "Embedding input_ids and offsets must be one-dimensional".into(),
            ));
        }
        let ids = ids.integers()?;
        let offsets = offsets.integers()?;
        if ids.len() > 65536
            || offsets.len() > 256
            || ids.iter().any(|i| *i < 0 || *i as usize >= VOCAB)
            || offsets.first().is_some_and(|o| *o != 0)
            || offsets.iter().any(|o| *o < 0 || *o as usize > ids.len())
            || offsets.windows(2).any(|o| o[0] > o[1])
            || (offsets.is_empty() && !ids.is_empty())
        {
            return Err(Error("Invalid embedding token IDs or offsets".into()));
        }
        let mut result = vec![0.0; offsets.len() * WIDTH];
        result
            .par_chunks_mut(WIDTH)
            .enumerate()
            .for_each(|(i, out)| {
                let start = offsets[i] as usize;
                let end = offsets.get(i + 1).map_or(ids.len(), |v| *v as usize);
                if start == end {
                    return;
                }
                for id in &ids[start..end] {
                    for (j, value) in out.iter_mut().enumerate() {
                        *value += self.weights[*id as usize * WIDTH + j];
                    }
                }
                for value in out.iter_mut() {
                    *value /= (end - start) as f32;
                }
                let norm = out.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-12);
                for value in out {
                    *value /= norm;
                }
            });
        Ok(BTreeMap::from([(
            "embeddings".into(),
            Tensor {
                shape: vec![offsets.len(), WIDTH],
                data: TensorData::F32(result),
            },
        )]))
    }
}

impl ModelEngine for PotionEngine {
    fn identity(&self) -> &ExecutionIdentity {
        &self.identity
    }
    fn run(&mut self, inputs: &Tensors) -> Result<Tensors> {
        self.pool.install(|| self.forward(inputs))
    }
}

pub struct PotionModel {
    engine: Box<dyn ModelEngine>,
    tokenizer: tokenizers::Tokenizer,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct RankedText {
    pub index: usize,
    pub similarity: f32,
}

impl PotionModel {
    pub fn load(
        root: &Path,
        kind: EngineKind,
        threads: usize,
        library: Option<&Path>,
        checksum: Option<&str>,
    ) -> Result<Self> {
        let engine = load_engine(ModelFamily::Potion, kind, root, threads, library, checksum)?;
        let mut tokenizer = tokenizers::Tokenizer::from_file(root.join("tokenizer.json"))
            .map_err(|e| Error(e.to_string()))?;
        tokenizer
            .with_truncation(None)
            .map_err(|e| Error(e.to_string()))?;
        tokenizer.with_padding(None);
        Ok(Self { engine, tokenizer })
    }

    pub fn identity(&self) -> &ExecutionIdentity {
        self.engine.identity()
    }

    pub fn prepare(&self, texts: &[&str]) -> Result<Tensors> {
        if texts.len() > 256 || texts.iter().map(|s| s.len()).sum::<usize>() > 128 * 1024 {
            return Err(Error("This embedding batch is too large".into()));
        }
        let mut ids = Vec::new();
        let mut offsets = Vec::with_capacity(texts.len());
        for text in texts {
            offsets.push(ids.len() as i64);
            ids.extend(
                self.tokenizer
                    .encode(*text, false)
                    .map_err(|e| Error(e.to_string()))?
                    .get_ids()
                    .iter()
                    .map(|v| i64::from(*v)),
            );
            if ids.len() > 65536 {
                return Err(Error("This embedding batch has too many tokens".into()));
            }
        }
        Ok(BTreeMap::from([
            (
                "input_ids".into(),
                Tensor {
                    shape: vec![ids.len()],
                    data: TensorData::I64(ids),
                },
            ),
            (
                "offsets".into(),
                Tensor {
                    shape: vec![offsets.len()],
                    data: TensorData::I64(offsets),
                },
            ),
        ]))
    }

    pub fn encode(&mut self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let inputs = self.prepare(texts)?;
        let outputs = self.engine.run(&inputs)?;
        let output = outputs
            .get("embeddings")
            .ok_or_else(|| Error("Missing embedding output".into()))?;
        if output.shape != [texts.len(), WIDTH] {
            return Err(Error("Unexpected embedding dimensions".into()));
        }
        Ok(output
            .floats()?
            .chunks(WIDTH)
            .map(<[f32]>::to_vec)
            .collect())
    }

    pub fn rank(&mut self, query: &str, candidates: &[&str]) -> Result<Vec<RankedText>> {
        let texts: Vec<&str> = std::iter::once(query)
            .chain(candidates.iter().copied())
            .collect();
        let embeddings = self.encode(&texts)?;
        let mut ranked: Vec<RankedText> = embeddings[1..]
            .iter()
            .enumerate()
            .map(|(index, embedding)| RankedText {
                index,
                similarity: embeddings[0]
                    .iter()
                    .zip(embedding)
                    .map(|(a, b)| a * b)
                    .sum(),
            })
            .collect();
        ranked.sort_by(|a, b| {
            b.similarity
                .total_cmp(&a.similarity)
                .then(a.index.cmp(&b.index))
        });
        Ok(ranked)
    }
}
