use std::{collections::HashMap, ops::Range};

use fancy_regex::Regex;
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
    GgufDecoderFamily, GgufTokenizerMetadata, GgufTokenizerModel, GgufTokenizerPretokenizer,
    PromptMessage, PromptMessageRole, TokenId, TokenSequence, TokenVocabulary, TokenizerBoundary,
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

/// Family-specific reasoning parser contract surfaced by Psionic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningParser {
    /// GPT-OSS / Harmony parser contract.
    GptOssHarmony,
}

impl ReasoningParser {
    /// Returns the stable parser label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::GptOssHarmony => "gpt_oss_harmony",
        }
    }
}

/// Source lane used to recover a typed reasoning-bearing response.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningParseSource {
    /// Structured output was parsed from token IDs.
    Tokens,
    /// Structured output was parsed from raw text.
    Text,
}

impl From<GptOssHarmonyParseSource> for ReasoningParseSource {
    fn from(value: GptOssHarmonyParseSource) -> Self {
        match value {
            GptOssHarmonyParseSource::Tokens => Self::Tokens,
            GptOssHarmonyParseSource::Text => Self::Text,
        }
    }
}

/// Typed lane for one parsed output part.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningResponsePartKind {
    /// Public final answer content.
    Final,
    /// Model reasoning or analysis content.
    Reasoning,
    /// Non-final assistant side-channel content.
    SideChannel,
    /// Assistant-emitted tool call content.
    ToolCall,
    /// Tool-result content.
    ToolResult,
    /// Any other parsed content that does not fit the current buckets.
    Other,
}

/// One typed parsed output part.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasoningResponsePart {
    /// Part classification derived from family parser semantics.
    pub kind: ReasoningResponsePartKind,
    /// Message role.
    pub role: PromptMessageRole,
    /// Part content.
    pub content: String,
    /// Author name when the role carries one, such as a named tool result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_name: Option<String>,
    /// Explicit recipient when the part targets a specific tool or assistant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipient: Option<String>,
    /// Explicit channel when the part carries one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    /// Explicit content-type suffix when the part carries one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
}

/// Typed reasoning-bearing response recovered by a family parser.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedReasoningResponse {
    /// Parser that produced this response envelope.
    pub parser: ReasoningParser,
    /// Source lane used during parsing.
    pub source: ReasoningParseSource,
    /// Final public answer content when the parser can recover it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_content: Option<String>,
    /// Reasoning content when the parser can recover it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    /// Typed parsed parts in source order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<ReasoningResponsePart>,
}

impl ParsedReasoningResponse {
    /// Returns a copy with reasoning parts removed.
    #[must_use]
    pub fn suppress_reasoning(&self) -> Self {
        let mut filtered = self.clone();
        filtered.reasoning_content = None;
        filtered
            .parts
            .retain(|part| part.kind != ReasoningResponsePartKind::Reasoning);
        filtered
    }
}

impl GptOssHarmonyParsedOutput {
    /// Builds the typed reasoning-bearing response envelope for Harmony output.
    #[must_use]
    pub fn reasoning_response(&self) -> ParsedReasoningResponse {
        let parts = self
            .messages
            .iter()
            .map(reasoning_response_part_from_prompt_message)
            .collect::<Vec<_>>();
        ParsedReasoningResponse {
            parser: ReasoningParser::GptOssHarmony,
            source: self.source.into(),
            final_content: join_response_parts(&parts, ReasoningResponsePartKind::Final),
            reasoning_content: join_response_parts(&parts, ReasoningResponsePartKind::Reasoning),
            parts,
        }
    }
}

/// Returns the reasoning parser contract for one decoder family when present.
#[must_use]
pub fn reasoning_parser_for_decoder_family(family: GgufDecoderFamily) -> Option<ReasoningParser> {
    match family {
        GgufDecoderFamily::GptOss => Some(ReasoningParser::GptOssHarmony),
        GgufDecoderFamily::Llama | GgufDecoderFamily::Qwen | GgufDecoderFamily::Mistral => None,
    }
}

/// Parses typed reasoning-bearing output from decoder-family token IDs.
pub fn parse_reasoning_response_tokens_for_decoder_family(
    family: GgufDecoderFamily,
    tokens: &[TokenId],
    options: GptOssHarmonyParseOptions,
) -> Result<Option<ParsedReasoningResponse>, GptOssHarmonyError> {
    match reasoning_parser_for_decoder_family(family) {
        Some(ReasoningParser::GptOssHarmony) => Ok(Some(
            parse_gpt_oss_harmony_tokens(tokens, options)?.reasoning_response(),
        )),
        None => Ok(None),
    }
}

/// Parses typed reasoning-bearing output from decoder-family raw text.
pub fn parse_reasoning_response_text_for_decoder_family(
    family: GgufDecoderFamily,
    text: &str,
    options: GptOssHarmonyParseOptions,
) -> Result<Option<ParsedReasoningResponse>, GptOssHarmonyError> {
    match reasoning_parser_for_decoder_family(family) {
        Some(ReasoningParser::GptOssHarmony) => Ok(Some(
            parse_gpt_oss_harmony_text(text, options)?.reasoning_response(),
        )),
        None => Ok(None),
    }
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
    /// Building or using the GGUF-backed GPT-OSS tokenizer failed.
    #[error("failed to build gpt-oss tokenizer: {message}")]
    Tokenizer {
        /// Lower-level failure summary.
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

fn reasoning_response_part_from_prompt_message(message: &PromptMessage) -> ReasoningResponsePart {
    ReasoningResponsePart {
        kind: reasoning_response_part_kind(message),
        role: message.role,
        content: message.content.clone(),
        author_name: message.author_name.clone(),
        recipient: message.recipient.clone(),
        channel: message.channel.clone(),
        content_type: message.content_type.clone(),
    }
}

fn reasoning_response_part_kind(message: &PromptMessage) -> ReasoningResponsePartKind {
    if message.recipient.is_some() {
        return ReasoningResponsePartKind::ToolCall;
    }
    if message.role == PromptMessageRole::Tool {
        return ReasoningResponsePartKind::ToolResult;
    }
    if message.role != PromptMessageRole::Assistant {
        return ReasoningResponsePartKind::Other;
    }
    match message.channel.as_deref() {
        Some("analysis") => ReasoningResponsePartKind::Reasoning,
        Some("final") | None => ReasoningResponsePartKind::Final,
        Some(_) => ReasoningResponsePartKind::SideChannel,
    }
}

fn join_response_parts(
    parts: &[ReasoningResponsePart],
    kind: ReasoningResponsePartKind,
) -> Option<String> {
    let joined = parts
        .iter()
        .filter(|part| part.kind == kind)
        .map(|part| part.content.as_str())
        .collect::<String>();
    (!joined.is_empty()).then_some(joined)
}

fn load_gpt_oss_harmony_encoding() -> Result<openai_harmony::HarmonyEncoding, GptOssHarmonyError> {
    load_harmony_encoding(HarmonyEncodingName::HarmonyGptOss).map_err(|error| {
        GptOssHarmonyError::LoadEncoding {
            message: error.to_string(),
        }
    })
}

const GPT_4O_BPE_PATTERN: &str = concat!(
    "[^\\r\\n\\p{L}\\p{N}]?[\\p{Lu}\\p{Lt}\\p{Lm}\\p{Lo}\\p{M}]*",
    "[\\p{Ll}\\p{Lm}\\p{Lo}\\p{M}]+(?i:'s|'t|'re|'ve|'m|'ll|'d)?|",
    "[^\\r\\n\\p{L}\\p{N}]?[\\p{Lu}\\p{Lt}\\p{Lm}\\p{Lo}\\p{M}]+",
    "[\\p{Ll}\\p{Lm}\\p{Lo}\\p{M}]*(?i:'s|'t|'re|'ve|'m|'ll|'d)?|",
    "\\p{N}{1,3}|",
    " ?[^\\s\\p{L}\\p{N}]+[\\r\\n/]*|",
    "\\s*[\\r\\n]+|",
    "\\s+(?!\\S)|",
    "\\s+"
);

const LLAMA_TOKEN_TYPE_UNKNOWN: i32 = 2;
const LLAMA_TOKEN_TYPE_CONTROL: i32 = 3;
const LLAMA_TOKEN_TYPE_USER_DEFINED: i32 = 4;
const LLAMA_TOKEN_TYPE_UNUSED: i32 = 5;

#[derive(Clone, Debug)]
struct GgufByteLevelBpeTokenizer {
    ordinary_encoder: HashMap<Vec<u8>, u32>,
    ordinary_decoder: HashMap<u32, Vec<u8>>,
    special_encoder: HashMap<String, u32>,
    special_decoder: HashMap<u32, String>,
    ordinary_regex: Regex,
    special_regex: Option<Regex>,
}

impl GgufByteLevelBpeTokenizer {
    fn from_gguf(tokenizer: &GgufTokenizerMetadata) -> Result<Self, GptOssHarmonyError> {
        let pattern = gpt_oss_tokenizer_pattern(tokenizer)?;
        let ordinary_regex =
            Regex::new(pattern).map_err(|error| GptOssHarmonyError::Tokenizer {
                message: format!("failed to compile tokenizer regex: {error}"),
            })?;
        let unicode_to_byte = gpt_unicode_to_byte_map();
        let mut ordinary_encoder = HashMap::new();
        let mut ordinary_decoder = HashMap::new();
        let mut special_encoder = HashMap::new();
        let mut special_decoder = HashMap::new();

        for (index, token) in tokenizer.vocabulary.tokens().iter().enumerate() {
            let token_id = index as u32;
            let token_type = tokenizer.token_types.get(index).copied();
            if gguf_token_is_special(token, token_type) {
                if special_encoder.insert(token.clone(), token_id).is_some() {
                    return Err(GptOssHarmonyError::Tokenizer {
                        message: format!("duplicate special token `{token}` in GGUF tokenizer"),
                    });
                }
                special_decoder.insert(token_id, token.clone());
                continue;
            }

            let raw_bytes = gguf_token_to_raw_bytes(token, &unicode_to_byte)?;
            if ordinary_encoder
                .insert(raw_bytes.clone(), token_id)
                .is_some()
            {
                return Err(GptOssHarmonyError::Tokenizer {
                    message: format!(
                        "duplicate ordinary token bytes for GGUF token id {token_id} (`{token}`)"
                    ),
                });
            }
            ordinary_decoder.insert(token_id, raw_bytes);
        }

        let special_regex = build_special_regex(&special_encoder)?;
        Ok(Self {
            ordinary_encoder,
            ordinary_decoder,
            special_encoder,
            special_decoder,
            ordinary_regex,
            special_regex,
        })
    }

    fn encode_with_special_tokens(&self, text: &str) -> Vec<u32> {
        let mut tokens = Vec::new();
        let mut start = 0;
        while start < text.len() {
            let next_special = self.find_next_special(text, start);
            let end = next_special.map_or(text.len(), |(match_start, _, _)| match_start);
            self.encode_ordinary_segment(&text[start..end], &mut tokens);
            match next_special {
                Some((_, match_end, token_id)) => {
                    tokens.push(token_id);
                    start = match_end;
                }
                None => break,
            }
        }
        tokens
    }

    fn decode_utf8(&self, tokens: &[TokenId]) -> Option<String> {
        let mut bytes = Vec::new();
        for token in tokens {
            let token_id = token.as_u32();
            if let Some(raw_bytes) = self.ordinary_decoder.get(&token_id) {
                bytes.extend_from_slice(raw_bytes);
                continue;
            }
            if let Some(special) = self.special_decoder.get(&token_id) {
                bytes.extend_from_slice(special.as_bytes());
                continue;
            }
            return None;
        }
        String::from_utf8(bytes).ok()
    }

    fn find_next_special(&self, text: &str, start: usize) -> Option<(usize, usize, u32)> {
        let regex = self.special_regex.as_ref()?;
        let matched = regex.find_from_pos(text, start).ok().flatten()?;
        let token = self
            .special_encoder
            .get(&text[matched.start()..matched.end()])?;
        Some((matched.start(), matched.end(), *token))
    }

    fn starts_with_special_token(&self, text: &str) -> bool {
        self.special_token_prefix_range(text)
            .map(|range| range.start == 0)
            .unwrap_or(false)
    }

    fn special_token_prefix_range(&self, text: &str) -> Option<Range<usize>> {
        self.find_next_special(text, 0)
            .and_then(|(start, end, _)| (start == 0).then_some(start..end))
    }

    fn encode_ordinary_segment(&self, text: &str, out: &mut Vec<u32>) {
        if text.is_empty() {
            return;
        }
        let matches = match self
            .ordinary_regex
            .find_iter(text)
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(matches) => matches,
            Err(_) => {
                out.extend(byte_pair_encode(text.as_bytes(), &self.ordinary_encoder));
                return;
            }
        };
        for matched in matches {
            let piece = matched.as_str().as_bytes();
            if let Some(token) = self.ordinary_encoder.get(piece) {
                out.push(*token);
                continue;
            }
            out.extend(byte_pair_encode(piece, &self.ordinary_encoder));
        }
    }
}

/// Runtime tokenizer for GPT-OSS models backed by the model's own GGUF tokenizer metadata.
#[derive(Clone)]
pub struct GptOssTokenizer {
    bpe: GgufByteLevelBpeTokenizer,
    vocabulary: TokenVocabulary,
    add_bos: bool,
    add_eos: bool,
    eos_token_ids: Vec<TokenId>,
}

impl std::fmt::Debug for GptOssTokenizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GptOssTokenizer")
            .field("tokenizer_contract", &"gguf_gpt_bpe")
            .field("vocabulary_len", &self.vocabulary.len())
            .field("add_bos", &self.add_bos)
            .field("add_eos", &self.add_eos)
            .field("eos_token_ids", &self.eos_token_ids)
            .finish_non_exhaustive()
    }
}

impl GptOssTokenizer {
    /// Builds the GPT-OSS runtime tokenizer from GGUF tokenizer metadata.
    pub fn from_gguf(tokenizer: &GgufTokenizerMetadata) -> Result<Self, GptOssHarmonyError> {
        if tokenizer.model != GgufTokenizerModel::Gpt2Bpe {
            return Err(GptOssHarmonyError::Tokenizer {
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
            bpe: GgufByteLevelBpeTokenizer::from_gguf(tokenizer)?,
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
            self.bpe
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
        let add_bos = self.add_bos && !self.bpe.starts_with_special_token(text);
        self.encode_with_special_tokens(text, add_bos, self.add_eos)
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
        self.bpe.decode_utf8(tokens).unwrap_or_else(|| {
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

fn gpt_oss_tokenizer_pattern(
    tokenizer: &GgufTokenizerMetadata,
) -> Result<&'static str, GptOssHarmonyError> {
    match tokenizer.pretokenizer.as_ref() {
        Some(GgufTokenizerPretokenizer::Custom(value)) if value == "gpt-4o" => {
            Ok(GPT_4O_BPE_PATTERN)
        }
        None => Ok(GPT_4O_BPE_PATTERN),
        Some(other) => Err(GptOssHarmonyError::Tokenizer {
            message: format!("unsupported gpt-oss GGUF pretokenizer `{other:?}`"),
        }),
    }
}

fn gguf_token_is_special(token: &str, token_type: Option<i32>) -> bool {
    matches!(
        token_type,
        Some(
            LLAMA_TOKEN_TYPE_UNKNOWN
                | LLAMA_TOKEN_TYPE_CONTROL
                | LLAMA_TOKEN_TYPE_USER_DEFINED
                | LLAMA_TOKEN_TYPE_UNUSED
        )
    ) || token.starts_with("<|") && token.ends_with("|>")
}

fn build_special_regex(
    special_encoder: &HashMap<String, u32>,
) -> Result<Option<Regex>, GptOssHarmonyError> {
    if special_encoder.is_empty() {
        return Ok(None);
    }
    let mut tokens = special_encoder
        .keys()
        .map(|token| fancy_regex::escape(token))
        .collect::<Vec<_>>();
    tokens.sort_by(|left, right| right.len().cmp(&left.len()).then_with(|| left.cmp(right)));
    Regex::new(&tokens.join("|"))
        .map(Some)
        .map_err(|error| GptOssHarmonyError::Tokenizer {
            message: format!("failed to compile special-token regex: {error}"),
        })
}

fn gpt_unicode_to_byte_map() -> HashMap<char, u8> {
    let mut mapping = HashMap::with_capacity(256);
    let mut assigned = [false; 256];
    for byte in 0x21_u32..=0x7e {
        let character = char::from_u32(byte).unwrap_or('\0');
        mapping.insert(character, byte as u8);
        assigned[byte as usize] = true;
    }
    for byte in 0xa1_u32..=0xac {
        let character = char::from_u32(byte).unwrap_or('\0');
        mapping.insert(character, byte as u8);
        assigned[byte as usize] = true;
    }
    for byte in 0xae_u32..=0xff {
        let character = char::from_u32(byte).unwrap_or('\0');
        mapping.insert(character, byte as u8);
        assigned[byte as usize] = true;
    }
    let mut next_codepoint = 256_u32;
    for (byte, is_assigned) in assigned.iter().enumerate() {
        if *is_assigned {
            continue;
        }
        let character = char::from_u32(next_codepoint).unwrap_or('\0');
        mapping.insert(character, byte as u8);
        next_codepoint += 1;
    }
    mapping
}

fn gguf_token_to_raw_bytes(
    token: &str,
    unicode_to_byte: &HashMap<char, u8>,
) -> Result<Vec<u8>, GptOssHarmonyError> {
    token
        .chars()
        .map(|character| {
            unicode_to_byte
                .get(&character)
                .copied()
                .ok_or_else(|| GptOssHarmonyError::Tokenizer {
                    message: format!(
                        "GGUF token contains non-byte-mapped character U+{:04X} in `{token}`",
                        character as u32
                    ),
                })
        })
        .collect()
}

fn byte_pair_encode(piece: &[u8], ranks: &HashMap<Vec<u8>, u32>) -> Vec<u32> {
    if piece.is_empty() {
        return Vec::new();
    }
    if piece.len() == 1 {
        return ranks
            .get(piece)
            .copied()
            .map_or_else(Vec::new, |rank| vec![rank]);
    }
    byte_pair_merge(piece, ranks)
        .windows(2)
        .flat_map(|part| {
            let segment = &piece[part[0].0..part[1].0];
            ranks
                .get(segment)
                .copied()
                .map_or_else(|| encode_bytes_as_tokens(segment, ranks), |rank| vec![rank])
        })
        .collect()
}

fn encode_bytes_as_tokens(bytes: &[u8], ranks: &HashMap<Vec<u8>, u32>) -> Vec<u32> {
    bytes
        .iter()
        .filter_map(|byte| ranks.get(&vec![*byte]).copied())
        .collect()
}

fn byte_pair_merge(piece: &[u8], ranks: &HashMap<Vec<u8>, u32>) -> Vec<(usize, u32)> {
    let mut parts = Vec::with_capacity(piece.len() + 1);
    let mut min_rank = (u32::MAX, usize::MAX);
    for index in 0..piece.len().saturating_sub(1) {
        let rank = *ranks.get(&piece[index..index + 2]).unwrap_or(&u32::MAX);
        if rank < min_rank.0 {
            min_rank = (rank, index);
        }
        parts.push((index, rank));
    }
    parts.push((piece.len().saturating_sub(1), u32::MAX));
    parts.push((piece.len(), u32::MAX));

    let get_rank = |parts: &Vec<(usize, u32)>, index: usize| {
        if index + 3 < parts.len() {
            *ranks
                .get(&piece[parts[index].0..parts[index + 3].0])
                .unwrap_or(&u32::MAX)
        } else {
            u32::MAX
        }
    };

    while min_rank.0 != u32::MAX {
        let index = min_rank.1;
        if index > 0 {
            parts[index - 1].1 = get_rank(&parts, index - 1);
        }
        parts[index].1 = get_rank(&parts, index);
        parts.remove(index + 1);

        min_rank = (u32::MAX, usize::MAX);
        for (scan_index, &(_, rank)) in parts[..parts.len().saturating_sub(1)].iter().enumerate() {
            if rank < min_rank.0 {
                min_rank = (rank, scan_index);
            }
        }
    }

    parts
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

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic_in_result_fn)]

    use std::path::PathBuf;

    use super::{
        GPT_4O_BPE_PATTERN, GptOssHarmonyRenderContext, GptOssTokenizer, LLAMA_TOKEN_TYPE_CONTROL,
        PromptChannelConfig, PromptMessage, PromptMessageRole, PromptReasoningEffort,
        PromptRenderOptions, gguf_token_to_raw_bytes, gpt_unicode_to_byte_map,
        render_gpt_oss_harmony_prompt,
    };
    use crate::{
        GgufContent, GgufTokenizerMetadata, GgufTokenizerModel, GgufTokenizerPretokenizer,
        GgufTokenizerVocabulary, TokenId, TokenizerBoundary, golden_tokenizer_fixture,
    };

    fn real_gpt_oss_gguf_path() -> Option<PathBuf> {
        let fixture = golden_tokenizer_fixture("gpt_oss_20b")?;
        [
            fixture.source_path,
            "/Users/christopherdavid/models/gpt-oss/gpt-oss-20b-mxfp4.gguf",
        ]
        .into_iter()
        .map(PathBuf::from)
        .find(|candidate| candidate.exists())
    }

    fn test_gpt_oss_tokenizer_metadata() -> GgufTokenizerMetadata {
        GgufTokenizerMetadata {
            model: GgufTokenizerModel::Gpt2Bpe,
            vocabulary: GgufTokenizerVocabulary {
                tokens: vec![
                    String::from("<|start|>"),
                    String::from("h"),
                    String::from("e"),
                    String::from("l"),
                    String::from("o"),
                    String::from("he"),
                    String::from("ll"),
                    String::from("hell"),
                    String::from("hello"),
                ],
                bos_token_id: Some(TokenId(0)),
                eos_token_ids: vec![TokenId(0)],
                pad_token_id: None,
                unknown_token_id: None,
            },
            scores: Vec::new(),
            token_types: vec![LLAMA_TOKEN_TYPE_CONTROL, 1, 1, 1, 1, 1, 1, 1, 1],
            merges: vec![
                String::from("h e"),
                String::from("l l"),
                String::from("he ll"),
                String::from("hell o"),
            ],
            add_bos: false,
            add_eos: false,
            pretokenizer: Some(GgufTokenizerPretokenizer::Custom(String::from("gpt-4o"))),
            token_type_count: None,
            digest: String::from("test-gpt-oss-tokenizer"),
        }
    }

    #[test]
    fn gpt_oss_tokenizer_encodes_special_tokens_and_merged_words() {
        let tokenizer =
            GptOssTokenizer::from_gguf(&test_gpt_oss_tokenizer_metadata()).expect("tokenizer");
        let encoded = tokenizer.encode("<|start|>hello");

        assert_eq!(encoded.as_slice(), &[TokenId(0), TokenId(8)]);
        assert_eq!(tokenizer.decode(encoded.as_slice()), "<|start|>hello");
    }

    #[test]
    fn gpt_oss_tokenizer_maps_gguf_tokens_back_to_raw_bytes() {
        let unicode_to_byte = gpt_unicode_to_byte_map();
        let raw = gguf_token_to_raw_bytes("hello", &unicode_to_byte).expect("raw bytes");

        assert_eq!(raw, b"hello");
        assert!(GPT_4O_BPE_PATTERN.contains("\\p{N}{1,3}"));
    }

    #[test]
    fn gpt_oss_real_gguf_prompt_token_count_matches_tracked_local_oracle()
    -> Result<(), Box<dyn std::error::Error>> {
        let Some(path) = real_gpt_oss_gguf_path() else {
            return Ok(());
        };

        let content = GgufContent::read_path(&path)?;
        let metadata = content.load_tokenizer()?;
        let tokenizer = GptOssTokenizer::from_gguf(&metadata)?;
        let messages = vec![
            PromptMessage::new(
                PromptMessageRole::Developer,
                "Be concise. Output exactly one sentence.",
            ),
            PromptMessage::new(
                PromptMessageRole::User,
                "Reply with exactly this sentence and nothing else: HTTPS protects users by encrypting traffic, preventing tampering, and confirming they are connected to the right website.",
            ),
        ];
        let prompt_options = PromptRenderOptions {
            gpt_oss_harmony: Some(GptOssHarmonyRenderContext {
                reasoning_effort: Some(PromptReasoningEffort::Low),
                channel_config: Some(PromptChannelConfig::default()),
                ..Default::default()
            }),
        };

        let rendered =
            render_gpt_oss_harmony_prompt(messages.as_slice(), true, Some(&prompt_options))?;
        let tokens = tokenizer.encode_with_defaults(rendered.as_str());

        assert_eq!(tokens.len(), 103);
        Ok(())
    }

    #[test]
    fn gpt_oss_real_short_contract_prompt_tokens_match_local_llama_cpp_oracle()
    -> Result<(), Box<dyn std::error::Error>> {
        let Some(path) = real_gpt_oss_gguf_path() else {
            return Ok(());
        };

        let content = GgufContent::read_path(&path)?;
        let metadata = content.load_tokenizer()?;
        let tokenizer = GptOssTokenizer::from_gguf(&metadata)?;
        let messages = vec![PromptMessage::new(
            PromptMessageRole::User,
            "What is 2 + 2?",
        )];
        let prompt_options = PromptRenderOptions {
            gpt_oss_harmony: Some(GptOssHarmonyRenderContext {
                reasoning_effort: Some(PromptReasoningEffort::Low),
                channel_config: Some(PromptChannelConfig::default()),
                ..Default::default()
            }),
        };

        let rendered =
            render_gpt_oss_harmony_prompt(messages.as_slice(), true, Some(&prompt_options))?;
        assert_eq!(
            rendered,
            concat!(
                "<|start|>system<|message|>",
                "You are ChatGPT, a large language model trained by OpenAI.\n",
                "Knowledge cutoff: 2024-06\n\n",
                "Reasoning: low\n\n",
                "# Valid channels: analysis, commentary, final. Channel must be included for every message.",
                "<|end|>",
                "<|start|>user<|message|>",
                "What is 2 + 2?",
                "<|end|>",
                "<|start|>assistant",
            )
        );

        let tokens = tokenizer.encode_with_defaults(rendered.as_str());
        assert_eq!(
            tokens.as_slice(),
            &[
                TokenId(200006),
                TokenId(17360),
                TokenId(200008),
                TokenId(3575),
                TokenId(553),
                TokenId(17554),
                TokenId(162016),
                TokenId(11),
                TokenId(261),
                TokenId(4410),
                TokenId(6439),
                TokenId(2359),
                TokenId(22203),
                TokenId(656),
                TokenId(7788),
                TokenId(17527),
                TokenId(558),
                TokenId(87447),
                TokenId(100594),
                TokenId(25),
                TokenId(220),
                TokenId(1323),
                TokenId(19),
                TokenId(12),
                TokenId(3218),
                TokenId(279),
                TokenId(30377),
                TokenId(289),
                TokenId(25),
                TokenId(4465),
                TokenId(279),
                TokenId(2),
                TokenId(13888),
                TokenId(18403),
                TokenId(25),
                TokenId(8450),
                TokenId(11),
                TokenId(49159),
                TokenId(11),
                TokenId(1721),
                TokenId(13),
                TokenId(21030),
                TokenId(2804),
                TokenId(413),
                TokenId(7360),
                TokenId(395),
                TokenId(1753),
                TokenId(3176),
                TokenId(13),
                TokenId(200007),
                TokenId(200006),
                TokenId(1428),
                TokenId(200008),
                TokenId(4827),
                TokenId(382),
                TokenId(220),
                TokenId(17),
                TokenId(659),
                TokenId(220),
                TokenId(17),
                TokenId(30),
                TokenId(200007),
                TokenId(200006),
                TokenId(173781),
            ]
        );
        Ok(())
    }
}
