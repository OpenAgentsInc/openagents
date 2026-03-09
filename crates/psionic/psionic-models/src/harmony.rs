use openai_harmony::chat::{
    Author as HarmonyAuthor, ChannelConfig as HarmonyChannelConfig,
    Conversation as HarmonyConversation, DeveloperContent as HarmonyDeveloperContent,
    Message as HarmonyMessage, ReasoningEffort as HarmonyReasoningEffort, Role as HarmonyRole,
    SystemContent as HarmonySystemContent, ToolDescription as HarmonyToolDescription,
    ToolNamespaceConfig as HarmonyToolNamespaceConfig,
};
use openai_harmony::{
    HarmonyEncodingName, ParseOptions as HarmonyParseOptions,
    StreamableParser as HarmonyStreamableParser, load_harmony_encoding,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::{
    GgufTokenizerMetadata, GgufTokenizerModel, PromptMessage, PromptMessageRole, TokenId,
    TokenSequence, TokenVocabulary, TokenizerBoundary,
};

/// GPT-OSS / Harmony reasoning-effort label.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptReasoningEffort {
    /// Lower-latency reasoning guidance.
    Low,
    /// Default reasoning guidance.
    Medium,
    /// Higher-effort reasoning guidance.
    High,
}

/// One prompt-visible tool definition for Harmony rendering.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptToolDefinition {
    /// Tool name inside the owning namespace.
    pub name: String,
    /// Human-readable tool description.
    pub description: String,
    /// OpenAPI-style JSON schema for tool parameters when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Value>,
}

/// One prompt-visible tool namespace for Harmony rendering.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptToolNamespace {
    /// Stable namespace name such as `functions`, `browser`, or `python`.
    pub name: String,
    /// Namespace-level description when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Tools carried by the namespace.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<PromptToolDefinition>,
}

/// Explicit channel rules surfaced to GPT-OSS / Harmony prompts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptChannelConfig {
    /// Valid channel labels for the conversation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub valid_channels: Vec<String>,
    /// Whether every assistant message must carry a channel.
    pub channel_required: bool,
}

impl PromptChannelConfig {
    /// Creates a required-channel configuration.
    #[must_use]
    pub fn require<I, T>(channels: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        Self {
            valid_channels: channels.into_iter().map(Into::into).collect(),
            channel_required: true,
        }
    }
}

impl Default for PromptChannelConfig {
    fn default() -> Self {
        Self::require(["analysis", "commentary", "final"])
    }
}

/// GPT-OSS / Harmony system-context overrides used during rendering.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GptOssHarmonyRenderContext {
    /// Model-identity override when the default should change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_identity: Option<String>,
    /// Reasoning-effort override when the default should change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<PromptReasoningEffort>,
    /// Function or builtin tool namespaces exposed to the model.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_namespaces: Vec<PromptToolNamespace>,
    /// Conversation date rendered into the Harmony system message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_start_date: Option<String>,
    /// Training cutoff rendered into the Harmony system message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub knowledge_cutoff: Option<String>,
    /// Explicit channel requirements for the Harmony system message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_config: Option<PromptChannelConfig>,
}

/// Prompt-render options surfaced by Psionic.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PromptRenderOptions {
    /// GPT-OSS / Harmony system-context overrides when the selected family uses Harmony.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpt_oss_harmony: Option<GptOssHarmonyRenderContext>,
}

/// Source lane used to recover structured Harmony output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GptOssHarmonyParseSource {
    /// Structured output was parsed from token IDs.
    Tokens,
    /// Structured output was parsed from raw text.
    Text,
}

/// Structured GPT-OSS / Harmony output recovered from model text or tokens.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GptOssHarmonyParsedOutput {
    /// Where the parser recovered the structure from.
    pub source: GptOssHarmonyParseSource,
    /// Fully parsed Harmony messages.
    pub messages: Vec<PromptMessage>,
}

/// Parser options for GPT-OSS / Harmony output recovery.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GptOssHarmonyParseOptions {
    /// Role hint when parsing a completion stream that starts after `<|start|>ROLE`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_hint: Option<PromptMessageRole>,
    /// Whether malformed output should be rejected instead of recovered leniently.
    pub strict: bool,
}

impl Default for GptOssHarmonyParseOptions {
    fn default() -> Self {
        Self {
            role_hint: None,
            strict: true,
        }
    }
}

/// GPT-OSS / Harmony render or parse failure.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum GptOssHarmonyError {
    /// Loading the Harmony reference encoding failed.
    #[error("failed to load harmony encoding: {message}")]
    LoadEncoding {
        /// Lower-level failure summary.
        message: String,
    },
    /// The prompt history cannot be represented honestly in Harmony.
    #[error("invalid gpt-oss harmony conversation: {message}")]
    InvalidConversation {
        /// Validation failure summary.
        message: String,
    },
    /// Rendering a Harmony prompt failed.
    #[error("failed to render gpt-oss harmony prompt: {message}")]
    Render {
        /// Lower-level failure summary.
        message: String,
    },
    /// Parsing Harmony output failed.
    #[error("failed to parse gpt-oss harmony output: {message}")]
    Parse {
        /// Lower-level failure summary.
        message: String,
    },
}

/// Incremental GPT-OSS / Harmony parser wrapper exposing Psionic-owned types.
pub struct GptOssHarmonyStreamParser {
    inner: HarmonyStreamableParser,
}

impl GptOssHarmonyStreamParser {
    /// Creates a stream parser using the GPT-OSS Harmony encoding.
    pub fn new(options: GptOssHarmonyParseOptions) -> Result<Self, GptOssHarmonyError> {
        let encoding = load_gpt_oss_harmony_encoding()?;
        let role_hint = harmony_role_hint(options.role_hint)?;
        let inner = HarmonyStreamableParser::new_with_options(
            encoding,
            role_hint,
            HarmonyParseOptions {
                strict: options.strict,
            },
        )
        .map_err(|error| GptOssHarmonyError::Parse {
            message: error.to_string(),
        })?;
        Ok(Self { inner })
    }

    /// Feeds one model token into the parser.
    pub fn process_token(&mut self, token: TokenId) -> Result<(), GptOssHarmonyError> {
        self.inner
            .process(token.0)
            .map(|_| ())
            .map_err(|error| GptOssHarmonyError::Parse {
                message: error.to_string(),
            })
    }

    /// Flushes EOS through the parser.
    pub fn process_eos(&mut self) -> Result<(), GptOssHarmonyError> {
        self.inner
            .process_eos()
            .map(|_| ())
            .map_err(|error| GptOssHarmonyError::Parse {
                message: error.to_string(),
            })
    }

    /// Returns the currently buffered content fragment.
    pub fn current_content(&self) -> Result<String, GptOssHarmonyError> {
        self.inner
            .current_content()
            .map_err(|error| GptOssHarmonyError::Parse {
                message: error.to_string(),
            })
    }

    /// Returns the current role when one is known.
    #[must_use]
    pub fn current_role(&self) -> Option<PromptMessageRole> {
        self.inner.current_role().map(prompt_role_from_harmony_role)
    }

    /// Returns the current content type when one is known.
    #[must_use]
    pub fn current_content_type(&self) -> Option<String> {
        self.inner.current_content_type()
    }

    /// Returns the most recent decoded content delta when one is available.
    pub fn last_content_delta(&self) -> Result<Option<String>, GptOssHarmonyError> {
        self.inner
            .last_content_delta()
            .map_err(|error| GptOssHarmonyError::Parse {
                message: error.to_string(),
            })
    }

    /// Returns all fully parsed messages so far.
    #[must_use]
    pub fn messages(&self) -> Vec<PromptMessage> {
        self.inner
            .messages()
            .iter()
            .map(prompt_message_from_harmony_message)
            .collect()
    }
}

/// Renders prompt messages into the GPT-OSS Harmony completion prompt format.
pub fn render_gpt_oss_harmony_prompt(
    messages: &[PromptMessage],
    add_generation_prompt: bool,
    options: Option<&PromptRenderOptions>,
) -> Result<String, GptOssHarmonyError> {
    let encoding = load_gpt_oss_harmony_encoding()?;
    let conversation = build_gpt_oss_harmony_conversation(
        messages,
        options.and_then(|value| value.gpt_oss_harmony.as_ref()),
    )?;
    let tokens = if add_generation_prompt {
        encoding
            .render_conversation_for_completion(&conversation, HarmonyRole::Assistant, None)
            .map_err(|error| GptOssHarmonyError::Render {
                message: error.to_string(),
            })?
    } else {
        encoding
            .render_conversation(&conversation, None)
            .map_err(|error| GptOssHarmonyError::Render {
                message: error.to_string(),
            })?
    };
    encoding
        .tokenizer()
        .decode_utf8(&tokens)
        .map_err(|error| GptOssHarmonyError::Render {
            message: error.to_string(),
        })
}

/// Parses GPT-OSS / Harmony output from token IDs.
pub fn parse_gpt_oss_harmony_tokens(
    tokens: &[TokenId],
    options: GptOssHarmonyParseOptions,
) -> Result<GptOssHarmonyParsedOutput, GptOssHarmonyError> {
    let encoding = load_gpt_oss_harmony_encoding()?;
    let role_hint = harmony_role_hint(options.role_hint)?;
    let parsed = encoding
        .parse_messages_from_completion_tokens_with_options(
            tokens.iter().map(|token| token.0),
            role_hint,
            HarmonyParseOptions {
                strict: options.strict,
            },
        )
        .map_err(|error| GptOssHarmonyError::Parse {
            message: error.to_string(),
        })?;
    Ok(GptOssHarmonyParsedOutput {
        source: GptOssHarmonyParseSource::Tokens,
        messages: parsed
            .iter()
            .map(prompt_message_from_harmony_message)
            .collect(),
    })
}

/// Parses GPT-OSS / Harmony output from raw text.
pub fn parse_gpt_oss_harmony_text(
    text: &str,
    options: GptOssHarmonyParseOptions,
) -> Result<GptOssHarmonyParsedOutput, GptOssHarmonyError> {
    let encoding = load_gpt_oss_harmony_encoding()?;
    let tokens = encoding.tokenizer().encode_with_special_tokens(text);
    let role_hint = harmony_role_hint(options.role_hint)?;
    let parsed = encoding
        .parse_messages_from_completion_tokens_with_options(
            tokens,
            role_hint,
            HarmonyParseOptions {
                strict: options.strict,
            },
        )
        .map_err(|error| GptOssHarmonyError::Parse {
            message: error.to_string(),
        })?;
    Ok(GptOssHarmonyParsedOutput {
        source: GptOssHarmonyParseSource::Text,
        messages: parsed
            .iter()
            .map(prompt_message_from_harmony_message)
            .collect(),
    })
}

fn load_gpt_oss_harmony_encoding() -> Result<openai_harmony::HarmonyEncoding, GptOssHarmonyError> {
    load_harmony_encoding(HarmonyEncodingName::HarmonyGptOss).map_err(|error| {
        GptOssHarmonyError::LoadEncoding {
            message: error.to_string(),
        }
    })
}

/// Runtime tokenizer for GPT-OSS models backed by the published Harmony encoding.
#[derive(Clone)]
pub struct GptOssTokenizer {
    encoding: openai_harmony::HarmonyEncoding,
    vocabulary: TokenVocabulary,
    add_bos: bool,
    add_eos: bool,
    eos_token_ids: Vec<TokenId>,
}

impl std::fmt::Debug for GptOssTokenizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GptOssTokenizer")
            .field("encoding", &self.encoding.name())
            .field("vocabulary_len", &self.vocabulary.len())
            .field("add_bos", &self.add_bos)
            .field("add_eos", &self.add_eos)
            .field("eos_token_ids", &self.eos_token_ids)
            .finish()
    }
}

impl GptOssTokenizer {
    /// Builds the GPT-OSS runtime tokenizer from GGUF tokenizer metadata.
    pub fn from_gguf(tokenizer: &GgufTokenizerMetadata) -> Result<Self, GptOssHarmonyError> {
        if tokenizer.model != GgufTokenizerModel::Gpt2Bpe {
            return Err(GptOssHarmonyError::InvalidConversation {
                message: format!(
                    "gpt-oss harmony tokenizer requires gguf gpt2 metadata, found {:?}",
                    tokenizer.model
                ),
            });
        }

        let bos_id = tokenizer
            .vocabulary
            .bos_token_id()
            .or_else(|| tokenizer.vocabulary.pad_token_id())
            .or_else(|| tokenizer.vocabulary.unknown_token_id())
            .unwrap_or(TokenId(0));
        let eos_id = tokenizer
            .vocabulary
            .eos_token_ids()
            .first()
            .copied()
            .or_else(|| tokenizer.vocabulary.pad_token_id())
            .or_else(|| tokenizer.vocabulary.bos_token_id())
            .unwrap_or(TokenId(0));
        let pad_id = tokenizer
            .vocabulary
            .pad_token_id()
            .or_else(|| tokenizer.vocabulary.bos_token_id())
            .or_else(|| tokenizer.vocabulary.unknown_token_id())
            .unwrap_or(eos_id);
        let unknown_id = tokenizer
            .vocabulary
            .unknown_token_id()
            .or_else(|| tokenizer.vocabulary.pad_token_id())
            .or_else(|| tokenizer.vocabulary.bos_token_id())
            .unwrap_or(eos_id);

        Ok(Self {
            encoding: load_gpt_oss_harmony_encoding()?,
            vocabulary: TokenVocabulary::new(
                tokenizer.vocabulary.tokens().to_vec(),
                pad_id,
                bos_id,
                eos_id,
                unknown_id,
            ),
            add_bos: tokenizer.add_bos,
            add_eos: tokenizer.add_eos,
            eos_token_ids: tokenizer.vocabulary.eos_token_ids().to_vec(),
        })
    }

    /// Encodes text and optionally prepends/appends GGUF-declared BOS/EOS tokens.
    #[must_use]
    pub fn encode_with_special_tokens(
        &self,
        text: &str,
        add_bos: bool,
        add_eos: bool,
    ) -> TokenSequence {
        let mut tokens = Vec::new();
        if add_bos {
            tokens.push(self.vocabulary.bos_id());
        }
        tokens.extend(
            self.encoding
                .tokenizer()
                .encode_with_special_tokens(text)
                .into_iter()
                .map(TokenId),
        );
        if add_eos {
            tokens.push(self.vocabulary.eos_id());
        }
        TokenSequence::new(tokens)
    }

    /// Encodes text using the GGUF tokenizer defaults.
    #[must_use]
    pub fn encode_with_defaults(&self, text: &str) -> TokenSequence {
        self.encode_with_special_tokens(text, self.add_bos, self.add_eos)
    }

    /// Returns whether the token is one of the declared EOS IDs.
    #[must_use]
    pub fn is_end_of_sequence(&self, token: TokenId) -> bool {
        self.eos_token_ids.contains(&token) || token == self.vocabulary.eos_id()
    }
}

impl TokenizerBoundary for GptOssTokenizer {
    fn encode(&self, text: &str) -> TokenSequence {
        self.encode_with_special_tokens(text, false, false)
    }

    fn decode(&self, tokens: &[TokenId]) -> String {
        self.encoding
            .tokenizer()
            .decode_utf8(tokens.iter().map(|token| token.as_u32()))
            .unwrap_or_else(|_| {
                tokens
                    .iter()
                    .filter_map(|token| self.vocabulary.token(*token))
                    .collect::<Vec<_>>()
                    .join("")
            })
    }

    fn vocabulary(&self) -> &TokenVocabulary {
        &self.vocabulary
    }
}

fn build_gpt_oss_harmony_conversation(
    messages: &[PromptMessage],
    context: Option<&GptOssHarmonyRenderContext>,
) -> Result<HarmonyConversation, GptOssHarmonyError> {
    let mut conversation_messages = Vec::new();
    let mut system = HarmonySystemContent::new();
    let mut developer = HarmonyDeveloperContent::new();
    let mut developer_sections = Vec::new();
    let mut index = 0;

    if let Some(context) = context {
        if let Some(model_identity) = &context.model_identity {
            system = system.with_model_identity(model_identity.clone());
        }
        if let Some(reasoning_effort) = context.reasoning_effort {
            system = system.with_reasoning_effort(harmony_reasoning_effort(reasoning_effort));
        }
        if let Some(conversation_start_date) = &context.conversation_start_date {
            system = system.with_conversation_start_date(conversation_start_date.clone());
        }
        if let Some(knowledge_cutoff) = &context.knowledge_cutoff {
            system = system.with_knowledge_cutoff(knowledge_cutoff.clone());
        }
        if let Some(channel_config) = &context.channel_config {
            system = system.with_channel_config(HarmonyChannelConfig {
                valid_channels: channel_config.valid_channels.clone(),
                channel_required: channel_config.channel_required,
            });
        }
        for namespace in &context.tool_namespaces {
            let namespace_config = prompt_tool_namespace_to_harmony(namespace);
            if matches!(namespace.name.as_str(), "browser" | "python") {
                system = system.with_tools(namespace_config);
            } else {
                developer = developer.with_tools(namespace_config);
            }
        }
    }

    while index < messages.len() {
        match messages[index].role {
            PromptMessageRole::System | PromptMessageRole::Developer => {
                developer_sections.push(messages[index].content.clone());
                index += 1;
            }
            _ => break,
        }
    }

    if messages[index..].iter().any(|message| {
        matches!(
            message.role,
            PromptMessageRole::System | PromptMessageRole::Developer
        )
    }) {
        return Err(GptOssHarmonyError::InvalidConversation {
            message: String::from(
                "gpt-oss harmony only accepts system/developer instruction messages before user/assistant/tool turns",
            ),
        });
    }

    conversation_messages.push(HarmonyMessage::from_role_and_content(
        HarmonyRole::System,
        system,
    ));

    if !developer_sections.is_empty() {
        developer = developer.with_instructions(developer_sections.join("\n\n"));
    }
    if developer.instructions.is_some()
        || developer
            .tools
            .as_ref()
            .is_some_and(|value| !value.is_empty())
    {
        conversation_messages.push(HarmonyMessage::from_role_and_content(
            HarmonyRole::Developer,
            developer,
        ));
    }

    for (offset, message) in messages[index..].iter().enumerate() {
        append_prompt_message_as_harmony_messages(
            &mut conversation_messages,
            message,
            &messages[index + offset + 1..],
        )?;
    }

    Ok(HarmonyConversation::from_messages(conversation_messages))
}

fn append_prompt_message_as_harmony_messages(
    out: &mut Vec<HarmonyMessage>,
    message: &PromptMessage,
    future_messages: &[PromptMessage],
) -> Result<(), GptOssHarmonyError> {
    if message.reasoning_content.is_some() && !matches!(message.role, PromptMessageRole::Assistant)
    {
        return Err(GptOssHarmonyError::InvalidConversation {
            message: String::from(
                "reasoning_content is only valid for assistant messages in gpt-oss harmony prompts",
            ),
        });
    }

    match message.role {
        PromptMessageRole::User => out.push(prompt_message_to_harmony_text_message(
            HarmonyRole::User,
            None,
            Some(message),
            &message.content,
        )),
        PromptMessageRole::Assistant => {
            let future_final_exists = future_messages
                .iter()
                .any(is_future_final_assistant_message);

            if let Some(reasoning_content) = &message.reasoning_content {
                out.push(prompt_message_to_harmony_text_message(
                    HarmonyRole::Assistant,
                    Some("analysis"),
                    Some(message),
                    reasoning_content,
                ));
            }

            if message.recipient.is_some() {
                let mut tool_call = prompt_message_to_harmony_text_message(
                    HarmonyRole::Assistant,
                    Some(message.channel.as_deref().unwrap_or("commentary")),
                    Some(message),
                    &message.content,
                );
                if tool_call.content_type.is_none() {
                    tool_call.content_type = Some(String::from("<|constrain|>json"));
                }
                if future_final_exists && message.reasoning_content.is_some() {
                    let _ = out.pop();
                }
                out.push(tool_call);
            } else {
                let assistant_channel = message.channel.as_deref().unwrap_or("final");
                out.push(prompt_message_to_harmony_text_message(
                    HarmonyRole::Assistant,
                    Some(assistant_channel),
                    Some(message),
                    &message.content,
                ));
            }
        }
        PromptMessageRole::Tool => {
            let author_name = message.author_name.as_deref().ok_or_else(|| {
                GptOssHarmonyError::InvalidConversation {
                    message: String::from(
                        "tool messages require author_name so harmony can render the tool identity",
                    ),
                }
            })?;
            let mut tool_message = prompt_message_to_harmony_text_message(
                HarmonyRole::Tool,
                Some(message.channel.as_deref().unwrap_or("commentary")),
                Some(message),
                &message.content,
            );
            tool_message.author = HarmonyAuthor::new(HarmonyRole::Tool, author_name.to_string());
            if tool_message.recipient.is_none() {
                tool_message.recipient = Some(String::from("assistant"));
            }
            out.push(tool_message);
        }
        PromptMessageRole::System | PromptMessageRole::Developer => {
            return Err(GptOssHarmonyError::InvalidConversation {
                message: String::from(
                    "gpt-oss harmony instruction messages must appear before user/assistant/tool turns",
                ),
            });
        }
    }

    Ok(())
}

fn prompt_message_to_harmony_text_message(
    role: HarmonyRole,
    default_channel: Option<&str>,
    message: Option<&PromptMessage>,
    content: &str,
) -> HarmonyMessage {
    let author = match message.and_then(|value| value.author_name.as_ref()) {
        Some(name) => HarmonyAuthor::new(role.clone(), name.clone()),
        None => role.clone().into(),
    };
    let mut rendered = HarmonyMessage::from_author_and_content(author, content.to_string());
    if let Some(message) = message {
        rendered.recipient.clone_from(&message.recipient);
        rendered.channel = message
            .channel
            .clone()
            .or_else(|| default_channel.map(str::to_string));
        rendered.content_type.clone_from(&message.content_type);
    } else {
        rendered.channel = default_channel.map(str::to_string);
    }
    rendered
}

fn prompt_tool_namespace_to_harmony(namespace: &PromptToolNamespace) -> HarmonyToolNamespaceConfig {
    HarmonyToolNamespaceConfig::new(
        namespace.name.clone(),
        namespace.description.clone(),
        namespace
            .tools
            .iter()
            .map(|tool| {
                HarmonyToolDescription::new(
                    tool.name.clone(),
                    tool.description.clone(),
                    tool.parameters.clone(),
                )
            })
            .collect(),
    )
}

fn harmony_reasoning_effort(value: PromptReasoningEffort) -> HarmonyReasoningEffort {
    match value {
        PromptReasoningEffort::Low => HarmonyReasoningEffort::Low,
        PromptReasoningEffort::Medium => HarmonyReasoningEffort::Medium,
        PromptReasoningEffort::High => HarmonyReasoningEffort::High,
    }
}

fn harmony_role_hint(
    role: Option<PromptMessageRole>,
) -> Result<Option<HarmonyRole>, GptOssHarmonyError> {
    role.map(harmony_role_from_prompt_role).transpose()
}

fn harmony_role_from_prompt_role(
    role: PromptMessageRole,
) -> Result<HarmonyRole, GptOssHarmonyError> {
    Ok(match role {
        PromptMessageRole::System => HarmonyRole::System,
        PromptMessageRole::Developer => HarmonyRole::Developer,
        PromptMessageRole::User => HarmonyRole::User,
        PromptMessageRole::Assistant => HarmonyRole::Assistant,
        PromptMessageRole::Tool => HarmonyRole::Tool,
    })
}

fn prompt_role_from_harmony_role(role: HarmonyRole) -> PromptMessageRole {
    match role {
        HarmonyRole::System => PromptMessageRole::System,
        HarmonyRole::Developer => PromptMessageRole::Developer,
        HarmonyRole::User => PromptMessageRole::User,
        HarmonyRole::Assistant => PromptMessageRole::Assistant,
        HarmonyRole::Tool => PromptMessageRole::Tool,
    }
}

fn is_future_final_assistant_message(message: &PromptMessage) -> bool {
    message.role == PromptMessageRole::Assistant
        && message.recipient.is_none()
        && !matches!(message.channel.as_deref(), Some("analysis"))
}

fn prompt_message_from_harmony_message(message: &HarmonyMessage) -> PromptMessage {
    let mut output = PromptMessage::new(
        prompt_role_from_harmony_role(message.author.role.clone()),
        harmony_message_text(message),
    );
    if let Some(author_name) = &message.author.name {
        output = output.with_author_name(author_name.clone());
    }
    if let Some(recipient) = &message.recipient {
        output = output.with_recipient(recipient.clone());
    }
    if let Some(channel) = &message.channel {
        output = output.with_channel(channel.clone());
    }
    if let Some(content_type) = &message.content_type {
        output = output.with_content_type(content_type.clone());
    }
    output
}

fn harmony_message_text(message: &HarmonyMessage) -> String {
    message
        .content
        .iter()
        .fold(String::new(), |mut output, content| {
            let segment = match content {
                openai_harmony::chat::Content::Text(text) => text.text.clone(),
                openai_harmony::chat::Content::SystemContent(system) => {
                    let mut sections = Vec::new();
                    if let Some(model_identity) = &system.model_identity {
                        sections.push(model_identity.clone());
                    }
                    if let Some(knowledge_cutoff) = &system.knowledge_cutoff {
                        sections.push(format!("Knowledge cutoff: {knowledge_cutoff}"));
                    }
                    if let Some(current_date) = &system.conversation_start_date {
                        sections.push(format!("Current date: {current_date}"));
                    }
                    sections.join("\n")
                }
                openai_harmony::chat::Content::DeveloperContent(developer) => {
                    developer.instructions.clone().unwrap_or_default()
                }
            };
            output.push_str(&segment);
            output
        })
}
