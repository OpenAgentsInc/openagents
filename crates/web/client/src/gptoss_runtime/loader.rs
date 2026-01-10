pub(crate) fn start_gptoss_load(state: Rc<RefCell<AppState>>) {
    let (raw_url, file_opt) = {
        let input_override = state
            .try_borrow()
            .ok()
            .and_then(|guard| {
                let value = guard.gptoss.gguf_input.get_value().trim().to_string();
                if value.is_empty() {
                    None
                } else {
                    Some(value)
                }
            });
        let file = state
            .try_borrow()
            .ok()
            .and_then(|guard| guard.gptoss.gguf_file.clone());
        let raw_url = input_override
            .or_else(|| read_query_param("gguf").filter(|url| !url.is_empty()))
            .unwrap_or_default();
        (raw_url, file)
    };

    let mut pylon_url = parse_pylon_url(&raw_url);
    if pylon_url.is_none() && raw_url.trim().is_empty() && file_opt.is_none() {
        pylon_url = Some(PYLON_API_URL.to_string());
    }

    if let Some(pylon_url) = pylon_url {
        let label = pylon_label_from_url(&pylon_url);
        {
            let Ok(mut guard) = state.try_borrow_mut() else {
                return;
            };
            if guard.gptoss.load_active {
                return;
            }
            reset_gptoss_state(&mut guard.gptoss);
            guard.gptoss.load_active = true;
            guard.gptoss.load_error = None;
            guard.gptoss.load_url = Some(label.clone());
            if guard.gptoss.gguf_input.get_value().trim().is_empty() {
                guard.gptoss.gguf_input.set_value(label.clone());
            }
        }

        let state_clone = state.clone();
        spawn_local(async move {
            if let Err(err) = run_gptoss_pylon(state_clone.clone(), &pylon_url, &label).await {
                if let Ok(mut guard) = state_clone.try_borrow_mut() {
                    guard.gptoss.load_active = false;
                    guard.gptoss.load_error = Some(err.clone());
                }
                emit_load_stage(
                    &state_clone,
                    "load_failed",
                    StageStatus::Failed,
                    Some(err),
                    None,
                    None,
                );
            }
        });
        return;
    }

    let mut gguf_source = None;
    let mut gguf_label = None;

    let wants_file = is_file_input(&raw_url) || (raw_url.trim().is_empty() && file_opt.is_some());
    if wants_file {
        if let Some(file) = file_opt.clone() {
            gguf_label = Some(gguf_file_label(&file));
            gguf_source = Some(GgufSource::File(file));
        } else {
            start_gptoss_file_pick(state);
            return;
        }
    }

    if gguf_source.is_none() {
        let gguf_url = match normalize_gguf_url(&raw_url) {
            Ok(url) => url,
            Err(err) => {
                if let Ok(mut guard) = state.try_borrow_mut() {
                    reset_gptoss_state(&mut guard.gptoss);
                    guard.gptoss.load_error = Some(err.clone());
                }
                emit_load_stage(
                    &state,
                    "load_failed",
                    StageStatus::Failed,
                    Some(err),
                    None,
                    None,
                );
                return;
            }
        };
        if gguf_url.is_empty() {
            start_gptoss_file_pick(state);
            return;
        }
        gguf_label = Some(gguf_url.clone());
        gguf_source = Some(GgufSource::Url(gguf_url));
    }

    let gguf_source = gguf_source.expect("gguf source set");
    let gguf_label = gguf_label.unwrap_or_else(|| gguf_source.label());

    {
        let Ok(mut guard) = state.try_borrow_mut() else {
            return;
        };
        if guard.gptoss.load_active {
            return;
        }
        reset_gptoss_state(&mut guard.gptoss);
        if guard.gptoss.gguf_input.get_value().trim().is_empty() {
            guard.gptoss.gguf_input.set_value(gguf_label.clone());
        }
        guard.gptoss.load_active = true;
        guard.gptoss.load_error = None;
        guard.gptoss.load_url = Some(gguf_label.clone());
    }

    let state_clone = state.clone();
    spawn_local(async move {
        if let Err(err) = run_gptoss_load(state_clone.clone(), gguf_source, gguf_label).await {
            if let Ok(mut guard) = state_clone.try_borrow_mut() {
                guard.gptoss.load_active = false;
                guard.gptoss.load_error = Some(err.clone());
            }
            emit_load_stage(
                &state_clone,
                "load_failed",
                StageStatus::Failed,
                Some(err),
                None,
                None,
            );
        }
    });
}

pub(crate) fn start_gptoss_file_pick(state: Rc<RefCell<AppState>>) {
    if let Ok(guard) = state.try_borrow() {
        if guard.gptoss.load_active {
            return;
        }
    }
    let state_clone = state.clone();
    spawn_local(async move {
        let file = match pick_gguf_file().await {
            Ok(file) => file,
            Err(err) => {
                if let Ok(mut guard) = state_clone.try_borrow_mut() {
                    guard.gptoss.load_error = Some(err);
                }
                return;
            }
        };
        let file_name = file.name().to_ascii_lowercase();
        if !file_name.ends_with(".gguf") {
            if let Ok(mut guard) = state_clone.try_borrow_mut() {
                guard.gptoss.load_error = Some("Selected file is not a .gguf".to_string());
            }
            return;
        }
        let input_label = gguf_file_input_label(&file);
        let display_label = gguf_file_label(&file);
        if let Ok(mut guard) = state_clone.try_borrow_mut() {
            guard.gptoss.gguf_file = Some(file);
            guard.gptoss.gguf_file_label = Some(display_label);
            guard.gptoss.gguf_input.set_value(input_label);
            guard.gptoss.load_error = None;
        }
        start_gptoss_load(state_clone);
    });
}

async fn run_gptoss_load(
    state: Rc<RefCell<AppState>>,
    gguf_source: GgufSource,
    gguf_label: String,
) -> Result<(), String> {
    emit_load_stage(
        &state,
        "load_start",
        StageStatus::Started,
        Some(format!("source={}", gguf_label)),
        None,
        None,
    );

    emit_load_stage(
        &state,
        "range_check",
        StageStatus::Started,
        None,
        None,
        None,
    );

    let (_probe, total) = fetch_range_with_total_source(&gguf_source, 0, 1)
        .await
        .map_err(|err| format_source_error(&gguf_source, &gguf_label, &err))?;
    let total_bytes = total.ok_or_else(|| {
        "Host does not support Range/CORS. Start gguf_serve.".to_string()
    })?;
    emit_load_stage(
        &state,
        "range_check",
        StageStatus::Completed,
        Some(format!("total={}", format_bytes(total_bytes))),
        None,
        None,
    );

    emit_load_stage(
        &state,
        "gguf_parse",
        StageStatus::Started,
        Some("reading gguf header".to_string()),
        None,
        None,
    );

    let index = Rc::new(
        fetch_and_parse_index_source(&gguf_source, DEFAULT_METADATA_BYTES, MAX_METADATA_ATTEMPTS)
            .await?,
    );
    emit_load_stage(
        &state,
        "gguf_parse",
        StageStatus::Completed,
        Some(format!(
            "tensors={} v{} data_offset={}",
            index.tensors.len(),
            index.version,
            format_bytes(index.tensor_data_offset)
        )),
        None,
        None,
    );

    emit_tensor_scan(&state, index.as_ref(), 18);
    emit_metadata_keys(&state, index.as_ref(), 18);
    let config = parse_config(index.as_ref())?;
    let (input_layers, input_max_kv, input_max_new) = read_input_overrides(&state)?;
    let requested_layers = input_layers.or_else(|| read_query_usize("layers"));
    let max_layers = config.block_count as usize;
    let mut active_layers = requested_layers.unwrap_or(max_layers);
    active_layers = active_layers.min(max_layers);
    let layer_detail = if active_layers == 0 {
        "layers=0 (lm_head only)".to_string()
    } else if active_layers == max_layers {
        if requested_layers.is_none() {
            format!("layers={active_layers}/{max_layers} default")
        } else {
            format!("layers={active_layers}/{max_layers}")
        }
    } else if requested_layers.is_none() {
        format!("layers={active_layers}/{max_layers} default")
    } else {
        format!("layers={active_layers}/{max_layers}")
    };
    emit_load_stage(
        &state,
        "layer_limit",
        StageStatus::Completed,
        Some(layer_detail),
        None,
        None,
    );
    let force_dense = read_query_param("attn")
        .map(|value| matches!(value.as_str(), "dense" | "full" | "0"))
        .unwrap_or(false);
    let moe_fallback = read_query_param("moe")
        .map(|value| matches!(value.as_str(), "fallback" | "off" | "0"))
        .unwrap_or(false);
    emit_config(&state, &config);
    if moe_fallback {
        emit_load_stage(
            &state,
            "moe_mode",
            StageStatus::Completed,
            Some("fallback expert=0".to_string()),
            None,
            None,
        );
    }

    let gpu = state
        .borrow()
        .gpu_context
        .clone()
        .ok_or_else(|| "WebGPU device unavailable (enable WebGPU in Chrome)".to_string())?;
    emit_gpu_limits(&state, &gpu);
    let mut max_kv_tokens = {
        let mut max_kv = input_max_kv
            .or_else(|| read_query_usize("max_kv"))
            .unwrap_or(DEFAULT_MAX_KV_TOKENS);
        if config.context_length > 0 {
            max_kv = max_kv.min(config.context_length as usize);
        }
        max_kv.max(1)
    };
    let kv_limit = kv_limit_for_gpu(&config, &gpu, DEFAULT_KV_BUDGET_BYTES);
    let mut kv_clamp: Option<(usize, KvLimit)> = None;
    if let Some(limit) = kv_limit {
        if max_kv_tokens > limit.max_tokens {
            kv_clamp = Some((max_kv_tokens, limit));
            max_kv_tokens = limit.max_tokens.max(1);
        }
    }
    let mut max_new_tokens = input_max_new
        .or_else(|| read_query_usize("max_new"))
        .unwrap_or(DEFAULT_MAX_NEW_TOKENS);
    max_new_tokens = if max_kv_tokens <= 1 {
        0
    } else {
        max_new_tokens
            .max(1)
            .min(max_kv_tokens.saturating_sub(1).max(1))
    };
    let max_prompt_tokens = max_kv_tokens.saturating_sub(max_new_tokens);
    let mut limit_detail = format!(
        "kv={max_kv_tokens} prompt={max_prompt_tokens} new={max_new_tokens}"
    );
    if let Some((requested, limit)) = kv_clamp {
        limit_detail.push_str(&format!(" clamp={requested}->{}", limit.max_tokens));
        if let Some(budget_max) = limit.budget_max {
            if budget_max == limit.max_tokens && budget_max < limit.per_layer_max {
                limit_detail.push_str(&format!(" budget={}", format_bytes(limit.budget_bytes)));
            }
        }
    }
    emit_load_stage(
        &state,
        "token_limits",
        StageStatus::Completed,
        Some(limit_detail),
        None,
        None,
    );

    let sampling_overrides = read_sampling_overrides(&state)?;
    let sampling = parse_sampling_config(sampling_overrides);

    let tokenizer = build_tokenizer(&state, index.as_ref())?;
    let prompt_tokens = encode_prompt(&state, &tokenizer, max_prompt_tokens)?;
    let stop_tokens = collect_stop_tokens(&tokenizer);

    {
        let gguf = gguf_source.clone();
        let state_clone = state.clone();
        let index_clone = index.clone();
        let gpu_clone = gpu.clone();
        spawn_local(async move {
            if let Err(err) = run_q8_0_probe(&state_clone, &gguf, index_clone.as_ref(), &gpu_clone)
                .await
            {
                emit_inference_stage(
                    &state_clone,
                    "q8_0_probe",
                    StageStatus::Failed,
                    None,
                    None,
                    Some(err),
                );
            }
        });
    }
    {
        let gguf = gguf_source.clone();
        let state_clone = state.clone();
        let index_clone = index.clone();
        let gpu_clone = gpu.clone();
        spawn_local(async move {
            if let Err(err) = run_mxfp4_probe(&state_clone, &gguf, index_clone.as_ref(), &gpu_clone)
                .await
            {
                emit_inference_stage(
                    &state_clone,
                    "mxfp4_probe",
                    StageStatus::Failed,
                    None,
                    None,
                    Some(err),
                );
            }
        });
    }
    {
        let gguf = gguf_source.clone();
        let state_clone = state.clone();
        let index_clone = index.clone();
        let gpu_clone = gpu.clone();
        let config_clone = config.clone();
        spawn_local(async move {
            if let Err(err) = run_rmsnorm_probe(
                &state_clone,
                &gguf,
                index_clone.as_ref(),
                &config_clone,
                &gpu_clone,
            )
            .await
            {
                emit_inference_stage(
                    &state_clone,
                    "rmsnorm_probe",
                    StageStatus::Failed,
                    None,
                    None,
                    Some(err),
                );
            }
        });
    }
    {
        let gguf = gguf_source.clone();
        let state_clone = state.clone();
        let index_clone = index.clone();
        let gpu_clone = gpu.clone();
        let config_clone = config.clone();
        spawn_local(async move {
            if let Err(err) =
                run_rope_probe(&state_clone, &gguf, index_clone.as_ref(), &config_clone, &gpu_clone)
                    .await
            {
                emit_inference_stage(
                    &state_clone,
                    "rope_probe",
                    StageStatus::Failed,
                    None,
                    None,
                    Some(err),
                );
            }
        });
    }
    {
        let gguf = gguf_source.clone();
        let state_clone = state.clone();
        let index_clone = index.clone();
        let gpu_clone = gpu.clone();
        let config_clone = config.clone();
        spawn_local(async move {
            if let Err(err) = run_attention_probe(
                &state_clone,
                &gguf,
                index_clone.as_ref(),
                &config_clone,
                &gpu_clone,
            )
            .await
            {
                emit_inference_stage(
                    &state_clone,
                    "attn_probe",
                    StageStatus::Failed,
                    None,
                    None,
                    Some(err),
                );
            }
        });
    }

    let stream_state = state.clone();
    let stream_source = gguf_source.clone();
    let stream_index = index.clone();
    let stream_future = async move {
        stream_full_weights(&stream_state, &stream_source, stream_index.as_ref(), total_bytes)
            .await
    };

    let gen_state = state.clone();
    let gen_source = gguf_source.clone();
    let gen_index = index.clone();
    let gen_future = async move {
        if let Err(err) = run_generation(
            &gen_state,
            &gen_source,
            gen_index.as_ref(),
            &gpu,
            &tokenizer,
            &config,
            &prompt_tokens,
            active_layers,
            moe_fallback,
            max_kv_tokens,
            max_new_tokens,
            force_dense,
            sampling,
            stop_tokens,
        )
        .await
        {
            emit_inference_stage(
                &gen_state,
                "generation",
                StageStatus::Failed,
                None,
                None,
                Some(err),
            );
        }
    };

    let (stream_res, _) = futures::join!(stream_future, gen_future);
    if let Err(err) = stream_res {
        emit_load_stage(
            &state,
            "weights_fetch",
            StageStatus::Failed,
            Some(format!("stream error: {err}")),
            None,
            None,
        );
    }

    if let Ok(mut guard) = state.try_borrow_mut() {
        guard.gptoss.load_active = false;
    }
    Ok(())
}

async fn run_gptoss_pylon(
    state: Rc<RefCell<AppState>>,
    base_url: &str,
    label: &str,
) -> Result<(), String> {
    emit_load_stage(
        &state,
        "pylon_connect",
        StageStatus::Started,
        Some(format!("source={label} url={base_url}")),
        None,
        None,
    );

    let model_id = if let Some(value) = read_query_param("model").filter(|v| !v.is_empty()) {
        value
    } else {
        fetch_pylon_model_id(base_url).await?
    };

    emit_load_stage(
        &state,
        "pylon_connect",
        StageStatus::Completed,
        Some(format!("model={model_id}")),
        None,
        None,
    );

    let (layers, max_kv, max_new) = read_input_overrides(&state)?;
    let sampling_overrides = read_sampling_overrides(&state)?;
    let sampling = parse_sampling_config(sampling_overrides);
    let user_prompt = state
        .try_borrow()
        .ok()
        .and_then(|guard| {
            let value = guard.gptoss.prompt_input.get_value().trim().to_string();
            if value.is_empty() {
                None
            } else {
                Some(value)
            }
        })
        .or_else(|| read_query_param("prompt").filter(|value| !value.is_empty()))
        .unwrap_or_else(default_user_prompt);

    let mut payload = serde_json::Map::new();
    payload.insert("model".to_string(), Value::String(model_id.clone()));
    payload.insert("prompt".to_string(), Value::String(user_prompt));
    payload.insert("stream".to_string(), Value::Bool(true));
    payload.insert(
        "temperature".to_string(),
        Value::from(if sampling.enabled { sampling.temperature } else { 0.0 }),
    );
    payload.insert("top_p".to_string(), Value::from(sampling.top_p));
    payload.insert(
        "top_k".to_string(),
        Value::from(sampling.top_k as u64),
    );
    payload.insert("sample".to_string(), Value::from(sampling.enabled));
    payload.insert("telemetry_top_k".to_string(), Value::from(5u64));
    payload.insert("harmony".to_string(), Value::from(true));

    if let Some(value) = max_new {
        payload.insert("max_tokens".to_string(), Value::from(value as u64));
    }
    if let Some(value) = layers {
        payload.insert("layers".to_string(), Value::from(value as u64));
    }
    if let Some(value) = max_kv {
        payload.insert("max_kv".to_string(), Value::from(value as u64));
    }
    if let Some(flag) = read_query_param("moe") {
        let fallback = matches!(flag.as_str(), "fallback" | "off" | "0");
        payload.insert("moe_fallback".to_string(), Value::from(fallback));
    }

    emit_inference_stage(
        &state,
        "pylon_stream",
        StageStatus::Started,
        None,
        None,
        Some(format!("model={model_id}")),
    );

    let base = base_url.trim_end_matches('/');
    let url = format!("{base}/v1/completions");

    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");
    let body = Value::Object(payload);
    opts.set_body(&JsValue::from_str(
        &serde_json::to_string(&body).map_err(|err| err.to_string())?,
    ));

    let headers = web_sys::Headers::new().map_err(|_| "failed to build headers")?;
    headers
        .set("Content-Type", "application/json")
        .map_err(|_| "failed to set headers")?;
    opts.set_headers(&headers);

    let request =
        web_sys::Request::new_with_str_and_init(&url, &opts).map_err(|_| "bad request")?;
    let window = web_sys::window().ok_or_else(|| "no window".to_string())?;
    let resp_value = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|_| "pylon request failed")?;
    let resp: web_sys::Response = resp_value
        .dyn_into()
        .map_err(|_| "pylon response invalid")?;

    if !resp.ok() {
        let status = resp.status();
        let text = wasm_bindgen_futures::JsFuture::from(resp.text().map_err(|_| "bad body")?)
            .await
            .map_err(|_| "bad body")?
            .as_string()
            .unwrap_or_default();
        return Err(format!("pylon HTTP {status}: {text}"));
    }

    let body_stream = resp
        .body()
        .ok_or_else(|| "pylon response empty".to_string())?;
    let reader: web_sys::ReadableStreamDefaultReader = body_stream
        .get_reader()
        .dyn_into()
        .map_err(|_| "pylon stream invalid".to_string())?;

    let mut buffer = String::new();
    let mut saw_token = false;
    loop {
        let result = wasm_bindgen_futures::JsFuture::from(reader.read())
            .await
            .map_err(|_| "pylon stream read failed".to_string())?;

        let done = js_sys::Reflect::get(&result, &JsValue::from_str("done"))
            .map_err(|_| "pylon stream missing done".to_string())?
            .as_bool()
            .unwrap_or(true);

        if done {
            break;
        }

        let value = js_sys::Reflect::get(&result, &JsValue::from_str("value"))
            .map_err(|_| "pylon stream missing value".to_string())?;
        if value.is_undefined() {
            continue;
        }

        let array: js_sys::Uint8Array = value
            .dyn_into()
            .map_err(|_| "pylon stream chunk invalid".to_string())?;
        let chunk = String::from_utf8_lossy(&array.to_vec()).to_string();
        buffer.push_str(&chunk);

        while let Some(pos) = buffer.find('\n') {
            let mut line = buffer[..pos].to_string();
            buffer = buffer[pos + 1..].to_string();
            if line.ends_with('\r') {
                line.pop();
            }
            if let Some(data) = line.strip_prefix("data:") {
                let data = data.trim();
                if data.is_empty() {
                    continue;
                }
                if data == "[DONE]" {
                    buffer.clear();
                    break;
                }

                let parsed: Value = match serde_json::from_str(data) {
                    Ok(value) => value,
                    Err(_) => continue,
                };

                if let Some(err) = parsed.get("error").and_then(|v| v.as_str()) {
                    return Err(err.to_string());
                }

                let mut handled_token = false;
                if let Some(extra) = parsed.get("extra") {
                    handled_token = apply_pylon_telemetry(&state, extra);
                }

                if handled_token {
                    saw_token = true;
                } else {
                    if let Some(token_text) = extract_pylon_delta(&parsed) {
                        let extra = parsed.get("extra");
                        emit_pylon_token_event(&state, &token_text, extra);
                        if !token_text.is_empty() {
                            saw_token = true;
                        }
                    }
                }
            }
        }
    }

    if !saw_token {
        emit_inference_stage(
            &state,
            "pylon_stream",
            StageStatus::Failed,
            None,
            None,
            Some("no tokens streamed".to_string()),
        );
    } else {
        emit_inference_stage(
            &state,
            "pylon_stream",
            StageStatus::Completed,
            None,
            None,
            None,
        );
    }

    if let Ok(mut guard) = state.try_borrow_mut() {
        guard.gptoss.load_active = false;
    }
    Ok(())
}

async fn fetch_pylon_model_id(base_url: &str) -> Result<String, String> {
    let base = base_url.trim_end_matches('/');
    let url = format!("{base}/v1/models");
    let window = web_sys::window().ok_or_else(|| "no window".to_string())?;
    let resp_value = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str(&url))
        .await
        .map_err(|_| "pylon model list failed".to_string())?;
    let resp: web_sys::Response = resp_value
        .dyn_into()
        .map_err(|_| "pylon model list invalid".to_string())?;
    if !resp.ok() {
        let status = resp.status();
        return Err(format!("pylon models HTTP {status}"));
    }
    let text = wasm_bindgen_futures::JsFuture::from(resp.text().map_err(|_| "bad body")?)
        .await
        .map_err(|_| "bad body")?
        .as_string()
        .unwrap_or_default();
    let value: Value = serde_json::from_str(&text).map_err(|_| "pylon model list parse failed".to_string())?;
    let data = value
        .get("data")
        .and_then(|data| data.as_array())
        .ok_or_else(|| "pylon model list empty".to_string())?;
    let mut fallback: Option<String> = None;
    for entry in data {
        let Some(id) = entry.get("id").and_then(|id| id.as_str()) else {
            continue;
        };
        if fallback.is_none() {
            fallback = Some(id.to_string());
        }
        if id.to_ascii_lowercase().contains("gpt-oss") {
            return Ok(id.to_string());
        }
    }
    fallback.ok_or_else(|| "pylon model list empty".to_string())
}

fn apply_pylon_telemetry(state: &Rc<RefCell<AppState>>, extra: &Value) -> bool {
    let Some(value) = extra.get("telemetry") else {
        return false;
    };
    let mut handled_token = false;
    if let Some(items) = value.as_array() {
        for item in items {
            handled_token |= push_pylon_telemetry(state, item);
        }
    } else {
        handled_token |= push_pylon_telemetry(state, value);
    }
    handled_token
}

fn push_pylon_telemetry(state: &Rc<RefCell<AppState>>, value: &Value) -> bool {
    let event = match serde_json::from_value::<GptOssTelemetry>(value.clone()) {
        Ok(event) => event,
        Err(_) => return false,
    };
    let handled_token = matches!(
        event,
        GptOssTelemetry::InferenceEvent {
            event: GptOssInferenceTelemetry::TokenGenerated { .. },
            ..
        }
    );
    push_gptoss_event(state, event);
    handled_token
}

fn extract_pylon_delta(parsed: &Value) -> Option<String> {
    let choices = parsed.get("choices")?.as_array()?;
    let choice = choices.first()?;
    if let Some(text) = choice.get("text").and_then(|v| v.as_str()) {
        return Some(text.to_string());
    }
    if let Some(delta) = choice.get("delta") {
        if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
            return Some(content.to_string());
        }
        if let Some(text) = delta.as_str() {
            return Some(text.to_string());
        }
    }
    None
}

fn emit_pylon_token_event(state: &Rc<RefCell<AppState>>, token_text: &str, extra: Option<&Value>) {
    let token_id = extra
        .and_then(|value| value.get("token_id"))
        .and_then(|value| value.as_u64())
        .unwrap_or(0) as u32;
    let entropy = extra
        .and_then(|value| value.get("entropy"))
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0) as f32;
    let tokens_per_sec = extra
        .and_then(|value| value.get("tokens_per_sec"))
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0) as f32;
    let top_k = extra
        .and_then(|value| value.get("top_k"))
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some(GptOssTokenCandidate {
                        token_id: item.get("token_id")?.as_u64()? as u32,
                        token_text: item.get("token_text")?.as_str()?.to_string(),
                        probability: item.get("probability")?.as_f64()? as f32,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    emit_inference_event(
        state,
        GptOssInferenceTelemetry::TokenGenerated {
            token_id,
            token_text: token_text.to_string(),
            top_k,
            entropy,
            tokens_per_sec,
        },
    );
}

async fn stream_full_weights(
    state: &Rc<RefCell<AppState>>,
    gguf_source: &GgufSource,
    index: &GgufIndex,
    total_bytes: u64,
) -> Result<(), String> {
    let start_ms = now_ms();
    emit_load_stage(
        state,
        "weights_fetch",
        StageStatus::Started,
        Some(format!("total={}", format_bytes(total_bytes))),
        Some(0),
        Some(total_bytes),
    );

    let mut offset = 0u64;
    let mut loaded = 0u64;
    let mut next_progress = PROGRESS_STEP_BYTES;
    let mut chunk_idx = 0u64;
    let mut tensor_cursor = tensor_start_cursor(index);
    let mut tensor_emitted = 0usize;

    while offset < total_bytes {
        let len = (total_bytes - offset).min(LOAD_CHUNK_BYTES);
        let chunk = fetch_range_source(gguf_source, offset, len).await?;
        loaded = loaded.saturating_add(chunk.len() as u64);
        offset = offset.saturating_add(len);
        chunk_idx = chunk_idx.saturating_add(1);

        if loaded >= next_progress || loaded >= total_bytes {
            let now = now_ms();
            let elapsed_ms = now.saturating_sub(start_ms).max(1);
            let rate_value = loaded as f64 / (elapsed_ms as f64 / 1000.0);
            let rate = format_rate(rate_value);
            let eta = if rate_value > 0.0 {
                let remaining = total_bytes.saturating_sub(loaded) as f64;
                format!("{:.1}s", remaining / rate_value)
            } else {
                "--".to_string()
            };
            emit_load_stage(
                state,
                "weights_fetch",
                StageStatus::Progress,
                Some(format!(
                    "chunk={} offset={} rate={} eta={}",
                    chunk_idx,
                    format_bytes(offset),
                    rate,
                    eta
                )),
                Some(loaded),
                Some(total_bytes),
            );
            next_progress = next_progress.saturating_add(PROGRESS_STEP_BYTES);
        }

        while let Some((next_offset, name)) = tensor_cursor.first().cloned() {
            if offset < next_offset {
                break;
            }
            tensor_cursor.remove(0);
            tensor_emitted = tensor_emitted.saturating_add(1);
            if tensor_emitted % 6 == 0 || tensor_emitted <= 12 {
                emit_load_stage(
                    state,
                    "tensor_scan",
                    StageStatus::Progress,
                    Some(name),
                    Some(loaded),
                    Some(total_bytes),
                );
            }
        }

        yield_to_browser().await;
    }

    emit_load_stage(
        state,
        "weights_fetch",
        StageStatus::Completed,
        Some(format!(
            "loaded={} elapsed={:.1}s",
            format_bytes(loaded),
            (now_ms().saturating_sub(start_ms) as f32 / 1000.0).max(0.1)
        )),
        Some(loaded),
        Some(total_bytes),
    );

    emit_load_stage(
        state,
        "load_complete",
        StageStatus::Completed,
        None,
        Some(loaded),
        Some(total_bytes),
    );
    Ok(())
}

fn reset_gptoss_state(state: &mut crate::state::GptOssVizState) {
    clear_gptoss_events();
    state.load_stages.clear();
    state.inference_stages.clear();
    state.events.clear();
    state.token_stream.clear();
    state.last_token_id = None;
    state.top_k.clear();
    state.probability_history.clear();
    state.tokens_per_sec = None;
    state.entropy = None;
    state.entropy_history.clear();
    state.memory_usage = None;
    state.gpu_limits = None;
    state.token_limits = None;
    state.cache_status.clear();
    state.resident_tensors.clear();
    state.recent_tensors.clear();
    state.attention_weights = None;
    state.attention_layer = 0;
    state.attention_head = 0;
    state.attention_selected_layer = 0;
    state.attention_selected_head = 0;
    state.layer_activations.clear();
    state.max_layers = 1;
    state.max_heads = 1;
    state.layer_slider_bounds = wgpui::Bounds::ZERO;
    state.head_slider_bounds = wgpui::Bounds::ZERO;
    state.layer_slider_dragging = false;
    state.head_slider_dragging = false;
    state.drop_active = false;
    state.attention_mode = None;
    state.moe_mode = None;
    state.sampling_mode = None;
    state.cpu_fallback = None;
    state.active_layers = None;
    state.load_progress = None;
    state.current_stage = None;
    state.load_error = None;
    state.load_url = None;
    state.inference_error = None;
    state.last_token_ts_ms = None;
    state.start_ts_ms = None;
}

fn emit_tensor_scan(state: &Rc<RefCell<AppState>>, index: &GgufIndex, limit: usize) {
    for (idx, tensor) in index.tensors.iter().take(limit).enumerate() {
        emit_load_stage(
            state,
            "tensor_index",
            StageStatus::Progress,
            Some(format!("{}: {}", idx + 1, tensor.name)),
            None,
            None,
        );
    }
}

fn emit_metadata_keys(state: &Rc<RefCell<AppState>>, index: &GgufIndex, limit: usize) {
    let mut keys = index.metadata.values.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    for (idx, key) in keys.iter().take(limit).enumerate() {
        emit_load_stage(
            state,
            "gguf_meta",
            StageStatus::Progress,
            Some(format!("{}: {}", idx + 1, key)),
            None,
            None,
        );
    }
}

fn emit_load_stage(
    state: &Rc<RefCell<AppState>>,
    stage: &str,
    status: StageStatus,
    detail: Option<String>,
    bytes: Option<u64>,
    total_bytes: Option<u64>,
) {
    push_gptoss_event(
        state,
        GptOssTelemetry::LoadStage {
            stage: stage.to_string(),
            status,
            detail,
            bytes,
            total_bytes,
            ts_ms: Some(now_ms()),
        },
    );
}

fn emit_inference_stage(
    state: &Rc<RefCell<AppState>>,
    stage: &str,
    status: StageStatus,
    step: Option<usize>,
    total_steps: Option<usize>,
    detail: Option<String>,
) {
    push_gptoss_event(
        state,
        GptOssTelemetry::InferenceStage {
            stage: stage.to_string(),
            status,
            step,
            total_steps,
            detail,
            ts_ms: Some(now_ms()),
        },
    );
}

fn emit_inference_event(
    state: &Rc<RefCell<AppState>>,
    event: GptOssInferenceTelemetry,
) {
    push_gptoss_event(
        state,
        GptOssTelemetry::InferenceEvent {
            event,
            ts_ms: Some(now_ms()),
        },
    );
}

fn emit_tensor_resident(state: &Rc<RefCell<AppState>>, name: String, bytes: usize, kind: &str) {
    emit_inference_event(
        state,
        GptOssInferenceTelemetry::TensorResident {
            name,
            bytes,
            kind: kind.to_string(),
        },
    );
}

fn emit_gpu_limits(state: &Rc<RefCell<AppState>>, gpu: &GpuContext) {
    let limits = gpu.device.limits();
    let features = gpu.device.features();
    let shader_f16 = features.contains(wgpu::Features::SHADER_F16);
    let detail = format!(
        "max_storage={} max_buffer={} bind_groups={} bindings_per_group={} storage_bindings={} dynamic_storage={} uniform_bindings={} f16={}",
        format_bytes(limits.max_storage_buffer_binding_size as u64),
        format_bytes(limits.max_buffer_size as u64),
        limits.max_bind_groups,
        limits.max_bindings_per_bind_group,
        limits.max_storage_buffers_per_shader_stage,
        limits.max_dynamic_storage_buffers_per_pipeline_layout,
        limits.max_uniform_buffers_per_shader_stage,
        shader_f16,
    );
    emit_load_stage(
        state,
        "gpu_limits",
        StageStatus::Completed,
        Some(detail),
        None,
        None,
    );
}

