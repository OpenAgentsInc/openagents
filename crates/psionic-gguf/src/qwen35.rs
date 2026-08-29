//! Hybrid Qwen 3.8 graph: Gated DeltaNet + gated full attention + FFN.
//!
//! Ported as a leaf from the sibling Psionic CPU path. Does not import
//! `psionic-serve`. Q8 GEMMs use Metal when the session binds a shared buffer.

use crate::format::GgufMeta;
use crate::generate::{decode_row, lookup, matvec, matvec_many};
use crate::mmap::MappedWeights;
use crate::tokenizer::TokenizerTables;

pub const GRAPH_HYBRID: &str = "qwen35_hybrid";
pub const GRAPH_STUB: &str = "embed_lmhead";

#[derive(Clone, Debug)]
pub struct DecodeState {
    pub position: usize,
    layers: Vec<LayerState>,
}

#[derive(Clone, Debug)]
enum LayerState {
    Hybrid {
        conv: Vec<f32>,
        delta: Vec<f32>,
    },
    Full {
        keys: Vec<Vec<f32>>,
        values: Vec<Vec<f32>>,
    },
    Skip,
}

#[derive(Clone, Copy)]
struct Shapes {
    hidden: usize,
    n_layers: usize,
    interval: usize,
    head_count: usize,
    kv_heads: usize,
    head_dim: usize,
    rotary_dim: usize,
    epsilon: f32,
    rope_theta: f32,
    mrope: [usize; 4],
    state_size: usize,
    group_count: usize,
    time_step_rank: usize,
    inner_size: usize,
    conv_kernel: usize,
    v_head_reordered: bool,
}

pub fn has_hybrid_graph(mapped: &MappedWeights) -> bool {
    mapped.tensors.contains_key("blk.0.ffn_down.weight")
}

pub fn graph_name(mapped: &MappedWeights) -> &'static str {
    if has_hybrid_graph(mapped) {
        GRAPH_HYBRID
    } else {
        GRAPH_STUB
    }
}

fn shapes(meta: &GgufMeta) -> Shapes {
    let hidden = meta.kv_u64("qwen35.embedding_length").unwrap_or(8) as usize;
    let block = meta.kv_u64("qwen35.block_count").unwrap_or(1) as usize;
    let nextn = meta
        .kv_u64("qwen35.nextn_predict_layers")
        .or_else(|| meta.kv_u64("qwen35.nextn.predict_layers"))
        .unwrap_or(0) as usize;
    let n_layers = block.saturating_sub(nextn).max(1);
    let interval = meta
        .kv_u64("qwen35.full_attention_interval")
        .unwrap_or(4)
        .max(1) as usize;
    let head_count = meta.kv_u64("qwen35.attention.head_count").unwrap_or(1) as usize;
    let kv_heads = meta
        .kv_u64("qwen35.attention.head_count_kv")
        .unwrap_or(1)
        .max(1) as usize;
    let head_dim = meta
        .kv_u64("qwen35.attention.key_length")
        .map(|v| v as usize)
        .unwrap_or(if hidden >= 5120 {
            256
        } else {
            (hidden / head_count.max(1)).max(1)
        });
    let rotary_dim = meta
        .kv_u64("qwen35.rope.dimension_count")
        .unwrap_or(head_dim as u64) as usize;
    let epsilon = meta
        .kv_f32("qwen35.attention.layer_norm_rms_epsilon")
        .unwrap_or(1e-6);
    let rope_theta = meta.kv_f32("qwen35.rope.freq_base").unwrap_or(10_000.0);
    let mut mrope = [0usize; 4];
    if let Some(secs) = meta.kv_i64_array("qwen35.rope.dimension_sections") {
        for (i, v) in secs.into_iter().take(4).enumerate() {
            mrope[i] = v.max(0) as usize;
        }
    }
    Shapes {
        hidden,
        n_layers,
        interval,
        head_count: head_count.max(1),
        kv_heads,
        head_dim: head_dim.max(1),
        rotary_dim: rotary_dim.max(2),
        epsilon,
        rope_theta,
        mrope,
        state_size: meta.kv_u64("qwen35.ssm.state_size").unwrap_or(4) as usize,
        group_count: meta.kv_u64("qwen35.ssm.group_count").unwrap_or(1) as usize,
        time_step_rank: meta.kv_u64("qwen35.ssm.time_step_rank").unwrap_or(1) as usize,
        inner_size: meta.kv_u64("qwen35.ssm.inner_size").unwrap_or(4) as usize,
        conv_kernel: meta.kv_u64("qwen35.ssm.conv_kernel").unwrap_or(4) as usize,
        v_head_reordered: meta
            .kv_u64("qwen35.ssm.v_head_reordered")
            .map(|v| v != 0)
            .unwrap_or(true),
    }
}

fn is_full(index: usize, interval: usize) -> bool {
    index % interval == interval - 1
}

pub fn new_state(meta: &GgufMeta, mapped: &MappedWeights) -> DecodeState {
    let s = shapes(meta);
    let mut layers = Vec::with_capacity(s.n_layers);
    for i in 0..s.n_layers {
        let prefix = format!("blk.{i}");
        if mapped
            .tensors
            .keys()
            .any(|name| name.starts_with(&format!("{prefix}.nextn.")))
        {
            layers.push(LayerState::Skip);
            continue;
        }
        if is_full(i, s.interval)
            && mapped
                .tensors
                .contains_key(&format!("{prefix}.attn_q.weight"))
        {
            layers.push(LayerState::Full {
                keys: Vec::new(),
                values: Vec::new(),
            });
        } else if mapped
            .tensors
            .contains_key(&format!("{prefix}.ssm_out.weight"))
        {
            let qkv_len = s
                .inner_size
                .saturating_add(s.group_count.saturating_mul(s.state_size).saturating_mul(2));
            let conv_len = qkv_len.saturating_mul(s.conv_kernel.saturating_sub(1));
            let delta_len = s
                .time_step_rank
                .saturating_mul(s.state_size)
                .saturating_mul(s.state_size);
            layers.push(LayerState::Hybrid {
                conv: vec![0.0; conv_len],
                delta: vec![0.0; delta_len],
            });
        } else {
            layers.push(LayerState::Skip);
        }
    }
    DecodeState {
        position: 0,
        layers,
    }
}

pub fn embed_and_forward(
    mapped: &MappedWeights,
    meta: &GgufMeta,
    token: u32,
    state: &mut DecodeState,
) -> Result<Vec<f32>, String> {
    let s = shapes(meta);
    let mut hidden = crate::generate::embed_token(mapped, token, s.hidden)
        .ok_or_else(|| String::from("embed"))?;
    for (index, layer) in state.layers.iter_mut().enumerate() {
        hidden = match layer {
            LayerState::Skip => hidden,
            LayerState::Hybrid { conv, delta } => {
                forward_hybrid(mapped, &s, index, hidden, conv, delta)?
            }
            LayerState::Full { keys, values } => {
                forward_full(mapped, &s, index, hidden, keys, values, state.position)?
            }
        };
    }
    state.position = state.position.saturating_add(1);
    Ok(hidden)
}

pub fn greedy_from_hidden(
    mapped: &MappedWeights,
    hidden: &[f32],
    tok: &TokenizerTables,
) -> Option<(u32, String)> {
    crate::generate::greedy_from_hidden(mapped, hidden, tok)
}

fn vec_f32(mapped: &MappedWeights, name: &str) -> Result<Vec<f32>, String> {
    let view = lookup(mapped, name).ok_or_else(|| format!("missing {name}"))?;
    let src = unsafe { std::slice::from_raw_parts(view.data, view.len) };
    match view.info.ggml_type {
        0 => {
            if src.len() % 4 != 0 {
                return Err(format!("{name} f32 length"));
            }
            let mut out = Vec::with_capacity(src.len() / 4);
            for chunk in src.chunks_exact(4) {
                out.push(f32::from_le_bytes(chunk.try_into().unwrap()));
            }
            Ok(out)
        }
        8 => decode_row(
            mapped,
            name,
            0,
            view.info.dims.first().copied().unwrap_or(0) as usize,
        )
        .ok_or_else(|| format!("{name} q8")),
        _ => Err(format!("{name} type {}", view.info.ggml_type)),
    }
}

fn forward_hybrid(
    mapped: &MappedWeights,
    s: &Shapes,
    index: usize,
    input: Vec<f32>,
    conv_state: &mut [f32],
    delta_state: &mut [f32],
) -> Result<Vec<f32>, String> {
    let p = format!("blk.{index}");
    let attn_norm = vec_f32(mapped, &format!("{p}.attn_norm.weight"))?;
    let hidden_norm = rms_norm_eps(input.as_slice(), attn_norm.as_slice(), s.epsilon);
    let qkv_n = format!("{p}.attn_qkv.weight");
    let z_n = format!("{p}.attn_gate.weight");
    let alpha_n = format!("{p}.ssm_alpha.weight");
    let beta_n = format!("{p}.ssm_beta.weight");
    let bundled = matvec_many(
        mapped,
        &[
            qkv_n.as_str(),
            z_n.as_str(),
            alpha_n.as_str(),
            beta_n.as_str(),
        ],
        hidden_norm.as_slice(),
    )
    .filter(|v| v.len() == 4);
    let (qkv, z, alpha, beta) = if let Some(mut v) = bundled {
        let beta = v.pop().unwrap();
        let alpha = v.pop().unwrap();
        let z = v.pop().unwrap();
        let qkv = v.pop().unwrap();
        (qkv, z, alpha, beta)
    } else {
        (
            matvec(mapped, &qkv_n, hidden_norm.as_slice())
                .ok_or_else(|| format!("{p}.attn_qkv"))?,
            matvec(mapped, &z_n, hidden_norm.as_slice()).ok_or_else(|| format!("{p}.attn_gate"))?,
            matvec(mapped, &alpha_n, hidden_norm.as_slice())
                .ok_or_else(|| format!("{p}.ssm_alpha"))?,
            matvec(mapped, &beta_n, hidden_norm.as_slice())
                .ok_or_else(|| format!("{p}.ssm_beta"))?,
        )
    };
    let conv_w = vec_f32(mapped, &format!("{p}.ssm_conv1d.weight"))?;
    let ssm_a = vec_f32(mapped, &format!("{p}.ssm_a"))
        .or_else(|_| vec_f32(mapped, &format!("{p}.ssm_a.weight")))?;
    let ssm_dt = vec_f32(mapped, &format!("{p}.ssm_dt.bias"))
        .or_else(|_| vec_f32(mapped, &format!("{p}.ssm_dt")))?;
    let ssm_norm = vec_f32(mapped, &format!("{p}.ssm_norm.weight"))?;

    let q_size = s.group_count.saturating_mul(s.state_size);
    let k_size = q_size;
    let v_size = s.inner_size;
    let v_offset = q_size.saturating_add(k_size);
    if qkv.len() != v_offset.saturating_add(v_size) {
        return Err(format!(
            "{p} qkv width {} expected {}",
            qkv.len(),
            v_offset + v_size
        ));
    }

    let mut conv = vec![0.0f32; qkv.len()];
    conv1d_step(
        qkv.as_slice(),
        conv_state,
        conv_w.as_slice(),
        s.conv_kernel,
        conv.as_mut_slice(),
    )?;
    for v in &mut conv {
        *v = silu(*v);
    }

    let mut decay = vec![0.0f32; alpha.len()];
    let mut beta_sig = vec![0.0f32; beta.len()];
    for i in 0..alpha.len() {
        let a = ssm_a.get(i).copied().unwrap_or(0.0);
        let dt = ssm_dt.get(i).copied().unwrap_or(0.0);
        decay[i] = (softplus(alpha[i] + dt) * a).exp();
        beta_sig[i] = sigmoid(beta.get(i).copied().unwrap_or(0.0));
    }

    let mut qkv_norm = vec![0.0f32; v_offset + v_size];
    qkv_norm[..q_size].copy_from_slice(&conv[..q_size]);
    qkv_norm[q_size..q_size + k_size].copy_from_slice(&conv[q_size..q_size + k_size]);
    qkv_norm[v_offset..v_offset + v_size].copy_from_slice(&conv[v_offset..v_offset + v_size]);

    let mut gated = vec![0.0f32; v_size];
    let mut norm_q = vec![0.0f32; s.state_size];
    let mut norm_k = vec![0.0f32; s.state_size];
    let mut kv_mem = vec![0.0f32; s.state_size];
    let mut delta = vec![0.0f32; s.state_size];
    let repeat = s.time_step_rank / s.group_count.max(1);
    for vh in 0..s.time_step_rank {
        let kh = if s.v_head_reordered {
            vh % s.group_count.max(1)
        } else if repeat > 0 {
            vh / repeat
        } else {
            0
        };
        let q = &qkv_norm[kh * s.state_size..(kh + 1) * s.state_size];
        let k = &qkv_norm[q_size + kh * s.state_size..q_size + (kh + 1) * s.state_size];
        let v = &qkv_norm[v_offset + vh * s.state_size..v_offset + (vh + 1) * s.state_size];
        let st = &mut delta_state
            [vh * s.state_size * s.state_size..(vh + 1) * s.state_size * s.state_size];
        let out = &mut gated[vh * s.state_size..(vh + 1) * s.state_size];
        delta_step(
            q,
            k,
            v,
            decay.get(vh).copied().unwrap_or(1.0),
            beta_sig.get(vh).copied().unwrap_or(0.0),
            st,
            &mut norm_q,
            &mut norm_k,
            &mut kv_mem,
            &mut delta,
            out,
        );
    }

    let hybrid_norm = per_head_rms(
        gated.as_slice(),
        s.time_step_rank,
        s.state_size,
        ssm_norm.as_slice(),
        s.epsilon,
    );
    let activated: Vec<f32> = hybrid_norm
        .iter()
        .zip(z.iter().chain(std::iter::repeat(&0.0)))
        .map(|(v, g)| v * silu(*g))
        .collect();
    let projected = matvec(mapped, &format!("{p}.ssm_out.weight"), activated.as_slice())
        .ok_or_else(|| format!("{p}.ssm_out"))?;
    ffn_residual(mapped, s, &p, add(&projected, &input)?)
}

fn forward_full(
    mapped: &MappedWeights,
    s: &Shapes,
    index: usize,
    input: Vec<f32>,
    keys: &mut Vec<Vec<f32>>,
    values: &mut Vec<Vec<f32>>,
    position: usize,
) -> Result<Vec<f32>, String> {
    let p = format!("blk.{index}");
    let attn_norm = vec_f32(mapped, &format!("{p}.attn_norm.weight"))?;
    let hidden_norm = rms_norm_eps(input.as_slice(), attn_norm.as_slice(), s.epsilon);
    let query_width = s.head_count.saturating_mul(s.head_dim);
    let q_n = format!("{p}.attn_q.weight");
    let k_n = format!("{p}.attn_k.weight");
    let v_n = format!("{p}.attn_v.weight");
    let bundled = matvec_many(
        mapped,
        &[q_n.as_str(), k_n.as_str(), v_n.as_str()],
        hidden_norm.as_slice(),
    )
    .filter(|v| v.len() == 3);
    let (qg, mut key, value) = if let Some(mut v) = bundled {
        let value = v.pop().unwrap();
        let key = v.pop().unwrap();
        let qg = v.pop().unwrap();
        (qg, key, value)
    } else {
        (
            matvec(mapped, &q_n, hidden_norm.as_slice()).ok_or_else(|| format!("{p}.attn_q"))?,
            matvec(mapped, &k_n, hidden_norm.as_slice()).ok_or_else(|| format!("{p}.attn_k"))?,
            matvec(mapped, &v_n, hidden_norm.as_slice()).ok_or_else(|| format!("{p}.attn_v"))?,
        )
    };
    if qg.len() != query_width.saturating_mul(2) {
        return Err(format!(
            "{p} query/gate width {} expected {}",
            qg.len(),
            query_width * 2
        ));
    }
    let mut query = vec![0.0f32; query_width];
    let mut gate = vec![0.0f32; query_width];
    for h in 0..s.head_count {
        let src = h * s.head_dim * 2;
        let dst = h * s.head_dim;
        query[dst..dst + s.head_dim].copy_from_slice(&qg[src..src + s.head_dim]);
        gate[dst..dst + s.head_dim].copy_from_slice(&qg[src + s.head_dim..src + s.head_dim * 2]);
    }
    let q_norm = vec_f32(mapped, &format!("{p}.attn_q_norm.weight"))?;
    let k_norm = vec_f32(mapped, &format!("{p}.attn_k_norm.weight"))?;
    query = per_head_rms(
        query.as_slice(),
        s.head_count,
        s.head_dim,
        q_norm.as_slice(),
        s.epsilon,
    );
    key = per_head_rms(
        key.as_slice(),
        s.kv_heads,
        s.head_dim,
        k_norm.as_slice(),
        s.epsilon,
    );
    apply_rope(
        query.as_mut_slice(),
        s.head_count,
        s.head_dim,
        s.rotary_dim,
        [position; 3],
        s.rope_theta,
        s.mrope,
    );
    apply_rope(
        key.as_mut_slice(),
        s.kv_heads,
        s.head_dim,
        s.rotary_dim,
        [position; 3],
        s.rope_theta,
        s.mrope,
    );
    let scale = (s.head_dim as f32).sqrt().recip();
    let attn = attend(
        query.as_slice(),
        key.as_slice(),
        value.as_slice(),
        keys,
        values,
        s.head_count,
        s.kv_heads,
        s.head_dim,
        scale,
    );
    keys.push(key);
    values.push(value);
    let gated: Vec<f32> = attn
        .iter()
        .zip(gate.iter())
        .map(|(v, g)| v * sigmoid(*g))
        .collect();
    let projected = matvec(mapped, &format!("{p}.attn_output.weight"), gated.as_slice())
        .ok_or_else(|| format!("{p}.attn_output"))?;
    ffn_residual(mapped, s, &p, add(&projected, &input)?)
}

fn ffn_residual(
    mapped: &MappedWeights,
    s: &Shapes,
    prefix: &str,
    residual: Vec<f32>,
) -> Result<Vec<f32>, String> {
    let post = vec_f32(mapped, &format!("{prefix}.post_attention_norm.weight"))?;
    let normed = rms_norm_eps(residual.as_slice(), post.as_slice(), s.epsilon);
    let gate_n = format!("{prefix}.ffn_gate.weight");
    let up_n = format!("{prefix}.ffn_up.weight");
    let down_n = format!("{prefix}.ffn_down.weight");
    if let Some(down) =
        crate::metal_gemm::try_q8_ffn(mapped, &gate_n, &up_n, &down_n, normed.as_slice())
    {
        return add(&residual, &down);
    }
    let gate =
        matvec(mapped, &gate_n, normed.as_slice()).ok_or_else(|| format!("{prefix}.ffn_gate"))?;
    let up = matvec(mapped, &up_n, normed.as_slice()).ok_or_else(|| format!("{prefix}.ffn_up"))?;
    let hid: Vec<f32> = gate
        .iter()
        .zip(up.iter())
        .map(|(g, u)| silu(*g) * *u)
        .collect();
    let down =
        matvec(mapped, &down_n, hid.as_slice()).ok_or_else(|| format!("{prefix}.ffn_down"))?;
    add(&residual, &down)
}

fn conv1d_step(
    input: &[f32],
    state: &mut [f32],
    weights: &[f32],
    kernel: usize,
    output: &mut [f32],
) -> Result<(), String> {
    let state_tokens = kernel.saturating_sub(1);
    if weights.len() != input.len().saturating_mul(kernel) {
        return Err(format!(
            "conv1d weights {} expected {}",
            weights.len(),
            input.len() * kernel
        ));
    }
    if state.len() != input.len().saturating_mul(state_tokens) {
        return Err(format!(
            "conv1d state {} expected {}",
            state.len(),
            input.len() * state_tokens
        ));
    }
    for row in 0..input.len() {
        let row_state = &state[row * state_tokens..(row + 1) * state_tokens];
        let row_w = &weights[row * kernel..(row + 1) * kernel];
        let mut acc = input[row] * row_w[state_tokens];
        for (st, w) in row_state.iter().zip(row_w.iter()) {
            acc += *st * *w;
        }
        output[row] = acc;
    }
    if state_tokens > 0 {
        for row in 0..input.len() {
            let row_state = &mut state[row * state_tokens..(row + 1) * state_tokens];
            row_state.rotate_left(1);
            row_state[state_tokens - 1] = input[row];
        }
    }
    Ok(())
}

fn delta_step(
    q: &[f32],
    k: &[f32],
    v: &[f32],
    decay: f32,
    beta: f32,
    state: &mut [f32],
    norm_q: &mut [f32],
    norm_k: &mut [f32],
    kv_mem: &mut [f32],
    delta: &mut [f32],
    output: &mut [f32],
) {
    let dim = q.len();
    l2(q, norm_q);
    let scale = (dim as f32).sqrt().recip();
    for x in &mut norm_q[..dim] {
        *x *= scale;
    }
    l2(k, norm_k);
    for s in state.iter_mut() {
        *s *= decay;
    }
    for row in 0..dim {
        let row_s = &state[row * dim..(row + 1) * dim];
        kv_mem[row] = dot(row_s, &norm_k[..dim]);
    }
    for row in 0..dim {
        delta[row] = (v[row] - kv_mem[row]) * beta;
    }
    for row in 0..dim {
        let d = delta[row];
        for col in 0..dim {
            state[row * dim + col] += d * norm_k[col];
        }
    }
    for row in 0..dim {
        let row_s = &state[row * dim..(row + 1) * dim];
        output[row] = dot(row_s, &norm_q[..dim]);
    }
}

fn attend(
    query: &[f32],
    key: &[f32],
    value: &[f32],
    cache_k: &[Vec<f32>],
    cache_v: &[Vec<f32>],
    head_count: usize,
    kv_heads: usize,
    head_dim: usize,
    scale: f32,
) -> Vec<f32> {
    let group = head_count / kv_heads.max(1);
    let mut out = vec![0.0f32; head_count * head_dim];
    for h in 0..head_count {
        let kvh = (h / group.max(1)).min(kv_heads.saturating_sub(1));
        let q = &query[h * head_dim..(h + 1) * head_dim];
        let ck = &key[kvh * head_dim..(kvh + 1) * head_dim];
        let cv = &value[kvh * head_dim..(kvh + 1) * head_dim];
        let mut logits = Vec::with_capacity(cache_k.len() + 1);
        for entry in cache_k {
            let ek = &entry[kvh * head_dim..(kvh + 1) * head_dim];
            logits.push(dot(q, ek) * scale);
        }
        logits.push(dot(q, ck) * scale);
        let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let mut w: Vec<f32> = logits.iter().map(|l| (l - max).exp()).collect();
        let denom = w.iter().copied().sum::<f32>().max(f32::MIN_POSITIVE);
        for x in &mut w {
            *x /= denom;
        }
        let dest = &mut out[h * head_dim..(h + 1) * head_dim];
        for (i, entry) in cache_v.iter().enumerate() {
            let ev = &entry[kvh * head_dim..(kvh + 1) * head_dim];
            for (d, v) in dest.iter_mut().zip(ev.iter()) {
                *d += *v * w[i];
            }
        }
        let last = *w.last().unwrap_or(&0.0);
        for (d, v) in dest.iter_mut().zip(cv.iter()) {
            *d += *v * last;
        }
    }
    out
}

fn apply_rope(
    values: &mut [f32],
    head_count: usize,
    head_dim: usize,
    rotary_dim: usize,
    position: [usize; 3],
    theta: f32,
    sections: [usize; 4],
) {
    let rotary_dim = rotary_dim.min(head_dim).max(2);
    let theta_scale = theta.powf(-2.0 / rotary_dim as f32);
    let has_mrope = sections.iter().any(|s| *s > 0);
    let pos = [
        position[0] as f32,
        position[1] as f32,
        position[2] as f32,
        0.0,
    ];
    for h in 0..head_count {
        let base = h * head_dim;
        for i0 in (0..rotary_dim).step_by(2) {
            let pair = i0 / 2;
            let i1 = base + pair;
            let i2 = base + pair + rotary_dim / 2;
            if i2 >= base + head_dim || i2 >= values.len() {
                continue;
            }
            let theta_base = if has_mrope {
                mrope_theta(pair, sections, pos) * theta_scale.powf(pair as f32)
            } else {
                position[0] as f32 * theta_scale.powf(pair as f32)
            };
            let (c, s) = (theta_base.cos(), theta_base.sin());
            let x0 = values[i1];
            let x1 = values[i2];
            values[i1] = x0 * c - x1 * s;
            values[i2] = x0 * s + x1 * c;
        }
    }
}

fn mrope_theta(pair: usize, sections: [usize; 4], pos: [f32; 4]) -> f32 {
    let dim = sections.iter().sum::<usize>();
    if dim == 0 {
        return pos[0];
    }
    let sector = pair % dim;
    let w = sections[0] + sections[1];
    let e = w + sections[2];
    if sector < sections[0] {
        pos[0]
    } else if sector < w {
        pos[1]
    } else if sector < e {
        pos[2]
    } else {
        pos[3]
    }
}

fn rms_norm_eps(x: &[f32], w: &[f32], eps: f32) -> Vec<f32> {
    let mut ss = 0.0f32;
    for v in x {
        ss += *v * *v;
    }
    let scale = (ss / x.len() as f32 + eps).sqrt().recip();
    x.iter()
        .zip(w.iter().chain(std::iter::repeat(&1.0)))
        .map(|(v, g)| v * scale * g)
        .collect()
}

fn per_head_rms(input: &[f32], heads: usize, dim: usize, weight: &[f32], eps: f32) -> Vec<f32> {
    let mut out = vec![0.0f32; input.len()];
    for h in 0..heads {
        let start = h * dim;
        let end = start + dim;
        if end > input.len() {
            break;
        }
        let head = &input[start..end];
        let ss = head.iter().map(|v| v * v).sum::<f32>() / dim as f32;
        let scale = (ss + eps).sqrt().recip();
        for (i, v) in head.iter().enumerate() {
            let g = weight.get(i).copied().unwrap_or(1.0);
            out[start + i] = *v * scale * g;
        }
    }
    out
}

fn add(a: &[f32], b: &[f32]) -> Result<Vec<f32>, String> {
    if a.len() != b.len() {
        return Err(format!("add {} vs {}", a.len(), b.len()));
    }
    Ok(a.iter().zip(b.iter()).map(|(x, y)| x + y).collect())
}

fn silu(v: f32) -> f32 {
    v / (1.0 + (-v).exp())
}

fn sigmoid(v: f32) -> f32 {
    1.0 / (1.0 + (-v).exp())
}

fn softplus(v: f32) -> f32 {
    if v > 20.0 {
        v
    } else {
        (1.0 + v.exp()).ln()
    }
}

fn l2(values: &[f32], out: &mut [f32]) {
    let n = values.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-6);
    for (o, v) in out.iter_mut().zip(values.iter()) {
        *o = *v / n;
    }
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::write_qwen35_hybrid_fixture;
    use crate::mmap::map_file;
    use crate::parse_path;
    use crate::tokenizer::load_tokenizer;

    #[test]
    fn hybrid_fixture_greedy_is_deterministic() {
        let dir = std::env::temp_dir().join("psionic-gguf-hybrid");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("qwen35.gguf");
        write_qwen35_hybrid_fixture(&path).unwrap();
        let meta = parse_path(&path).unwrap();
        let mapped = map_file(&path, &meta).unwrap();
        assert!(has_hybrid_graph(&mapped));
        let tok = load_tokenizer(&meta).unwrap();
        let mut a = new_state(&meta, &mapped);
        let mut b = new_state(&meta, &mapped);
        let ha = embed_and_forward(&mapped, &meta, 0, &mut a).unwrap();
        let hb = embed_and_forward(&mapped, &meta, 0, &mut b).unwrap();
        assert_eq!(ha, hb);
        assert_eq!(
            greedy_from_hidden(&mapped, &ha, &tok),
            greedy_from_hidden(&mapped, &hb, &tok)
        );
    }
}
