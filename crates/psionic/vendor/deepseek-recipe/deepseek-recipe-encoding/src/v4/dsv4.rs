//! The DeepSeek V4 prompt encoding, [`DeepseekV4Encoding`].

use deepseek_recipe_core::conversation::{Conversation, ReasoningEffort, ResponseFormat};
use deepseek_recipe_core::json::stringify_spaced;
use deepseek_recipe_core::messages::{InputMessage, ToolCall};
use deepseek_recipe_core::prompt;
use deepseek_recipe_core::prompt::Prompt;
use deepseek_recipe_core::special_tokens::{SpecialToken, SpecialTokens};
use deepseek_recipe_core::tools::{ToolChoice, ToolDefinition};

use super::{
    EncodingComponents, into_rendered_prompt, is_prefix_continuation, parse_tool_call_arguments,
    renders_tool_prompt, tool_call_instructions, tool_definition_json,
};
use crate::{EncodingError, PromptEncoding, RenderedPrompt, TokenizerEncoder};

const REASONING_EFFORT_HIGH: &str = r"Reasoning Effort: Absolute maximum with no shortcuts permitted.
You MUST be very thorough in your thinking and comprehensively decompose the problem to resolve the root cause, rigorously stress-testing your logic against all potential paths, edge cases, and adversarial scenarios.
Explicitly write out your entire deliberation process, documenting every intermediate step, considered alternative, and rejected hypothesis to ensure absolutely no assumption is left unchecked.

";
const REASONING_EFFORT_MAX: &str = r"Reasoning Effort: Beyond maximum — exhaustive, relentless, and uncompromising.
You MUST reason with the utmost depth and rigor, leaving absolutely nothing to chance: exhaustively decompose the problem into its most fundamental components, trace every causal chain to its root, and resolve the underlying cause rather than any surface symptom.
Do not stop reasoning until you have independently verified the solution from multiple angles and are certain that no assumption remains unchecked and no error remains undiscovered.

";

const JSON_OBJECT_SYSTEM_PROMPT: &str = "You are a helpful assistant designed to output JSON.\n## Response Format:\n\nReply with JSON object ONLY.";

/// The DeepSeek V4 prompt encoding: prompt rendering and, with an attached
/// tokenizer, token encoding.
///
/// Rendering writes the non-empty system messages, in their original order,
/// after the BOS token and any reasoning effort prompt, separated by blank
/// lines and without a system token. The tool prompt and the response format
/// follow them as additional system messages.
#[derive(Clone)]
pub struct DeepseekV4Encoding {
    components: EncodingComponents,
}

impl DeepseekV4Encoding {
    /// Create a prompt encoding with the default special tokens and no
    /// attached tokenizer.
    ///
    /// Token encoding requires a tokenizer, attached with
    /// [`Self::with_tokenizer`].
    pub fn new() -> Self {
        Self {
            components: EncodingComponents::new(),
        }
    }

    /// Attach the tokenizer that [`PromptEncoding::encode`] uses.
    pub fn with_tokenizer(self, tokenizer: impl TokenizerEncoder + 'static) -> Self {
        Self {
            components: self.components.with_tokenizer(tokenizer),
        }
    }

    /// Replace the special token mapping used by prompt rendering.
    pub fn with_special_tokens(self, tokens: impl SpecialTokens + 'static) -> Self {
        Self {
            components: self.components.with_special_tokens(tokens),
        }
    }

    /// Render the tool prompt. Tool schemas are plain text.
    fn render_tool_prompt(&self, tools: &[ToolDefinition]) -> Prompt {
        let mut prompt = tool_call_instructions(
            self.components.special_tokens.as_ref(),
            "tool_calls",
            "invoke",
            "parameter",
        );
        for tool in tools.iter().filter(|tool| !tool.defer_loading) {
            prompt.push_plain(stringify_spaced(&tool_definition_json(tool)));
            prompt.push_plain("\n");
        }
        prompt.push_plain(
            "\nYou MUST strictly follow the above defined tool name and parameter schemas to invoke tool calls.\n",
        );
        prompt
    }

    /// Append the DSML tool-call block of `tool_calls` to `prompt`.
    fn push_tool_calls(&self, prompt: &mut Prompt, tool_calls: &[ToolCall]) {
        let dsml = self.components.special(SpecialToken::Dsml);
        *prompt += prompt!("\n\n<{dsml}tool_calls>\n");
        for tool_call in tool_calls {
            let name = &tool_call.name;
            *prompt += prompt!("<{dsml}invoke name=\"{name}\">\n");
            for (key, value) in &parse_tool_call_arguments(tool_call) {
                let (is_str, value) = match value.as_str() {
                    Some(value) => ("true", value.to_owned()),
                    None => ("false", stringify_spaced(value)),
                };
                *prompt += prompt!(
                    "<{dsml}parameter name=\"{key}\" string=\"{is_str}\">{value}</{dsml}parameter>\n"
                );
            }
            *prompt += prompt!("</{dsml}invoke>\n");
        }
        *prompt += prompt!("</{dsml}tool_calls>");
    }
}

impl Default for DeepseekV4Encoding {
    fn default() -> Self {
        Self::new()
    }
}

impl PromptEncoding for DeepseekV4Encoding {
    fn encode(&self, conversation: &Conversation) -> Result<Vec<u32>, EncodingError> {
        self.components
            .encode(&self.render_conversation(conversation))
    }

    fn render_conversation(&self, conversation: &Conversation) -> RenderedPrompt {
        let mut messages = normalize_messages(&conversation.messages);
        let renders_tool_prompt = renders_tool_prompt(conversation);
        add_system_messages(
            &mut messages,
            renders_tool_prompt.then(|| self.render_tool_prompt(&conversation.tools)),
            &conversation.response_format,
        );
        // Earlier reasoning content is rendered only with the tool prompt.
        let drop_thinking = !renders_tool_prompt;
        let thinking_mode = conversation.thinking_mode;

        let mut prompt = prompt!("{}", self.components.special(SpecialToken::Bos));
        if thinking_mode {
            match conversation.reasoning_effort {
                Some(ReasoningEffort::Low) => {}
                None | Some(ReasoningEffort::High) | Some(ReasoningEffort::Xhigh) => {
                    prompt.push_plain(REASONING_EFFORT_HIGH);
                }
                Some(ReasoningEffort::Max) => prompt.push_plain(REASONING_EFFORT_MAX),
            }
        }

        let mut has_system = false;
        let mut previous_is_user = false;
        let mut previous_is_tool = false;
        for (index, message) in messages.iter().enumerate() {
            match message {
                InputMessage::System { content, .. } => {
                    if has_system {
                        prompt.push_plain("\n\n");
                    }
                    prompt += content;
                    has_system = true;
                }
                InputMessage::User { content, .. } => {
                    if previous_is_user || previous_is_tool {
                        prompt.push_plain("\n\n");
                    } else {
                        prompt += prompt!("{}", self.components.special(SpecialToken::User));
                    }
                    prompt += content;
                }
                InputMessage::Assistant {
                    content,
                    reasoning_content,
                    tool_calls,
                    prefix,
                } => {
                    prompt += prompt!("{}", self.components.special(SpecialToken::Assistant));
                    if let Some(reasoning_content) = reasoning_content
                        && (*prefix || !drop_thinking)
                    {
                        prompt +=
                            prompt!("{}", self.components.special(SpecialToken::ThinkingStart));
                        self.components
                            .push_assistant_content(&mut prompt, reasoning_content);
                    }
                    if !thinking_mode || !*prefix || !content.is_empty() {
                        prompt += prompt!("{}", self.components.special(SpecialToken::ThinkingEnd));
                    }
                    self.components.push_assistant_content(&mut prompt, content);
                    if let Some(tool_calls) = tool_calls {
                        self.push_tool_calls(&mut prompt, tool_calls);
                    }
                    if index + 1 < messages.len() || !*prefix {
                        prompt += prompt!("{}", self.components.special(SpecialToken::Eos));
                    }
                }
                InputMessage::Tool { content, .. } => {
                    if previous_is_tool {
                        prompt.push_plain("\n\n");
                    } else {
                        prompt += prompt!("{}", self.components.special(SpecialToken::User));
                    }
                    prompt += prompt!("<tool_result>{content}</tool_result>");
                }
                InputMessage::LatestReminder { content } => {
                    let latest_reminder = self.components.special(SpecialToken::LatestReminder);
                    prompt += prompt!("{latest_reminder}{content}");
                }
            }
            previous_is_user = matches!(message, InputMessage::User { .. });
            previous_is_tool = matches!(message, InputMessage::Tool { .. });
        }
        if !is_prefix_continuation(&messages) {
            let assistant = self.components.special(SpecialToken::Assistant);
            let thinking_marker = self.components.special(if thinking_mode {
                SpecialToken::ThinkingStart
            } else {
                SpecialToken::ThinkingEnd
            });
            prompt += prompt!("{assistant}{thinking_marker}");
        }
        if conversation.tool_choice == ToolChoice::Required && renders_tool_prompt {
            let dsml = self.components.special(SpecialToken::Dsml);
            prompt += prompt!("\n\n<{dsml}tool_calls>\n");
        }
        into_rendered_prompt(prompt, &messages)
    }
}

/// Merge messages into the shape the template renders.
///
/// Non-empty system messages are moved to the start in their original order
/// and empty ones are dropped. After the system messages are moved, adjacent
/// user messages are joined into one message: a blank line separates their
/// non-empty contents, and their image sources are concatenated. The tool
/// calls of each assistant message are sorted by ID, and the messages that
/// follow it, one per tool call, are sorted by tool call ID; a tool result
/// without an ID sorts first, and messages that are not tool results follow
/// the tool results in their original order.
fn normalize_messages(messages: &[InputMessage]) -> Vec<InputMessage> {
    let mut system_messages = Vec::new();
    let mut other_messages: Vec<InputMessage> = Vec::new();
    for message in messages.iter().cloned() {
        match (other_messages.last_mut(), message) {
            (
                _,
                InputMessage::System {
                    content,
                    image_sources,
                    ..
                },
            ) => {
                if !content.is_empty() {
                    system_messages.push(InputMessage::System {
                        content,
                        image_sources,
                        tools: None,
                    });
                }
            }
            (
                Some(InputMessage::User {
                    content: previous,
                    image_sources: previous_image_sources,
                }),
                InputMessage::User {
                    content,
                    image_sources,
                },
            ) => {
                if !content.is_empty() {
                    if !previous.is_empty() {
                        previous.push_plain("\n\n");
                    }
                    *previous += content;
                }
                previous_image_sources.extend(image_sources);
            }
            (_, message) => other_messages.push(message),
        }
    }

    let mut tool_result_ranges = Vec::new();
    for (index, message) in other_messages.iter_mut().enumerate() {
        if let InputMessage::Assistant {
            tool_calls: Some(tool_calls),
            ..
        } = message
        {
            tool_calls.sort_by(|a, b| a.id.cmp(&b.id));
            if !tool_calls.is_empty() {
                tool_result_ranges.push(index + 1..index + 1 + tool_calls.len());
            }
        }
    }
    for range in tool_result_ranges {
        // A range that extends past the last message has no complete set of
        // results and keeps its order.
        let Some(results) = other_messages.get_mut(range) else {
            continue;
        };
        results.sort_by(|a, b| tool_result_sort_key(a).cmp(&tool_result_sort_key(b)));
    }

    system_messages.extend(other_messages);
    system_messages
}

/// Return the sort key of a message in a tool-result range: tool results sort
/// before other messages and, among themselves, by tool call ID with a
/// missing ID first. The key is a total order, so the sort is defined for a
/// range that contains a message that is not a tool result.
fn tool_result_sort_key(message: &InputMessage) -> (bool, Option<&str>) {
    match message {
        InputMessage::Tool { tool_call_id, .. } => (false, tool_call_id.as_deref()),
        _ => (true, None),
    }
}

/// Insert the system messages that hold the tool prompt and the response
/// format after the leading system messages. A conversation without a leading
/// system message first receives an empty one.
fn add_system_messages(
    messages: &mut Vec<InputMessage>,
    tool_prompt: Option<Prompt>,
    response_format: &ResponseFormat,
) {
    let response_format = match response_format {
        ResponseFormat::Text | ResponseFormat::Regex(_) => None,
        ResponseFormat::JsonObject => Some(Prompt::plain(JSON_OBJECT_SYSTEM_PROMPT)),
        ResponseFormat::JsonSchema(schema) => {
            let schema = stringify_spaced(schema);
            Some(prompt!(
                "## Response Format:\n\nYou MUST strictly adhere to the following schema to reply:\n{schema}"
            ))
        }
    };
    if tool_prompt.is_none() && response_format.is_none() {
        return;
    }
    let mut index = messages
        .iter()
        .position(|message| !matches!(message, InputMessage::System { .. }))
        .unwrap_or(messages.len());
    if index == 0 {
        messages.insert(
            0,
            InputMessage::System {
                content: Prompt::new(),
                image_sources: Vec::new(),
                tools: None,
            },
        );
        index = 1;
    }
    for content in [tool_prompt, response_format].into_iter().flatten() {
        messages.insert(
            index,
            InputMessage::System {
                content,
                image_sources: Vec::new(),
                tools: None,
            },
        );
        index += 1;
    }
}
