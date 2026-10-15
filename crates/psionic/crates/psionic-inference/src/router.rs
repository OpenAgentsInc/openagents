//! Preparation and calibrated answers shared by both router engines.

use std::{collections::BTreeMap, io::Read, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokenizers::{Tokenizer, TruncationParams, TruncationStrategy};

use crate::{Error, ModelEngine, Result, Tensor, TensorData, Tensors};

pub const CATEGORIES: [&str; 9] = [
    "coding",
    "agentic",
    "math",
    "knowledge",
    "long_context",
    "tool_use",
    "design",
    "summarisation",
    "general",
];
pub const DIFFICULTY: [&str; 5] = ["trivial", "easy", "moderate", "hard", "frontier"];
pub const STAKES: [&str; 4] = ["negligible", "low", "medium", "high"];
pub const FLAGS: [&str; 4] = [
    "needs_tools",
    "needs_vision",
    "needs_long_context",
    "follow_up",
];
pub const MAX_TOKENS: usize = 512;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Artifact {
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RouterManifest {
    pub model: String,
    pub repository: String,
    pub revision: String,
    pub license: String,
    pub temperatures: BTreeMap<String, f32>,
    pub files: BTreeMap<String, Artifact>,
}

pub fn pinned_manifest() -> Result<RouterManifest> {
    Ok(serde_json::from_str(include_str!(
        "../fixtures/s1-router-manifest.json"
    ))?)
}

pub fn verify_native_router(root: &Path) -> Result<()> {
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/s1-router-native.json"))?;
    let actual: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("native.json"))?)?;
    if actual != expected {
        return Err(Error(
            "Converted router configuration does not match its pinned reference".into(),
        ));
    }
    let expected_hash = expected["weights_sha256"]
        .as_str()
        .ok_or_else(|| Error("Missing converted router checksum".into()))?;
    if file_sha256(&root.join("native.safetensors"))? != expected_hash {
        return Err(Error(
            "Converted router weights do not match their pinned reference".into(),
        ));
    }
    Ok(())
}

pub fn file_sha256(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buf = [0_u8; 65536];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

impl RouterManifest {
    pub fn verify(&self, root: &Path) -> Result<()> {
        for (name, expected) in &self.files {
            let path = Path::new(name);
            if path.is_absolute()
                || path
                    .components()
                    .any(|x| !matches!(x, std::path::Component::Normal(_)))
            {
                return Err(Error(
                    "Model file path must stay inside its directory".into(),
                ));
            }
            let path = root.join(path);
            if path.metadata()?.len() != expected.bytes || file_sha256(&path)? != expected.sha256 {
                return Err(Error(format!(
                    "Model file does not match its expected checksum: {name}"
                )));
            }
        }
        for name in ["category", "difficulty", "stakes", "flags"] {
            let temperature = self.temperatures.get(name).copied().unwrap_or(f32::NAN);
            if !temperature.is_finite() || temperature <= 0.0 {
                return Err(Error(format!("Invalid temperature for {name}")));
            }
        }
        Ok(())
    }
}

pub struct RouterTokenizer {
    tokenizer: Tokenizer,
}

impl RouterTokenizer {
    pub fn load(path: &Path) -> Result<Self> {
        let mut tokenizer = Tokenizer::from_file(path).map_err(|e| Error(e.to_string()))?;
        tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: MAX_TOKENS,
                strategy: TruncationStrategy::OnlySecond,
                ..Default::default()
            }))
            .map_err(|e| Error(e.to_string()))?;
        tokenizer.with_padding(None);
        Ok(Self { tokenizer })
    }

    pub fn prepare(&self, request: &str, context: Option<&str>) -> Result<Tensors> {
        let context: String = context
            .unwrap_or("(new conversation)")
            .chars()
            .take(600)
            .collect();
        let encoded = self
            .tokenizer
            .encode((context.as_str(), request), true)
            .map_err(|e| Error(format!("Cannot prepare this request: {e}")))?;
        let ids: Vec<i64> = encoded.get_ids().iter().map(|x| i64::from(*x)).collect();
        let n = ids.len();
        if n == 0 || n > MAX_TOKENS {
            return Err(Error("Request does not fit the router token limit".into()));
        }
        Ok(BTreeMap::from([
            (
                "input_ids".into(),
                Tensor {
                    shape: vec![1, n],
                    data: TensorData::I64(ids),
                },
            ),
            (
                "attention_mask".into(),
                Tensor {
                    shape: vec![1, n],
                    data: TensorData::I64(vec![1; n]),
                },
            ),
        ]))
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Distribution {
    pub label: String,
    pub probabilities: BTreeMap<String, f32>,
    pub expected: Option<f32>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RouterAnswer {
    pub category: Distribution,
    pub difficulty: Distribution,
    pub stakes: Distribution,
    pub flags: BTreeMap<String, f32>,
}

fn logits<'a>(outputs: &'a Tensors, name: &str, count: usize) -> Result<&'a [f32]> {
    let tensor = outputs
        .get(name)
        .ok_or_else(|| Error(format!("Missing model output: {name}")))?;
    if tensor.shape != [1, count] {
        return Err(Error(format!(
            "Unexpected dimensions for model output: {name}"
        )));
    }
    tensor.floats()
}

fn temperature(manifest: &RouterManifest, name: &str) -> Result<f32> {
    manifest
        .temperatures
        .get(name)
        .copied()
        .filter(|t| t.is_finite() && *t > 0.0)
        .ok_or_else(|| Error(format!("Invalid temperature for {name}")))
}

fn distribution(values: &[f32], labels: &[&str], t: f32, ordered: bool) -> Distribution {
    let max = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut probabilities: Vec<f32> = values.iter().map(|x| ((x - max) / t).exp()).collect();
    let sum: f32 = probabilities.iter().sum();
    for p in &mut probabilities {
        *p /= sum;
    }
    // Keep source label order for ties, independently of the map's key order.
    let mut best = 0;
    for i in 1..probabilities.len() {
        if probabilities[i] > probabilities[best] {
            best = i;
        }
    }
    Distribution {
        label: labels[best].into(),
        expected: ordered.then(|| {
            probabilities
                .iter()
                .enumerate()
                .map(|(i, p)| i as f32 * p)
                .sum()
        }),
        probabilities: labels
            .iter()
            .zip(probabilities)
            .map(|(k, v)| ((*k).into(), v))
            .collect(),
    }
}

pub fn answer(outputs: &Tensors, manifest: &RouterManifest) -> Result<RouterAnswer> {
    Ok(RouterAnswer {
        category: distribution(
            logits(outputs, "category", 9)?,
            &CATEGORIES,
            temperature(manifest, "category")?,
            false,
        ),
        difficulty: distribution(
            logits(outputs, "difficulty", 5)?,
            &DIFFICULTY,
            temperature(manifest, "difficulty")?,
            true,
        ),
        stakes: distribution(
            logits(outputs, "stakes", 4)?,
            &STAKES,
            temperature(manifest, "stakes")?,
            true,
        ),
        flags: FLAGS
            .iter()
            .zip(logits(outputs, "flags", 4)?)
            .map(|(name, x)| {
                let t = temperature(manifest, "flags")?;
                Ok(((*name).into(), 1.0 / (1.0 + (-*x / t).exp())))
            })
            .collect::<Result<_>>()?,
    })
}

pub fn classify(
    engine: &mut dyn ModelEngine,
    tokenizer: &RouterTokenizer,
    manifest: &RouterManifest,
    request: &str,
    context: Option<&str>,
) -> Result<RouterAnswer> {
    answer(
        &engine.run(&tokenizer.prepare(request, context)?)?,
        manifest,
    )
}
