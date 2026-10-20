//! The DeepSeek V4.1 prompt encoding, [`DeepseekV41Encoding`].

use deepseek_recipe_core::conversation::{Conversation, ReasoningEffort, ResponseFormat};
use deepseek_recipe_core::json::stringify_python_style;
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

const TOOL_CALLS_BLOCK_NAME: &str = " calls";
const TOOL_CALL_TAG_NAME: &str = " invoke";
const TOOL_PARAMETER_TAG_NAME: &str = " parameter";

fn reasoning_effort_template(effort: Option<ReasoningEffort>) -> Prompt {
    let effort_score = match effort {
        Some(ReasoningEffort::Low) => "50",
        None | Some(ReasoningEffort::High) | Some(ReasoningEffort::Xhigh) => "75",
        Some(ReasoningEffort::Max) => "100",
    };
    prompt!(
        "Reasoning Effort: {effort_score} (range 1-100, the higher the value, the more thorough the reasoning)\n\n"
    )
}

/// The DeepSeek V4.1 prompt encoding: prompt rendering and, with an attached
/// tokenizer, token encoding.
///
/// Rendering appends the tool prompt and the response format to the first
/// system message, after inserting an empty system message at the start when
/// the conversation does not start with one.
#[derive(Clone)]
pub struct DeepseekV41Encoding {
    components: EncodingComponents,
}

impl DeepseekV41Encoding {
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
    ///
    /// Downstream change: public so that Psionic can place the tool prompt in
    /// a later system message, as the checkpoint's reference encoder does.
    pub fn render_tool_prompt(&self, tools: &[ToolDefinition]) -> Prompt {
        let tools_json = tools
            .iter()
            .filter(|tool| !tool.defer_loading)
            .map(|tool| stringify_python_style(&tool_definition_json(tool)))
            .collect::<Vec<_>>()
            .join("\n");
        let mut prompt = tool_call_instructions(
            self.components.special_tokens.as_ref(),
            TOOL_CALLS_BLOCK_NAME,
            TOOL_CALL_TAG_NAME,
            TOOL_PARAMETER_TAG_NAME,
        );
        prompt.push_plain(tools_json);
        prompt.push_plain(
            "\n\nYou MUST strictly follow the above defined tool name and parameter schemas to invoke tool calls.\n",
        );
        prompt
    }

    /// Render the DSML tool-call block of `tool_calls`.
    fn render_tool_calls(&self, tool_calls: &[ToolCall]) -> Prompt {
        let dsml = self.components.special(SpecialToken::Dsml);
        let mut invocations = Prompt::new();
        for (index, tool_call) in tool_calls.iter().enumerate() {
            if index > 0 {
                invocations.push_plain("\n");
            }
            let mut arguments = Prompt::new();
            for (argument_index, (key, value)) in
                parse_tool_call_arguments(tool_call).iter().enumerate()
            {
                let (is_str, value) = match value.as_str() {
                    Some(value) => ("true", value.to_owned()),
                    None => ("false", stringify_python_style(value)),
                };
                if argument_index > 0 {
                    arguments.push_plain("\n");
                }
                arguments += prompt!(
                    "<{dsml}{TOOL_PARAMETER_TAG_NAME} name=\"{key}\" string=\"{is_str}\">{value}</{dsml}{TOOL_PARAMETER_TAG_NAME}>"
                );
            }
            let name = &tool_call.name;
            invocations += prompt!(
                "<{dsml}{TOOL_CALL_TAG_NAME} name=\"{name}\">\n{arguments}\n</{dsml}{TOOL_CALL_TAG_NAME}>"
            );
        }
        prompt!(
            "\n\n<{dsml}{TOOL_CALLS_BLOCK_NAME}>\n{invocations}\n</{dsml}{TOOL_CALLS_BLOCK_NAME}>"
        )
    }

    /// Render one message.
    ///
    /// In thinking mode, rendering writes the system token and the reasoning
    /// effort prompt before the first message; outside thinking mode, it
    /// writes the system token only before a first message that is a system
    /// message. In thinking mode, an assistant message after the first message
    /// renders its reasoning content when `drop_thinking` is unset or the
    /// message has `prefix` set; absent reasoning content renders as an empty
    /// reasoning block. A message with `prefix` set has no end-of-sentence
    /// token, and its reasoning block has no thinking end token when the
    /// message has no content and no tool calls.
    fn render_message(
        &self,
        index: usize,
        messages: &[InputMessage],
        thinking_mode: bool,
        reasoning_effort: Option<ReasoningEffort>,
        drop_thinking: bool,
    ) -> Prompt {
        let message = &messages[index];
        let previous = messages[..index].last();
        let reasoning_effort_prompt = if index == 0 && thinking_mode {
            reasoning_effort_template(reasoning_effort)
        } else {
            Prompt::new()
        };
        let mut prompt = Prompt::new();
        if index == 0
            && (!reasoning_effort_prompt.is_empty()
                || matches!(message, InputMessage::System { .. }))
        {
            prompt += prompt!("{}", self.components.special(SpecialToken::System));
        }
        prompt += reasoning_effort_prompt;
        let user_or_separator = |prompt: &mut Prompt| {
            if matches!(
                previous,
                Some(InputMessage::User { .. } | InputMessage::Tool { .. })
            ) {
                prompt.push_plain("\n\n");
            } else {
                *prompt += prompt!("{}", self.components.special(SpecialToken::User));
            }
        };
        match message {
            InputMessage::System { content, tools, .. } => {
                if index > 0 {
                    prompt += prompt!("{}", self.components.special(SpecialToken::System));
                }
                prompt += content;
                if let Some(tools) = tools.as_ref().filter(|tools| !tools.is_empty()) {
                    let schemas = tools
                        .iter()
                        .map(|tool| stringify_python_style(&tool_definition_json(tool)))
                        .collect::<Vec<_>>()
                        .join("\n");
                    prompt.push_plain(format!("\n\n### Updated Tool Schemas\n\n{schemas}"));
                }
            }
            InputMessage::User { content, .. } => {
                user_or_separator(&mut prompt);
                prompt += content;
            }
            InputMessage::LatestReminder { content } => {
                let latest_reminder = self.components.special(SpecialToken::LatestReminder);
                prompt += prompt!("{latest_reminder}{content}");
            }
            InputMessage::Tool { content, .. } => {
                user_or_separator(&mut prompt);
                prompt += prompt!("<tool_result>{content}</tool_result>");
            }
            InputMessage::Assistant {
                content,
                reasoning_content,
                tool_calls,
                prefix,
            } => {
                let tool_calls = match tool_calls {
                    Some(tool_calls) if !tool_calls.is_empty() => {
                        self.render_tool_calls(tool_calls)
                    }
                    _ => Prompt::new(),
                };
                let renders_reasoning = thinking_mode && index > 0 && (!drop_thinking || *prefix);
                let mut reasoning_block = Prompt::new();
                if renders_reasoning {
                    if let Some(reasoning_content) = reasoning_content {
                        self.components
                            .push_assistant_content(&mut reasoning_block, reasoning_content);
                    }
                    if !(*prefix && content.is_empty() && tool_calls.is_empty()) {
                        reasoning_block +=
                            prompt!("{}", self.components.special(SpecialToken::ThinkingEnd));
                    }
                }
                let thinking_marker =
                    if !reasoning_block.is_empty() || (renders_reasoning && *prefix) {
                        SpecialToken::ThinkingStart
                    } else {
                        SpecialToken::ThinkingEnd
                    };
                let assistant = self.components.special(SpecialToken::Assistant);
                let thinking_marker = self.components.special(thinking_marker);
                prompt += prompt!("{assistant}{thinking_marker}{reasoning_block}");
                self.components.push_assistant_content(&mut prompt, content);
                prompt += tool_calls;
                if !*prefix {
                    prompt += prompt!("{}", self.components.special(SpecialToken::Eos));
                }
            }
        }
        prompt
    }
}

impl Default for DeepseekV41Encoding {
    fn default() -> Self {
        Self::new()
    }
}

impl PromptEncoding for DeepseekV41Encoding {
    fn encode(&self, conversation: &Conversation) -> Result<Vec<u32>, EncodingError> {
        self.components
            .encode(&self.render_conversation(conversation))
    }

    fn render_conversation(&self, conversation: &Conversation) -> RenderedPrompt {
        let mut messages = normalize_messages(&conversation.messages);
        let renders_tool_prompt = renders_tool_prompt(conversation);
        let response_format = match &conversation.response_format {
            ResponseFormat::Text | ResponseFormat::Regex(_) => None,
            ResponseFormat::JsonObject => Some(serde_json::json!({"type": "json_object"})),
            ResponseFormat::JsonSchema(schema) => Some(schema.clone()),
        };
        // The tool prompt and the response format are appended to the first
        // system message, after its content.
        if renders_tool_prompt || response_format.is_some() {
            if !matches!(messages.first(), Some(InputMessage::System { .. })) {
                messages.insert(
                    0,
                    InputMessage::System {
                        content: Prompt::new(),
                        image_sources: Vec::new(),
                        tools: None,
                    },
                );
            }
            let Some(InputMessage::System { content, .. }) = messages.first_mut() else {
                unreachable!("a system message was inserted first");
            };
            if renders_tool_prompt {
                let tool_prompt = self.render_tool_prompt(&conversation.tools);
                *content += prompt!("\n\n{tool_prompt}");
            }
            if let Some(schema) = response_format {
                let schema = stringify_python_style(&schema);
                *content += prompt!(
                    "\n\n## Response Format:\n\nYou MUST strictly adhere to the following schema to reply:\n{schema}"
                );
            }
        }
        // Earlier reasoning content is rendered only with the tool prompt.
        let drop_thinking = !renders_tool_prompt;

        let mut prompt = prompt!("{}", self.components.special(SpecialToken::Bos));
        for index in 0..messages.len() {
            prompt += self.render_message(
                index,
                &messages,
                conversation.thinking_mode,
                conversation.reasoning_effort,
                drop_thinking,
            );
        }
        if !is_prefix_continuation(&messages) {
            let assistant = self.components.special(SpecialToken::Assistant);
            let thinking_marker = self.components.special(if conversation.thinking_mode {
                SpecialToken::ThinkingStart
            } else {
                SpecialToken::ThinkingEnd
            });
            prompt += prompt!("{assistant}{thinking_marker}");
        }
        if conversation.tool_choice == ToolChoice::Required && renders_tool_prompt {
            let dsml = self.components.special(SpecialToken::Dsml);
            prompt += prompt!("\n\n<{dsml}{TOOL_CALLS_BLOCK_NAME}>\n");
        }
        into_rendered_prompt(prompt, &messages)
    }
}

/// Merge messages into the shape the template renders.
///
/// Consecutive user messages are joined with a blank line, and a latest
/// reminder that directly follows another latest reminder is dropped. Tool
/// results with an ID that follow an assistant message with tool calls are
/// sorted by the order of its calls.
fn normalize_messages(messages: &[InputMessage]) -> Vec<InputMessage> {
    let mut normalized: Vec<InputMessage> = Vec::new();
    for message in messages.iter().cloned() {
        match (normalized.last_mut(), message) {
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
                *previous += prompt!("\n\n{content}");
                previous_image_sources.extend(image_sources);
            }
            (Some(InputMessage::LatestReminder { .. }), InputMessage::LatestReminder { .. }) => {}
            (_, message) => normalized.push(message),
        }
    }
    sort_tool_results_by_call_order(&mut normalized);
    normalized
}

/// Sort the tool results with call IDs in each run of user and tool messages
/// by the order of the calls of the preceding assistant message with tool
/// calls.
///
/// The sorted results occupy the positions of the results with call IDs, and
/// other messages keep their positions. A result whose ID matches no call
/// sorts as the first call.
fn sort_tool_results_by_call_order(messages: &mut [InputMessage]) {
    let mut call_ids_in_order: Vec<String> = Vec::new();
    let mut index = 0;
    while index < messages.len() {
        match &messages[index] {
            InputMessage::Assistant {
                tool_calls: Some(tool_calls),
                ..
            } if !tool_calls.is_empty() => {
                call_ids_in_order = tool_calls
                    .iter()
                    .map(|tool_call| tool_call.id.clone())
                    .collect();
                index += 1;
            }
            InputMessage::User { .. } | InputMessage::Tool { .. } => {
                let start = index;
                while index < messages.len()
                    && matches!(
                        messages[index],
                        InputMessage::User { .. } | InputMessage::Tool { .. }
                    )
                {
                    index += 1;
                }
                let tool_result_indices: Vec<usize> = (start..index)
                    .filter(|&result_index| {
                        matches!(
                            messages[result_index],
                            InputMessage::Tool {
                                tool_call_id: Some(_),
                                ..
                            }
                        )
                    })
                    .collect();
                if tool_result_indices.len() > 1 && !call_ids_in_order.is_empty() {
                    let mut tools: Vec<InputMessage> = tool_result_indices
                        .iter()
                        .map(|&result_index| {
                            // The loop below writes every slot back, so the
                            // placeholder message is never rendered.
                            std::mem::replace(
                                &mut messages[result_index],
                                InputMessage::LatestReminder {
                                    content: String::new(),
                                },
                            )
                        })
                        .collect();
                    // A result whose ID matches no call sorts first.
                    tools.sort_by_key(|message| match message {
                        InputMessage::Tool { tool_call_id, .. } => call_ids_in_order
                            .iter()
                            .position(|id| Some(id) == tool_call_id.as_ref())
                            .unwrap_or(0),
                        _ => 0,
                    });
                    for (&result_index, tool) in tool_result_indices.iter().zip(tools) {
                        messages[result_index] = tool;
                    }
                }
            }
            _ => index += 1,
        }
    }
}
