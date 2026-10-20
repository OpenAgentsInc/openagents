use std::collections::HashMap;
use std::sync::Arc;

use deepseek_recipe_core::special_tokens::{SpecialTokens, default_special_tokens};

use super::ConversionError;

/// The treatment of supported `web_search` declarations and tool choices by an
/// adapter. Messages also applies this setting to assistant server-tool content
/// blocks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum WebSearchBehavior {
    /// Drop matching declarations and choices, plus Messages server-tool blocks.
    /// A tool choice that names a dropped tool no longer forces a tool call.
    #[default]
    Drop,
    /// Reject matching declarations and choices, plus Messages server-tool blocks.
    Reject,
}

/// The treatment of assistant messages that omit `reasoning_content` during
/// conversion.
///
/// In thinking mode, rendering expects every assistant message to carry
/// reasoning content. A protocol request might omit the reasoning content of
/// a historical assistant message; this setting selects whether conversion
/// fills it with an empty string or rejects the request.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReasoningContentBehavior {
    /// Fill missing reasoning content with an empty string.
    #[default]
    FillEmpty,
    /// Reject the request when an assistant message without reasoning content
    /// meets all of the following conditions:
    ///
    /// - It follows the last user message of the converted conversation, or
    ///   the converted conversation has no user message. Messages `system`
    ///   role messages and Responses `developer` messages convert to system
    ///   messages in V4.1 or user messages in V4.
    /// - It has tool calls, or the request declares tools. Responses counts
    ///   only the tool declarations that conversion keeps.
    /// - None of its tool call IDs carries the empty-thinking marker. The
    ///   chunk generators of this crate put the marker in the IDs of tool
    ///   calls from a thinking-mode turn that produced no reasoning content.
    ///
    /// Other missing reasoning content becomes an empty string.
    Reject,
}

/// The model version a converted conversation targets.
///
/// Conversion resolves it from the request's model name through
/// [`ConversionOptions::model_versions_by_name`]. V4 takes no image input:
/// image blocks become the text `[Unsupported Image]` without validation.
/// Responses namespaces and the `apply_patch` description also follow the model
/// version; see the Responses adapter documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ModelVersion {
    /// DeepSeek V4 Pro, rendered by `DeepseekV4Encoding`.
    DeepseekV4Pro,
    /// DeepSeek V4.1 Flash, rendered by `DeepseekV41Encoding`.
    DeepseekV41Flash,
}

impl ModelVersion {
    /// Return whether the model accepts image input.
    pub fn supports_image(self) -> bool {
        self == Self::DeepseekV41Flash
    }

    /// Return whether the model supports mid-conversation system messages.
    pub fn supports_mid_conversation_system(self) -> bool {
        self == Self::DeepseekV41Flash
    }

    /// Resolve the final `top_p` of the inference request from the requested
    /// value.
    ///
    /// In thinking mode, V4.1 raises `top_p` to at least 0.95 and V4 uses 0.95
    /// with tools and 1.0 without. Outside thinking mode, V4.1 uses 1.0 and V4
    /// uses the requested value. An unspecified value counts as 1.0.
    pub fn resolve_top_p(self, top_p: Option<f32>, thinking_mode: bool, has_tools: bool) -> f32 {
        match (self, thinking_mode) {
            (Self::DeepseekV41Flash, true) => top_p.unwrap_or(1.0).max(0.95),
            (Self::DeepseekV4Pro, true) if has_tools => 0.95,
            (Self::DeepseekV4Pro, true) | (Self::DeepseekV41Flash, false) => 1.0,
            (Self::DeepseekV4Pro, false) => top_p.unwrap_or(1.0),
        }
    }
}

/// The default inclusive upper bound of `max_tokens` in both thinking modes.
pub const DEFAULT_MAX_TOKENS_UPPER_BOUND: u32 = 393_216;

/// Settings that control conversion of protocol requests.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ConversionOptions {
    /// Whether requests that do not specify a thinking mode use thinking mode.
    ///
    /// Defaults to `true`.
    pub default_thinking_mode: bool,
    /// Behavior for missing assistant `reasoning_content` in thinking mode.
    ///
    /// Defaults to [`ReasoningContentBehavior::FillEmpty`].
    pub reasoning_content_behavior: ReasoningContentBehavior,
    /// Behavior for Responses `web_search` tool declarations and tool choices.
    /// Under either setting, conversion drops historical `web_search_call`
    /// input items and rejects the request when an earlier tool call has no
    /// output before such an item.
    ///
    /// Defaults to [`WebSearchBehavior::Drop`].
    pub responses_web_search: WebSearchBehavior,
    /// Behavior for the Messages `web_search` server tool, its named tool
    /// choice, and its `server_tool_use` / `web_search_tool_result` blocks.
    ///
    /// Defaults to [`WebSearchBehavior::Reject`].
    pub messages_web_search: WebSearchBehavior,
    /// The target model version of each accepted request model name.
    ///
    /// Defaults to the model names `DeepSeek-V4-Pro` for V4 and
    /// `DeepSeek-V4.1-Flash` for V4.1. A request whose model is not a key here
    /// is rejected. [`Self::with_model_alias`] adds or replaces one name, such
    /// as an API model name that a service accepts;
    /// [`Self::with_model_versions_by_name`] replaces the whole table.
    pub model_versions_by_name: HashMap<String, ModelVersion>,
    /// Largest accepted `max_tokens` of a request in thinking mode. Chat
    /// Completions and Responses reject a larger value; Messages lowers it to
    /// this bound. [`Self::resolve_max_tokens_upper_bound`] selects between
    /// this field and [`Self::non_thinking_max_tokens_upper_bound`].
    ///
    /// Defaults to [`DEFAULT_MAX_TOKENS_UPPER_BOUND`].
    pub thinking_max_tokens_upper_bound: u32,
    /// Largest accepted `max_tokens` of a request outside thinking mode. Chat
    /// Completions and Responses reject a larger value; Messages lowers it to
    /// this bound. [`Self::thinking_max_tokens_upper_bound`] applies in
    /// thinking mode.
    ///
    /// Defaults to [`DEFAULT_MAX_TOKENS_UPPER_BOUND`].
    pub non_thinking_max_tokens_upper_bound: u32,
    /// Special token mapping used by conversion, validation, and parsing.
    ///
    /// Defaults to [`default_special_tokens`].
    pub special_tokens: Arc<dyn SpecialTokens>,
}

impl ConversionOptions {
    /// Construct options that enable thinking for requests that do not
    /// specify a thinking mode.
    pub fn new() -> Self {
        Self {
            default_thinking_mode: true,
            reasoning_content_behavior: ReasoningContentBehavior::FillEmpty,
            responses_web_search: WebSearchBehavior::Drop,
            messages_web_search: WebSearchBehavior::Reject,
            model_versions_by_name: HashMap::from([
                ("DeepSeek-V4-Pro".to_string(), ModelVersion::DeepseekV4Pro),
                (
                    "DeepSeek-V4.1-Flash".to_string(),
                    ModelVersion::DeepseekV41Flash,
                ),
            ]),
            thinking_max_tokens_upper_bound: DEFAULT_MAX_TOKENS_UPPER_BOUND,
            non_thinking_max_tokens_upper_bound: DEFAULT_MAX_TOKENS_UPPER_BOUND,
            special_tokens: default_special_tokens(),
        }
    }

    /// Accept `model` as a name of `model_version`, consuming and returning
    /// `self`.
    ///
    /// Adds the name, or replaces the version of an existing one.
    #[must_use]
    pub fn with_model_alias(
        mut self,
        model: impl Into<String>,
        model_version: ModelVersion,
    ) -> Self {
        self.model_versions_by_name
            .insert(model.into(), model_version);
        self
    }

    /// Replace the whole table of accepted model names, consuming and returning
    /// `self`.
    ///
    /// Drops the default names unless `model_versions_by_name` includes them.
    /// Use [`Self::with_model_alias`] to add or replace a single name.
    #[must_use]
    pub fn with_model_versions_by_name<I, M>(mut self, model_versions_by_name: I) -> Self
    where
        I: IntoIterator<Item = (M, ModelVersion)>,
        M: Into<String>,
    {
        self.model_versions_by_name = model_versions_by_name
            .into_iter()
            .map(|(model, model_version)| (model.into(), model_version))
            .collect();
        self
    }

    /// Resolve the target model version of a request model name through
    /// [`Self::model_versions_by_name`].
    ///
    /// # Errors
    ///
    /// Returns a [`ConversionError::BadRequest`] that lists every accepted name
    /// when `model` is not in the table.
    pub fn resolve_model_version(&self, model: &str) -> Result<ModelVersion, ConversionError> {
        self.model_versions_by_name
            .get(model)
            .copied()
            .ok_or_else(|| {
                let mut names: Vec<&str> = self
                    .model_versions_by_name
                    .keys()
                    .map(String::as_str)
                    .collect();
                names.sort_unstable();
                ConversionError::bad_request(format!(
                    "The supported API model names are {}, but you passed {model}.",
                    names.join(", ")
                ))
            })
    }

    /// Return the largest accepted `max_tokens` of a request in the given
    /// thinking mode: [`Self::thinking_max_tokens_upper_bound`] when
    /// `thinking_mode` is `true`, otherwise
    /// [`Self::non_thinking_max_tokens_upper_bound`].
    pub fn resolve_max_tokens_upper_bound(&self, thinking_mode: bool) -> u32 {
        if thinking_mode {
            self.thinking_max_tokens_upper_bound
        } else {
            self.non_thinking_max_tokens_upper_bound
        }
    }

    /// Set the largest accepted `max_tokens` of a request in thinking mode,
    /// consuming and returning `self`.
    #[must_use]
    pub fn with_thinking_max_tokens_upper_bound(
        mut self,
        thinking_max_tokens_upper_bound: u32,
    ) -> Self {
        self.thinking_max_tokens_upper_bound = thinking_max_tokens_upper_bound;
        self
    }

    /// Set the largest accepted `max_tokens` of a request outside thinking
    /// mode, consuming and returning `self`.
    #[must_use]
    pub fn with_non_thinking_max_tokens_upper_bound(
        mut self,
        non_thinking_max_tokens_upper_bound: u32,
    ) -> Self {
        self.non_thinking_max_tokens_upper_bound = non_thinking_max_tokens_upper_bound;
        self
    }

    /// Set the default thinking mode, consuming and returning `self`.
    #[must_use]
    pub fn with_default_thinking_mode(mut self, default_thinking_mode: bool) -> Self {
        self.default_thinking_mode = default_thinking_mode;
        self
    }

    /// Replace the special token mapping, consuming and returning `self`.
    #[must_use]
    pub fn with_special_tokens(mut self, special_tokens: impl SpecialTokens + 'static) -> Self {
        self.special_tokens = Arc::new(special_tokens);
        self
    }

    /// Set the missing-reasoning-content behavior, consuming and returning `self`.
    #[must_use]
    pub fn with_reasoning_content_behavior(
        mut self,
        reasoning_content_behavior: ReasoningContentBehavior,
    ) -> Self {
        self.reasoning_content_behavior = reasoning_content_behavior;
        self
    }

    /// Set the Responses `web_search` behavior, consuming and returning `self`.
    #[must_use]
    pub fn with_responses_web_search(mut self, responses_web_search: WebSearchBehavior) -> Self {
        self.responses_web_search = responses_web_search;
        self
    }

    /// Set the Messages `web_search` behavior, consuming and returning `self`.
    #[must_use]
    pub fn with_messages_web_search(mut self, messages_web_search: WebSearchBehavior) -> Self {
        self.messages_web_search = messages_web_search;
        self
    }
}

impl Default for ConversionOptions {
    fn default() -> Self {
        Self::new()
    }
}
