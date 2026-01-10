fn parse_config(index: &GgufIndex) -> Result<GptOssConfig, String> {
    let block_count = read_meta_u32(index, "llama.block_count")?;
    let context_length = read_meta_u32_optional(index, "gpt-oss.context_length")
        .or_else(|| read_meta_u32_optional(index, "llama.context_length"))
        .unwrap_or(0);
    let embedding_length = read_meta_u32(index, "llama.embedding_length")?;
    let feed_forward_length = read_meta_u32(index, "llama.feed_forward_length")?;
    let head_count = read_meta_u32(index, "llama.attention.head_count")?;
    let head_count_kv = read_meta_u32(index, "llama.attention.head_count_kv")?;
    let rope_dimension_count = read_meta_u32(index, "llama.rope.dimension_count")?;
    let rope_theta = read_meta_f32(index, "llama.rope.freq_base")?;
    let rope_scaling_factor =
        read_meta_f32_optional(index, "gpt-oss.rope.scaling.factor").unwrap_or(1.0);
    let rope_scaling_original_context = read_meta_u32_optional(
        index,
        "gpt-oss.rope.scaling.original_context_length",
    )
    .unwrap_or(0);
    let rms_epsilon = read_meta_f32(index, "llama.attention.layer_norm_rms_epsilon")?;
    let sliding_window = read_meta_u32(index, "llama.sliding_window")?;
    let expert_count = read_meta_u32(index, "llama.expert_count")?;
    let experts_per_token = read_meta_u32(index, "llama.expert_used_count")?;

    Ok(GptOssConfig {
        block_count,
        context_length,
        embedding_length,
        feed_forward_length,
        head_count,
        head_count_kv,
        rope_dimension_count,
        rope_theta,
        rope_scaling_factor,
        rope_scaling_original_context,
        rms_epsilon,
        sliding_window,
        expert_count,
        experts_per_token,
    })
}

fn emit_config(state: &Rc<RefCell<AppState>>, config: &GptOssConfig) {
    let rope_scale = if config.rope_scaling_factor > 1.0
        && config.rope_scaling_original_context > 0
    {
        format!(
            "yarn x{:.2} orig={}",
            config.rope_scaling_factor, config.rope_scaling_original_context
        )
    } else {
        "none".to_string()
    };
    emit_load_stage(
        state,
        "model_config",
        StageStatus::Completed,
        Some(format!(
            "blocks={} ctx={} embd={} ffn={} heads={} kv_heads={} rope_dim={} rope_theta={} rope_scale={} rms_eps={} window={} experts={} topk={}",
            config.block_count,
            if config.context_length > 0 {
                config.context_length.to_string()
            } else {
                "-".to_string()
            },
            config.embedding_length,
            config.feed_forward_length,
            config.head_count,
            config.head_count_kv,
            config.rope_dimension_count,
            config.rope_theta,
            rope_scale,
            config.rms_epsilon,
            config.sliding_window,
            config.expert_count,
            config.experts_per_token,
        )),
        None,
        None,
    );
}

fn read_meta_u32(index: &GgufIndex, key: &str) -> Result<u32, String> {
    let value = lookup_meta(index, key);
    if value.is_none() && key.ends_with("rope.dimension_count") {
        if let Ok(key_len) = read_meta_u32(index, "gpt-oss.attention.key_length") {
            return Ok(key_len);
        }
        if let Ok(value_len) = read_meta_u32(index, "gpt-oss.attention.value_length") {
            return Ok(value_len);
        }
        let embedding = read_meta_u32(index, "llama.embedding_length")?;
        let heads = read_meta_u32(index, "llama.attention.head_count")?;
        if heads > 0 {
            return Ok(embedding / heads);
        }
    }
    let Some(value) = value else {
        return Err(format!("missing gguf metadata key: {key}"));
    };
    match value {
        crate::gguf_web::GgufScalar::U32(v) => Ok(*v),
        crate::gguf_web::GgufScalar::I32(v) => Ok((*v).max(0) as u32),
        crate::gguf_web::GgufScalar::U64(v) => Ok((*v).min(u64::from(u32::MAX)) as u32),
        crate::gguf_web::GgufScalar::I64(v) => Ok((*v).max(0).min(i64::from(u32::MAX)) as u32),
        _ => Err(format!("gguf metadata {key} has non-integer type")),
    }
}

fn read_meta_u32_optional(index: &GgufIndex, key: &str) -> Option<u32> {
    let value = lookup_meta(index, key)?;
    let out = match value {
        crate::gguf_web::GgufScalar::U32(v) => *v,
        crate::gguf_web::GgufScalar::I32(v) => (*v).max(0) as u32,
        crate::gguf_web::GgufScalar::U64(v) => (*v).min(u64::from(u32::MAX)) as u32,
        crate::gguf_web::GgufScalar::I64(v) => (*v).max(0).min(i64::from(u32::MAX)) as u32,
        crate::gguf_web::GgufScalar::F32(v) => (*v).max(0.0) as u32,
        crate::gguf_web::GgufScalar::F64(v) => (*v).max(0.0) as u32,
        _ => return None,
    };
    Some(out)
}

fn read_meta_f32(index: &GgufIndex, key: &str) -> Result<f32, String> {
    let Some(value) = lookup_meta(index, key) else {
        return Err(format!("missing gguf metadata key: {key}"));
    };
    match value {
        crate::gguf_web::GgufScalar::F32(v) => Ok(*v),
        crate::gguf_web::GgufScalar::F64(v) => Ok(*v as f32),
        crate::gguf_web::GgufScalar::U32(v) => Ok(*v as f32),
        crate::gguf_web::GgufScalar::I32(v) => Ok(*v as f32),
        crate::gguf_web::GgufScalar::U64(v) => Ok(*v as f32),
        crate::gguf_web::GgufScalar::I64(v) => Ok(*v as f32),
        _ => Err(format!("gguf metadata {key} has non-float type")),
    }
}

fn read_meta_f32_optional(index: &GgufIndex, key: &str) -> Option<f32> {
    let value = lookup_meta(index, key)?;
    let out = match value {
        crate::gguf_web::GgufScalar::F32(v) => *v,
        crate::gguf_web::GgufScalar::F64(v) => *v as f32,
        crate::gguf_web::GgufScalar::U32(v) => *v as f32,
        crate::gguf_web::GgufScalar::I32(v) => *v as f32,
        crate::gguf_web::GgufScalar::U64(v) => *v as f32,
        crate::gguf_web::GgufScalar::I64(v) => *v as f32,
        _ => return None,
    };
    Some(out)
}

fn lookup_meta<'a>(index: &'a GgufIndex, key: &str) -> Option<&'a crate::gguf_web::GgufScalar> {
    if let Some(value) = index.metadata.values.get(key) {
        return Some(value);
    }
    if key == "llama.sliding_window" {
        return index.metadata.values.get("gpt-oss.attention.sliding_window");
    }
    if key == "gpt-oss.attention.sliding_window" {
        return index.metadata.values.get("llama.sliding_window");
    }
    let fallback = key
        .strip_prefix("llama.")
        .map(|rest| format!("gpt-oss.{rest}"))
        .or_else(|| key.strip_prefix("gpt-oss.").map(|rest| format!("llama.{rest}")))?;
    index.metadata.values.get(&fallback)
}

fn build_tokenizer(
    state: &Rc<RefCell<AppState>>,
    index: &GgufIndex,
) -> Result<GptOssTokenizer, String> {
    emit_load_stage(
        state,
        "tokenizer_load",
        StageStatus::Started,
        Some("building BPE".to_string()),
        None,
        None,
    );

    let Some(tokenizer_meta) = index.metadata.tokenizer.clone() else {
        let err = "gguf tokenizer metadata missing".to_string();
        emit_load_stage(
            state,
            "tokenizer_load",
            StageStatus::Failed,
            Some(err.clone()),
            None,
            None,
        );
        return Err(err);
    };

    let token_count = tokenizer_meta.tokens.len();
    let merges_len = tokenizer_meta.merges.len();
    let model = tokenizer_meta
        .model
        .as_deref()
        .unwrap_or("-");
    let pre = tokenizer_meta
        .pre
        .as_deref()
        .unwrap_or("-");
    let chat_len = tokenizer_meta
        .chat_template
        .as_ref()
        .map(|value| value.len())
        .unwrap_or(0);
    let bos = tokenizer_meta
        .bos_token_id
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string());
    let eos = tokenizer_meta
        .eos_token_id
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string());
    let pad = tokenizer_meta
        .pad_token_id
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string());

    let tokenizer = match GptOssTokenizer::from_gguf(tokenizer_meta.clone()) {
        Ok(tok) => tok,
        Err(err) => {
            emit_load_stage(
                state,
                "tokenizer_load",
                StageStatus::Failed,
                Some(err.clone()),
                None,
                None,
            );
            return Err(err);
        }
    };

    emit_load_stage(
        state,
        "tokenizer_load",
        StageStatus::Completed,
        Some(format!(
            "vocab={token_count} merges={merges_len} model={model} pre={pre} template={chat_len}b bos={bos} eos={eos} pad={pad}",
        )),
        None,
        None,
    );
    Ok(tokenizer)
}

