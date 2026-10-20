//! DeepSeek-V4.1-Flash prompt rendering, token encoding, and output parsing.
//!
//! This module adapts the official `deepseek-recipe` (vendored at
//! [`DEEPSEEK_RECIPE_COMMIT`]) to Psionic. Rendering keeps the recipe's
//! explicit special-token ranges, so that a special-token spelling in user
//! text encodes as ordinary text. Streaming uses the recipe's token-boundary
//! decoder and output parser. Everything is keyed by the pinned checkpoint
//! revision and tokenizer digest; these encoders are not R1, V2, or V3
//! templates.
//!
//! This qualifies the protocol only. No Psionic executor runs V4.1 yet, and
//! nothing here adds the model to a public model list.

use std::sync::Arc;

use deepseek_recipe::request::{
    ConversationRequest, DEFAULT_MAX_TOKENS_UPPER_BOUND, ModelVersion, OutputConstraint,
};
use deepseek_recipe::stream::{ParsingOptions, ThinkingInitialStage};
use deepseek_recipe_core::conversation::{Conversation, ReasoningEffort};
use deepseek_recipe_core::messages::{InputMessage, ToolCall};
use deepseek_recipe_core::prompt::Prompt;
use deepseek_recipe_core::special_tokens::{SpecialToken, default_special_tokens};
use deepseek_recipe_core::tools::{ToolChoice, ToolDefinition};
use deepseek_recipe_encoding::dsv41::DeepseekV41Encoding;
use deepseek_recipe_encoding::{PromptEncoding, PromptTokenizer, RenderedPrompt, TokenizerEncoder};
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokenizers::Tokenizer;

pub use deepseek_recipe;
pub use deepseek_recipe_core;
pub use deepseek_recipe_encoding;

/// Hugging Face repository of the pinned checkpoint.
pub const DEEPSEEK_V41_FLASH_REPO: &str = "deepseek-ai/DeepSeek-V4.1-Flash-1017";
/// Pinned checkpoint revision.
pub const DEEPSEEK_V41_FLASH_REVISION: &str = "1bae0e1088993b9b82de08f59de5be5a34e42170";
/// `deepseek-recipe` commit that the vendored crates come from.
pub const DEEPSEEK_RECIPE_COMMIT: &str = "935817c66f5d6e64838f419692545f06787e485f";
/// SHA-256 of the checkpoint's `tokenizer.json`.
pub const DEEPSEEK_V41_TOKENIZER_SHA256: &str =
    "c90dfa01249db1be4245780a052ede752e1361c612ac6d08e2bdada7d599476b";
/// SHA-256 of the checkpoint's `tokenizer_config.json`.
pub const DEEPSEEK_V41_TOKENIZER_CONFIG_SHA256: &str =
    "6ac8c8dc065ed118161d02dd532749ae3f52c243deac27872134fae2f50d8547";
/// The recipe's model version for every V4.1 option and default.
pub const DEEPSEEK_V41_MODEL_VERSION: ModelVersion = ModelVersion::DeepseekV41Flash;

/// The checkpoint's image placeholder. The recipe's bundled tokenizer spells it
/// `<｜image｜>`, but the pinned checkpoint does not.
const IMAGE_PLACEHOLDER: &str = "<｜deepseek_image｜>";
const TASK_TOKENS: [(&str, &str); 6] = [
    ("action", "<｜action｜>"),
    ("query", "<｜query｜>"),
    ("authority", "<｜authority｜>"),
    ("domain", "<｜domain｜>"),
    ("title", "<｜title｜>"),
    ("read_url", "<｜read_url｜>"),
];
/// Token IDs that the protocol relies on, checked when a tokenizer loads.
const EXPECTED_TOKEN_IDS: [(&str, u32); 4] = [
    ("<｜begin▁of▁sentence｜>", 0),
    ("<｜end▁of▁sentence｜>", 1),
    ("<｜deepseek_image｜>", 129_264),
    ("<｜System｜>", 128_799),
];
const REQUIRED_TOKENS: [&str; 7] = [
    "<think>",
    "</think>",
    "｜DSML｜",
    "<｜User｜>",
    "<｜Assistant｜>",
    "<｜latest_reminder｜>",
    "<｜action｜>",
];

/// Where a protocol's tokenizer came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeepSeekV41TokenizerSource {
    /// The checkpoint's own `tokenizer.json`, matched against its pin.
    Checkpoint,
    /// The reduced test fixture, which encodes only its own corpus.
    ReducedFixture,
}

/// The exact model and tokenizer identity that keys V4.1 rendering and parsing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DeepSeekV41Identity {
    pub repo: &'static str,
    pub revision: &'static str,
    pub recipe_commit: &'static str,
    pub tokenizer_sha256: String,
    pub tokenizer_config_sha256: String,
    pub tokenizer_source: DeepSeekV41TokenizerSource,
}

impl DeepSeekV41Identity {
    /// Returns the cache key for prompts and parsed output of this identity.
    #[must_use]
    pub fn key(&self) -> String {
        format!(
            "{}@{}#tokenizer-sha256:{}",
            self.repo, self.revision, self.tokenizer_sha256
        )
    }
}

/// How far V4.1 tool calling is qualified.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeepSeekV41ToolCalling {
    /// Tool calls render and parse, but no model has produced one in Psionic.
    ProtocolOnly,
}

/// What V4.1 support covers today.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct DeepSeekV41Capabilities {
    pub qualification: &'static str,
    pub native_inference: bool,
    pub structured_outputs: bool,
    pub vision: bool,
    pub tool_calling: DeepSeekV41ToolCalling,
    pub enforced_output_constraints: bool,
}

/// V4.1 is protocol-qualified only: no executor, no structured outputs, no vision.
pub const DEEPSEEK_V41_CAPABILITIES: DeepSeekV41Capabilities = DeepSeekV41Capabilities {
    qualification: "protocol",
    native_inference: false,
    structured_outputs: false,
    vision: false,
    tool_calling: DeepSeekV41ToolCalling::ProtocolOnly,
    enforced_output_constraints: false,
};

/// A V4.1 protocol failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum DeepSeekV41Error {
    #[error("{file} has SHA-256 {actual}, but DeepSeek-V4.1-Flash pins {expected}")]
    TokenizerDigest {
        file: &'static str,
        expected: &'static str,
        actual: String,
    },
    #[error("the DeepSeek-V4.1-Flash tokenizer is invalid: {0}")]
    Tokenizer(String),
    #[error("failed to encode the DeepSeek-V4.1-Flash prompt: {0}")]
    Encode(String),
    #[error("{0}")]
    Unsupported(String),
    #[error("{0}")]
    InvalidRequest(String),
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Rendering, encoding, and a stream tokenizer for one pinned V4.1 identity.
#[derive(Clone)]
pub struct DeepSeekV41Protocol {
    identity: DeepSeekV41Identity,
    tokenizer: Arc<Tokenizer>,
    encoding: DeepseekV41Encoding,
}

impl DeepSeekV41Protocol {
    /// Loads the checkpoint's own tokenizer files after checking their pins.
    ///
    /// # Errors
    ///
    /// Refuses files whose SHA-256 differs from the pin, and tokenizers whose
    /// configuration or special tokens differ from the checkpoint's.
    pub fn from_checkpoint_files(
        tokenizer_json: &[u8],
        tokenizer_config_json: &[u8],
    ) -> Result<Self, DeepSeekV41Error> {
        for (file, bytes, expected) in [
            (
                "tokenizer.json",
                tokenizer_json,
                DEEPSEEK_V41_TOKENIZER_SHA256,
            ),
            (
                "tokenizer_config.json",
                tokenizer_config_json,
                DEEPSEEK_V41_TOKENIZER_CONFIG_SHA256,
            ),
        ] {
            let actual = sha256_hex(bytes);
            if actual != expected {
                return Err(DeepSeekV41Error::TokenizerDigest {
                    file,
                    expected,
                    actual,
                });
            }
        }
        let tokenizer = Tokenizer::from_bytes(tokenizer_json)
            .map_err(|error| DeepSeekV41Error::Tokenizer(error.to_string()))?;
        Self::new(
            tokenizer,
            tokenizer_config_json,
            DeepSeekV41TokenizerSource::Checkpoint,
            DEEPSEEK_V41_TOKENIZER_SHA256.into(),
        )
    }

    fn new(
        tokenizer: Tokenizer,
        tokenizer_config_json: &[u8],
        tokenizer_source: DeepSeekV41TokenizerSource,
        tokenizer_sha256: String,
    ) -> Result<Self, DeepSeekV41Error> {
        check_tokenizer_config(tokenizer_config_json)?;
        for (token, id) in EXPECTED_TOKEN_IDS {
            if tokenizer.token_to_id(token) != Some(id) {
                return Err(DeepSeekV41Error::Tokenizer(format!(
                    "expected {token} to have ID {id}"
                )));
            }
        }
        let added = tokenizer.get_added_vocabulary();
        for token in REQUIRED_TOKENS {
            if added.token_to_id(token, tokenizer.get_model()).is_none() {
                return Err(DeepSeekV41Error::Tokenizer(format!(
                    "the added vocabulary has no {token}"
                )));
            }
        }
        if tokenizer.get_padding().is_some() || tokenizer.get_truncation().is_some() {
            return Err(DeepSeekV41Error::Tokenizer(
                "padding and truncation must be off".into(),
            ));
        }
        let encoding = DeepseekV41Encoding::new().with_tokenizer(PromptTokenizer::new(&tokenizer));
        Ok(Self {
            identity: DeepSeekV41Identity {
                repo: DEEPSEEK_V41_FLASH_REPO,
                revision: DEEPSEEK_V41_FLASH_REVISION,
                recipe_commit: DEEPSEEK_RECIPE_COMMIT,
                tokenizer_sha256,
                tokenizer_config_sha256: sha256_hex(tokenizer_config_json),
                tokenizer_source,
            },
            tokenizer: Arc::new(tokenizer),
            encoding,
        })
    }

    #[must_use]
    pub fn identity(&self) -> &DeepSeekV41Identity {
        &self.identity
    }

    /// The tokenizer to attach to the recipe's `StreamProcessor`.
    #[must_use]
    pub fn tokenizer(&self) -> Arc<Tokenizer> {
        Arc::clone(&self.tokenizer)
    }

    /// Renders a conversation followed by the start of the next assistant turn.
    #[must_use]
    pub fn render(&self, conversation: &Conversation) -> RenderedPrompt {
        self.encoding.render_conversation(conversation)
    }

    /// Encodes a rendered prompt, inserting only its explicit special tokens.
    ///
    /// # Errors
    ///
    /// Fails when the tokenizer cannot encode the prompt.
    pub fn encode_rendered(&self, rendered: &RenderedPrompt) -> Result<Vec<u32>, DeepSeekV41Error> {
        PromptTokenizer::new(&self.tokenizer)
            .encode_ids(&rendered.prompt, &rendered.special_ranges)
            .map_err(DeepSeekV41Error::Encode)
    }

    /// Renders and encodes a conversation.
    ///
    /// # Errors
    ///
    /// Fails when the tokenizer cannot encode the prompt.
    pub fn encode(&self, conversation: &Conversation) -> Result<Vec<u32>, DeepSeekV41Error> {
        self.encoding
            .encode(conversation)
            .map_err(|error| DeepSeekV41Error::Encode(error.to_string()))
    }
}

fn check_tokenizer_config(bytes: &[u8]) -> Result<(), DeepSeekV41Error> {
    let config: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|error| DeepSeekV41Error::Tokenizer(error.to_string()))?;
    let content = |key: &str| config[key]["content"].as_str().map(str::to_owned);
    if config["add_bos_token"] != serde_json::json!(false)
        || config["add_eos_token"] != serde_json::json!(false)
        || content("bos_token").as_deref() != Some("<｜begin▁of▁sentence｜>")
        || content("eos_token").as_deref() != Some("<｜end▁of▁sentence｜>")
    {
        return Err(DeepSeekV41Error::Tokenizer(
            "tokenizer_config.json differs from the checkpoint's".into(),
        ));
    }
    Ok(())
}

/// Returns the recipe's parsing options for a V4.1 conversation.
///
/// Thinking mode starts in reasoning, so unfinished reasoning never becomes
/// answer content. Answer whitespace is kept byte for byte, because the V4.1
/// template replays an earlier answer verbatim.
#[must_use]
pub fn deepseek_v41_parsing_options(
    conversation: &Conversation,
    stop_sequences: Vec<String>,
) -> ParsingOptions {
    let parse_tool_calls =
        !conversation.tools.is_empty() && conversation.tool_choice != ToolChoice::None;
    ParsingOptions {
        parse_tool_calls,
        starts_in_tool_calls: parse_tool_calls && conversation.tool_choice == ToolChoice::Required,
        parse_json_output: false,
        thinking_initial_stage: conversation
            .thinking_mode
            .then(|| thinking_initial_stage(&conversation.messages)),
        stop_sequences,
        special_tokens: default_special_tokens(),
        preserves_answer_whitespace: true,
    }
}

/// Mirrors the recipe's Chat Completions adapter: a final prefix message
/// continues its answer or its reasoning.
fn thinking_initial_stage(messages: &[InputMessage]) -> ThinkingInitialStage {
    match messages.last() {
        Some(InputMessage::Assistant {
            content,
            reasoning_content,
            prefix: true,
            ..
        }) => {
            if !content.is_empty() {
                ThinkingInitialStage::ContentContinuation
            } else if reasoning_content
                .as_deref()
                .is_some_and(|text| !text.is_empty())
            {
                ThinkingInitialStage::ReasoningContinuation
            } else {
                ThinkingInitialStage::ReasoningStart
            }
        }
        _ => ThinkingInitialStage::ReasoningStart,
    }
}

/// Sampling values a caller requested, before V4.1 defaults apply.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DeepSeekV41SamplingRequest {
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<u32>,
}

/// Psionic's own runtime limits, which apply on top of the recipe's defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeepSeekV41RuntimeBudget {
    pub context_tokens: u32,
    pub max_output_tokens: u32,
}

/// Resolved V4.1 generation options for one request.
#[derive(Clone, Debug, PartialEq)]
pub struct DeepSeekV41GenerationOptions {
    pub temperature: f32,
    pub top_p: f32,
    pub max_tokens: u32,
    /// The constraint constrained decoding would enforce. Psionic records it
    /// but enforces nothing until constrained decoding exists.
    pub output_constraint: Option<OutputConstraint>,
}

/// Resolves V4.1 sampling defaults and the output budget for an encoded prompt.
///
/// In thinking mode the temperature is 1.0 and `top_p` is at least 0.95, as
/// the recipe resolves them for V4.1. The recipe's default `max_tokens`
/// then shrinks to Psionic's context and output budget.
///
/// # Errors
///
/// Refuses out-of-range sampling values, a `max_tokens` above the runtime's
/// output budget, and prompts that leave no room in the context.
pub fn resolve_deepseek_v41_generation(
    conversation: &Conversation,
    requested: DeepSeekV41SamplingRequest,
    output_constraint: Option<OutputConstraint>,
    prompt_tokens: u32,
    budget: DeepSeekV41RuntimeBudget,
) -> Result<DeepSeekV41GenerationOptions, DeepSeekV41Error> {
    if requested
        .temperature
        .is_some_and(|value| !(0.0..=2.0).contains(&value))
    {
        return Err(DeepSeekV41Error::InvalidRequest(
            "temperature must be between 0 and 2".into(),
        ));
    }
    if requested
        .top_p
        .is_some_and(|value| !value.is_finite() || value <= 0.0 || value > 1.0)
    {
        return Err(DeepSeekV41Error::InvalidRequest(
            "top_p must be greater than 0 and at most 1".into(),
        ));
    }
    let upper_bound = DEFAULT_MAX_TOKENS_UPPER_BOUND.min(budget.max_output_tokens);
    if requested
        .max_tokens
        .is_some_and(|value| !(1..=upper_bound).contains(&value))
    {
        return Err(DeepSeekV41Error::InvalidRequest(format!(
            "max_tokens must be between 1 and {upper_bound}"
        )));
    }
    let thinking_mode = conversation.thinking_mode;
    let mut request = ConversationRequest::new(conversation.clone(), DEEPSEEK_V41_MODEL_VERSION);
    request.inference_options.max_tokens = requested.max_tokens;
    let max_tokens = request
        .resolve_max_tokens(prompt_tokens, budget.context_tokens)
        .map_err(|error| DeepSeekV41Error::InvalidRequest(error.to_string()))?
        .min(budget.max_output_tokens);
    let has_tools = !conversation.tools.is_empty() && conversation.tool_choice != ToolChoice::None;
    Ok(DeepSeekV41GenerationOptions {
        temperature: if thinking_mode {
            1.0
        } else {
            requested.temperature.unwrap_or(1.0)
        },
        top_p: DEEPSEEK_V41_MODEL_VERSION.resolve_top_p(requested.top_p, thinking_mode, has_tools),
        max_tokens,
        output_constraint,
    })
}

/// Refuses output constraints that only constrained decoding could honor.
///
/// A tool-call block with an optional, non-strict tool choice parses without
/// enforcement and stays allowed. JSON, schemas, regular expressions, a
/// required tool call, and strict tools need roadmap R2's constrained decoder.
///
/// # Errors
///
/// Returns [`DeepSeekV41Error::Unsupported`] for every constraint that needs
/// enforcement.
pub fn refuse_unenforced_deepseek_v41_constraint(
    constraint: Option<&OutputConstraint>,
) -> Result<(), DeepSeekV41Error> {
    let refusal = match constraint {
        None => return Ok(()),
        Some(OutputConstraint::Json { schema: None }) => "JSON output",
        Some(OutputConstraint::Json { schema: Some(_) }) => "JSON schema output",
        Some(OutputConstraint::Regex(_)) => "regular-expression output",
        Some(OutputConstraint::ToolCalls { required: true, .. }) => "a required tool call",
        Some(OutputConstraint::ToolCalls { tools, .. })
            if tools.iter().any(|tool| tool.strict == Some(true)) =>
        {
            "strict tools"
        }
        Some(OutputConstraint::ToolCalls { .. }) => return Ok(()),
    };
    Err(DeepSeekV41Error::Unsupported(format!(
        "DeepSeek-V4.1-Flash can't guarantee {refusal} yet."
    )))
}

/// Renders one of the checkpoint's `encoding/tests/test_input_*.json` cases.
///
/// The cases use the reference encoder's message format, which has three
/// features that the recipe's API conversion doesn't: tool declarations on a
/// later system message, internal task tokens, and a transcript that ends in
/// a finished assistant turn without a generation prompt. This function maps
/// them onto the recipe's renderer and refuses combinations it can't render
/// exactly.
///
/// # Errors
///
/// Refuses malformed cases and unsupported combinations.
pub fn render_deepseek_v41_checkpoint_case(
    case_json: &str,
) -> Result<RenderedPrompt, DeepSeekV41Error> {
    let encoding = DeepseekV41Encoding::new();
    let invalid = |message: &str| DeepSeekV41Error::InvalidRequest(message.into());
    let value: serde_json::Value =
        serde_json::from_str(case_json).map_err(|error| invalid(&error.to_string()))?;
    let (raw_messages, case) = match &value {
        serde_json::Value::Array(messages) => (messages.clone(), serde_json::Map::new()),
        serde_json::Value::Object(case) => (
            case.get("messages")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .ok_or_else(|| invalid("a case needs messages"))?,
            case.clone(),
        ),
        _ => return Err(invalid("a case is an object or a message list")),
    };
    if case
        .get("context")
        .is_some_and(|context| !context.is_null())
    {
        return Err(DeepSeekV41Error::Unsupported(
            "checkpoint cases with context aren't supported".into(),
        ));
    }
    let thinking_mode = match case
        .get("thinking_mode")
        .and_then(serde_json::Value::as_str)
    {
        Some("thinking") => true,
        Some("chat") | None => false,
        Some(other) => return Err(invalid(&format!("unknown thinking_mode {other}"))),
    };
    let reasoning_effort = match case.get("reasoning_effort") {
        None | Some(serde_json::Value::Null) => None,
        Some(effort) => Some(match (effort.as_str(), effort.as_u64()) {
            (Some("low"), _) | (_, Some(50)) => ReasoningEffort::Low,
            (Some("high"), _) | (_, Some(75)) => ReasoningEffort::High,
            (Some("max"), _) | (_, Some(100)) => ReasoningEffort::Max,
            _ => {
                return Err(DeepSeekV41Error::Unsupported(
                    "the recipe renders reasoning effort 50, 75, or 100 only".into(),
                ));
            }
        }),
    };
    let case_tools = match case.get("tools") {
        Some(tools) => Some(tool_definitions(tools)?),
        None => None,
    };

    let mut messages = Vec::new();
    let mut conversation_tools = Vec::new();
    let mut task = None;
    for (index, raw) in raw_messages.iter().enumerate() {
        let field = |name: &str| raw.get(name).filter(|value| !value.is_null());
        for unsupported in ["response_format", "wo_eos", "content_blocks", "namespace"] {
            if field(unsupported).is_some() {
                return Err(DeepSeekV41Error::Unsupported(format!(
                    "checkpoint cases with {unsupported} aren't supported"
                )));
            }
        }
        let text = |name: &str| -> Result<String, DeepSeekV41Error> {
            match field(name) {
                None => Ok(String::new()),
                Some(serde_json::Value::String(text)) => Ok(text.clone()),
                Some(_) => Err(invalid(&format!("{name} must be text"))),
            }
        };
        let mut tools = match field("tools") {
            Some(tools) => Some(tool_definitions(tools)?),
            None => None,
        };
        if index == 0 && case_tools.is_some() {
            tools.clone_from(&case_tools);
        }
        let role = field("role")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if role != "system" && tools.is_some() {
            return Err(DeepSeekV41Error::Unsupported(
                "only system messages declare tools".into(),
            ));
        }
        if let Some(name) = field("task") {
            let name = name.as_str().unwrap_or("");
            let token = TASK_TOKENS
                .iter()
                .find(|(task, _)| *task == name)
                .map(|(_, token)| *token)
                .ok_or_else(|| invalid(&format!("unknown task {name}")))?;
            if role != "user" || index + 2 < raw_messages.len() || task.is_some() {
                return Err(DeepSeekV41Error::Unsupported(
                    "a task is supported only on the last user message".into(),
                ));
            }
            task = Some((name == "action", token, messages.len()));
        }
        let message = match role {
            "system" => {
                let mut content = Prompt::plain(text("content")?);
                if let Some(tools) = tools.filter(|tools| !tools.is_empty()) {
                    if index == 0 {
                        conversation_tools = tools;
                    } else if thinking_mode {
                        return Err(DeepSeekV41Error::Unsupported(
                            "tools on a later system message render only in chat mode".into(),
                        ));
                    } else {
                        content.push_plain("\n\n");
                        content += encoding.render_tool_prompt(&tools);
                    }
                }
                InputMessage::System {
                    content,
                    image_sources: Vec::new(),
                    tools: None,
                }
            }
            "user" => InputMessage::User {
                content: user_content(field("content"))?,
                image_sources: Vec::new(),
            },
            "tool" => InputMessage::Tool {
                content: Prompt::plain(text("content")?),
                image_sources: Vec::new(),
                tool_call_id: field("tool_call_id")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
            },
            "latest_reminder" => {
                if matches!(messages.last(), Some(InputMessage::LatestReminder { .. })) {
                    return Err(DeepSeekV41Error::Unsupported(
                        "consecutive latest reminders aren't supported".into(),
                    ));
                }
                InputMessage::LatestReminder {
                    content: text("content")?,
                }
            }
            "assistant" => InputMessage::Assistant {
                content: text("content")?,
                reasoning_content: field("reasoning_content")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                tool_calls: match field("tool_calls") {
                    Some(calls) => Some(tool_calls(calls)?),
                    None => None,
                },
                prefix: false,
            },
            other => return Err(invalid(&format!("unknown role {other}"))),
        };
        messages.push(message);
    }
    if conversation_tools.is_empty() && case_tools.as_ref().is_some_and(|tools| !tools.is_empty()) {
        return Err(DeepSeekV41Error::Unsupported(
            "case tools need a first system message".into(),
        ));
    }

    let mut conversation = Conversation {
        messages,
        thinking_mode,
        tools: conversation_tools,
        tool_choice: ToolChoice::Auto,
        reasoning_effort,
        ..Conversation::default()
    };
    if let Some((is_action, token, user_index)) = task {
        let trailing = conversation.messages.split_off(user_index + 1);
        let mut rendered = encoding.render_conversation(&conversation);
        if !is_action {
            strip_generation_prompt(&mut rendered)?;
        }
        push_special(&mut rendered, token);
        match trailing.as_slice() {
            [] => {}
            [
                InputMessage::Assistant {
                    content,
                    tool_calls,
                    ..
                },
            ] if tool_calls.as_ref().is_none_or(Vec::is_empty) => {
                rendered.prompt.push_str(content);
                push_special(&mut rendered, SpecialToken::Eos.default_str());
            }
            _ => {
                return Err(DeepSeekV41Error::Unsupported(
                    "a task may be followed only by a final assistant answer".into(),
                ));
            }
        }
        return Ok(rendered);
    }
    // A finished assistant turn renders as a prefix continuation followed by
    // its end-of-sentence token, as the reference encoder writes a transcript.
    let message_count = conversation.messages.len();
    if let Some(InputMessage::Assistant {
        content,
        tool_calls,
        prefix,
        ..
    }) = conversation.messages.last_mut()
    {
        *prefix = true;
        // The recipe leaves a prefix's reasoning open when nothing follows it.
        let closes_reasoning = thinking_mode
            && content.is_empty()
            && tool_calls.as_ref().is_none_or(Vec::is_empty)
            && message_count > 1;
        let mut rendered = encoding.render_conversation(&conversation);
        if closes_reasoning {
            push_special(&mut rendered, SpecialToken::ThinkingEnd.default_str());
        }
        push_special(&mut rendered, SpecialToken::Eos.default_str());
        return Ok(rendered);
    }
    Ok(encoding.render_conversation(&conversation))
}

fn push_special(rendered: &mut RenderedPrompt, token: &str) {
    let start = rendered.prompt.len();
    rendered.prompt.push_str(token);
    rendered.special_ranges.push(start..rendered.prompt.len());
}

/// Removes the trailing `<｜Assistant｜>` and thinking marker that open the
/// next assistant turn.
fn strip_generation_prompt(rendered: &mut RenderedPrompt) -> Result<(), DeepSeekV41Error> {
    let assistant = SpecialToken::Assistant.default_str();
    let markers = [
        SpecialToken::ThinkingStart.default_str(),
        SpecialToken::ThinkingEnd.default_str(),
    ];
    let count = rendered.special_ranges.len();
    let ok = count >= 2 && {
        let marker = &rendered.special_ranges[count - 1];
        let header = &rendered.special_ranges[count - 2];
        marker.end == rendered.prompt.len()
            && header.end == marker.start
            && &rendered.prompt[header.clone()] == assistant
            && markers.contains(&&rendered.prompt[marker.clone()])
    };
    if !ok {
        return Err(DeepSeekV41Error::Unsupported(
            "the prompt doesn't end with a generation prompt".into(),
        ));
    }
    let start = rendered.special_ranges[count - 2].start;
    rendered.special_ranges.truncate(count - 2);
    rendered.prompt.truncate(start);
    Ok(())
}

fn user_content(content: Option<&serde_json::Value>) -> Result<Prompt, DeepSeekV41Error> {
    let refuse_placeholder = |text: &str| {
        if text.contains(IMAGE_PLACEHOLDER) {
            Err(DeepSeekV41Error::InvalidRequest(
                "text can't contain the image placeholder".into(),
            ))
        } else {
            Ok(())
        }
    };
    match content {
        None => Ok(Prompt::new()),
        Some(serde_json::Value::String(text)) => {
            refuse_placeholder(text)?;
            Ok(Prompt::plain(text))
        }
        Some(serde_json::Value::Array(blocks)) => {
            let mut prompt = Prompt::new();
            for (index, block) in blocks.iter().enumerate() {
                if index > 0 {
                    prompt.push_plain("\n\n");
                }
                match block.get("type").and_then(serde_json::Value::as_str) {
                    Some("text") => {
                        let text = block
                            .get("text")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("");
                        refuse_placeholder(text)?;
                        prompt.push_plain(text);
                    }
                    // Rendering only: the placeholder stands where the image
                    // goes. Serving refuses images before this point.
                    Some("image_url" | "image") => prompt.push_special(IMAGE_PLACEHOLDER),
                    _ => {
                        return Err(DeepSeekV41Error::Unsupported(
                            "user content blocks are text or images".into(),
                        ));
                    }
                }
            }
            Ok(prompt)
        }
        Some(_) => Err(DeepSeekV41Error::InvalidRequest(
            "user content must be text or blocks".into(),
        )),
    }
}

fn tool_definitions(tools: &serde_json::Value) -> Result<Vec<ToolDefinition>, DeepSeekV41Error> {
    let unsupported = || {
        DeepSeekV41Error::Unsupported(
            "tools must be functions with a name, description, and parameters, in that order"
                .into(),
        )
    };
    tools
        .as_array()
        .ok_or_else(unsupported)?
        .iter()
        .map(|tool| {
            let function = tool
                .get("function")
                .and_then(serde_json::Value::as_object)
                .ok_or_else(unsupported)?;
            if tool.get("namespace").is_some()
                || !function
                    .keys()
                    .map(String::as_str)
                    .eq(["name", "description", "parameters"])
            {
                return Err(unsupported());
            }
            Ok(ToolDefinition {
                name: function["name"]
                    .as_str()
                    .ok_or_else(unsupported)?
                    .to_owned(),
                description: Some(
                    function["description"]
                        .as_str()
                        .ok_or_else(unsupported)?
                        .to_owned(),
                ),
                parameters: function["parameters"].clone(),
                strict: None,
                defer_loading: false,
            })
        })
        .collect()
}

fn tool_calls(calls: &serde_json::Value) -> Result<Vec<ToolCall>, DeepSeekV41Error> {
    let invalid = || DeepSeekV41Error::InvalidRequest("tool calls must be function calls".into());
    calls
        .as_array()
        .ok_or_else(invalid)?
        .iter()
        .map(|call| {
            let function = call.get("function").ok_or_else(invalid)?;
            let name = function
                .get("name")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(invalid)?;
            if name.contains("::") {
                return Err(DeepSeekV41Error::Unsupported(
                    "namespaced tool names aren't supported".into(),
                ));
            }
            let arguments = match function.get("arguments") {
                Some(serde_json::Value::String(text)) => text.clone(),
                Some(object @ serde_json::Value::Object(_)) => object.to_string(),
                _ => return Err(invalid()),
            };
            Ok(ToolCall {
                id: call
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
                name: name.to_owned(),
                arguments,
            })
        })
        .collect()
}

/// The checkpoint's encoding tests and a reduced tokenizer, for tests only.
///
/// The reduced tokenizer keeps the checkpoint's configuration and every
/// vocabulary entry and merge that the fixture texts use, with their original
/// IDs. It encodes and decodes those texts exactly as the full tokenizer does
/// and nothing else. `scripts/bench/pylon-models/refresh_deepseek_v41.py`
/// regenerates it.
#[cfg(any(test, feature = "deepseek-v41-fixtures"))]
pub mod fixture {
    use super::{DeepSeekV41Protocol, DeepSeekV41TokenizerSource, sha256_hex};
    use serde::Deserialize;
    use std::str::FromStr;
    use tokenizers::Tokenizer;

    pub const REDUCED_TOKENIZER: &str =
        include_str!("testdata/deepseek_v41/tokenizer.reduced.json");
    pub const TOKENIZER_CONFIG: &str = include_str!("testdata/deepseek_v41/tokenizer_config.json");
    pub const GOLDEN: &str = include_str!("testdata/deepseek_v41/golden.json");
    /// The checkpoint's encoding tests, as `(input, expected output)`.
    pub const ENCODING_TESTS: [(&str, &str); 5] = [
        (
            include_str!("testdata/deepseek_v41/encoding/tests/test_input_1.json"),
            include_str!("testdata/deepseek_v41/encoding/tests/test_output_1.txt"),
        ),
        (
            include_str!("testdata/deepseek_v41/encoding/tests/test_input_2.json"),
            include_str!("testdata/deepseek_v41/encoding/tests/test_output_2.txt"),
        ),
        (
            include_str!("testdata/deepseek_v41/encoding/tests/test_input_3.json"),
            include_str!("testdata/deepseek_v41/encoding/tests/test_output_3.txt"),
        ),
        (
            include_str!("testdata/deepseek_v41/encoding/tests/test_input_4.json"),
            include_str!("testdata/deepseek_v41/encoding/tests/test_output_4.txt"),
        ),
        (
            include_str!("testdata/deepseek_v41/encoding/tests/test_input_5.json"),
            include_str!("testdata/deepseek_v41/encoding/tests/test_output_5.txt"),
        ),
    ];

    #[derive(Debug, Deserialize)]
    pub struct Golden {
        pub source: GoldenSource,
        pub reduced_tokenizer: GoldenReducedTokenizer,
        pub prompt_cases: Vec<GoldenPromptCase>,
        pub stream_cases: Vec<GoldenStreamCase>,
    }

    #[derive(Debug, Deserialize)]
    pub struct GoldenSource {
        pub repo: String,
        pub revision: String,
        pub recipe_commit: String,
        pub files: std::collections::BTreeMap<String, String>,
    }

    #[derive(Debug, Deserialize)]
    pub struct GoldenReducedTokenizer {
        pub sha256: String,
        pub vocabulary_size: u32,
    }

    #[derive(Debug, Deserialize)]
    pub struct GoldenPromptCase {
        pub name: String,
        pub input_sha256: String,
        pub output_sha256: String,
        pub token_ids: Vec<u32>,
    }

    #[derive(Debug, Deserialize)]
    pub struct GoldenStreamCase {
        pub name: String,
        pub pieces: Vec<(String, String)>,
        pub token_ids: Vec<u32>,
    }

    /// Parses the golden fixture.
    ///
    /// # Panics
    ///
    /// Panics when the checked-in fixture is malformed.
    #[must_use]
    pub fn golden() -> Golden {
        serde_json::from_str(GOLDEN).expect("the V4.1 golden fixture parses")
    }

    /// Returns the token IDs of a named streaming case.
    ///
    /// # Panics
    ///
    /// Panics when the case doesn't exist.
    #[must_use]
    pub fn stream_case(name: &str) -> Vec<u32> {
        golden()
            .stream_cases
            .into_iter()
            .find(|case| case.name == name)
            .unwrap_or_else(|| panic!("missing stream case {name}"))
            .token_ids
    }

    /// Loads the reduced tokenizer, filling unused IDs below the model's
    /// vocabulary size so that added tokens keep their checkpoint IDs.
    ///
    /// # Panics
    ///
    /// Panics when the fixture doesn't load or its digest differs.
    #[must_use]
    pub fn protocol() -> DeepSeekV41Protocol {
        let golden = golden();
        assert_eq!(
            sha256_hex(REDUCED_TOKENIZER.as_bytes()),
            golden.reduced_tokenizer.sha256
        );
        let mut definition: serde_json::Value =
            serde_json::from_str(REDUCED_TOKENIZER).expect("the reduced tokenizer parses");
        let vocab = definition["model"]["vocab"]
            .as_object_mut()
            .expect("the reduced tokenizer has a vocabulary");
        let used = vocab
            .values()
            .filter_map(serde_json::Value::as_u64)
            .collect::<std::collections::BTreeSet<_>>();
        for id in 0..u64::from(golden.reduced_tokenizer.vocabulary_size) {
            if !used.contains(&id) {
                vocab.insert(format!("\u{0}unused{id}"), id.into());
            }
        }
        let tokenizer =
            Tokenizer::from_str(&definition.to_string()).expect("the reduced tokenizer loads");
        DeepSeekV41Protocol::new(
            tokenizer,
            TOKENIZER_CONFIG.as_bytes(),
            DeepSeekV41TokenizerSource::ReducedFixture,
            golden.reduced_tokenizer.sha256,
        )
        .expect("the reduced tokenizer matches the checkpoint's special tokens")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deepseek_recipe::request::resolve_output_constraint;
    use deepseek_recipe_core::conversation::ResponseFormat;

    fn user(text: &str) -> InputMessage {
        InputMessage::User {
            content: Prompt::plain(text),
            image_sources: Vec::new(),
        }
    }

    fn weather_tool(strict: Option<bool>) -> ToolDefinition {
        ToolDefinition {
            name: "get_weather".into(),
            description: Some("Get the weather".into()),
            parameters: serde_json::json!({"type": "object"}),
            strict,
            defer_loading: false,
        }
    }

    #[test]
    fn checkpoint_encoding_tests_render_byte_for_byte_with_official_token_ids()
    -> Result<(), Box<dyn std::error::Error>> {
        let golden = fixture::golden();
        assert_eq!(golden.source.repo, DEEPSEEK_V41_FLASH_REPO);
        assert_eq!(golden.source.revision, DEEPSEEK_V41_FLASH_REVISION);
        assert_eq!(golden.source.recipe_commit, DEEPSEEK_RECIPE_COMMIT);
        assert_eq!(
            golden.source.files["tokenizer.json"],
            DEEPSEEK_V41_TOKENIZER_SHA256
        );
        assert_eq!(
            golden.source.files["tokenizer_config.json"],
            DEEPSEEK_V41_TOKENIZER_CONFIG_SHA256
        );
        assert_eq!(
            sha256_hex(fixture::TOKENIZER_CONFIG.as_bytes()),
            DEEPSEEK_V41_TOKENIZER_CONFIG_SHA256
        );
        let protocol = fixture::protocol();
        assert_eq!(
            protocol.identity().tokenizer_source,
            DeepSeekV41TokenizerSource::ReducedFixture
        );
        assert_eq!(golden.prompt_cases.len(), fixture::ENCODING_TESTS.len());
        for (index, ((input, output), case)) in fixture::ENCODING_TESTS
            .iter()
            .zip(&golden.prompt_cases)
            .enumerate()
        {
            let number = index + 1;
            assert_eq!(case.name, format!("test_{number}"));
            assert_eq!(sha256_hex(input.as_bytes()), case.input_sha256);
            assert_eq!(sha256_hex(output.as_bytes()), case.output_sha256);
            assert_eq!(
                golden.source.files[&format!("encoding/tests/test_input_{number}.json")],
                case.input_sha256
            );
            let rendered = render_deepseek_v41_checkpoint_case(input)?;
            assert_eq!(
                &rendered.prompt, output,
                "test_{number} renders differently"
            );
            assert!(rendered.image_sources.is_empty());
            let ids = protocol.encode_rendered(&rendered)?;
            assert_eq!(ids, case.token_ids, "test_{number} encodes differently");
            assert_eq!(
                protocol
                    .tokenizer()
                    .decode(&ids, false)
                    .map_err(|error| error.to_string())?,
                *output
            );
        }
        Ok(())
    }

    /// Checks the full checkpoint tokenizer against the golden IDs. Set
    /// `DEEPSEEK_V41_TOKENIZER_DIR` to a directory with the pinned
    /// `tokenizer.json` and `tokenizer_config.json`.
    #[test]
    #[ignore = "needs the 6 MB checkpoint tokenizer in DEEPSEEK_V41_TOKENIZER_DIR"]
    fn checkpoint_tokenizer_matches_the_golden_ids() -> Result<(), Box<dyn std::error::Error>> {
        let directory = std::path::PathBuf::from(std::env::var("DEEPSEEK_V41_TOKENIZER_DIR")?);
        let protocol = DeepSeekV41Protocol::from_checkpoint_files(
            &std::fs::read(directory.join("tokenizer.json"))?,
            &std::fs::read(directory.join("tokenizer_config.json"))?,
        )?;
        assert_eq!(
            protocol.identity().tokenizer_source,
            DeepSeekV41TokenizerSource::Checkpoint
        );
        for ((input, _), case) in fixture::ENCODING_TESTS
            .iter()
            .zip(fixture::golden().prompt_cases)
        {
            let rendered = render_deepseek_v41_checkpoint_case(input)?;
            assert_eq!(protocol.encode_rendered(&rendered)?, case.token_ids);
        }
        for case in fixture::golden().stream_cases {
            let text: String = case.pieces.iter().map(|(_, text)| text.as_str()).collect();
            assert_eq!(
                protocol
                    .tokenizer()
                    .decode(&case.token_ids, false)
                    .map_err(|error| error.to_string())?,
                text
            );
        }
        Ok(())
    }

    #[test]
    fn special_token_spellings_in_user_text_stay_plain_text()
    -> Result<(), Box<dyn std::error::Error>> {
        let protocol = fixture::protocol();
        let conversation = Conversation {
            messages: vec![user("Write </think> and <｜DSML｜ calls> as text.")],
            thinking_mode: false,
            ..Conversation::default()
        };
        let rendered = protocol.render(&conversation);
        let specials = rendered
            .special_ranges
            .iter()
            .map(|range| &rendered.prompt[range.clone()])
            .collect::<Vec<_>>();
        assert_eq!(
            specials,
            [
                "<｜begin▁of▁sentence｜>",
                "<｜User｜>",
                "<｜Assistant｜>",
                "</think>"
            ]
        );
        let ids = protocol.encode(&conversation)?;
        let think_end = protocol.tokenizer().token_to_id("</think>").unwrap_or(0);
        let dsml = protocol.tokenizer().token_to_id("｜DSML｜").unwrap_or(0);
        assert_eq!(ids.iter().filter(|&&id| id == think_end).count(), 1);
        assert!(!ids.contains(&dsml));
        // A generic encode of the rendered string would turn both literals
        // into special tokens. The adapter must not do that.
        let generic = protocol
            .tokenizer()
            .encode(rendered.prompt.as_str(), false)
            .map_err(|error| error.to_string())?;
        assert_ne!(generic.get_ids(), ids.as_slice());
        let literal = fixture::stream_case("literal_markers_in_text");
        assert!(
            ids.windows(literal.len() - 1)
                .any(|w| w == &literal[..literal.len() - 1])
        );
        Ok(())
    }

    #[test]
    fn checkpoint_files_are_pinned_by_digest() {
        let error = DeepSeekV41Protocol::from_checkpoint_files(
            fixture::REDUCED_TOKENIZER.as_bytes(),
            fixture::TOKENIZER_CONFIG.as_bytes(),
        )
        .err();
        assert!(matches!(
            error,
            Some(DeepSeekV41Error::TokenizerDigest {
                file: "tokenizer.json",
                ..
            })
        ));
        let protocol = fixture::protocol();
        assert!(protocol.identity().key().starts_with(
            "deepseek-ai/DeepSeek-V4.1-Flash-1017@1bae0e1088993b9b82de08f59de5be5a34e42170#tokenizer-sha256:"
        ));
        assert!(check_tokenizer_config(br#"{"add_bos_token": true}"#).is_err());
    }

    #[test]
    fn checkpoint_cases_refuse_what_they_cannot_render_exactly() {
        for case in [
            r#"[{"role": "user", "content": "a", "task": "action"}, {"role": "user", "content": "b"}]"#,
            r#"[{"role": "user", "content": "a", "task": "unknown"}]"#,
            r#"{"thinking_mode": "thinking", "messages": [{"role": "system", "content": "s"}, {"role": "system", "content": "t", "tools": [{"type": "function", "function": {"name": "f", "description": "d", "parameters": {}}}]}]}"#,
            r#"[{"role": "user", "content": "x <｜deepseek_image｜>"}]"#,
            r#"[{"role": "assistant", "content": "a", "wo_eos": true}]"#,
            r#"{"reasoning_effort": 10, "messages": [{"role": "user", "content": "a"}]}"#,
            r#"[{"role": "latest_reminder", "content": "a"}, {"role": "latest_reminder", "content": "b"}]"#,
        ] {
            assert!(
                render_deepseek_v41_checkpoint_case(case).is_err(),
                "accepted {case}"
            );
        }
    }

    #[test]
    fn options_use_v41_defaults_within_psionic_budget() -> Result<(), DeepSeekV41Error> {
        let budget = DeepSeekV41RuntimeBudget {
            context_tokens: 4096,
            max_output_tokens: 1024,
        };
        let thinking = Conversation {
            messages: vec![user("a")],
            tools: vec![weather_tool(None)],
            ..Conversation::default()
        };
        let resolved = resolve_deepseek_v41_generation(
            &thinking,
            DeepSeekV41SamplingRequest {
                temperature: Some(0.2),
                top_p: Some(0.5),
                max_tokens: None,
            },
            None,
            100,
            budget,
        )?;
        assert_eq!((resolved.temperature, resolved.top_p), (1.0, 0.95));
        // The recipe's 64 Ki thinking default shrinks to the runtime budget.
        assert_eq!(resolved.max_tokens, 1024);
        let chat = Conversation {
            thinking_mode: false,
            ..thinking.clone()
        };
        let resolved = resolve_deepseek_v41_generation(
            &chat,
            DeepSeekV41SamplingRequest {
                temperature: Some(0.2),
                top_p: Some(0.5),
                max_tokens: Some(16),
            },
            None,
            4000,
            budget,
        )?;
        assert_eq!(
            (resolved.temperature, resolved.top_p, resolved.max_tokens),
            (0.2, 1.0, 16)
        );
        let tight = resolve_deepseek_v41_generation(
            &chat,
            DeepSeekV41SamplingRequest::default(),
            None,
            4090,
            budget,
        )?;
        assert_eq!(tight.max_tokens, 6);
        for (request, prompt_tokens) in [
            (
                DeepSeekV41SamplingRequest {
                    max_tokens: Some(1025),
                    ..DeepSeekV41SamplingRequest::default()
                },
                10,
            ),
            (
                DeepSeekV41SamplingRequest {
                    max_tokens: Some(100),
                    ..DeepSeekV41SamplingRequest::default()
                },
                4000,
            ),
            (DeepSeekV41SamplingRequest::default(), 5000),
            (
                DeepSeekV41SamplingRequest {
                    top_p: Some(0.0),
                    ..DeepSeekV41SamplingRequest::default()
                },
                10,
            ),
        ] {
            assert!(
                resolve_deepseek_v41_generation(&chat, request, None, prompt_tokens, budget)
                    .is_err()
            );
        }
        Ok(())
    }

    #[test]
    fn output_constraints_are_recorded_but_strict_ones_are_refused()
    -> Result<(), Box<dyn std::error::Error>> {
        let tools = [weather_tool(None)];
        let optional =
            resolve_output_constraint(&tools, ToolChoice::Auto, &ResponseFormat::Text, false)
                .map_err(|error| error.to_string())?;
        assert!(matches!(
            optional,
            Some(OutputConstraint::ToolCalls {
                required: false,
                ..
            })
        ));
        refuse_unenforced_deepseek_v41_constraint(optional.as_ref())?;
        for constraint in [
            resolve_output_constraint(&tools, ToolChoice::Required, &ResponseFormat::Text, false),
            resolve_output_constraint(
                &[weather_tool(Some(true))],
                ToolChoice::Auto,
                &ResponseFormat::Text,
                false,
            ),
            resolve_output_constraint(&[], ToolChoice::Auto, &ResponseFormat::JsonObject, true),
            resolve_output_constraint(
                &[],
                ToolChoice::Auto,
                &ResponseFormat::JsonSchema(serde_json::json!({"type": "object"})),
                true,
            ),
            resolve_output_constraint(
                &[],
                ToolChoice::Auto,
                &ResponseFormat::Regex("[0-9]+".into()),
                true,
            ),
        ] {
            let constraint = constraint.map_err(|error| error.to_string())?;
            assert!(constraint.is_some());
            assert!(matches!(
                refuse_unenforced_deepseek_v41_constraint(constraint.as_ref()),
                Err(DeepSeekV41Error::Unsupported(_))
            ));
        }
        Ok(())
    }

    #[test]
    fn capabilities_are_protocol_only() {
        let capabilities = DEEPSEEK_V41_CAPABILITIES;
        assert_eq!(capabilities.qualification, "protocol");
        assert!(!capabilities.native_inference);
        assert!(!capabilities.structured_outputs);
        assert!(!capabilities.vision);
        assert!(!capabilities.enforced_output_constraints);
        assert_eq!(
            capabilities.tool_calling,
            DeepSeekV41ToolCalling::ProtocolOnly
        );
    }

    #[test]
    fn parsing_options_keep_answer_whitespace_and_follow_tool_choice() {
        let mut conversation = Conversation {
            messages: vec![user("a")],
            tools: vec![weather_tool(None)],
            ..Conversation::default()
        };
        let options = deepseek_v41_parsing_options(&conversation, Vec::new());
        assert!(options.preserves_answer_whitespace && options.parse_tool_calls);
        assert!(!options.starts_in_tool_calls && !options.parse_json_output);
        assert!(matches!(
            options.thinking_initial_stage,
            Some(ThinkingInitialStage::ReasoningStart)
        ));
        conversation.tool_choice = ToolChoice::None;
        conversation.thinking_mode = false;
        let options = deepseek_v41_parsing_options(&conversation, Vec::new());
        assert!(!options.parse_tool_calls && options.thinking_initial_stage.is_none());
    }
}
