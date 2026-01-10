fn collect_stop_tokens(tokenizer: &GptOssTokenizer) -> Vec<u32> {
    let mut tokens = Vec::new();
    for name in ["<|return|>", "<|call|>", "<|end|>"] {
        if let Some(id) = tokenizer.token_id(name) {
            tokens.push(id);
        }
    }
    if let Some(id) = tokenizer.eos_token_id() {
        tokens.push(id);
    }
    tokens.sort_unstable();
    tokens.dedup();
    tokens
}

fn coherence_score(text: &str) -> f32 {
    let mut total = 0u32;
    let mut readable = 0u32;
    for ch in text.chars() {
        total += 1;
        if ch.is_ascii_alphanumeric() || ch.is_ascii_whitespace() || ch.is_ascii_punctuation() {
            readable += 1;
        }
    }
    if total == 0 {
        return 0.0;
    }
    readable as f32 / total as f32
}

fn hex_preview(bytes: &[u8], len: usize) -> String {
    let take = bytes.len().min(len);
    let mut out = String::new();
    for (idx, byte) in bytes.iter().take(take).enumerate() {
        if idx > 0 {
            out.push(' ');
        }
        out.push_str(&format!("{:02x}", byte));
    }
    out
}

fn top_k_from_logits(
    logits: &[f32],
    tokenizer: &GptOssTokenizer,
    k: usize,
) -> Result<(Vec<GptOssTokenCandidate>, f32, u32, String), String> {
    if logits.is_empty() {
        return Err("empty logits".to_string());
    }
    let mut max_logit = f32::NEG_INFINITY;
    for &logit in logits {
        if logit > max_logit {
            max_logit = logit;
        }
    }

    let mut sum_exp = 0.0f32;
    for &logit in logits {
        sum_exp += (logit - max_logit).exp();
    }
    if sum_exp <= 0.0 {
        return Err("softmax sum is zero".to_string());
    }

    let mut entropy = 0.0f32;
    for &logit in logits {
        let p = (logit - max_logit).exp() / sum_exp;
        if p > 0.0 {
            entropy -= p * p.ln();
        }
    }

    let mut top: Vec<(usize, f32)> = Vec::with_capacity(k.min(logits.len()));
    for (idx, &logit) in logits.iter().enumerate() {
        if top.len() < k {
            top.push((idx, logit));
            top.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        } else if let Some(last) = top.last() {
            if logit > last.1 {
                top.pop();
                top.push((idx, logit));
                top.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            }
        }
    }

    let mut candidates = Vec::with_capacity(top.len());
    for (idx, logit) in top.iter() {
        let prob = (logit - max_logit).exp() / sum_exp;
        let token_id = *idx as u32;
        candidates.push(GptOssTokenCandidate {
            token_id,
            token_text: tokenizer.decode_utf8_lossy(&[token_id]),
            probability: prob,
        });
    }

    let (best_idx, _) = top
        .first()
        .copied()
        .unwrap_or((0usize, logits[0]));
    let best_token_id = best_idx as u32;
    let best_text = tokenizer.decode_utf8_lossy(&[best_token_id]);

    Ok((candidates, entropy, best_token_id, best_text))
}

fn sample_from_logits(
    logits: &[f32],
    tokenizer: &GptOssTokenizer,
    sampling: SamplingConfig,
    display_k: usize,
) -> Result<(Vec<GptOssTokenCandidate>, f32, u32, String), String> {
    let (top_k, entropy, best_id, best_text) = top_k_from_logits(logits, tokenizer, display_k)?;
    if !sampling.enabled {
        return Ok((top_k, entropy, best_id, best_text));
    }

    let effective_top_k = if sampling.top_k == 0 {
        DEFAULT_SAMPLE_TOP_K
    } else {
        sampling.top_k
    };
    let (mut indices, mut weights) =
        top_k_softmax_scaled(logits, effective_top_k, sampling.temperature)?;
    apply_top_p(&mut indices, &mut weights, sampling.top_p);
    let selected = sample_index(&indices, &weights);
    let token_id = selected as u32;
    let token_text = tokenizer.decode_utf8_lossy(&[token_id]);

    Ok((top_k, entropy, token_id, token_text))
}

fn top_k_softmax(values: &[f32], k: usize) -> Result<(Vec<usize>, Vec<f32>), String> {
    if values.is_empty() {
        return Err("empty values".to_string());
    }
    let k = k.max(1).min(values.len());
    let mut top: Vec<(usize, f32)> = Vec::with_capacity(k);
    for (idx, &value) in values.iter().enumerate() {
        if top.len() < k {
            top.push((idx, value));
            top.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        } else if let Some(last) = top.last() {
            if value > last.1 {
                top.pop();
                top.push((idx, value));
                top.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            }
        }
    }

    let mut max_val = f32::NEG_INFINITY;
    for &(_, value) in &top {
        if value > max_val {
            max_val = value;
        }
    }
    let mut sum = 0.0f32;
    for &(_, value) in &top {
        sum += (value - max_val).exp();
    }
    if sum <= 0.0 {
        return Err("softmax sum is zero".to_string());
    }

    let mut indices = Vec::with_capacity(top.len());
    let mut weights = Vec::with_capacity(top.len());
    for &(idx, value) in &top {
        indices.push(idx);
        weights.push((value - max_val).exp() / sum);
    }

    Ok((indices, weights))
}

fn top_k_softmax_scaled(
    values: &[f32],
    k: usize,
    temperature: f32,
) -> Result<(Vec<usize>, Vec<f32>), String> {
    if values.is_empty() {
        return Err("empty values".to_string());
    }
    let temp = temperature.max(1e-6);
    let k = k.max(1).min(values.len());
    let mut top: Vec<(usize, f32)> = Vec::with_capacity(k);
    for (idx, &value) in values.iter().enumerate() {
        if top.len() < k {
            top.push((idx, value));
            top.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        } else if let Some(last) = top.last() {
            if value > last.1 {
                top.pop();
                top.push((idx, value));
                top.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            }
        }
    }

    let mut max_val = f32::NEG_INFINITY;
    for &(_, value) in &top {
        let scaled = value / temp;
        if scaled > max_val {
            max_val = scaled;
        }
    }
    let mut sum = 0.0f32;
    for &(_, value) in &top {
        sum += (value / temp - max_val).exp();
    }
    if sum <= 0.0 {
        return Err("softmax sum is zero".to_string());
    }

    let mut indices = Vec::with_capacity(top.len());
    let mut weights = Vec::with_capacity(top.len());
    for &(idx, value) in &top {
        indices.push(idx);
        weights.push((value / temp - max_val).exp() / sum);
    }

    Ok((indices, weights))
}

fn apply_top_p(indices: &mut Vec<usize>, weights: &mut Vec<f32>, top_p: f32) {
    if indices.is_empty() || weights.is_empty() || top_p >= 1.0 {
        return;
    }
    let mut cumulative = 0.0f32;
    let mut cutoff = weights.len();
    for (idx, weight) in weights.iter().enumerate() {
        cumulative += *weight;
        if cumulative >= top_p {
            cutoff = idx + 1;
            break;
        }
    }
    indices.truncate(cutoff);
    weights.truncate(cutoff);
    let sum: f32 = weights.iter().sum();
    if sum > 0.0 {
        for weight in weights.iter_mut() {
            *weight /= sum;
        }
    }
}

fn sample_index(indices: &[usize], weights: &[f32]) -> usize {
    if indices.is_empty() || weights.is_empty() {
        return 0;
    }
    let draw = js_sys::Math::random() as f32;
    let mut cumulative = 0.0f32;
    for (idx, weight) in weights.iter().enumerate() {
        cumulative += *weight;
        if draw <= cumulative {
            return indices[idx];
        }
    }
    *indices.last().unwrap_or(&0)
}

fn tensor_start_cursor(index: &GgufIndex) -> Vec<(u64, String)> {
    let mut entries = index
        .tensors
        .iter()
        .map(|tensor| (tensor.absolute_offset, tensor.name.clone()))
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.0);
    entries
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.1}GB", bytes as f32 / 1_000_000_000.0)
    } else if bytes >= 1_000_000 {
        format!("{:.1}MB", bytes as f32 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.1}KB", bytes as f32 / 1_000.0)
    } else {
        format!("{bytes}B")
    }
}

fn format_rate(bytes_per_sec: f64) -> String {
    let rate = bytes_per_sec.max(0.0) as u64;
    format!("{}/s", format_bytes(rate))
}

fn ensure_storage_limit(label: &str, size: usize, max: usize) -> Result<(), String> {
    if max == 0 {
        return Ok(());
    }
    if size > max {
        return Err(format!(
            "{label} size {} exceeds max storage {}",
            format_bytes(size as u64),
            format_bytes(max as u64)
        ));
    }
    Ok(())
}

fn ensure_buffer_limit(
    label: &str,
    size: usize,
    max_storage: u64,
    max_buffer: u64,
) -> Result<(), String> {
    if max_storage > 0 && size as u64 > max_storage {
        return Err(format!(
            "{label} size {} exceeds max storage {}",
            format_bytes(size as u64),
            format_bytes(max_storage)
        ));
    }
    if max_buffer > 0 && size as u64 > max_buffer {
        return Err(format!(
            "{label} size {} exceeds max buffer {}",
            format_bytes(size as u64),
            format_bytes(max_buffer)
        ));
    }
    Ok(())
}

