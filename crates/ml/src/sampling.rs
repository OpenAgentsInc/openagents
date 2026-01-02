use crate::error::{MlError, Result};
use rand::Rng;

#[derive(Debug, Clone)]
pub struct GenerationConfig {
    pub max_new_tokens: usize,
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: usize,
    pub repetition_penalty: f32,
    pub stop_tokens: Vec<u32>,
    pub seed: Option<u64>,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            max_new_tokens: 256,
            temperature: 0.7,
            top_p: 0.9,
            top_k: 50,
            repetition_penalty: 1.1,
            stop_tokens: Vec::new(),
            seed: None,
        }
    }
}

pub fn sample_from_logits(
    logits: &[f32],
    config: &GenerationConfig,
    prev_tokens: &[u32],
    rng: &mut impl Rng,
) -> Result<u32> {
    if logits.is_empty() {
        return Err(MlError::Model("empty logits".to_string()));
    }

    let mut scores = logits.to_vec();
    apply_repetition_penalty(&mut scores, config.repetition_penalty, prev_tokens);

    if config.temperature > 0.0 {
        for score in &mut scores {
            *score /= config.temperature;
        }
    }

    if config.temperature <= 0.0 {
        return Ok(argmax(&scores));
    }

    let probs = softmax(&scores);
    let filtered = filter_top_k_top_p(&probs, config.top_k, config.top_p);
    if filtered.is_empty() {
        return Ok(argmax(&probs));
    }

    let mut cumsum = 0.0f32;
    let r: f32 = rng.random();
    for (idx, p) in &filtered {
        cumsum += p;
        if cumsum >= r {
            return Ok(*idx as u32);
        }
    }

    Ok(filtered.last().map(|(idx, _)| *idx as u32).unwrap_or(0))
}

pub(crate) fn apply_repetition_penalty(logits: &mut [f32], penalty: f32, prev_tokens: &[u32]) {
    if penalty <= 1.0 || prev_tokens.is_empty() {
        return;
    }

    for &token in prev_tokens {
        let idx = token as usize;
        if idx >= logits.len() {
            continue;
        }
        let value = logits[idx];
        logits[idx] = if value >= 0.0 { value / penalty } else { value * penalty };
    }
}

pub(crate) fn softmax(scores: &[f32]) -> Vec<f32> {
    let max = scores
        .iter()
        .cloned()
        .fold(f32::NEG_INFINITY, f32::max);
    let exp: Vec<f32> = scores.iter().map(|v| (v - max).exp()).collect();
    let sum: f32 = exp.iter().sum();
    if sum <= 0.0 {
        return vec![1.0 / scores.len() as f32; scores.len()];
    }
    exp.iter().map(|v| v / sum).collect()
}

fn argmax(values: &[f32]) -> u32 {
    values
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(idx, _)| idx as u32)
        .unwrap_or(0)
}

fn filter_top_k_top_p(probs: &[f32], top_k: usize, top_p: f32) -> Vec<(usize, f32)> {
    let mut indexed: Vec<(usize, f32)> = probs.iter().cloned().enumerate().collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut keep = indexed;
    if top_k > 0 && top_k < keep.len() {
        keep.truncate(top_k);
    }

    if top_p > 0.0 && top_p < 1.0 {
        let mut cumsum = 0.0f32;
        let mut cutoff = keep.len();
        for (idx, (_, prob)) in keep.iter().enumerate() {
            cumsum += prob;
            if cumsum >= top_p {
                cutoff = idx + 1;
                break;
            }
        }
        keep.truncate(cutoff);
    }

    let sum: f32 = keep.iter().map(|(_, p)| p).sum();
    if sum <= 0.0 {
        return Vec::new();
    }

    keep.into_iter().map(|(i, p)| (i, p / sum)).collect()
}
