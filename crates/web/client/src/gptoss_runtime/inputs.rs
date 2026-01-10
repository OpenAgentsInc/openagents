fn encode_prompt(
    state: &Rc<RefCell<AppState>>,
    tokenizer: &GptOssTokenizer,
    max_prompt_tokens: usize,
) -> Result<Vec<u32>, String> {
    if max_prompt_tokens == 0 {
        return Err("prompt token limit is zero (raise max_kv or lower max_new)".to_string());
    }
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
    let prompt = build_harmony_prompt(&user_prompt);

    emit_inference_stage(
        state,
        "prompt_encode",
        StageStatus::Started,
        Some(0),
        None,
        Some(format!("format=harmony chars={}", prompt.len())),
    );

    let mut tokens = tokenizer.encode_with_special_tokens(&prompt)?;
    let total = tokens.len();
    let mut truncated = 0usize;
    if total > max_prompt_tokens {
        truncated = total - max_prompt_tokens;
        tokens = tokens.split_off(total - max_prompt_tokens);
    }
    if tokens.is_empty() {
        return Err("prompt token list is empty".to_string());
    }

    emit_inference_stage(
        state,
        "prompt_encode",
        StageStatus::Completed,
        Some(tokens.len()),
        Some(tokens.len()),
        Some(format!(
            "format=harmony tokens={total} kept={} truncated={truncated}",
            tokens.len(),
        )),
    );

    Ok(tokens)
}

fn now_ms() -> u64 {
    js_sys::Date::now().max(0.0) as u64
}

async fn yield_to_browser() {
    TimeoutFuture::new(0).await;
}

pub(crate) fn read_query_param(key: &str) -> Option<String> {
    let window = web_sys::window()?;
    let search = window.location().search().ok()?;
    let params = web_sys::UrlSearchParams::new_with_str(&search).ok()?;
    params.get(key)
}

fn read_query_usize(key: &str) -> Option<usize> {
    read_query_param(key)
        .and_then(|value| value.parse::<usize>().ok())
}

fn read_query_f32(key: &str) -> Option<f32> {
    read_query_param(key)
        .and_then(|value| value.parse::<f32>().ok())
}

fn parse_usize_override(raw: &str, label: &str) -> Result<Option<usize>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    trimmed
        .parse::<usize>()
        .map(Some)
        .map_err(|_| format!("invalid {label} value: {trimmed}"))
}

fn parse_layers_override(raw: &str) -> Result<Option<usize>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("all") {
        return Ok(None);
    }
    trimmed
        .parse::<usize>()
        .map(Some)
        .map_err(|_| format!("invalid layers value: {trimmed}"))
}

fn parse_bool_override(raw: &str, label: &str) -> Result<Option<bool>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let value = match trimmed.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => true,
        "0" | "false" | "no" | "off" => false,
        _ => {
            return Err(format!(
                "invalid {label} value: {trimmed} (use on/off)"
            ))
        }
    };
    Ok(Some(value))
}

fn parse_f32_override(raw: &str, label: &str) -> Result<Option<f32>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    trimmed
        .parse::<f32>()
        .map(Some)
        .map_err(|_| format!("invalid {label} value: {trimmed}"))
}

fn read_input_overrides(
    state: &Rc<RefCell<AppState>>,
) -> Result<(Option<usize>, Option<usize>, Option<usize>), String> {
    let Ok(guard) = state.try_borrow() else {
        return Ok((None, None, None));
    };
    let layers = parse_layers_override(guard.gptoss.layers_input.get_value())?;
    let max_kv = parse_usize_override(guard.gptoss.max_kv_input.get_value(), "max_kv")?;
    let max_new = parse_usize_override(guard.gptoss.max_new_input.get_value(), "max_new")?;
    Ok((layers, max_kv, max_new))
}

fn read_sampling_overrides(state: &Rc<RefCell<AppState>>) -> Result<SamplingOverrides, String> {
    let Ok(guard) = state.try_borrow() else {
        return Ok(SamplingOverrides::default());
    };
    Ok(SamplingOverrides {
        enabled: parse_bool_override(guard.gptoss.sample_input.get_value(), "sample")?,
        temperature: parse_f32_override(guard.gptoss.temp_input.get_value(), "temp")?,
        top_k: parse_usize_override(guard.gptoss.top_k_input.get_value(), "top_k")?,
        top_p: parse_f32_override(guard.gptoss.top_p_input.get_value(), "top_p")?,
    })
}

pub(crate) fn default_gguf_url() -> String {
    let window = match web_sys::window() {
        Some(window) => window,
        None => return String::new(),
    };
    let location = window.location();
    let host = location.hostname().ok();
    let local = matches!(host.as_deref(), Some("localhost") | Some("127.0.0.1"));
    if local {
        PYLON_SOURCE_LABEL.to_string()
    } else {
        String::new()
    }
}

fn is_file_input(raw: &str) -> bool {
    let trimmed = raw.trim().to_ascii_lowercase();
    trimmed == "file" || trimmed.starts_with("file:")
}

pub(crate) fn gguf_file_input_label(file: &web_sys::File) -> String {
    format!("file:{}", file.name())
}

pub(crate) fn gguf_file_label(file: &web_sys::File) -> String {
    format!("file:{} ({})", file.name(), format_bytes(file.size() as u64))
}

fn normalize_gguf_url(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Ok(trimmed.to_string());
    }
    if trimmed.starts_with('/') {
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("/users/") || lower.starts_with("/home/") {
            return Err(format!(
                "Local file paths are not supported in the browser.\nClick PICK FILE, drop a GGUF, or run: {LOCAL_GGUF_SERVE_CMD}\nThen use: {LOCAL_GGUF_URL}"
            ));
        }
        let window = web_sys::window().ok_or_else(|| "no window".to_string())?;
        let origin = window
            .location()
            .origin()
            .map_err(|_| "failed to read window origin".to_string())?;
        return Ok(format!("{origin}{trimmed}"));
    }
    if trimmed.starts_with("file://") || trimmed.starts_with('~') {
        return Err(format!(
            "Local file paths are not supported in the browser.\nClick PICK FILE, drop a GGUF, or run: {LOCAL_GGUF_SERVE_CMD}\nThen use: {LOCAL_GGUF_URL}"
        ));
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("localhost")
        || lower.starts_with("127.0.0.1")
        || lower.starts_with("0.0.0.0")
    {
        return Ok(format!("http://{trimmed}"));
    }
    Err("GGUF URL must start with http:// or https://".to_string())
}

fn parse_pylon_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower == "pylon" || lower == "pylon://" || lower == "pylon:" {
        return Some(PYLON_API_URL.to_string());
    }
    if lower.starts_with("pylon://") {
        let rest = trimmed[8..].trim();
        return Some(normalize_pylon_base(rest));
    }
    if lower.starts_with("pylon:") {
        let rest = trimmed[6..].trim();
        return Some(normalize_pylon_base(rest));
    }
    None
}

fn normalize_pylon_base(rest: &str) -> String {
    if rest.is_empty() {
        return PYLON_API_URL.to_string();
    }
    if rest.starts_with("http://") || rest.starts_with("https://") {
        return rest.to_string();
    }
    format!("http://{rest}")
}

fn pylon_label_from_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if let Some(stripped) = trimmed.strip_prefix("http://") {
        return format!("pylon://{stripped}");
    }
    if let Some(stripped) = trimmed.strip_prefix("https://") {
        return format!("pylon+https://{stripped}");
    }
    format!("pylon://{trimmed}")
}

pub(crate) fn local_gguf_path() -> &'static str {
    LOCAL_GGUF_PATH
}

pub(crate) fn local_gguf_url() -> &'static str {
    LOCAL_GGUF_URL
}

pub(crate) fn local_gguf_dev_url() -> &'static str {
    LOCAL_GGUF_DEV_URL
}

pub(crate) fn local_gguf_serve_cmd() -> &'static str {
    LOCAL_GGUF_SERVE_CMD
}

pub(crate) fn default_user_prompt() -> String {
    DEFAULT_USER_PROMPT.to_string()
}

pub(crate) fn default_max_kv_tokens() -> usize {
    DEFAULT_MAX_KV_TOKENS
}

pub(crate) fn default_max_new_tokens() -> usize {
    DEFAULT_MAX_NEW_TOKENS
}

pub(crate) fn default_sample_temp() -> f32 {
    DEFAULT_SAMPLE_TEMP
}

pub(crate) fn default_sample_top_k() -> usize {
    DEFAULT_SAMPLE_TOP_K
}

pub(crate) fn default_sample_top_p() -> f32 {
    DEFAULT_SAMPLE_TOP_P
}

fn is_local_url(url: &str) -> bool {
    let url = url.to_ascii_lowercase();
    url.starts_with("http://localhost")
        || url.starts_with("http://127.0.0.1")
        || url.starts_with("https://localhost")
        || url.starts_with("https://127.0.0.1")
}

fn is_bun_dev_url(url: &str) -> bool {
    let url = url.to_ascii_lowercase();
    url.starts_with("http://localhost:3000")
        || url.starts_with("http://127.0.0.1:3000")
}

fn format_source_error(source: &GgufSource, label: &str, err: &str) -> String {
    if source.is_file() {
        return format!("Local GGUF read failed ({label}): {err}");
    }
    format_range_error(label, err)
}

fn format_range_error(url: &str, err: &str) -> String {
    let lower = err.to_ascii_lowercase();
    let detail = if lower.contains("fetch failed: 404") || lower.contains(" 404") {
        format!("GGUF not found at {url}")
    } else if lower.contains("fetch failed: 416") || lower.contains(" 416") {
        format!("Range request rejected by {url}")
    } else if lower.contains("failed to fetch")
        || lower.contains("networkerror")
        || lower.contains("load failed")
    {
        format!("Cannot connect to {url}")
    } else {
        format!("Range/CORS check failed for {url}: {err}")
    };

    if is_bun_dev_url(url) {
        format!("{detail}\nRun: cd crates/web && bun run build && bun run dev")
    } else if is_local_url(url) {
        format!("{detail}\nRun: {LOCAL_GGUF_SERVE_CMD}\nOr click PICK FILE / drop a GGUF")
    } else if detail.contains("Range/CORS") {
        detail
    } else {
        format!("{detail}. Host must support Range + CORS.")
    }
}

fn build_harmony_prompt(user_prompt: &str) -> String {
    let system_prompt = format!(
        "You are ChatGPT, a large language model trained by OpenAI.\n\
Knowledge cutoff: 2024-06\n\
Current date: {CURRENT_DATE}\n\n\
Reasoning: low\n\n\
# Valid channels: analysis, commentary, final. Channel must be included for every message."
    );
    let developer_prompt = if DEFAULT_DEVELOPER_PROMPT.trim().is_empty() {
        None
    } else {
        Some(format!("# Instructions\n\n{DEFAULT_DEVELOPER_PROMPT}"))
    };

    let mut prompt = String::new();
    prompt.push_str("<|start|>system<|message|>");
    prompt.push_str(&system_prompt);
    if let Some(developer_prompt) = developer_prompt {
        prompt.push_str("<|end|><|start|>developer<|message|>");
        prompt.push_str(&developer_prompt);
    }
    prompt.push_str("<|end|><|start|>user<|message|>");
    prompt.push_str(user_prompt);
    prompt.push_str("<|end|><|start|>assistant");
    prompt
}

fn parse_sampling_config(overrides: SamplingOverrides) -> SamplingConfig {
    let mut enabled = false;
    let input_present = overrides.enabled.is_some()
        || overrides.temperature.is_some()
        || overrides.top_k.is_some()
        || overrides.top_p.is_some();

    let temp = if let Some(value) = overrides.temperature {
        enabled = true;
        value
    } else if !input_present {
        read_query_f32("temp")
            .map(|value| {
                enabled = true;
                value
            })
            .unwrap_or(1.0)
    } else {
        1.0
    };

    let top_k = if let Some(value) = overrides.top_k {
        enabled = true;
        value
    } else if !input_present {
        read_query_usize("top_k")
            .map(|value| {
                enabled = true;
                value
            })
            .unwrap_or(DEFAULT_SAMPLE_TOP_K)
    } else {
        DEFAULT_SAMPLE_TOP_K
    };

    let top_p = if let Some(value) = overrides.top_p {
        enabled = true;
        value
    } else if !input_present {
        read_query_f32("top_p")
            .map(|value| {
                enabled = true;
                value
            })
            .unwrap_or(1.0)
    } else {
        1.0
    };

    if let Some(flag) = overrides.enabled {
        enabled = flag;
    } else if !input_present {
        if let Some(flag) = read_query_param("sample") {
            enabled = matches!(flag.as_str(), "1" | "true" | "yes" | "on");
        }
    }

    SamplingConfig {
        enabled,
        temperature: temp.max(1e-4),
        top_k,
        top_p: top_p.clamp(0.0, 1.0),
    }
}

