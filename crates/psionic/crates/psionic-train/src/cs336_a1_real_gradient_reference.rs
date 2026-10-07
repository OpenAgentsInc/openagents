use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable lane identifier for the bounded real-gradient A1 reference lane.
pub const CS336_A1_REAL_GRADIENT_LANE_ID: &str = "psion_cs336_a1_real_gradient_reference_v1";
/// Claim boundary for the bounded real-gradient A1 reference lane.
pub const CS336_A1_REAL_GRADIENT_CLAIM_BOUNDARY: &str = "a bounded f64 reference trainer \
     proving analytic-gradient correctness on the A1 architecture shape (embedding, RMSNorm, \
     single-head causal attention, SwiGLU, cross-entropy) at tiny scale, gradient-checked \
     against central differences; no scalable-pretraining claim, no GPU claim, and no \
     promotion into the actual-pretraining operator lane";

/// Model dimensions for one bounded run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A1RealGradConfig {
    /// Vocabulary size.
    pub vocab_size: usize,
    /// Model width.
    pub d_model: usize,
    /// SwiGLU hidden width.
    pub d_ff: usize,
    /// Sequence length per training example.
    pub seq_len: usize,
}

/// Failure for one bounded real-gradient computation.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum Cs336A1RealGradError {
    /// A dimension is zero or a token is out of range.
    #[error("invalid configuration or token: {detail}")]
    Invalid {
        /// Plain-language reason.
        detail: &'static str,
    },
}

/// One full parameter set, stored as flat row-major matrices.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A1RealGradParams {
    /// Token embedding, `vocab_size x d_model`.
    pub embedding: Vec<f64>,
    /// First RMSNorm gain, `d_model`.
    pub norm1_gain: Vec<f64>,
    /// Query projection, `d_model x d_model`.
    pub w_query: Vec<f64>,
    /// Key projection, `d_model x d_model`.
    pub w_key: Vec<f64>,
    /// Value projection, `d_model x d_model`.
    pub w_value: Vec<f64>,
    /// Output projection, `d_model x d_model`.
    pub w_output: Vec<f64>,
    /// Second RMSNorm gain, `d_model`.
    pub norm2_gain: Vec<f64>,
    /// SwiGLU gate projection, `d_model x d_ff`.
    pub w_gate: Vec<f64>,
    /// SwiGLU value projection, `d_model x d_ff`.
    pub w_up: Vec<f64>,
    /// SwiGLU down projection, `d_ff x d_model`.
    pub w_down: Vec<f64>,
    /// Unembedding, `d_model x vocab_size`.
    pub unembedding: Vec<f64>,
}

impl Cs336A1RealGradParams {
    /// Deterministically initializes parameters from one seed.
    #[must_use]
    pub fn seeded(config: &Cs336A1RealGradConfig, seed: u64) -> Self {
        let mut state = seed;
        let mut next = |count: usize, scale: f64| -> Vec<f64> {
            (0..count)
                .map(|_| {
                    state = state
                        .wrapping_add(0x9E37_79B9_7F4A_7C15)
                        .rotate_left(13)
                        .wrapping_mul(0xBF58_476D_1CE4_E5B9);
                    let unit = (state >> 11) as f64 / (1_u64 << 53) as f64;
                    (unit - 0.5) * 2.0 * scale
                })
                .collect()
        };
        let d = config.d_model;
        let scale = 1.0 / (d as f64).sqrt();
        Self {
            embedding: next(config.vocab_size * d, scale),
            norm1_gain: vec![1.0; d],
            w_query: next(d * d, scale),
            w_key: next(d * d, scale),
            w_value: next(d * d, scale),
            w_output: next(d * d, scale),
            norm2_gain: vec![1.0; d],
            w_gate: next(d * config.d_ff, scale),
            w_up: next(d * config.d_ff, scale),
            w_down: next(config.d_ff * d, scale),
            unembedding: next(d * config.vocab_size, scale),
        }
    }

    fn tensors_mut(&mut self) -> Vec<(&'static str, &mut Vec<f64>)> {
        vec![
            ("embedding", &mut self.embedding),
            ("norm1_gain", &mut self.norm1_gain),
            ("w_query", &mut self.w_query),
            ("w_key", &mut self.w_key),
            ("w_value", &mut self.w_value),
            ("w_output", &mut self.w_output),
            ("norm2_gain", &mut self.norm2_gain),
            ("w_gate", &mut self.w_gate),
            ("w_up", &mut self.w_up),
            ("w_down", &mut self.w_down),
            ("unembedding", &mut self.unembedding),
        ]
    }
}

/// Gradients with the same layout as the parameters.
pub type Cs336A1RealGradGradients = Cs336A1RealGradParams;

const RMS_EPSILON: f64 = 1e-6;

fn matmul(a: &[f64], b: &[f64], rows: usize, inner: usize, cols: usize) -> Vec<f64> {
    let mut out = vec![0.0; rows * cols];
    for r in 0..rows {
        for i in 0..inner {
            let av = a[r * inner + i];
            if av == 0.0 {
                continue;
            }
            for c in 0..cols {
                out[r * cols + c] += av * b[i * cols + c];
            }
        }
    }
    out
}

fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

struct ForwardState {
    h0: Vec<f64>,
    n1: Vec<f64>,
    r1: Vec<f64>,
    q: Vec<f64>,
    k: Vec<f64>,
    v: Vec<f64>,
    attn_weights: Vec<f64>,
    attn_mix: Vec<f64>,
    h1: Vec<f64>,
    n2: Vec<f64>,
    r2: Vec<f64>,
    gate_pre: Vec<f64>,
    up_pre: Vec<f64>,
    ffn_hidden: Vec<f64>,
    logits: Vec<f64>,
    probs: Vec<f64>,
}

fn rmsnorm_forward(x: &[f64], gain: &[f64], rows: usize, d: usize) -> (Vec<f64>, Vec<f64>) {
    let mut out = vec![0.0; rows * d];
    let mut inv_rms = vec![0.0; rows];
    for r in 0..rows {
        let row = &x[r * d..(r + 1) * d];
        let mean_square = row.iter().map(|v| v * v).sum::<f64>() / d as f64;
        let inv = 1.0 / (mean_square + RMS_EPSILON).sqrt();
        inv_rms[r] = inv;
        for c in 0..d {
            out[r * d + c] = row[c] * inv * gain[c];
        }
    }
    (out, inv_rms)
}

fn rmsnorm_backward(
    x: &[f64],
    gain: &[f64],
    inv_rms: &[f64],
    grad_out: &[f64],
    rows: usize,
    d: usize,
    grad_x: &mut [f64],
    grad_gain: &mut [f64],
) {
    for r in 0..rows {
        let row = &x[r * d..(r + 1) * d];
        let gout = &grad_out[r * d..(r + 1) * d];
        let inv = inv_rms[r];
        let mut dot = 0.0;
        for c in 0..d {
            dot += gout[c] * gain[c] * row[c];
            grad_gain[c] += gout[c] * row[c] * inv;
        }
        let scale = dot * inv * inv * inv / d as f64;
        for c in 0..d {
            grad_x[r * d + c] += gout[c] * gain[c] * inv - row[c] * scale;
        }
    }
}

fn forward(
    config: &Cs336A1RealGradConfig,
    params: &Cs336A1RealGradParams,
    tokens: &[usize],
) -> Result<(ForwardState, f64), Cs336A1RealGradError> {
    let d = config.d_model;
    let t_count = config.seq_len;
    if tokens.len() != t_count + 1 {
        return Err(Cs336A1RealGradError::Invalid {
            detail: "tokens must hold seq_len + 1 entries",
        });
    }
    if tokens.iter().any(|t| *t >= config.vocab_size) {
        return Err(Cs336A1RealGradError::Invalid {
            detail: "token out of vocabulary range",
        });
    }
    // Embedding.
    let mut h0 = vec![0.0; t_count * d];
    for t in 0..t_count {
        h0[t * d..(t + 1) * d]
            .copy_from_slice(&params.embedding[tokens[t] * d..(tokens[t] + 1) * d]);
    }
    // Pre-attention RMSNorm.
    let (n1, r1) = rmsnorm_forward(&h0, &params.norm1_gain, t_count, d);
    // Single-head causal attention.
    let q = matmul(&n1, &params.w_query, t_count, d, d);
    let k = matmul(&n1, &params.w_key, t_count, d, d);
    let v = matmul(&n1, &params.w_value, t_count, d, d);
    let inv_sqrt_d = 1.0 / (d as f64).sqrt();
    let mut attn_weights = vec![0.0; t_count * t_count];
    for t in 0..t_count {
        let mut max_score = f64::NEG_INFINITY;
        let mut scores = vec![f64::NEG_INFINITY; t_count];
        for (s, score_slot) in scores.iter_mut().enumerate().take(t + 1) {
            let mut score = 0.0;
            for c in 0..d {
                score += q[t * d + c] * k[s * d + c];
            }
            let score = score * inv_sqrt_d;
            *score_slot = score;
            max_score = max_score.max(score);
        }
        let mut total = 0.0;
        for value in scores.iter_mut().take(t + 1) {
            *value = (*value - max_score).exp();
            total += *value;
        }
        for (s, value) in scores.iter().enumerate().take(t + 1) {
            attn_weights[t * t_count + s] = value / total;
        }
    }
    let attn_mix = matmul(&attn_weights, &v, t_count, t_count, d);
    let attn_out = matmul(&attn_mix, &params.w_output, t_count, d, d);
    let mut h1 = h0.clone();
    for i in 0..t_count * d {
        h1[i] += attn_out[i];
    }
    // Pre-FFN RMSNorm and SwiGLU.
    let (n2, r2) = rmsnorm_forward(&h1, &params.norm2_gain, t_count, d);
    let gate_pre = matmul(&n2, &params.w_gate, t_count, d, config.d_ff);
    let up_pre = matmul(&n2, &params.w_up, t_count, d, config.d_ff);
    let mut ffn_hidden = vec![0.0; t_count * config.d_ff];
    for i in 0..ffn_hidden.len() {
        let silu = gate_pre[i] * sigmoid(gate_pre[i]);
        ffn_hidden[i] = silu * up_pre[i];
    }
    let ffn_out = matmul(&ffn_hidden, &params.w_down, t_count, config.d_ff, d);
    let mut h2 = h1.clone();
    for i in 0..t_count * d {
        h2[i] += ffn_out[i];
    }
    // Unembedding and mean next-token cross-entropy.
    let logits = matmul(&h2, &params.unembedding, t_count, d, config.vocab_size);
    let mut probs = vec![0.0; t_count * config.vocab_size];
    let mut loss = 0.0;
    for t in 0..t_count {
        let row = &logits[t * config.vocab_size..(t + 1) * config.vocab_size];
        let max_logit = row.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let mut total = 0.0;
        for (c, value) in row.iter().enumerate() {
            let e = (value - max_logit).exp();
            probs[t * config.vocab_size + c] = e;
            total += e;
        }
        for c in 0..config.vocab_size {
            probs[t * config.vocab_size + c] /= total;
        }
        loss -= probs[t * config.vocab_size + tokens[t + 1]].ln();
    }
    loss /= t_count as f64;
    Ok((
        ForwardState {
            h0,
            n1,
            r1,
            q,
            k,
            v,
            attn_weights,
            attn_mix,
            h1,
            n2,
            r2,
            gate_pre,
            up_pre,
            ffn_hidden,
            logits,
            probs,
        },
        loss,
    ))
}

/// Computes the mean next-token cross-entropy loss.
pub fn cs336_a1_real_grad_loss(
    config: &Cs336A1RealGradConfig,
    params: &Cs336A1RealGradParams,
    tokens: &[usize],
) -> Result<f64, Cs336A1RealGradError> {
    forward(config, params, tokens).map(|(_, loss)| loss)
}

/// Computes the loss and analytic gradients for every parameter tensor.
pub fn cs336_a1_real_grad_backward(
    config: &Cs336A1RealGradConfig,
    params: &Cs336A1RealGradParams,
    tokens: &[usize],
) -> Result<(f64, Cs336A1RealGradGradients), Cs336A1RealGradError> {
    let d = config.d_model;
    let t_count = config.seq_len;
    let vocab = config.vocab_size;
    let d_ff = config.d_ff;
    let (state, loss) = forward(config, params, tokens)?;
    let mut grads = Cs336A1RealGradParams {
        embedding: vec![0.0; vocab * d],
        norm1_gain: vec![0.0; d],
        w_query: vec![0.0; d * d],
        w_key: vec![0.0; d * d],
        w_value: vec![0.0; d * d],
        w_output: vec![0.0; d * d],
        norm2_gain: vec![0.0; d],
        w_gate: vec![0.0; d * d_ff],
        w_up: vec![0.0; d * d_ff],
        w_down: vec![0.0; d_ff * d],
        unembedding: vec![0.0; d * vocab],
    };
    // d loss / d logits = (probs - onehot) / t_count.
    let mut grad_logits = state.probs.clone();
    for t in 0..t_count {
        grad_logits[t * vocab + tokens[t + 1]] -= 1.0;
    }
    for value in &mut grad_logits {
        *value /= t_count as f64;
    }
    // Unembedding: logits = h2 U.
    let mut h2 = state.h1.clone();
    {
        let ffn_out = matmul(&state.ffn_hidden, &params.w_down, t_count, d_ff, d);
        for i in 0..t_count * d {
            h2[i] += ffn_out[i];
        }
    }
    let mut grad_h2 = vec![0.0; t_count * d];
    for t in 0..t_count {
        for c in 0..vocab {
            let g = grad_logits[t * vocab + c];
            if g == 0.0 {
                continue;
            }
            for i in 0..d {
                grads.unembedding[i * vocab + c] += h2[t * d + i] * g;
                grad_h2[t * d + i] += params.unembedding[i * vocab + c] * g;
            }
        }
    }
    // FFN residual: h2 = h1 + ffn_hidden W_down.
    let grad_h1_from_residual = grad_h2.clone();
    let mut grad_ffn_hidden = vec![0.0; t_count * d_ff];
    for t in 0..t_count {
        for i in 0..d_ff {
            let hv = state.ffn_hidden[t * d_ff + i];
            for c in 0..d {
                let g = grad_h2[t * d + c];
                grads.w_down[i * d + c] += hv * g;
                grad_ffn_hidden[t * d_ff + i] += params.w_down[i * d + c] * g;
            }
        }
    }
    // SwiGLU: hidden = silu(gate_pre) * up_pre.
    let mut grad_gate_pre = vec![0.0; t_count * d_ff];
    let mut grad_up_pre = vec![0.0; t_count * d_ff];
    for i in 0..t_count * d_ff {
        let a = state.gate_pre[i];
        let s = sigmoid(a);
        let silu = a * s;
        let silu_prime = s * (1.0 + a * (1.0 - s));
        grad_gate_pre[i] = grad_ffn_hidden[i] * state.up_pre[i] * silu_prime;
        grad_up_pre[i] = grad_ffn_hidden[i] * silu;
    }
    // gate_pre = n2 W_gate; up_pre = n2 W_up.
    let mut grad_n2 = vec![0.0; t_count * d];
    for t in 0..t_count {
        for i in 0..d {
            let nv = state.n2[t * d + i];
            for c in 0..d_ff {
                grads.w_gate[i * d_ff + c] += nv * grad_gate_pre[t * d_ff + c];
                grads.w_up[i * d_ff + c] += nv * grad_up_pre[t * d_ff + c];
                grad_n2[t * d + i] += params.w_gate[i * d_ff + c] * grad_gate_pre[t * d_ff + c]
                    + params.w_up[i * d_ff + c] * grad_up_pre[t * d_ff + c];
            }
        }
    }
    // Second RMSNorm.
    let mut grad_h1 = grad_h1_from_residual;
    rmsnorm_backward(
        &state.h1,
        &params.norm2_gain,
        &state.r2,
        &grad_n2,
        t_count,
        d,
        &mut grad_h1,
        &mut grads.norm2_gain,
    );
    // Attention residual: h1 = h0 + attn_mix W_output.
    let grad_h0_from_residual = grad_h1.clone();
    let mut grad_attn_mix = vec![0.0; t_count * d];
    for t in 0..t_count {
        for i in 0..d {
            let mv = state.attn_mix[t * d + i];
            for c in 0..d {
                let g = grad_h1[t * d + c];
                grads.w_output[i * d + c] += mv * g;
                grad_attn_mix[t * d + i] += params.w_output[i * d + c] * g;
            }
        }
    }
    // attn_mix = attn_weights V.
    let mut grad_attn_weights = vec![0.0; t_count * t_count];
    let mut grad_v = vec![0.0; t_count * d];
    for t in 0..t_count {
        for s in 0..=t {
            let w = state.attn_weights[t * t_count + s];
            let mut dot = 0.0;
            for c in 0..d {
                let g = grad_attn_mix[t * d + c];
                dot += g * state.v[s * d + c];
                grad_v[s * d + c] += w * g;
            }
            grad_attn_weights[t * t_count + s] = dot;
        }
    }
    // Softmax backward over each causal row.
    let inv_sqrt_d = 1.0 / (d as f64).sqrt();
    let mut grad_scores = vec![0.0; t_count * t_count];
    for t in 0..t_count {
        let mut weighted = 0.0;
        for s in 0..=t {
            weighted += grad_attn_weights[t * t_count + s] * state.attn_weights[t * t_count + s];
        }
        for s in 0..=t {
            let w = state.attn_weights[t * t_count + s];
            grad_scores[t * t_count + s] =
                w * (grad_attn_weights[t * t_count + s] - weighted) * inv_sqrt_d;
        }
    }
    // scores = q k^T (scaled inside grad_scores).
    let mut grad_q = vec![0.0; t_count * d];
    let mut grad_k = vec![0.0; t_count * d];
    for t in 0..t_count {
        for s in 0..=t {
            let g = grad_scores[t * t_count + s];
            if g == 0.0 {
                continue;
            }
            for c in 0..d {
                grad_q[t * d + c] += g * state.k[s * d + c];
                grad_k[s * d + c] += g * state.q[t * d + c];
            }
        }
    }
    // q = n1 Wq, k = n1 Wk, v = n1 Wv.
    let mut grad_n1 = vec![0.0; t_count * d];
    for t in 0..t_count {
        for i in 0..d {
            let nv = state.n1[t * d + i];
            for c in 0..d {
                grads.w_query[i * d + c] += nv * grad_q[t * d + c];
                grads.w_key[i * d + c] += nv * grad_k[t * d + c];
                grads.w_value[i * d + c] += nv * grad_v[t * d + c];
                grad_n1[t * d + i] += params.w_query[i * d + c] * grad_q[t * d + c]
                    + params.w_key[i * d + c] * grad_k[t * d + c]
                    + params.w_value[i * d + c] * grad_v[t * d + c];
            }
        }
    }
    // First RMSNorm.
    let mut grad_h0 = grad_h0_from_residual;
    rmsnorm_backward(
        &state.h0,
        &params.norm1_gain,
        &state.r1,
        &grad_n1,
        t_count,
        d,
        &mut grad_h0,
        &mut grads.norm1_gain,
    );
    // Embedding rows.
    for t in 0..t_count {
        let token = tokens[t];
        for c in 0..d {
            grads.embedding[token * d + c] += grad_h0[t * d + c];
        }
    }
    let _ = (&state.logits, &state.h0);
    Ok((loss, grads))
}

/// One digest-pinned training report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A1RealGradTrainingReport {
    /// Lane identifier.
    pub lane_id: String,
    /// Steps trained.
    pub steps: usize,
    /// Loss before training.
    pub initial_loss: f64,
    /// Loss after training.
    pub final_loss: f64,
}

impl Cs336A1RealGradTrainingReport {
    /// Returns a stable digest over the report encoding.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"cs336_a1_real_grad_training_report|");
        hasher.update(serde_json::to_vec(self).unwrap_or_default());
        hex::encode(hasher.finalize())
    }
}

/// Trains with inline AdamW on one repeated token sequence.
pub fn cs336_a1_real_grad_train(
    config: &Cs336A1RealGradConfig,
    params: &mut Cs336A1RealGradParams,
    tokens: &[usize],
    steps: usize,
    learning_rate: f64,
) -> Result<Cs336A1RealGradTrainingReport, Cs336A1RealGradError> {
    let initial_loss = cs336_a1_real_grad_loss(config, params, tokens)?;
    let (beta1, beta2, eps, weight_decay): (f64, f64, f64, f64) = (0.9, 0.999, 1e-8, 0.01);
    let mut first_moments: Vec<Vec<f64>> = Vec::new();
    let mut second_moments: Vec<Vec<f64>> = Vec::new();
    for (_, tensor) in params.clone().tensors_mut() {
        first_moments.push(vec![0.0; tensor.len()]);
        second_moments.push(vec![0.0; tensor.len()]);
    }
    let mut final_loss = initial_loss;
    for step in 1..=steps {
        let (loss, mut grads) = cs336_a1_real_grad_backward(config, params, tokens)?;
        final_loss = loss;
        let bias1 = 1.0 - beta1.powi(step as i32);
        let bias2 = 1.0 - beta2.powi(step as i32);
        for (slot, ((_, parameter), (_, gradient))) in params
            .tensors_mut()
            .into_iter()
            .zip(grads.tensors_mut())
            .enumerate()
        {
            for i in 0..parameter.len() {
                let g = gradient[i];
                first_moments[slot][i] = beta1 * first_moments[slot][i] + (1.0 - beta1) * g;
                second_moments[slot][i] = beta2 * second_moments[slot][i] + (1.0 - beta2) * g * g;
                let m_hat = first_moments[slot][i] / bias1;
                let v_hat = second_moments[slot][i] / bias2;
                parameter[i] -=
                    learning_rate * (m_hat / (v_hat.sqrt() + eps) + weight_decay * parameter[i]);
            }
        }
    }
    Ok(Cs336A1RealGradTrainingReport {
        lane_id: CS336_A1_REAL_GRADIENT_LANE_ID.to_string(),
        steps,
        initial_loss,
        final_loss,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn tiny_config() -> Cs336A1RealGradConfig {
        Cs336A1RealGradConfig {
            vocab_size: 11,
            d_model: 6,
            d_ff: 10,
            seq_len: 5,
        }
    }

    fn tiny_tokens() -> Vec<usize> {
        vec![1, 4, 2, 7, 3, 9]
    }

    /// The load-bearing bar: every parameter tensor's analytic gradient
    /// matches central differences at tight f64 tolerance.
    #[test]
    fn analytic_gradients_match_central_differences_for_every_tensor() {
        let config = tiny_config();
        let tokens = tiny_tokens();
        let params = Cs336A1RealGradParams::seeded(&config, 0xC5336A1);
        let (_, grads) = cs336_a1_real_grad_backward(&config, &params, &tokens).expect("backward");
        let epsilon = 1e-5;
        let mut grad_copy = grads.clone();
        let analytic = grad_copy.tensors_mut();
        for (tensor_index, (name, analytic_tensor)) in analytic.into_iter().enumerate() {
            // Spot-check up to 8 entries per tensor to keep runtime bounded.
            let stride = (analytic_tensor.len() / 8).max(1);
            for i in (0..analytic_tensor.len()).step_by(stride) {
                let mut plus = params.clone();
                let mut minus = params.clone();
                plus.tensors_mut()[tensor_index].1[i] += epsilon;
                minus.tensors_mut()[tensor_index].1[i] -= epsilon;
                let loss_plus = cs336_a1_real_grad_loss(&config, &plus, &tokens).expect("loss+");
                let loss_minus = cs336_a1_real_grad_loss(&config, &minus, &tokens).expect("loss-");
                let numeric = (loss_plus - loss_minus) / (2.0 * epsilon);
                let denominator = numeric.abs().max(analytic_tensor[i].abs()).max(1e-8);
                let relative = (numeric - analytic_tensor[i]).abs() / denominator;
                assert!(
                    relative < 1e-5,
                    "{name}[{i}]: analytic {} vs numeric {numeric} (rel {relative})",
                    analytic_tensor[i]
                );
            }
        }
    }

    #[test]
    fn training_decreases_the_loss() {
        let config = tiny_config();
        let tokens = tiny_tokens();
        let mut params = Cs336A1RealGradParams::seeded(&config, 0xC5336A1);
        let report =
            cs336_a1_real_grad_train(&config, &mut params, &tokens, 40, 0.01).expect("trains");
        assert!(
            report.final_loss < report.initial_loss * 0.5,
            "report: {report:?}"
        );
        assert!(report.final_loss.is_finite());
    }

    #[test]
    fn training_reports_are_deterministic() {
        let config = tiny_config();
        let tokens = tiny_tokens();
        let mut a_params = Cs336A1RealGradParams::seeded(&config, 7);
        let mut b_params = Cs336A1RealGradParams::seeded(&config, 7);
        let a =
            cs336_a1_real_grad_train(&config, &mut a_params, &tokens, 10, 0.01).expect("trains");
        let b =
            cs336_a1_real_grad_train(&config, &mut b_params, &tokens, 10, 0.01).expect("trains");
        assert_eq!(a.stable_digest(), b.stable_digest());
        assert_eq!(a_params, b_params);
    }

    #[test]
    fn invalid_inputs_refuse() {
        let config = tiny_config();
        let params = Cs336A1RealGradParams::seeded(&config, 1);
        assert!(matches!(
            cs336_a1_real_grad_loss(&config, &params, &[1, 2]),
            Err(Cs336A1RealGradError::Invalid { .. })
        ));
        assert!(matches!(
            cs336_a1_real_grad_loss(&config, &params, &[1, 2, 3, 4, 5, 99]),
            Err(Cs336A1RealGradError::Invalid { .. })
        ));
    }
}
