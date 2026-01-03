use std::collections::HashSet;
use std::env;
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};

use openai_harmony::chat::{Conversation, Message, Role, SystemContent};
use openai_harmony::{HarmonyEncoding, HarmonyEncodingName, load_harmony_encoding};
use rand::rngs::OsRng;
use rand::RngCore;

use crate::error::{GptOssMetalError, Result};
use crate::ffi;

const DEFAULT_MAX_TOKENS: usize = 256;
const DEFAULT_TEMPERATURE: f32 = 0.7;

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
        let model_path = env::var("GPT_OSS_METAL_MODEL_PATH")
            .map(PathBuf::from)
            .map_err(|_| {
                GptOssMetalError::InvalidConfig(
                    "GPT_OSS_METAL_MODEL_PATH is not set".to_string(),
                )
            })?;

        let model_id = env::var("GPT_OSS_METAL_MODEL_ID")
            .unwrap_or_else(|_| "gpt-oss-metal".to_string());

        let context_length = parse_usize("GPT_OSS_METAL_CONTEXT_LENGTH")?;
        let max_batch_tokens = parse_usize("GPT_OSS_METAL_MAX_BATCH_TOKENS")?;
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
        let context_length = config.context_length.unwrap_or(max_context);

        if context_length == 0 || context_length > max_context {
            return Err(GptOssMetalError::InvalidConfig(format!(
                "context length {context_length} exceeds model max {max_context}"
            )));
        }

        if let Some(max_batch_tokens) = config.max_batch_tokens
            && max_batch_tokens > context_length
        {
            return Err(GptOssMetalError::InvalidConfig(format!(
                "max batch tokens {max_batch_tokens} exceeds context length {context_length}"
            )));
        }

        let encoding = load_harmony_encoding(HarmonyEncodingName::HarmonyGptOss)
            .map_err(|err| GptOssMetalError::HarmonyError(err.to_string()))?;

        let stop_tokens = encoding
            .stop_tokens_for_assistant_actions()
            .map_err(|err| GptOssMetalError::HarmonyError(err.to_string()))?
            .into_iter()
            .collect::<HashSet<_>>();

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
        mut on_token: F,
    ) -> Result<GptOssMetalCompletion>
    where
        F: FnMut(&str) -> Result<()>,
    {
        let prompt_tokens = self.render_prompt_tokens(prompt)?;
        let prompt_token_count = prompt_tokens.len();

        let available_tokens = self.context_length.saturating_sub(prompt_tokens.len());
        if available_tokens == 0 {
            return Err(GptOssMetalError::InvalidConfig(
                "prompt exceeds context length".to_string(),
            ));
        }

        let max_tokens = max_tokens
            .unwrap_or(self.config.default_max_tokens)
            .min(available_tokens);
        let temperature = temperature.unwrap_or(self.config.default_temperature);
        let stop_sequences = stop_sequences.unwrap_or(&[]);

        let seed = self
            .config
            .seed
            .unwrap_or_else(|| OsRng.next_u64());

        let mut model = self.model.lock().map_err(|_| {
            GptOssMetalError::FfiError("model mutex poisoned".to_string())
        })?;

        let context = Context::create(
            &model,
            self.context_length,
            self.config.max_batch_tokens.unwrap_or(0),
        )?;
        context.append_tokens(&prompt_tokens)?;

        let mut output = String::new();
        let mut sent_len = 0usize;
        let mut completion_tokens = 0usize;
        let mut finish_reason = "length".to_string();

        for _ in 0..max_tokens {
            let token = context.sample_one(temperature, seed)?;

            if self.stop_tokens.contains(&token) {
                finish_reason = "stop".to_string();
                break;
            }

            let token_text = self
                .encoding
                .tokenizer()
                .decode_utf8(&[token])
                .map_err(|err| GptOssMetalError::HarmonyError(err.to_string()))?;

            output.push_str(&token_text);
            completion_tokens += 1;

            if let Some(trim_idx) = find_stop_index(&output, stop_sequences) {
                if trim_idx > sent_len {
                    on_token(&output[sent_len..trim_idx])?;
                }
                output.truncate(trim_idx);
                finish_reason = "stop".to_string();
                break;
            }

            if output.len() > sent_len {
                on_token(&output[sent_len..])?;
                sent_len = output.len();
            }
        }

        Ok(GptOssMetalCompletion {
            text: output,
            prompt_tokens: prompt_token_count,
            completion_tokens,
            finish_reason,
        })
    }

    fn render_prompt_tokens(&self, prompt: &str) -> Result<Vec<u32>> {
        let mut messages = Vec::new();
        let system = SystemContent::new();
        messages.push(Message::from_role_and_content(Role::System, system));
        messages.push(Message::from_role_and_content(Role::User, prompt.to_string()));

        let convo = Conversation::from_messages(messages);
        self.encoding
            .render_conversation_for_completion(&convo, Role::Assistant, None)
            .map_err(|err| GptOssMetalError::HarmonyError(err.to_string()))
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

    fn sample_one(&self, temperature: f32, seed: u64) -> Result<u32> {
        let mut token_out = 0u32;
        let mut num_tokens_out = 0usize;
        let status = unsafe {
            ffi::gptoss_context_sample(
                self.ptr.as_ptr(),
                temperature,
                seed,
                1,
                &mut token_out,
                &mut num_tokens_out,
            )
        };
        check_status(status, "gptoss_context_sample")?;

        if num_tokens_out == 0 {
            return Err(GptOssMetalError::FfiError(
                "gptoss_context_sample produced no tokens".to_string(),
            ));
        }

        Ok(token_out)
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
