use std::{collections::BTreeMap, fs, path::Path};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokenizers::Tokenizer;

use crate::QWEN38_27B_MODEL_ID;

pub const QWEN38_TEMPLATE_ID: &str = "qwen3.8.chat_template.v1";
pub const QWEN38_TEMPLATE_SHA256: &str =
    "c3cf9e34abf4f9e36c2d72165aa9c132d3e2a725b6c2586aaa3a8af9d7a81041";
pub const QWEN38_TOKENIZER_SHA256: &str =
    "0997f410c57a1f4e53b09e4be8f4a172d90edd9564368fb0847030937229b9f3";

const QWEN38_XHIGH_INSTRUCTION: &str = "Reasoning effort is set to xhigh. Please think carefully through the task, validate key assumptions, consider plausible alternatives, and prioritize correctness, consistency, and clarity in the final answer.";
const QWEN38_LOW_INSTRUCTION: &str = "Reasoning effort is set to low. Keep your thinking brief and focused, moving directly to the conclusion without unnecessary elaboration.";
const QWEN38_TOOL_PREAMBLE: &str =
    "# Tools\n\nYou have access to the following functions:\n\n<tools>";
const QWEN38_TOOL_SUFFIX: &str = "\n</tools>\n\nIf you choose to call a function ONLY reply in the following format with NO suffix:\n\n<tool_call>\n<function=example_function_name>\n<parameter=example_parameter_1>\nvalue_1\n</parameter>\n<parameter=example_parameter_2>\nThis is the value for the second parameter\nthat can span\nmultiple lines\n</parameter>\n</function>\n</tool_call>\n\n<IMPORTANT>\nReminder:\n- Function calls MUST follow the specified format: an inner <function=...></function> block must be nested within <tool_call></tool_call> XML tags\n- Required parameters MUST be specified\n- You may provide optional reasoning for your function call in natural language BEFORE the function call, but NOT after\n- If there is no function call available, answer the question like normal with your current knowledge and do not tell the user about function calls\n</IMPORTANT>";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Qwen38ReasoningEffort {
    Xhigh,
    Medium,
    Low,
}

impl Qwen38ReasoningEffort {
    fn parse(label: &str) -> Result<Self, Qwen38PromptError> {
        match label {
            "xhigh" => Ok(Self::Xhigh),
            "medium" => Ok(Self::Medium),
            "low" => Ok(Self::Low),
            other => Err(Qwen38PromptError::UnsupportedReasoningEffort {
                value: String::from(other),
            }),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Xhigh => "xhigh",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }

    fn system_instruction(self) -> Option<&'static str> {
        match self {
            Self::Xhigh => Some(QWEN38_XHIGH_INSTRUCTION),
            Self::Medium => None,
            Self::Low => Some(QWEN38_LOW_INSTRUCTION),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Qwen38PromptRole {
    System,
    Developer,
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Qwen38PromptContent {
    Text(String),
    Parts(Vec<Qwen38PromptContentPart>),
}

impl Default for Qwen38PromptContent {
    fn default() -> Self {
        Self::Text(String::new())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Qwen38PromptContentPart {
    Text { text: String },
    Image { image: Value },
    ImageUrl { image_url: Value },
    Video { video: Value },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38ToolFunctionDefinition {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub parameters: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: Qwen38ToolFunctionDefinition,
}

impl Qwen38ToolDefinition {
    pub fn function(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Value,
    ) -> Self {
        Self {
            tool_type: String::from("function"),
            function: Qwen38ToolFunctionDefinition {
                name: name.into(),
                description: Some(description.into()),
                parameters,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38ToolCall {
    pub name: String,
    #[serde(default)]
    pub arguments: BTreeMap<String, Value>,
}

impl Qwen38ToolCall {
    pub fn new(name: impl Into<String>, arguments: BTreeMap<String, Value>) -> Self {
        Self {
            name: name.into(),
            arguments,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38PromptMessage {
    pub role: Qwen38PromptRole,
    #[serde(default)]
    pub content: Qwen38PromptContent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<Qwen38ToolCall>,
}

impl Qwen38PromptMessage {
    pub fn text(role: Qwen38PromptRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: Qwen38PromptContent::Text(content.into()),
            reasoning_content: None,
            tool_calls: Vec::new(),
        }
    }

    pub fn parts(role: Qwen38PromptRole, parts: Vec<Qwen38PromptContentPart>) -> Self {
        Self {
            role,
            content: Qwen38PromptContent::Parts(parts),
            reasoning_content: None,
            tool_calls: Vec::new(),
        }
    }

    pub fn with_reasoning_content(mut self, reasoning_content: impl Into<String>) -> Self {
        self.reasoning_content = Some(reasoning_content.into());
        self
    }

    pub fn with_tool_calls(mut self, tool_calls: Vec<Qwen38ToolCall>) -> Self {
        self.tool_calls = tool_calls;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38PromptOptions {
    pub enable_thinking: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
    pub preserve_thinking: bool,
    pub add_generation_prompt: bool,
    pub add_vision_id: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<Qwen38ToolDefinition>,
}

impl Default for Qwen38PromptOptions {
    fn default() -> Self {
        Self {
            enable_thinking: true,
            reasoning_effort: None,
            preserve_thinking: true,
            add_generation_prompt: true,
            add_vision_id: false,
            tools: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38PromptReceipt {
    pub schema_version: String,
    pub model_id: String,
    pub template_id: String,
    pub template_sha256: String,
    pub tokenizer_sha256: String,
    pub thinking_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<Qwen38ReasoningEffort>,
    pub preserve_thinking: bool,
    pub add_generation_prompt: bool,
    pub add_vision_id: bool,
    pub tool_count: usize,
    pub rendered_sha256: String,
    pub prompt_cache_identity: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38RenderedPrompt {
    pub text: String,
    pub receipt: Qwen38PromptReceipt,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum Qwen38PromptError {
    #[error("Qwen3.8 prompt rendering requires at least one message")]
    EmptyMessages,
    #[error("unsupported Qwen3.8 reasoning effort `{value}`; expected xhigh, medium, or low")]
    UnsupportedReasoningEffort { value: String },
    #[error("Qwen3.8 system message cannot contain images")]
    ImageInSystemMessage,
    #[error("Qwen3.8 system message cannot contain videos")]
    VideoInSystemMessage,
    #[error("invalid Qwen3.8 conversation: {message}")]
    InvalidConversation { message: String },
    #[error("failed to serialize Qwen3.8 prompt state: {message}")]
    Serialization { message: String },
}

#[derive(Debug, Error)]
pub enum Qwen38TokenizerError {
    #[error("failed to read Qwen3.8 tokenizer `{path}`: {message}")]
    Read { path: String, message: String },
    #[error("Qwen3.8 tokenizer digest mismatch: expected {expected}, found {actual}")]
    DigestMismatch { expected: String, actual: String },
    #[error("failed to load Qwen3.8 tokenizer: {0}")]
    Load(String),
    #[error("failed to encode Qwen3.8 prompt: {0}")]
    Encode(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38TokenizedPrompt {
    pub token_ids: Vec<u32>,
    pub token_ids_sha256: String,
    pub token_count: usize,
    pub tokenizer_sha256: String,
    pub prompt_cache_identity: String,
}

pub struct Qwen38Tokenizer {
    inner: Tokenizer,
    sha256: String,
}

impl Qwen38Tokenizer {
    pub fn from_official_file(path: impl AsRef<Path>) -> Result<Self, Qwen38TokenizerError> {
        Self::from_file_with_digest(path, QWEN38_TOKENIZER_SHA256)
    }

    pub fn from_file_with_digest(
        path: impl AsRef<Path>,
        expected_sha256: &str,
    ) -> Result<Self, Qwen38TokenizerError> {
        let path = path.as_ref();
        let bytes = fs::read(path).map_err(|error| Qwen38TokenizerError::Read {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        let sha256 = hex::encode(Sha256::digest(&bytes));
        if sha256 != expected_sha256 {
            return Err(Qwen38TokenizerError::DigestMismatch {
                expected: String::from(expected_sha256),
                actual: sha256,
            });
        }
        let inner = Tokenizer::from_bytes(&bytes)
            .map_err(|error| Qwen38TokenizerError::Load(error.to_string()))?;
        Ok(Self { inner, sha256 })
    }

    pub fn tokenize(
        &self,
        prompt: &Qwen38RenderedPrompt,
    ) -> Result<Qwen38TokenizedPrompt, Qwen38TokenizerError> {
        let token_ids = self.encode_text(prompt.text.as_str())?;
        let mut token_bytes = Vec::with_capacity(token_ids.len() * size_of::<u32>());
        for token in &token_ids {
            token_bytes.extend_from_slice(&token.to_le_bytes());
        }
        Ok(Qwen38TokenizedPrompt {
            token_ids_sha256: hex::encode(Sha256::digest(token_bytes)),
            token_count: token_ids.len(),
            token_ids,
            tokenizer_sha256: self.sha256.clone(),
            prompt_cache_identity: prompt.receipt.prompt_cache_identity.clone(),
        })
    }

    pub fn encode_text(&self, text: &str) -> Result<Vec<u32>, Qwen38TokenizerError> {
        self.inner
            .encode(text, false)
            .map(|encoding| encoding.get_ids().to_vec())
            .map_err(|error| Qwen38TokenizerError::Encode(error.to_string()))
    }
}

#[derive(Default)]
struct Qwen38VisionCounters {
    images: usize,
    videos: usize,
}

pub fn render_qwen38_prompt(
    messages: &[Qwen38PromptMessage],
    options: &Qwen38PromptOptions,
) -> Result<Qwen38RenderedPrompt, Qwen38PromptError> {
    if messages.is_empty() {
        return Err(Qwen38PromptError::EmptyMessages);
    }

    let messages = normalize_instruction_messages(messages)?;
    let reasoning_effort = if options.enable_thinking {
        Some(Qwen38ReasoningEffort::parse(
            options.reasoning_effort.as_deref().unwrap_or("xhigh"),
        )?)
    } else {
        None
    };
    let reasoning_instruction = reasoning_effort.and_then(|value| value.system_instruction());

    let mut rendered = render_system_prefix(
        messages.first(),
        reasoning_instruction,
        options.tools.as_slice(),
    )?;
    let last_query_index = find_last_query_index(&messages)?;
    let mut counters = Qwen38VisionCounters::default();
    let start_index = usize::from(
        messages
            .first()
            .is_some_and(|message| message.role == Qwen38PromptRole::System),
    );

    let mut index = start_index;
    while index < messages.len() {
        let message = &messages[index];
        let content = render_content(
            &message.content,
            true,
            false,
            options.add_vision_id,
            &mut counters,
        )?;
        let content = content.trim();
        match message.role {
            Qwen38PromptRole::System | Qwen38PromptRole::Developer => {
                return Err(Qwen38PromptError::InvalidConversation {
                    message: String::from(
                        "system and developer messages must precede all user, assistant, and tool turns",
                    ),
                });
            }
            Qwen38PromptRole::User => {
                rendered.push_str("<|im_start|>user\n");
                rendered.push_str(content);
                rendered.push_str("<|im_end|>\n");
            }
            Qwen38PromptRole::Assistant => {
                rendered.push_str("<|im_start|>assistant\n");
                if options.preserve_thinking || index > last_query_index {
                    rendered.push_str("<think>\n");
                    rendered.push_str(
                        message
                            .reasoning_content
                            .as_deref()
                            .unwrap_or_default()
                            .trim(),
                    );
                    rendered.push_str("\n</think>\n\n");
                }
                rendered.push_str(content);
                render_tool_calls(&mut rendered, content, message.tool_calls.as_slice())?;
                rendered.push_str("<|im_end|>\n");
            }
            Qwen38PromptRole::Tool => {
                if index == start_index || messages[index - 1].role != Qwen38PromptRole::Tool {
                    rendered.push_str("<|im_start|>user");
                }
                rendered.push_str("\n<tool_response>\n");
                rendered.push_str(content);
                rendered.push_str("\n</tool_response>");
                if index + 1 == messages.len() || messages[index + 1].role != Qwen38PromptRole::Tool
                {
                    rendered.push_str("<|im_end|>\n");
                }
            }
        }
        index += 1;
    }

    if options.add_generation_prompt {
        rendered.push_str("<|im_start|>assistant\n");
        if options.enable_thinking {
            rendered.push_str("<think>\n");
        } else {
            rendered.push_str("<think>\n\n</think>\n\n");
        }
    }

    let rendered_sha256 = hex::encode(Sha256::digest(rendered.as_bytes()));
    let prompt_cache_identity =
        prompt_cache_identity(rendered_sha256.as_str(), reasoning_effort, options)?;
    Ok(Qwen38RenderedPrompt {
        text: rendered,
        receipt: Qwen38PromptReceipt {
            schema_version: String::from("psionic.qwen38.prompt_receipt.v1"),
            model_id: String::from(QWEN38_27B_MODEL_ID),
            template_id: String::from(QWEN38_TEMPLATE_ID),
            template_sha256: String::from(QWEN38_TEMPLATE_SHA256),
            tokenizer_sha256: String::from(QWEN38_TOKENIZER_SHA256),
            thinking_enabled: options.enable_thinking,
            reasoning_effort,
            preserve_thinking: options.preserve_thinking,
            add_generation_prompt: options.add_generation_prompt,
            add_vision_id: options.add_vision_id,
            tool_count: options.tools.len(),
            rendered_sha256,
            prompt_cache_identity,
        },
    })
}

fn normalize_instruction_messages(
    messages: &[Qwen38PromptMessage],
) -> Result<Vec<Qwen38PromptMessage>, Qwen38PromptError> {
    let mut normalized = Vec::new();
    let mut instructions = Vec::new();
    let mut index = 0usize;
    let mut counters = Qwen38VisionCounters::default();
    while index < messages.len()
        && matches!(
            messages[index].role,
            Qwen38PromptRole::System | Qwen38PromptRole::Developer
        )
    {
        let instruction =
            render_content(&messages[index].content, false, true, false, &mut counters)?;
        let instruction = instruction.trim();
        if !instruction.is_empty() {
            instructions.push(String::from(instruction));
        }
        index += 1;
    }
    if messages[index..].iter().any(|message| {
        matches!(
            message.role,
            Qwen38PromptRole::System | Qwen38PromptRole::Developer
        )
    }) {
        return Err(Qwen38PromptError::InvalidConversation {
            message: String::from(
                "system and developer messages must precede all user, assistant, and tool turns",
            ),
        });
    }
    if !instructions.is_empty() {
        normalized.push(Qwen38PromptMessage::text(
            Qwen38PromptRole::System,
            instructions.join("\n\n"),
        ));
    }
    normalized.extend_from_slice(&messages[index..]);
    Ok(normalized)
}

fn render_system_prefix(
    first_message: Option<&Qwen38PromptMessage>,
    reasoning_instruction: Option<&str>,
    tools: &[Qwen38ToolDefinition],
) -> Result<String, Qwen38PromptError> {
    let mut counters = Qwen38VisionCounters::default();
    let system_content = if let Some(message) = first_message
        && message.role == Qwen38PromptRole::System
    {
        render_content(&message.content, false, true, false, &mut counters)?
            .trim()
            .to_string()
    } else {
        String::new()
    };

    if !tools.is_empty() {
        let mut rendered = String::from("<|im_start|>system\n");
        if let Some(instruction) = reasoning_instruction {
            rendered.push_str(instruction);
            rendered.push_str("\n\n");
        }
        rendered.push_str(QWEN38_TOOL_PREAMBLE);
        for tool in tools {
            rendered.push('\n');
            rendered.push_str(jinja_tool_json(tool)?.as_str());
        }
        rendered.push_str(QWEN38_TOOL_SUFFIX);
        if !system_content.is_empty() {
            rendered.push_str("\n\n");
            rendered.push_str(system_content.as_str());
        }
        rendered.push_str("<|im_end|>\n");
        return Ok(rendered);
    }

    if !system_content.is_empty() || reasoning_instruction.is_some() {
        let mut rendered = String::from("<|im_start|>system\n");
        if let Some(instruction) = reasoning_instruction {
            rendered.push_str(instruction);
            if !system_content.is_empty() {
                rendered.push_str("\n\n");
            }
        }
        rendered.push_str(system_content.as_str());
        rendered.push_str("<|im_end|>\n");
        return Ok(rendered);
    }
    Ok(String::new())
}

fn find_last_query_index(messages: &[Qwen38PromptMessage]) -> Result<usize, Qwen38PromptError> {
    let mut counters = Qwen38VisionCounters::default();
    for (index, message) in messages.iter().enumerate().rev() {
        if message.role != Qwen38PromptRole::User {
            continue;
        }
        let content = render_content(&message.content, false, false, false, &mut counters)?;
        let content = content.trim();
        if !(content.starts_with("<tool_response>") && content.ends_with("</tool_response>")) {
            return Ok(index);
        }
    }
    Err(Qwen38PromptError::InvalidConversation {
        message: String::from("no user query found in messages"),
    })
}

fn render_content(
    content: &Qwen38PromptContent,
    count_vision: bool,
    system_content: bool,
    add_vision_id: bool,
    counters: &mut Qwen38VisionCounters,
) -> Result<String, Qwen38PromptError> {
    let Qwen38PromptContent::Parts(parts) = content else {
        return match content {
            Qwen38PromptContent::Text(text) => Ok(text.clone()),
            Qwen38PromptContent::Parts(_) => unreachable!("parts handled above"),
        };
    };
    let mut rendered = String::new();
    for part in parts {
        match part {
            Qwen38PromptContentPart::Text { text } => rendered.push_str(text),
            Qwen38PromptContentPart::Image { .. } | Qwen38PromptContentPart::ImageUrl { .. } => {
                if system_content {
                    return Err(Qwen38PromptError::ImageInSystemMessage);
                }
                if count_vision {
                    counters.images += 1;
                }
                if add_vision_id {
                    rendered.push_str(format!("Picture {}: ", counters.images).as_str());
                }
                rendered.push_str("<|vision_start|><|image_pad|><|vision_end|>");
            }
            Qwen38PromptContentPart::Video { .. } => {
                if system_content {
                    return Err(Qwen38PromptError::VideoInSystemMessage);
                }
                if count_vision {
                    counters.videos += 1;
                }
                if add_vision_id {
                    rendered.push_str(format!("Video {}: ", counters.videos).as_str());
                }
                rendered.push_str("<|vision_start|><|video_pad|><|vision_end|>");
            }
        }
    }
    Ok(rendered)
}

fn render_tool_calls(
    rendered: &mut String,
    content: &str,
    tool_calls: &[Qwen38ToolCall],
) -> Result<(), Qwen38PromptError> {
    for (index, tool_call) in tool_calls.iter().enumerate() {
        if index == 0 && !content.trim().is_empty() {
            rendered.push_str("\n\n");
        } else if index > 0 {
            rendered.push('\n');
        }
        rendered.push_str("<tool_call>\n<function=");
        rendered.push_str(tool_call.name.as_str());
        rendered.push_str(">\n");
        for (name, value) in &tool_call.arguments {
            rendered.push_str("<parameter=");
            rendered.push_str(name.as_str());
            rendered.push_str(">\n");
            if let Some(value) = value.as_str() {
                rendered.push_str(value);
            } else {
                rendered.push_str(jinja_json(value)?.as_str());
            }
            rendered.push_str("\n</parameter>\n");
        }
        rendered.push_str("</function>\n</tool_call>");
    }
    Ok(())
}

fn jinja_tool_json(tool: &Qwen38ToolDefinition) -> Result<String, Qwen38PromptError> {
    let tool_type = jinja_json_string(&tool.tool_type)?;
    let name = jinja_json_string(&tool.function.name)?;
    let mut function = format!("{{\"name\": {name}");
    if let Some(description) = &tool.function.description {
        let description = jinja_json_string(description)?;
        function.push_str(format!(", \"description\": {description}").as_str());
    }
    function.push_str(
        format!(
            ", \"parameters\": {}}}",
            jinja_json(&tool.function.parameters)?
        )
        .as_str(),
    );
    Ok(format!(
        "{{\"type\": {tool_type}, \"function\": {function}}}"
    ))
}

fn jinja_json(value: &Value) -> Result<String, Qwen38PromptError> {
    match value {
        Value::String(value) => jinja_json_string(value),
        Value::Null | Value::Bool(_) | Value::Number(_) => {
            serde_json::to_string(value).map_err(|error| Qwen38PromptError::Serialization {
                message: error.to_string(),
            })
        }
        Value::Array(values) => values
            .iter()
            .map(jinja_json)
            .collect::<Result<Vec<_>, _>>()
            .map(|values| format!("[{}]", values.join(", "))),
        Value::Object(values) => values
            .iter()
            .map(|(key, value)| {
                let key = jinja_json_string(key)?;
                Ok(format!("{key}: {}", jinja_json(value)?))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|values| format!("{{{}}}", values.join(", "))),
    }
}

fn jinja_json_string(value: &str) -> Result<String, Qwen38PromptError> {
    serde_json::to_string(value).map_err(|error| Qwen38PromptError::Serialization {
        message: error.to_string(),
    })
}

fn prompt_cache_identity(
    rendered_sha256: &str,
    reasoning_effort: Option<Qwen38ReasoningEffort>,
    options: &Qwen38PromptOptions,
) -> Result<String, Qwen38PromptError> {
    let value = serde_json::json!({
        "schema_version": "psionic.qwen38.prompt_cache_identity.v1",
        "model_id": QWEN38_27B_MODEL_ID,
        "template_id": QWEN38_TEMPLATE_ID,
        "template_sha256": QWEN38_TEMPLATE_SHA256,
        "tokenizer_sha256": QWEN38_TOKENIZER_SHA256,
        "rendered_sha256": rendered_sha256,
        "thinking_enabled": options.enable_thinking,
        "reasoning_effort": reasoning_effort.map(Qwen38ReasoningEffort::as_str),
        "preserve_thinking": options.preserve_thinking,
        "add_generation_prompt": options.add_generation_prompt,
        "add_vision_id": options.add_vision_id,
        "tools": options.tools,
    });
    let bytes = serde_json::to_vec(&value).map_err(|error| Qwen38PromptError::Serialization {
        message: error.to_string(),
    })?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOLDEN_FIXTURE: &str =
        include_str!("../../../fixtures/qwen38/qwen38_prompt_tokenizer_golden_v1.json");
    const RELEASE_TEMPLATE_FIXTURE: &str =
        include_str!("../../../fixtures/qwen38/qwen38_release_template_cases_v1.json");

    #[derive(Deserialize)]
    struct GoldenFixture {
        schema_version: String,
        source: GoldenSource,
        prompt_cases: Vec<GoldenPromptCase>,
        tokenizer_cases: Vec<GoldenTokenizerCase>,
    }

    #[derive(Deserialize)]
    struct GoldenSource {
        revision: String,
        template_sha256: String,
        tokenizer_sha256: String,
        reference: String,
        llama_cpp_comparison: GoldenLlamaCppComparison,
    }

    #[derive(Deserialize)]
    struct GoldenLlamaCppComparison {
        revision: String,
        executable: String,
        artifact: String,
        matched_case_count: usize,
        total_case_count: usize,
        mismatches: Vec<GoldenTokenizerMismatch>,
    }

    #[derive(Deserialize)]
    struct GoldenTokenizerMismatch {
        text: String,
        official_token_ids: Vec<u32>,
        llama_cpp_token_ids: Vec<u32>,
        reason: String,
    }

    #[derive(Deserialize)]
    struct GoldenPromptCase {
        case_id: String,
        rendered_sha256: String,
        token_ids: Vec<u32>,
    }

    #[derive(Deserialize)]
    struct GoldenTokenizerCase {
        text: String,
        token_ids: Vec<u32>,
    }

    #[derive(Deserialize)]
    struct ReleaseTemplateFixture {
        schema_version: String,
        source: ReleaseTemplateSource,
        cases: Vec<ReleaseTemplateCase>,
    }

    #[derive(Deserialize)]
    struct ReleaseTemplateSource {
        upstream_repository_id: String,
        upstream_revision: String,
        template_sha256: String,
        comparator_implementation: String,
        comparator_revision: String,
        comparator_endpoint: String,
    }

    #[derive(Deserialize)]
    struct ReleaseTemplateCase {
        case_id: String,
        messages: Vec<Qwen38PromptMessage>,
        chat_template_kwargs: BTreeMap<String, String>,
        expected_rendered_sha256: String,
    }

    fn user(content: &str) -> Qwen38PromptMessage {
        Qwen38PromptMessage::text(Qwen38PromptRole::User, content)
    }

    fn golden_fixture() -> GoldenFixture {
        serde_json::from_str(GOLDEN_FIXTURE).expect("Qwen3.8 prompt/tokenizer golden fixture")
    }

    fn golden_prompt_case<'a>(fixture: &'a GoldenFixture, case_id: &str) -> &'a GoldenPromptCase {
        fixture
            .prompt_cases
            .iter()
            .find(|case| case.case_id == case_id)
            .expect("Qwen3.8 golden prompt case")
    }

    fn qwen38_golden_prompts() -> BTreeMap<String, Qwen38RenderedPrompt> {
        let mut prompts = BTreeMap::new();
        prompts.insert(
            String::from("default_xhigh"),
            render_qwen38_prompt(&[user("Hello")], &Qwen38PromptOptions::default())
                .expect("default prompt"),
        );
        prompts.insert(
            String::from("medium_normalized_developer"),
            render_qwen38_prompt(
                &[
                    Qwen38PromptMessage::text(Qwen38PromptRole::System, "system"),
                    Qwen38PromptMessage::text(Qwen38PromptRole::Developer, "developer"),
                    user("second"),
                ],
                &Qwen38PromptOptions {
                    reasoning_effort: Some(String::from("medium")),
                    ..Qwen38PromptOptions::default()
                },
            )
            .expect("normalized developer prompt"),
        );
        prompts.insert(
            String::from("non_thinking"),
            render_qwen38_prompt(
                &[user("Hello")],
                &Qwen38PromptOptions {
                    enable_thinking: false,
                    ..Qwen38PromptOptions::default()
                },
            )
            .expect("non-thinking prompt"),
        );
        prompts.insert(
            String::from("preserve_thinking_disabled"),
            render_qwen38_prompt(
                &[
                    user("first"),
                    Qwen38PromptMessage::text(Qwen38PromptRole::Assistant, "answer")
                        .with_reasoning_content("reason"),
                    user("second"),
                ],
                &Qwen38PromptOptions {
                    reasoning_effort: Some(String::from("medium")),
                    preserve_thinking: false,
                    add_generation_prompt: false,
                    ..Qwen38PromptOptions::default()
                },
            )
            .expect("preserve-thinking-disabled prompt"),
        );

        let mut weather_arguments = BTreeMap::new();
        weather_arguments.insert(String::from("city"), Value::String(String::from("Paris")));
        prompts.insert(
            String::from("tools_and_results"),
            render_qwen38_prompt(
                &[
                    user("Check Paris."),
                    Qwen38PromptMessage::text(Qwen38PromptRole::Assistant, "Calling weather")
                        .with_reasoning_content("Need current data")
                        .with_tool_calls(vec![Qwen38ToolCall::new("weather", weather_arguments)]),
                    Qwen38PromptMessage::text(Qwen38PromptRole::Tool, "sunny"),
                    Qwen38PromptMessage::text(Qwen38PromptRole::Tool, "20 C"),
                ],
                &Qwen38PromptOptions {
                    reasoning_effort: Some(String::from("low")),
                    add_generation_prompt: false,
                    tools: vec![Qwen38ToolDefinition::function(
                        "weather",
                        "Get weather",
                        serde_json::json!({
                            "properties": {"city": {"type": "string"}},
                            "required": ["city"],
                            "type": "object"
                        }),
                    )],
                    ..Qwen38PromptOptions::default()
                },
            )
            .expect("tool prompt"),
        );
        prompts.insert(
            String::from("media_markers"),
            render_qwen38_prompt(
                &[Qwen38PromptMessage::parts(
                    Qwen38PromptRole::User,
                    vec![
                        Qwen38PromptContentPart::Text {
                            text: String::from("inspect "),
                        },
                        Qwen38PromptContentPart::ImageUrl {
                            image_url: Value::String(String::from("image.png")),
                        },
                        Qwen38PromptContentPart::Video {
                            video: Value::String(String::from("video.mp4")),
                        },
                    ],
                )],
                &Qwen38PromptOptions {
                    reasoning_effort: Some(String::from("medium")),
                    add_vision_id: true,
                    ..Qwen38PromptOptions::default()
                },
            )
            .expect("media prompt"),
        );
        prompts
    }

    #[test]
    fn qwen38_prompt_matches_pinned_transformers_golden_hashes() {
        let fixture = golden_fixture();
        assert_eq!(
            fixture.schema_version,
            "psionic.qwen38.prompt_tokenizer_golden.v1"
        );
        assert_eq!(fixture.source.template_sha256, QWEN38_TEMPLATE_SHA256);
        assert_eq!(fixture.source.tokenizer_sha256, QWEN38_TOKENIZER_SHA256);
        assert_eq!(
            fixture.source.revision,
            "1d4bf0f2ff6012fd82039f2fa52739d0dd7c60c0"
        );
        assert_eq!(fixture.source.reference, "transformers 5.15.0");
        assert_eq!(
            fixture.source.llama_cpp_comparison.revision,
            "9b05354ec6fb58b4e665e9a39ebc40285c015638"
        );
        assert_eq!(
            fixture.source.llama_cpp_comparison.executable,
            "llama-tokenize"
        );
        assert_eq!(
            fixture.source.llama_cpp_comparison.artifact,
            "Qwen3.8-27B-UD-Q3_K_XL.gguf"
        );
        assert_eq!(fixture.source.llama_cpp_comparison.matched_case_count, 8);
        assert_eq!(fixture.source.llama_cpp_comparison.total_case_count, 9);
        let mismatch = fixture
            .source
            .llama_cpp_comparison
            .mismatches
            .first()
            .expect("retained llama.cpp NFC mismatch");
        assert_eq!(mismatch.text, "café café");
        assert_eq!(mismatch.official_token_ids, [895, 56868, 50203]);
        assert_eq!(mismatch.llama_cpp_token_ids, [895, 56868, 39579, 52033]);
        assert!(mismatch.reason.contains("NFC normalizer"));

        let prompts = qwen38_golden_prompts();
        assert_eq!(prompts.len(), fixture.prompt_cases.len());
        for (case_id, rendered) in prompts {
            assert_eq!(
                rendered.receipt.rendered_sha256,
                golden_prompt_case(&fixture, case_id.as_str()).rendered_sha256,
                "rendered bytes drifted for {case_id}"
            );
        }
    }

    #[test]
    fn qwen38_release_template_cases_bind_all_reasoning_efforts() {
        let fixture: ReleaseTemplateFixture = serde_json::from_str(RELEASE_TEMPLATE_FIXTURE)
            .expect("Qwen3.8 release template fixture");
        assert_eq!(
            fixture.schema_version,
            "psionic.qwen38.release_template_cases.v1"
        );
        assert_eq!(fixture.source.upstream_repository_id, QWEN38_27B_MODEL_ID);
        assert_eq!(
            fixture.source.upstream_revision,
            "1d4bf0f2ff6012fd82039f2fa52739d0dd7c60c0"
        );
        assert_eq!(fixture.source.template_sha256, QWEN38_TEMPLATE_SHA256);
        assert_eq!(
            fixture.source.comparator_implementation,
            "ggml-org/llama.cpp"
        );
        assert_eq!(
            fixture.source.comparator_revision,
            "9b05354ec6fb58b4e665e9a39ebc40285c015638"
        );
        assert_eq!(fixture.source.comparator_endpoint, "/apply-template");
        assert_eq!(fixture.cases.len(), 3);

        let mut observed_efforts = Vec::new();
        for case in fixture.cases {
            let reasoning_effort = case
                .chat_template_kwargs
                .get("reasoning_effort")
                .cloned()
                .expect("reasoning_effort template argument");
            let rendered = render_qwen38_prompt(
                &case.messages,
                &Qwen38PromptOptions {
                    reasoning_effort: Some(reasoning_effort.clone()),
                    ..Qwen38PromptOptions::default()
                },
            )
            .unwrap_or_else(|error| panic!("{} failed to render: {error}", case.case_id));
            assert_eq!(
                rendered.receipt.rendered_sha256, case.expected_rendered_sha256,
                "{} rendered digest",
                case.case_id
            );
            observed_efforts.push(reasoning_effort);
        }
        observed_efforts.sort();
        assert_eq!(observed_efforts, ["low", "medium", "xhigh"]);
    }

    #[test]
    fn qwen38_tokenizer_matches_official_golden_ids_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let path = std::env::var("PSIONIC_QWEN38_TOKENIZER_PATH")
            .unwrap_or_else(|_| String::from("target/models/qwen/Qwen3.8-27B/tokenizer.json"));
        if !Path::new(path.as_str()).exists() {
            return Ok(());
        }
        let fixture = golden_fixture();
        let tokenizer = Qwen38Tokenizer::from_official_file(path)?;
        for case in &fixture.tokenizer_cases {
            assert_eq!(
                tokenizer.encode_text(case.text.as_str())?,
                case.token_ids,
                "token ids drifted for {:?}",
                case.text
            );
        }
        for (case_id, rendered) in qwen38_golden_prompts() {
            let expected = &golden_prompt_case(&fixture, case_id.as_str()).token_ids;
            if expected.is_empty() {
                continue;
            }
            assert_eq!(
                tokenizer.tokenize(&rendered)?.token_ids,
                *expected,
                "rendered token ids drifted for {case_id}"
            );
        }
        Ok(())
    }

    #[test]
    fn qwen38_prompt_requires_a_user_query_and_supported_effort() {
        assert_eq!(
            render_qwen38_prompt(&[], &Qwen38PromptOptions::default()),
            Err(Qwen38PromptError::EmptyMessages)
        );
        let options = Qwen38PromptOptions {
            reasoning_effort: Some(String::from("high")),
            ..Qwen38PromptOptions::default()
        };
        assert_eq!(
            render_qwen38_prompt(&[user("hello")], &options),
            Err(Qwen38PromptError::UnsupportedReasoningEffort {
                value: String::from("high")
            })
        );
        assert!(matches!(
            render_qwen38_prompt(
                &[Qwen38PromptMessage::text(
                    Qwen38PromptRole::System,
                    "instructions"
                )],
                &Qwen38PromptOptions::default()
            ),
            Err(Qwen38PromptError::InvalidConversation { .. })
        ));
    }

    #[test]
    fn qwen38_prompt_renders_reasoning_and_non_thinking_generation_frames() {
        let messages = [user("Hello")];
        let xhigh =
            render_qwen38_prompt(&messages, &Qwen38PromptOptions::default()).expect("xhigh prompt");
        assert_eq!(
            xhigh.text,
            format!(
                "<|im_start|>system\n{QWEN38_XHIGH_INSTRUCTION}<|im_end|>\n<|im_start|>user\nHello<|im_end|>\n<|im_start|>assistant\n<think>\n"
            )
        );

        let medium = render_qwen38_prompt(
            &messages,
            &Qwen38PromptOptions {
                reasoning_effort: Some(String::from("medium")),
                ..Qwen38PromptOptions::default()
            },
        )
        .expect("medium prompt");
        assert_eq!(
            medium.text,
            "<|im_start|>user\nHello<|im_end|>\n<|im_start|>assistant\n<think>\n"
        );

        let low = render_qwen38_prompt(
            &messages,
            &Qwen38PromptOptions {
                reasoning_effort: Some(String::from("low")),
                ..Qwen38PromptOptions::default()
            },
        )
        .expect("low prompt");
        assert!(low.text.contains(QWEN38_LOW_INSTRUCTION));

        let direct = render_qwen38_prompt(
            &messages,
            &Qwen38PromptOptions {
                enable_thinking: false,
                ..Qwen38PromptOptions::default()
            },
        )
        .expect("direct prompt");
        assert_eq!(
            direct.text,
            "<|im_start|>user\nHello<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
        );
    }

    #[test]
    fn qwen38_prompt_normalizes_leading_developer_and_preserves_reasoning() {
        let messages = [
            Qwen38PromptMessage::text(Qwen38PromptRole::System, "system"),
            Qwen38PromptMessage::text(Qwen38PromptRole::Developer, "developer"),
            user("first"),
            Qwen38PromptMessage::text(Qwen38PromptRole::Assistant, "answer")
                .with_reasoning_content("reason"),
            user("second"),
        ];
        let preserved = render_qwen38_prompt(
            &messages,
            &Qwen38PromptOptions {
                reasoning_effort: Some(String::from("medium")),
                add_generation_prompt: false,
                ..Qwen38PromptOptions::default()
            },
        )
        .expect("preserved prompt");
        assert!(
            preserved
                .text
                .starts_with("<|im_start|>system\nsystem\n\ndeveloper<|im_end|>\n")
        );
        assert!(
            preserved
                .text
                .contains("<think>\nreason\n</think>\n\nanswer")
        );

        let stripped = render_qwen38_prompt(
            &messages,
            &Qwen38PromptOptions {
                reasoning_effort: Some(String::from("medium")),
                preserve_thinking: false,
                add_generation_prompt: false,
                ..Qwen38PromptOptions::default()
            },
        )
        .expect("stripped prompt");
        assert!(!stripped.text.contains("<think>"));
        assert!(
            stripped
                .text
                .contains("<|im_start|>assistant\nanswer<|im_end|>")
        );
        assert_ne!(
            preserved.receipt.prompt_cache_identity,
            stripped.receipt.prompt_cache_identity
        );
    }

    #[test]
    fn qwen38_prompt_groups_tool_results_and_renders_multiple_calls() {
        let mut weather = BTreeMap::new();
        weather.insert(String::from("city"), Value::String(String::from("Paris")));
        let mut clock = BTreeMap::new();
        clock.insert(String::from("zone"), Value::String(String::from("UTC")));
        let messages = [
            user("Check both."),
            Qwen38PromptMessage::text(Qwen38PromptRole::Assistant, "Calling tools")
                .with_tool_calls(vec![
                    Qwen38ToolCall::new("weather", weather),
                    Qwen38ToolCall::new("clock", clock),
                ]),
            Qwen38PromptMessage::text(Qwen38PromptRole::Tool, "sunny"),
            Qwen38PromptMessage::text(Qwen38PromptRole::Tool, "12:00"),
        ];
        let rendered = render_qwen38_prompt(
            &messages,
            &Qwen38PromptOptions {
                reasoning_effort: Some(String::from("medium")),
                add_generation_prompt: false,
                ..Qwen38PromptOptions::default()
            },
        )
        .expect("tool prompt");
        assert!(rendered.text.contains(
            "Calling tools\n\n<tool_call>\n<function=weather>\n<parameter=city>\nParis\n</parameter>\n</function>\n</tool_call>\n<tool_call>\n<function=clock>"
        ));
        assert!(rendered.text.contains(
            "<|im_start|>user\n<tool_response>\nsunny\n</tool_response>\n<tool_response>\n12:00\n</tool_response><|im_end|>\n"
        ));
    }

    #[test]
    fn qwen38_tool_json_matches_transformers_unescaped_tojson_filter() {
        assert_eq!(
            jinja_json_string("<tag>&' café").expect("JSON string"),
            "\"<tag>&' café\""
        );
    }

    #[test]
    fn qwen38_prompt_projects_media_markers_and_refuses_system_media() {
        let messages = [Qwen38PromptMessage::parts(
            Qwen38PromptRole::User,
            vec![
                Qwen38PromptContentPart::Text {
                    text: String::from("inspect "),
                },
                Qwen38PromptContentPart::ImageUrl {
                    image_url: Value::String(String::from("image.png")),
                },
                Qwen38PromptContentPart::Video {
                    video: Value::String(String::from("video.mp4")),
                },
            ],
        )];
        let rendered = render_qwen38_prompt(
            &messages,
            &Qwen38PromptOptions {
                reasoning_effort: Some(String::from("medium")),
                add_vision_id: true,
                ..Qwen38PromptOptions::default()
            },
        )
        .expect("media markers");
        assert!(rendered.text.contains(
            "inspect Picture 1: <|vision_start|><|image_pad|><|vision_end|>Video 1: <|vision_start|><|video_pad|><|vision_end|>"
        ));

        for part in [
            Qwen38PromptContentPart::Image {
                image: Value::String(String::from("image.png")),
            },
            Qwen38PromptContentPart::Video {
                video: Value::String(String::from("video.mp4")),
            },
        ] {
            let messages = [
                Qwen38PromptMessage::parts(Qwen38PromptRole::System, vec![part]),
                user("hello"),
            ];
            assert!(matches!(
                render_qwen38_prompt(&messages, &Qwen38PromptOptions::default()),
                Err(Qwen38PromptError::ImageInSystemMessage
                    | Qwen38PromptError::VideoInSystemMessage)
            ));
        }
    }

    #[test]
    fn qwen38_prompt_cache_identity_changes_with_reasoning_settings() {
        let messages = [user("hello")];
        let xhigh =
            render_qwen38_prompt(&messages, &Qwen38PromptOptions::default()).expect("xhigh prompt");
        let low = render_qwen38_prompt(
            &messages,
            &Qwen38PromptOptions {
                reasoning_effort: Some(String::from("low")),
                ..Qwen38PromptOptions::default()
            },
        )
        .expect("low prompt");
        assert_ne!(
            xhigh.receipt.prompt_cache_identity,
            low.receipt.prompt_cache_identity
        );
        assert_eq!(xhigh.receipt.template_sha256, QWEN38_TEMPLATE_SHA256);
        assert_eq!(xhigh.receipt.tokenizer_sha256, QWEN38_TOKENIZER_SHA256);
    }
}
