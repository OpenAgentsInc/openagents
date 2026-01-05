use std::collections::HashSet;
use std::env;
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use openai_harmony::chat::{Conversation, Message, Role, SystemContent};
use openai_harmony::{HarmonyEncoding, HarmonyEncodingName, load_harmony_encoding};
use rand::rngs::OsRng;
use rand::RngCore;
use tracing::{info, warn};

use crate::error::{GptOssMetalError, Result};
use crate::ffi;

const DEFAULT_CONTEXT_LENGTH: usize = 8192;
const DEFAULT_MAX_BATCH_TOKENS: usize = 128;
const DEFAULT_MAX_TOKENS: usize = 256;
const DEFAULT_TEMPERATURE: f32 = 0.7;
const DEFAULT_SAMPLE_CHUNK_TOKENS: usize = 8;

#[derive(Debug, Clone)]
pub struct GptOssMetalConfig {
    pub model_path: PathBuf,
    pub model_id: String,
    pub context_length: Option<usize>,
    pub max_batch_tokens: Option<usize>,
    pub default_temperature: f32,
    pub default_max_tokens: usize,
    pub seed: Option<u64>,
}

impl GptOssMetalConfig {
    pub fn from_env() -> Result<Self> {
        ensure_tiktoken_cache_dir();

        let model_path = match env::var("GPT_OSS_METAL_MODEL_PATH") {
            Ok(value) => PathBuf::from(value),
            Err(_) => default_model_path().ok_or_else(|| {
                GptOssMetalError::InvalidConfig(
                    "GPT_OSS_METAL_MODEL_PATH is not set and no default model.bin was found \
                    (tried ~/models/gpt-oss-20b/metal/model.bin and \
                    ~/models/gpt-oss-120b/metal/model.bin)"
                        .to_string(),
                )
            })?,
        };

        let model_id = env::var("GPT_OSS_METAL_MODEL_ID")
            .ok()
            .or_else(|| infer_model_id(&model_path))
            .unwrap_or_else(|| "gpt-oss-metal".to_string());

        let context_length = parse_usize("GPT_OSS_METAL_CONTEXT_LENGTH")?;
        let max_batch_tokens =
            parse_usize("GPT_OSS_METAL_MAX_BATCH_TOKENS")?.filter(|value| *value > 0);
        let default_max_tokens = parse_usize("GPT_OSS_METAL_MAX_TOKENS")?
            .unwrap_or(DEFAULT_MAX_TOKENS);
        let default_temperature = parse_f32("GPT_OSS_METAL_TEMPERATURE")?
            .unwrap_or(DEFAULT_TEMPERATURE);
        let seed = parse_u64("GPT_OSS_METAL_SEED")?;

        Ok(Self {
            model_path,
            model_id,
            context_length,
            max_batch_tokens,
            default_temperature,
            default_max_tokens,
            seed,
        })
    }
}

#[derive(Debug, Clone)]
pub struct GptOssMetalCompletion {
    pub text: String,
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub finish_reason: String,
}

pub struct GptOssMetalEngine {
    config: GptOssMetalConfig,
    model: Arc<Mutex<Model>>,
    encoding: HarmonyEncoding,
    stop_tokens: HashSet<u32>,
    context_length: usize,
}

impl GptOssMetalEngine {
    pub fn new(config: GptOssMetalConfig) -> Result<Self> {
        let model = Model::load(&config.model_path)?;
        let max_context = model.max_context_length;
        let mut context_length = config
            .context_length
            .unwrap_or(DEFAULT_CONTEXT_LENGTH);

        if context_length > max_context {
            if config.context_length.is_some() {
                return Err(GptOssMetalError::InvalidConfig(format!(
                    "context length {context_length} exceeds model max {max_context}"
                )));
            }
            info!(
                context_length,
                max_context,
                "Default context length exceeds model max; clamping"
            );
            context_length = max_context;
        }

        if context_length == 0 {
            return Err(GptOssMetalError::InvalidConfig(
                "context length must be greater than 0".to_string(),
            ));
        }

        if let Some(max_batch_tokens) = config.max_batch_tokens {
            if max_batch_tokens > context_length {
                return Err(GptOssMetalError::InvalidConfig(format!(
                    "max batch tokens {max_batch_tokens} exceeds context length {context_length}"
                )));
            }
        }

        let encoding = load_harmony_encoding(HarmonyEncodingName::HarmonyGptOss)
            .map_err(|err| GptOssMetalError::HarmonyError(err.to_string()))?;

        let stop_tokens = encoding
            .stop_tokens_for_assistant_actions()
            .map_err(|err| GptOssMetalError::HarmonyError(err.to_string()))?
            .into_iter()
            .collect::<HashSet<_>>();

        info!(
            model_id = %config.model_id,
            model_path = %config.model_path.display(),
            context_length,
            max_context,
            max_batch_tokens = config.max_batch_tokens.unwrap_or(DEFAULT_MAX_BATCH_TOKENS),
            default_max_tokens = config.default_max_tokens,
            default_temperature = config.default_temperature,
            "GPT-OSS Metal engine initialized"
        );

        Ok(Self {
            config,
            model: Arc::new(Mutex::new(model)),
            encoding,
            stop_tokens,
            context_length,
        })
    }

    pub fn model_id(&self) -> &str {
        &self.config.model_id
    }

    pub fn context_length(&self) -> usize {
        self.context_length
    }

    pub fn generate_with_callback<F>(
        &self,
        prompt: &str,
        max_tokens: Option<usize>,
        temperature: Option<f32>,
        stop_sequences: Option<&[String]>,
        use_harmony_prompt: bool,
        mut on_token: F,
    ) -> Result<GptOssMetalCompletion>
    where
        F: FnMut(&str) -> Result<()>,
    {
        let tokenize_start = Instant::now();
        let prompt_tokens = self.render_prompt_tokens(prompt, use_harmony_prompt)?;
        let prompt_token_count = prompt_tokens.len();
        let tokenize_elapsed = tokenize_start.elapsed();

        let available_tokens = self.context_length.saturating_sub(prompt_tokens.len());
        if available_tokens == 0 {
            return Err(GptOssMetalError::InvalidConfig(
                "prompt exceeds context length".to_string(),
            ));
        }

        let requested_max_tokens = max_tokens.unwrap_or(self.config.default_max_tokens);
        let max_tokens = requested_max_tokens.min(available_tokens);
        if max_tokens < requested_max_tokens {
            info!(
                requested_max_tokens,
                max_tokens,
                "Clamped max tokens to fit context length"
            );
        }
        let temperature = temperature.unwrap_or(self.config.default_temperature);
        let stop_sequences = stop_sequences.unwrap_or(&[]);
        let sample_chunk_tokens = parse_usize("GPT_OSS_METAL_SAMPLE_CHUNK_TOKENS")?
            .unwrap_or(DEFAULT_SAMPLE_CHUNK_TOKENS)
            .max(1);

        let seed = self
            .config
            .seed
            .unwrap_or_else(|| OsRng.next_u64());

        let mut max_batch_tokens = self
            .config
            .max_batch_tokens
            .unwrap_or(DEFAULT_MAX_BATCH_TOKENS)
            .max(prompt_token_count);
        if let Some(configured) = self.config.max_batch_tokens {
            if configured < prompt_token_count {
                warn!(
                    configured,
                    prompt_token_count,
                    "max batch tokens below prompt length; bumping to prompt length"
                );
            }
        }
        if max_batch_tokens > self.context_length {
            max_batch_tokens = self.context_length;
        }

        info!(
            model_id = %self.config.model_id,
            prompt_tokens = prompt_token_count,
            max_tokens,
            temperature,
            stop_sequences = stop_sequences.len(),
            context_length = self.context_length,
            max_batch_tokens,
            harmony = use_harmony_prompt,
            tokenize_ms = tokenize_elapsed.as_millis(),
            sample_chunk_tokens,
            "GPT-OSS Metal inference started"
        );

        let model = self.model.lock().map_err(|_| {
            GptOssMetalError::FfiError("model mutex poisoned".to_string())
        })?;

        let context = Context::create(
            &model,
            self.context_length,
            max_batch_tokens,
        )?;
        context.append_tokens(&prompt_tokens)?;

        let mut output = String::new();
        let mut sent_len = 0usize;
        let mut completion_tokens = 0usize;
        let mut finish_reason = "length".to_string();
        let generation_start = Instant::now();
        let mut first_token_at = None;
        let mut remaining_tokens = max_tokens;
        let mut stop_generation = false;

        while remaining_tokens > 0 {
            let step_tokens = sample_chunk_tokens.min(remaining_tokens);

            let tokens = context.sample(temperature, seed, step_tokens)?;
            if tokens.is_empty() {
                break;
            }

            for token in tokens {
                if self.stop_tokens.contains(&token) {
                    finish_reason = "stop".to_string();
                    stop_generation = true;
                    break;
                }

                let token_text = self
                    .encoding
                    .tokenizer()
                    .decode_utf8(&[token])
                    .map_err(|err| GptOssMetalError::HarmonyError(err.to_string()))?;

                if first_token_at.is_none() {
                    let now = Instant::now();
                    first_token_at = Some(now);
                    info!(
                        first_token_ms = now.duration_since(generation_start).as_millis(),
                        "GPT-OSS Metal time to first token"
                    );
                }

                output.push_str(&token_text);
                completion_tokens += 1;
                remaining_tokens = remaining_tokens.saturating_sub(1);

                if let Some(trim_idx) = find_stop_index(&output, stop_sequences) {
                    if trim_idx > sent_len {
                        on_token(&output[sent_len..trim_idx])?;
                    }
                    output.truncate(trim_idx);
                    finish_reason = "stop".to_string();
                    stop_generation = true;
                    break;
                }

                if output.len() > sent_len {
                    on_token(&output[sent_len..])?;
                    sent_len = output.len();
                }

                if remaining_tokens == 0 {
                    break;
                }
            }

            if stop_generation {
                break;
            }
        }

        let generation_elapsed = generation_start.elapsed();
        if completion_tokens > 0 {
            let seconds = generation_elapsed.as_secs_f64().max(1e-6);
            let tokens_per_second = completion_tokens as f64 / seconds;
            info!(
                tokens_per_second,
                generation_ms = generation_elapsed.as_millis(),
                "GPT-OSS Metal throughput"
            );
        }

        let completion = GptOssMetalCompletion {
            text: output,
            prompt_tokens: prompt_token_count,
            completion_tokens,
            finish_reason,
        };

        info!(
            model_id = %self.config.model_id,
            prompt_tokens = completion.prompt_tokens,
            completion_tokens = completion.completion_tokens,
            finish_reason = %completion.finish_reason,
            "GPT-OSS Metal inference completed"
        );

        Ok(completion)
    }

    fn render_prompt_tokens(&self, prompt: &str, use_harmony_prompt: bool) -> Result<Vec<u32>> {
        if use_harmony_prompt {
            let mut messages = Vec::new();
            let system = SystemContent::new();
            messages.push(Message::from_role_and_content(Role::System, system));
            messages.push(Message::from_role_and_content(Role::User, prompt.to_string()));

            let convo = Conversation::from_messages(messages);
            self.encoding
                .render_conversation_for_completion(&convo, Role::Assistant, None)
                .map_err(|err| GptOssMetalError::HarmonyError(err.to_string()))
        } else {
            Ok(self
                .encoding
                .tokenizer()
                .encode_with_special_tokens(prompt))
        }
    }
}

struct Model {
    ptr: NonNull<ffi::gptoss_model>,
    max_context_length: usize,
}

unsafe impl Send for Model {}

impl Model {
    fn load(path: &Path) -> Result<Self> {
        let c_path = path_to_cstring(path)?;
        let mut model_ptr = std::ptr::null_mut();
        let status = unsafe { ffi::gptoss_model_create_from_file(c_path.as_ptr(), &mut model_ptr) };
        check_status(status, "gptoss_model_create_from_file")?;

        let model_ptr = NonNull::new(model_ptr).ok_or_else(|| {
            GptOssMetalError::FfiError("model pointer was null".to_string())
        })?;

        let mut max_context_length = 0usize;
        let status = unsafe {
            ffi::gptoss_model_get_max_context_length(model_ptr.as_ptr(), &mut max_context_length)
        };
        check_status(status, "gptoss_model_get_max_context_length")?;

        Ok(Self {
            ptr: model_ptr,
            max_context_length,
        })
    }
}

impl Drop for Model {
    fn drop(&mut self) {
        unsafe {
            let _ = ffi::gptoss_model_release(self.ptr.as_ptr());
        }
    }
}

struct Context {
    ptr: NonNull<ffi::gptoss_context>,
}

impl Context {
    fn create(model: &Model, context_length: usize, max_batch_tokens: usize) -> Result<Self> {
        let mut context_ptr = std::ptr::null_mut();
        let status = unsafe {
            ffi::gptoss_context_create(
                model.ptr.as_ptr(),
                context_length,
                max_batch_tokens,
                &mut context_ptr,
            )
        };
        check_status(status, "gptoss_context_create")?;
        let context_ptr = NonNull::new(context_ptr).ok_or_else(|| {
            GptOssMetalError::FfiError("context pointer was null".to_string())
        })?;

        Ok(Self { ptr: context_ptr })
    }

    fn append_tokens(&self, tokens: &[u32]) -> Result<()> {
        if tokens.is_empty() {
            return Ok(());
        }

        let status = unsafe {
            ffi::gptoss_context_append_tokens(self.ptr.as_ptr(), tokens.len(), tokens.as_ptr())
        };
        check_status(status, "gptoss_context_append_tokens")
    }

    fn sample(&self, temperature: f32, seed: u64, max_tokens: usize) -> Result<Vec<u32>> {
        if max_tokens == 0 {
            return Ok(Vec::new());
        }

        let mut tokens_out = vec![0u32; max_tokens];
        let mut num_tokens_out = 0usize;
        let status = unsafe {
            ffi::gptoss_context_sample(
                self.ptr.as_ptr(),
                temperature,
                seed,
                max_tokens,
                tokens_out.as_mut_ptr(),
                &mut num_tokens_out,
            )
        };
        check_status(status, "gptoss_context_sample")?;

        if num_tokens_out == 0 {
            return Err(GptOssMetalError::FfiError(
                "gptoss_context_sample produced no tokens".to_string(),
            ));
        }

        tokens_out.truncate(num_tokens_out);
        Ok(tokens_out)
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            let _ = ffi::gptoss_context_release(self.ptr.as_ptr());
        }
    }
}

fn check_status(status: ffi::gptoss_status, action: &str) -> Result<()> {
    if status == ffi::gptoss_status::gptoss_status_success {
        return Ok(());
    }

    Err(GptOssMetalError::FfiError(format!(
        "{action} failed: {status:?}"
    )))
}

fn find_stop_index(output: &str, stop_sequences: &[String]) -> Option<usize> {
    stop_sequences
        .iter()
        .filter_map(|stop| output.find(stop))
        .min()
}

fn parse_usize(key: &str) -> Result<Option<usize>> {
    match env::var(key) {
        Ok(value) => value.parse().map(Some).map_err(|_| {
            GptOssMetalError::InvalidConfig(format!("invalid {key}: {value}"))
        }),
        Err(_) => Ok(None),
    }
}

fn parse_f32(key: &str) -> Result<Option<f32>> {
    match env::var(key) {
        Ok(value) => value.parse().map(Some).map_err(|_| {
            GptOssMetalError::InvalidConfig(format!("invalid {key}: {value}"))
        }),
        Err(_) => Ok(None),
    }
}

fn parse_u64(key: &str) -> Result<Option<u64>> {
    match env::var(key) {
        Ok(value) => value.parse().map(Some).map_err(|_| {
            GptOssMetalError::InvalidConfig(format!("invalid {key}: {value}"))
        }),
        Err(_) => Ok(None),
    }
}

fn path_to_cstring(path: &Path) -> Result<CString> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        return Ok(CString::new(path.as_os_str().as_bytes())?);
    }

    #[cfg(not(unix))]
    {
        let path_str = path.to_str().ok_or_else(|| {
            GptOssMetalError::InvalidConfig("non-utf8 model path".to_string())
        })?;
        Ok(CString::new(path_str)?)
    }
}

fn default_model_path() -> Option<PathBuf> {
    let home = env::var("HOME").ok()?;
    let home = PathBuf::from(home);
    let candidates = [
        home.join("models/gpt-oss-20b/metal/model.bin"),
        home.join("models/gpt-oss-120b/metal/model.bin"),
    ];
    candidates.into_iter().find(|path| path.exists())
}

fn infer_model_id(path: &Path) -> Option<String> {
    for component in path.components().rev() {
        if let std::path::Component::Normal(name) = component {
            if let Some(name) = name.to_str()
                && name.starts_with("gpt-oss-")
            {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn ensure_tiktoken_cache_dir() {
    if env::var_os("TIKTOKEN_RS_CACHE_DIR").is_some() {
        return;
    }

    let home = match env::var("HOME") {
        Ok(home) => PathBuf::from(home),
        Err(_) => return,
    };
    let cache_dir = home.join(".cache/tiktoken-rs");
    if let Err(err) = std::fs::create_dir_all(&cache_dir) {
        warn!(
            cache_dir = %cache_dir.display(),
            error = %err,
            "Failed to create tiktoken cache dir"
        );
        return;
    }
    // Safe here: we set the default cache dir during startup before inference threads spawn.
    unsafe {
        env::set_var("TIKTOKEN_RS_CACHE_DIR", &cache_dir);
    }
    info!(
        cache_dir = %cache_dir.display(),
        "Defaulted TIKTOKEN_RS_CACHE_DIR"
    );
}
