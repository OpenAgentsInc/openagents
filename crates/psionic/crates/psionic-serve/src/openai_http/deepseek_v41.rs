//! DeepSeek-V4.1-Flash request and stream mapping for Chat Completions.
//!
//! Requests become the recipe's `Conversation`, rendered and encoded by
//! [`DeepSeekV41Protocol`]. Generated token IDs go through the recipe's
//! `StreamProcessor`, and [`DeepSeekV41ChunkGenerator`] maps its parsed output
//! onto Psionic's existing chunk shape: `reasoning_content` apart from
//! `content`, incremental tool-call arguments, finish reasons, and usage.
//!
//! This is protocol-qualified only. No loaded model selects this path yet,
//! and the public model list names no V4.1 model.

// Nothing outside the tests calls this mapping until a V4.1 executor exists.
#![cfg_attr(not(test), allow(dead_code))]

use psionic_models::deepseek_v41::deepseek_recipe::request::{
    OutputConstraint, RequestReasoningEffort, ThinkingType, resolve_output_constraint,
    resolve_thinking_mode,
};
use psionic_models::deepseek_v41::deepseek_recipe::stream::{
    ChunkGenerator, CompletionUsage, FinishReason, OutputChunk, ParsingOptions, PromptUsage,
};
use psionic_models::deepseek_v41::deepseek_recipe_core::conversation::{
    Conversation, ResponseFormat,
};
use psionic_models::deepseek_v41::deepseek_recipe_core::messages::{InputMessage, ToolCall};
use psionic_models::deepseek_v41::deepseek_recipe_core::prompt::Prompt;
use psionic_models::deepseek_v41::deepseek_recipe_core::tools::{ToolChoice, ToolDefinition};
use psionic_models::deepseek_v41::{
    DeepSeekV41Error, DeepSeekV41GenerationOptions, DeepSeekV41Protocol, DeepSeekV41RuntimeBudget,
    DeepSeekV41SamplingRequest, deepseek_v41_parsing_options,
    refuse_unenforced_deepseek_v41_constraint, resolve_deepseek_v41_generation,
};

use super::{
    ChatCompletionChunk, ChatCompletionChunkChoice, ChatCompletionChunkDelta,
    ChatCompletionChunkToolCall, ChatCompletionChunkToolCallFunctionDelta,
    ChatCompletionContentPart, ChatCompletionMessageContent, ChatCompletionRequest,
    ChatCompletionToolCall, ChatCompletionToolCallFunction, ChatCompletionUsage,
    OpenAiCompatHttpError, ParsedCompletionChoice, PsionicReasoningMode, StopSequences,
    ToolChoiceRequest,
};

const IMAGE_REFUSAL: &str = "DeepSeek-V4.1-Flash doesn't accept images in Psionic yet.";

impl From<DeepSeekV41Error> for OpenAiCompatHttpError {
    fn from(error: DeepSeekV41Error) -> Self {
        match error {
            DeepSeekV41Error::Unsupported(_) | DeepSeekV41Error::InvalidRequest(_) => {
                Self::BadRequest(error.to_string())
            }
            _ => Self::Internal(error.to_string()),
        }
    }
}

/// A Chat Completions request rendered, encoded, and resolved for V4.1.
#[derive(Debug)]
pub(super) struct DeepSeekV41PreparedChat {
    pub(super) conversation: Conversation,
    pub(super) prompt: String,
    pub(super) prompt_token_ids: Vec<u32>,
    pub(super) generation: DeepSeekV41GenerationOptions,
    pub(super) parsing_options: ParsingOptions,
    pub(super) reasoning_mode: PsionicReasoningMode,
}

/// Converts and checks a Chat Completions request, then renders and encodes it.
pub(super) fn prepare_deepseek_v41_chat(
    protocol: &DeepSeekV41Protocol,
    request: &ChatCompletionRequest,
    budget: DeepSeekV41RuntimeBudget,
) -> Result<DeepSeekV41PreparedChat, OpenAiCompatHttpError> {
    let bad = |message: &str| OpenAiCompatHttpError::BadRequest(message.into());
    if request.psionic_grammar.is_some() || request.psionic_structured_output.is_some() {
        return Err(bad(
            "DeepSeek-V4.1-Flash can't guarantee structured output yet.",
        ));
    }
    let reasoning_mode = match &request.psionic_reasoning {
        None => PsionicReasoningMode::Separate,
        Some(reasoning) if reasoning.parser.is_none() => reasoning.mode,
        Some(_) => {
            return Err(bad(
                "DeepSeek-V4.1-Flash uses its own reasoning parser; leave psionic_reasoning.parser unset.",
            ));
        }
    };
    if request.parallel_tool_calls == Some(false) {
        return Err(bad(
            "DeepSeek-V4.1-Flash can't limit an answer to one tool call yet.",
        ));
    }
    let tools = request
        .tools
        .iter()
        .map(|tool| {
            if tool.kind != "function" {
                return Err(bad("DeepSeek-V4.1-Flash supports function tools only."));
            }
            Ok(ToolDefinition {
                name: tool.function.name.clone(),
                description: tool.function.description.clone(),
                parameters: tool
                    .function
                    .parameters
                    .clone()
                    .unwrap_or_else(|| serde_json::json!({})),
                strict: None,
                defer_loading: false,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let tool_choice = match &request.tool_choice {
        None => ToolChoice::Auto,
        Some(ToolChoiceRequest::Mode(mode)) => match mode.as_str() {
            "auto" => ToolChoice::Auto,
            "none" => ToolChoice::None,
            "required" => ToolChoice::Required,
            _ => return Err(bad("tool_choice must be auto, none, or required.")),
        },
        Some(ToolChoiceRequest::Named(_)) => {
            return Err(bad(
                "DeepSeek-V4.1-Flash can't force a named tool call yet.",
            ));
        }
    };
    let (response_format, has_response_format) = match &request.response_format {
        None => (ResponseFormat::Text, false),
        Some(format) => (
            match format.kind.as_str() {
                "text" => ResponseFormat::Text,
                "json_object" => ResponseFormat::JsonObject,
                "json_schema" => ResponseFormat::JsonSchema(
                    format
                        .json_schema
                        .as_ref()
                        .map(|schema| schema.schema.clone())
                        .or_else(|| format.schema.clone())
                        .ok_or_else(|| bad("json_schema needs a schema."))?,
                ),
                _ => return Err(bad("Unknown response_format type.")),
            },
            true,
        ),
    };
    // Record the constraint the recipe derives, then refuse what Psionic can't
    // enforce before constrained decoding exists.
    let output_constraint: Option<OutputConstraint> =
        resolve_output_constraint(&tools, tool_choice, &response_format, has_response_format)
            .map_err(|error| bad(&error.to_string()))?;
    refuse_unenforced_deepseek_v41_constraint(output_constraint.as_ref())?;

    let thinking_type = request.psionic_enable_thinking.map(|enabled| {
        if enabled {
            ThinkingType::Enabled
        } else {
            ThinkingType::Disabled
        }
    });
    let effort = request
        .reasoning_effort
        .as_ref()
        .map(|effort| {
            serde_json::from_value::<RequestReasoningEffort>(serde_json::Value::String(
                effort.clone(),
            ))
            .map_err(|_| bad("Unknown reasoning_effort."))
        })
        .transpose()?;
    let (thinking_mode, reasoning_effort) = resolve_thinking_mode(true, thinking_type, effort);

    let mut messages = Vec::with_capacity(request.messages.len());
    for message in &request.messages {
        let text = message_text(&message.content)?;
        messages.push(match message.role.as_str() {
            "system" | "developer" => InputMessage::System {
                content: Prompt::plain(text),
                image_sources: Vec::new(),
                tools: None,
            },
            "user" => InputMessage::User {
                content: Prompt::plain(text),
                image_sources: Vec::new(),
            },
            "tool" => InputMessage::Tool {
                content: Prompt::plain(text),
                image_sources: Vec::new(),
                tool_call_id: message.tool_call_id.clone(),
            },
            "assistant" => InputMessage::Assistant {
                content: text,
                reasoning_content: message.reasoning_content.clone(),
                tool_calls: message.tool_calls.as_ref().map(|calls| {
                    calls
                        .iter()
                        .map(|call| ToolCall {
                            id: call.id.clone(),
                            name: call.function.name.clone(),
                            arguments: call.function.arguments.clone(),
                        })
                        .collect()
                }),
                prefix: false,
            },
            other => {
                return Err(bad(&format!(
                    "DeepSeek-V4.1-Flash doesn't accept {other} messages."
                )));
            }
        });
    }
    if !matches!(
        messages.last(),
        Some(InputMessage::User { .. } | InputMessage::Tool { .. })
    ) {
        return Err(bad("The last message must be a user or tool message."));
    }
    let conversation = Conversation {
        messages,
        thinking_mode,
        tools,
        tool_choice,
        reasoning_effort,
        response_format,
    };
    let rendered = protocol.render(&conversation);
    let prompt_token_ids = protocol.encode_rendered(&rendered)?;
    let prompt_tokens =
        u32::try_from(prompt_token_ids.len()).map_err(|_| bad("The prompt is too long."))?;
    let generation = resolve_deepseek_v41_generation(
        &conversation,
        DeepSeekV41SamplingRequest {
            temperature: request.temperature,
            top_p: request.top_p,
            max_tokens: request
                .max_tokens
                .map(|value| u32::try_from(value).unwrap_or(u32::MAX)),
        },
        output_constraint,
        prompt_tokens,
        budget,
    )?;
    let stop_sequences = match &request.stop {
        None => Vec::new(),
        Some(StopSequences::One(stop)) => vec![stop.clone()],
        Some(StopSequences::Many(stops)) => stops.clone(),
    };
    let parsing_options = deepseek_v41_parsing_options(&conversation, stop_sequences);
    Ok(DeepSeekV41PreparedChat {
        conversation,
        prompt: rendered.prompt,
        prompt_token_ids,
        generation,
        parsing_options,
        reasoning_mode,
    })
}

/// Returns a message's text and refuses images, audio, and video.
fn message_text(content: &ChatCompletionMessageContent) -> Result<String, OpenAiCompatHttpError> {
    match content {
        ChatCompletionMessageContent::Text(text) => Ok(text.clone()),
        ChatCompletionMessageContent::Parts(parts) => parts
            .iter()
            .map(|part| match part {
                ChatCompletionContentPart::Text { text }
                | ChatCompletionContentPart::InputText { text } => Ok(text.as_str()),
                ChatCompletionContentPart::ImageUrl { .. }
                | ChatCompletionContentPart::InputImage { .. } => {
                    Err(OpenAiCompatHttpError::BadRequest(IMAGE_REFUSAL.into()))
                }
                ChatCompletionContentPart::InputAudio { .. }
                | ChatCompletionContentPart::VideoUrl { .. } => {
                    Err(OpenAiCompatHttpError::BadRequest(
                        "DeepSeek-V4.1-Flash accepts text only.".into(),
                    ))
                }
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|texts| texts.concat()),
    }
}

/// One event of a mapped V4.1 stream.
#[derive(Clone, Debug)]
pub(super) enum DeepSeekV41StreamEvent {
    /// A Chat Completions chunk, including the final one with a finish reason.
    Chunk(ChatCompletionChunk),
    /// Token counts, sent after the finish chunk.
    Usage(ChatCompletionUsage),
    /// The backend stopped without a finish event. No finish reason is
    /// claimed for this output; the caller reports the request as failed.
    Incomplete(ChatCompletionUsage),
}

/// Maps the recipe's parsed output onto Psionic Chat Completions chunks.
pub(super) struct DeepSeekV41ChunkGenerator {
    request_id: String,
    model: String,
    created: u64,
    reasoning_mode: PsionicReasoningMode,
    prompt_usage: PromptUsage,
    tool_calls: usize,
}

impl DeepSeekV41ChunkGenerator {
    pub(super) fn new(
        request_id: impl Into<String>,
        model: impl Into<String>,
        created: u64,
        reasoning_mode: PsionicReasoningMode,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            model: model.into(),
            created,
            reasoning_mode,
            prompt_usage: PromptUsage::default(),
            tool_calls: 0,
        }
    }

    fn chunk(
        &self,
        delta: ChatCompletionChunkDelta,
        finish_reason: Option<&'static str>,
    ) -> DeepSeekV41StreamEvent {
        DeepSeekV41StreamEvent::Chunk(ChatCompletionChunk {
            id: self.request_id.clone(),
            object: "chat.completion.chunk",
            created: self.created,
            model: self.model.clone(),
            choices: vec![ChatCompletionChunkChoice {
                index: 0,
                delta,
                finish_reason,
            }],
        })
    }

    fn tool_call_delta(
        &self,
        index: usize,
        name: Option<String>,
        arguments: String,
    ) -> ChatCompletionChunkDelta {
        let starts = name.is_some();
        ChatCompletionChunkDelta {
            tool_calls: Some(vec![ChatCompletionChunkToolCall {
                index,
                id: starts.then(|| format!("{}-tool-{index}", self.request_id)),
                kind: starts.then_some("function"),
                function: Some(ChatCompletionChunkToolCallFunctionDelta {
                    name,
                    arguments: Some(arguments),
                }),
            }]),
            ..ChatCompletionChunkDelta::default()
        }
    }

    fn usage(&self, completion: CompletionUsage) -> ChatCompletionUsage {
        ChatCompletionUsage {
            prompt_tokens: self.prompt_usage.prompt_tokens,
            completion_tokens: completion.completion_tokens,
            total_tokens: self.prompt_usage.prompt_tokens + completion.completion_tokens,
        }
    }
}

impl ChunkGenerator for DeepSeekV41ChunkGenerator {
    type Chunk = DeepSeekV41StreamEvent;

    async fn generate(&mut self, chunk: OutputChunk) -> Vec<Self::Chunk> {
        match chunk {
            OutputChunk::Start { prompt_usage, .. } => {
                self.prompt_usage = prompt_usage;
                vec![self.chunk(
                    ChatCompletionChunkDelta {
                        role: Some("assistant"),
                        ..ChatCompletionChunkDelta::default()
                    },
                    None,
                )]
            }
            OutputChunk::Content { content, .. } => vec![self.chunk(
                ChatCompletionChunkDelta {
                    content: Some(content),
                    ..ChatCompletionChunkDelta::default()
                },
                None,
            )],
            OutputChunk::Reasoning { content, .. } => match self.reasoning_mode {
                PsionicReasoningMode::Separate => vec![self.chunk(
                    ChatCompletionChunkDelta {
                        reasoning_content: Some(content),
                        ..ChatCompletionChunkDelta::default()
                    },
                    None,
                )],
                PsionicReasoningMode::Suppress => Vec::new(),
            },
            OutputChunk::ToolCallBegin => Vec::new(),
            OutputChunk::ToolCallNamed { tool_name } => {
                let index = self.tool_calls;
                self.tool_calls += 1;
                let delta = self.tool_call_delta(index, Some(tool_name), String::new());
                vec![self.chunk(delta, None)]
            }
            OutputChunk::ToolCallArgumentsDelta { arguments } => {
                let Some(index) = self.tool_calls.checked_sub(1) else {
                    return Vec::new();
                };
                let delta = self.tool_call_delta(index, None, arguments);
                vec![self.chunk(delta, None)]
            }
            OutputChunk::Finish {
                finish_reason,
                completion_usage,
                ..
            } => {
                let usage = self.usage(completion_usage);
                let reason = match finish_reason {
                    FinishReason::Stop | FinishReason::StopSequence => "stop",
                    FinishReason::Length => "length",
                    FinishReason::ToolCalls => "tool_calls",
                    FinishReason::ContentFilter => "content_filter",
                    FinishReason::EndOfStream => {
                        return vec![DeepSeekV41StreamEvent::Incomplete(usage)];
                    }
                };
                vec![
                    self.chunk(ChatCompletionChunkDelta::default(), Some(reason)),
                    DeepSeekV41StreamEvent::Usage(usage),
                ]
            }
        }
    }
}

/// Collects a mapped stream into one non-streaming choice and its usage.
pub(super) fn collect_deepseek_v41_choice(
    events: &[DeepSeekV41StreamEvent],
) -> Result<(ParsedCompletionChoice, ChatCompletionUsage), OpenAiCompatHttpError> {
    let mut content: Option<String> = None;
    let mut reasoning_content: Option<String> = None;
    let mut tool_calls: Vec<ChatCompletionToolCall> = Vec::new();
    let mut finish_reason = None;
    let mut usage = None;
    for event in events {
        match event {
            DeepSeekV41StreamEvent::Chunk(chunk) => {
                for choice in &chunk.choices {
                    if let Some(text) = &choice.delta.content {
                        content.get_or_insert_with(String::new).push_str(text);
                    }
                    if let Some(text) = &choice.delta.reasoning_content {
                        reasoning_content
                            .get_or_insert_with(String::new)
                            .push_str(text);
                    }
                    for call in choice.delta.tool_calls.iter().flatten() {
                        let function = call.function.as_ref();
                        if let Some(id) = &call.id {
                            tool_calls.push(ChatCompletionToolCall {
                                id: id.clone(),
                                kind: "function".into(),
                                function: ChatCompletionToolCallFunction {
                                    name: function
                                        .and_then(|function| function.name.clone())
                                        .unwrap_or_default(),
                                    arguments: String::new(),
                                },
                            });
                        }
                        if let (Some(target), Some(arguments)) = (
                            tool_calls.get_mut(call.index),
                            function.and_then(|function| function.arguments.as_ref()),
                        ) {
                            target.function.arguments.push_str(arguments);
                        }
                    }
                    finish_reason = choice.finish_reason.or(finish_reason);
                }
            }
            DeepSeekV41StreamEvent::Usage(value) => usage = Some(value.clone()),
            DeepSeekV41StreamEvent::Incomplete(_) => {
                return Err(OpenAiCompatHttpError::Internal(
                    "The model stopped before finishing its answer.".into(),
                ));
            }
        }
    }
    let (Some(finish_reason), Some(usage)) = (finish_reason, usage) else {
        return Err(OpenAiCompatHttpError::Internal(
            "The answer ended without a finish reason.".into(),
        ));
    };
    Ok((
        ParsedCompletionChoice {
            content,
            reasoning_content,
            tool_calls,
            finish_reason,
        },
        usage,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use psionic_models::deepseek_v41::deepseek_recipe::stream::{
        InferenceChunk, InferenceFinishReason, StreamProcessor,
    };
    use psionic_models::deepseek_v41::{
        DEEPSEEK_V41_CAPABILITIES, DeepSeekV41ToolCalling, fixture,
    };
    use tokio_stream::StreamExt;

    const BUDGET: DeepSeekV41RuntimeBudget = DeepSeekV41RuntimeBudget {
        context_tokens: 8192,
        max_output_tokens: 2048,
    };

    fn options(thinking: bool, tools: bool) -> ParsingOptions {
        let mut conversation = Conversation {
            thinking_mode: thinking,
            ..Conversation::default()
        };
        if tools {
            conversation.tools.push(ToolDefinition {
                name: "search".into(),
                description: None,
                parameters: serde_json::json!({"type": "object"}),
                strict: None,
                defer_loading: false,
            });
        }
        deepseek_v41_parsing_options(&conversation, Vec::new())
    }

    /// Loads the reduced fixture tokenizer once for the many chunkings.
    fn fixture_protocol() -> &'static DeepSeekV41Protocol {
        static PROTOCOL: std::sync::OnceLock<DeepSeekV41Protocol> = std::sync::OnceLock::new();
        PROTOCOL.get_or_init(fixture::protocol)
    }

    /// Streams `ids` from a mock backend in chunks of `chunk_size` tokens.
    async fn run(
        ids: &[u32],
        chunk_size: usize,
        finish: Option<InferenceFinishReason>,
        options: ParsingOptions,
        mode: PsionicReasoningMode,
    ) -> Vec<DeepSeekV41StreamEvent> {
        let chunks = ids.chunks(chunk_size).map(<[u32]>::to_vec).collect();
        run_chunks(chunks, finish, options, mode).await
    }

    /// Streams the given backend outputs from a mock backend.
    async fn run_chunks(
        chunks: Vec<Vec<u32>>,
        finish: Option<InferenceFinishReason>,
        options: ParsingOptions,
        mode: PsionicReasoningMode,
    ) -> Vec<DeepSeekV41StreamEvent> {
        let mut backend = vec![InferenceChunk::Ready {
            system_fingerprint: None,
            prompt_usage: PromptUsage {
                prompt_tokens: 7,
                prompt_cache_hit_tokens: 0,
            },
        }];
        backend.extend(chunks.into_iter().map(|token_ids| InferenceChunk::Tokens {
            token_ids,
            logprobs: None,
        }));
        backend.extend(finish.map(|finish_reason| InferenceChunk::Finish { finish_reason }));
        let generator = DeepSeekV41ChunkGenerator::new("req-1", "deepseek-v4.1-flash", 1, mode);
        StreamProcessor::new(generator, options)
            .with_tokenizer(fixture_protocol().tokenizer())
            .into_protocol_stream(tokio_stream::iter(backend))
            .map(|event| event.expect("the fixture tokenizer decodes every case"))
            .collect()
            .await
    }

    /// Drops the end-of-sentence token, which a backend reports as a finish.
    fn without_eos(mut ids: Vec<u32>) -> Vec<u32> {
        assert_eq!(ids.pop(), Some(1));
        ids
    }

    fn chunks(events: &[DeepSeekV41StreamEvent]) -> Vec<&ChatCompletionChunkDelta> {
        events
            .iter()
            .filter_map(|event| match event {
                DeepSeekV41StreamEvent::Chunk(chunk) => Some(&chunk.choices[0].delta),
                _ => None,
            })
            .collect()
    }

    #[tokio::test]
    async fn reasoning_then_final_answer_keeps_answer_whitespace()
    -> Result<(), Box<dyn std::error::Error>> {
        let ids = without_eos(fixture::stream_case("reasoning_then_final"));
        for chunk_size in [1, 3, ids.len()] {
            let events = run(
                &ids,
                chunk_size,
                Some(InferenceFinishReason::Stop),
                options(true, false),
                PsionicReasoningMode::Separate,
            )
            .await;
            let (choice, usage) =
                collect_deepseek_v41_choice(&events).map_err(|error| error.to_string())?;
            assert_eq!(
                choice.reasoning_content.as_deref(),
                Some("One plus one is two.")
            );
            assert_eq!(choice.content.as_deref(), Some("\n\nThe answer is 2.\n"));
            assert_eq!(choice.finish_reason, "stop");
            assert_eq!(
                (usage.prompt_tokens, usage.completion_tokens),
                (7, ids.len())
            );
            assert_eq!(usage.total_tokens, 7 + ids.len());
        }
        let suppressed = run(
            &ids,
            2,
            Some(InferenceFinishReason::Stop),
            options(true, false),
            PsionicReasoningMode::Suppress,
        )
        .await;
        let (choice, _) =
            collect_deepseek_v41_choice(&suppressed).map_err(|error| error.to_string())?;
        assert_eq!(choice.reasoning_content, None);
        assert_eq!(choice.content.as_deref(), Some("\n\nThe answer is 2.\n"));
        Ok(())
    }

    #[tokio::test]
    async fn long_tool_arguments_arrive_incrementally_across_chunks()
    -> Result<(), Box<dyn std::error::Error>> {
        let ids = without_eos(fixture::stream_case("tool_call_long_arguments"));
        let events = run(
            &ids,
            2,
            Some(InferenceFinishReason::Stop),
            options(true, true),
            PsionicReasoningMode::Separate,
        )
        .await;
        let argument_deltas = chunks(&events)
            .iter()
            .flat_map(|delta| delta.tool_calls.iter().flatten())
            .filter(|call| call.index == 0 && call.id.is_none())
            .count();
        assert!(argument_deltas > 10, "only {argument_deltas} deltas");
        let (choice, _) =
            collect_deepseek_v41_choice(&events).map_err(|error| error.to_string())?;
        assert_eq!(choice.finish_reason, "tool_calls");
        assert_eq!(
            choice.reasoning_content.as_deref(),
            Some("I should search.")
        );
        assert_eq!(choice.content, None);
        assert_eq!(choice.tool_calls.len(), 2);
        assert_eq!(choice.tool_calls[0].id, "req-1-tool-0");
        assert_eq!(choice.tool_calls[0].function.name, "search");
        let arguments: serde_json::Value =
            serde_json::from_str(&choice.tool_calls[0].function.arguments)?;
        let golden = fixture::golden();
        let query = golden
            .stream_cases
            .iter()
            .find(|case| case.name == "tool_call_long_arguments")
            .and_then(|case| {
                case.pieces.iter().find_map(|(_, text)| {
                    text.strip_prefix(" parameter name=\"query\" string=\"true\">")
                        .and_then(|rest| rest.strip_suffix("</"))
                })
            })
            .ok_or("missing query fixture")?;
        assert!(query.len() > 400);
        assert_eq!(arguments, serde_json::json!({"query": query, "limit": 5}));
        assert_eq!(choice.tool_calls[1].function.name, "get_weather");
        assert_eq!(
            choice.tool_calls[1].function.arguments,
            r#"{"location": "Paris"}"#
        );
        Ok(())
    }

    #[tokio::test]
    async fn literal_marker_spellings_in_text_stay_content()
    -> Result<(), Box<dyn std::error::Error>> {
        let ids = without_eos(fixture::stream_case("literal_markers_in_text"));
        let events = run(
            &ids,
            1,
            Some(InferenceFinishReason::Stop),
            options(false, true),
            PsionicReasoningMode::Separate,
        )
        .await;
        let (choice, _) =
            collect_deepseek_v41_choice(&events).map_err(|error| error.to_string())?;
        assert_eq!(
            choice.content.as_deref(),
            Some("Write </think> and <｜DSML｜ calls> as text.")
        );
        assert!(choice.tool_calls.is_empty() && choice.reasoning_content.is_none());
        assert_eq!(choice.finish_reason, "stop");
        Ok(())
    }

    #[tokio::test]
    async fn split_utf8_is_held_until_the_character_completes()
    -> Result<(), Box<dyn std::error::Error>> {
        let ids = without_eos(fixture::stream_case("split_utf8"));
        let events = run(
            &ids,
            1,
            Some(InferenceFinishReason::Stop),
            options(false, false),
            PsionicReasoningMode::Separate,
        )
        .await;
        for delta in chunks(&events) {
            assert!(!delta.content.as_deref().unwrap_or("").contains('\u{FFFD}'));
        }
        let (choice, _) =
            collect_deepseek_v41_choice(&events).map_err(|error| error.to_string())?;
        assert_eq!(choice.content.as_deref(), Some("Crab: 🦀 and 𝔘."));
        Ok(())
    }

    #[tokio::test]
    async fn truncation_inside_reasoning_never_becomes_the_answer()
    -> Result<(), Box<dyn std::error::Error>> {
        let ids = fixture::stream_case("truncated_reasoning");
        let events = run(
            &ids,
            1,
            Some(InferenceFinishReason::Length),
            options(true, false),
            PsionicReasoningMode::Separate,
        )
        .await;
        let (choice, usage) =
            collect_deepseek_v41_choice(&events).map_err(|error| error.to_string())?;
        assert_eq!(choice.content, None);
        assert_eq!(
            choice.reasoning_content.as_deref(),
            Some("Let me think about the first step and")
        );
        assert_eq!(choice.finish_reason, "length");
        assert_eq!(usage.completion_tokens, ids.len());
        Ok(())
    }

    #[tokio::test]
    async fn a_stream_without_a_finish_event_is_incomplete() {
        let ids = fixture::stream_case("no_finish");
        let events = run(
            &ids,
            1,
            None,
            options(false, false),
            PsionicReasoningMode::Separate,
        )
        .await;
        assert!(matches!(
            events.last(),
            Some(DeepSeekV41StreamEvent::Incomplete(usage)) if usage.completion_tokens == ids.len()
        ));
        let content: String = chunks(&events)
            .iter()
            .filter_map(|delta| delta.content.as_deref())
            .collect();
        assert_eq!(content, "\n\nPartial answer");
        assert!(events.iter().all(|event| !matches!(
            event,
            DeepSeekV41StreamEvent::Chunk(chunk) if chunk.choices[0].finish_reason.is_some()
        )));
        assert!(collect_deepseek_v41_choice(&events).is_err());
    }

    /// Everything a client sees from one stream, for comparing chunkings.
    fn summary(events: &[DeepSeekV41StreamEvent]) -> String {
        let terminal = match events.last() {
            Some(DeepSeekV41StreamEvent::Usage(usage)) => {
                format!("usage {}", usage.completion_tokens)
            }
            Some(DeepSeekV41StreamEvent::Incomplete(usage)) => {
                format!("incomplete {}", usage.completion_tokens)
            }
            _ => "missing".into(),
        };
        let delta = |pick: fn(&ChatCompletionChunkDelta) -> Option<&String>| -> String {
            chunks(events)
                .into_iter()
                .filter_map(pick)
                .map(String::as_str)
                .collect()
        };
        let mut calls: Vec<(String, String)> = Vec::new();
        let mut finish = None;
        for event in events {
            if let DeepSeekV41StreamEvent::Chunk(chunk) = event {
                finish = chunk.choices[0].finish_reason.or(finish);
                for call in chunk.choices[0].delta.tool_calls.iter().flatten() {
                    let function = call.function.as_ref();
                    if call.id.is_some() {
                        calls.push((
                            function.and_then(|f| f.name.clone()).unwrap_or_default(),
                            String::new(),
                        ));
                    }
                    if let Some(arguments) = function.and_then(|f| f.arguments.as_ref()) {
                        calls[call.index].1.push_str(arguments);
                    }
                }
            }
        }
        format!(
            "content={:?} reasoning={:?} calls={calls:?} finish={finish:?} {terminal}",
            delta(|d| d.content.as_ref()),
            delta(|d| d.reasoning_content.as_ref()),
        )
    }

    #[tokio::test]
    async fn output_does_not_depend_on_where_the_backend_splits_tokens() {
        let cases = [
            (
                "reasoning_then_final",
                true,
                false,
                InferenceFinishReason::Stop,
            ),
            (
                "tool_call_long_arguments",
                true,
                true,
                InferenceFinishReason::Stop,
            ),
            (
                "literal_markers_in_text",
                false,
                true,
                InferenceFinishReason::Stop,
            ),
            ("split_utf8", false, false, InferenceFinishReason::Stop),
            (
                "truncated_reasoning",
                true,
                false,
                InferenceFinishReason::Length,
            ),
        ];
        for (name, thinking, tools, finish) in cases {
            let mut ids = fixture::stream_case(name);
            if ids.last() == Some(&1) {
                ids.pop();
            }
            let whole = summary(
                &run(
                    &ids,
                    ids.len(),
                    Some(finish),
                    options(thinking, tools),
                    PsionicReasoningMode::Separate,
                )
                .await,
            );
            let mut chunkings: Vec<Vec<Vec<u32>>> = (1..=ids.len())
                .map(|size| ids.chunks(size).map(<[u32]>::to_vec).collect())
                .collect();
            // Every single split point, and every pair of split points for
            // shorter cases.
            for first in 1..ids.len() {
                chunkings.push(vec![ids[..first].to_vec(), ids[first..].to_vec()]);
                if ids.len() <= 40 {
                    for second in first + 1..ids.len() {
                        chunkings.push(vec![
                            ids[..first].to_vec(),
                            ids[first..second].to_vec(),
                            ids[second..].to_vec(),
                        ]);
                    }
                }
            }
            for chunking in chunkings {
                let split = summary(
                    &run_chunks(
                        chunking.clone(),
                        Some(finish),
                        options(thinking, tools),
                        PsionicReasoningMode::Separate,
                    )
                    .await,
                );
                assert_eq!(split, whole, "{name} differs when split as {chunking:?}");
            }
        }
    }

    #[tokio::test]
    async fn streams_cut_off_mid_output_never_complete_an_answer() {
        let tool_ids = without_eos(fixture::stream_case("tool_call_long_arguments"));
        let utf8_ids = without_eos(fixture::stream_case("split_utf8"));
        let crab = utf8_ids
            .iter()
            .position(|&id| {
                fixture::protocol()
                    .tokenizer()
                    .decode(&[id], false)
                    .is_ok_and(|text| text.contains('\u{FFFD}'))
            })
            .expect("the split UTF-8 case splits a character");
        let reasoning_ids = without_eos(fixture::stream_case("reasoning_then_final"));
        let cuts = [
            ("mid-reasoning", reasoning_ids[..3].to_vec(), true, false),
            (
                "mid-tool-arguments",
                tool_ids[..tool_ids.len() / 2].to_vec(),
                true,
                true,
            ),
            ("mid-UTF-8", utf8_ids[..=crab].to_vec(), false, false),
        ];
        for (name, ids, thinking, tools) in cuts {
            // A dropped connection: the backend never sends a finish event.
            let events = run(
                &ids,
                1,
                None,
                options(thinking, tools),
                PsionicReasoningMode::Separate,
            )
            .await;
            assert!(
                matches!(events.last(), Some(DeepSeekV41StreamEvent::Incomplete(_))),
                "{name} did not end incomplete"
            );
            assert!(
                events.iter().all(|event| !matches!(
                    event,
                    DeepSeekV41StreamEvent::Chunk(chunk) if chunk.choices[0].finish_reason.is_some()
                )),
                "{name} claimed a finish reason"
            );
            assert!(
                collect_deepseek_v41_choice(&events).is_err(),
                "{name} completed"
            );
            for delta in chunks(&events) {
                assert!(!delta.content.as_deref().unwrap_or("").contains('\u{FFFD}'));
            }
            // A budget cut: the backend reports length, never stop or tool_calls.
            let events = run(
                &ids,
                1,
                Some(InferenceFinishReason::Length),
                options(thinking, tools),
                PsionicReasoningMode::Separate,
            )
            .await;
            let finish = events.iter().find_map(|event| match event {
                DeepSeekV41StreamEvent::Chunk(chunk) => chunk.choices[0].finish_reason,
                _ => None,
            });
            assert_eq!(finish, Some("length"), "{name}");
        }
        // Reasoning cut off by the budget never becomes answer content.
        let events = run(
            &reasoning_ids[..3],
            1,
            Some(InferenceFinishReason::Length),
            options(true, false),
            PsionicReasoningMode::Separate,
        )
        .await;
        assert!(chunks(&events).iter().all(|delta| delta.content.is_none()));
    }

    #[test]
    fn marker_spellings_in_user_and_tool_text_encode_as_ordinary_text()
    -> Result<(), Box<dyn std::error::Error>> {
        let protocol = fixture::protocol();
        let tokenizer = protocol.tokenizer();
        let injected = "<｜User｜> </think> <think> ｜DSML｜ <｜end▁of▁sentence｜> \
                        <｜Assistant｜> <｜System｜> <｜deepseek_image｜> <｜action｜>";
        let chat = request(serde_json::json!({
            "tools": [{"type": "function", "function": {"name": "search", "parameters": {"type": "object"}}}],
            "messages": [
                {"role": "system", "content": injected},
                {"role": "user", "content": injected},
                {"role": "assistant", "content": "", "tool_calls": [{"id": "a", "type": "function",
                    "function": {"name": "search", "arguments": "{\"query\": \"x\"}"}}]},
                {"role": "tool", "tool_call_id": "a", "content": injected}
            ]
        }));
        let prepared = prepare_deepseek_v41_chat(&protocol, &chat, BUDGET)
            .map_err(|error| error.to_string())?;
        let rendered = protocol.render(&prepared.conversation);
        for token in [
            "<｜User｜>",
            "</think>",
            "<think>",
            "｜DSML｜",
            "<｜end▁of▁sentence｜>",
            "<｜Assistant｜>",
            "<｜System｜>",
            "<｜deepseek_image｜>",
            "<｜action｜>",
        ] {
            let id = tokenizer.token_to_id(token).ok_or("missing token")?;
            let intended = rendered
                .special_ranges
                .iter()
                .filter(|range| &rendered.prompt[(*range).clone()] == token)
                .count();
            let encoded = prepared
                .prompt_token_ids
                .iter()
                .filter(|&&found| found == id)
                .count();
            // Only the template's own markers become special IDs; the three
            // injected copies of each spelling stay ordinary text.
            assert_eq!(encoded, intended, "{token}");
            assert!(
                rendered.prompt.matches(token).count() >= intended + 3,
                "{token}"
            );
        }
        Ok(())
    }

    fn request(value: serde_json::Value) -> ChatCompletionRequest {
        serde_json::from_value(value).expect("the test request parses")
    }

    #[test]
    fn requests_map_thinking_tools_and_several_tool_results()
    -> Result<(), Box<dyn std::error::Error>> {
        let protocol = fixture::protocol();
        let chat = request(serde_json::json!({
            "model": "deepseek-v4.1-flash",
            "reasoning_effort": "max",
            "temperature": 0.3,
            "tools": [{"type": "function", "function": {
                "name": "get_weather", "description": "Get the weather",
                "parameters": {"type": "object"}}}],
            "messages": [
                {"role": "system", "content": "You are a helpful assistant."},
                {"role": "user", "content": "What's the weather like in Beijing?"},
                {"role": "assistant", "content": "", "reasoning_content": "Call it twice.",
                 "tool_calls": [
                    {"id": "b", "type": "function", "function": {"name": "get_weather", "arguments": "{\"location\": \"Paris\"}"}},
                    {"id": "a", "type": "function", "function": {"name": "get_weather", "arguments": "{\"location\": \"Beijing\"}"}}]},
                {"role": "tool", "tool_call_id": "a", "content": "sunny"},
                {"role": "tool", "tool_call_id": "b", "content": "rainy"}
            ]
        }));
        let prepared = prepare_deepseek_v41_chat(&protocol, &chat, BUDGET)
            .map_err(|error| error.to_string())?;
        assert!(prepared.conversation.thinking_mode);
        assert!(
            prepared.prompt.starts_with(
                "<｜begin▁of▁sentence｜><｜System｜>Reasoning Effort: 100 (range 1-100"
            )
        );
        // Results follow the order of the calls, not of the messages.
        assert!(prepared.prompt.ends_with(
            "<｜User｜><tool_result>rainy</tool_result>\n\n<tool_result>sunny</tool_result><｜Assistant｜><think>"
        ));
        assert!(prepared.prompt.contains("<think>Call it twice.</think>"));
        assert_eq!(
            prepared.prompt_token_ids.len(),
            protocol.encode(&prepared.conversation)?.len()
        );
        assert_eq!(
            (prepared.generation.temperature, prepared.generation.top_p),
            (1.0, 1.0)
        );
        assert!(matches!(
            prepared.generation.output_constraint,
            Some(OutputConstraint::ToolCalls {
                required: false,
                ..
            })
        ));
        assert!(prepared.parsing_options.parse_tool_calls);
        assert_eq!(prepared.reasoning_mode, PsionicReasoningMode::Separate);
        assert!(prepared.generation.max_tokens <= BUDGET.max_output_tokens);
        Ok(())
    }

    #[test]
    fn images_strict_constraints_and_named_tools_are_refused() {
        let protocol = fixture::protocol();
        let user = serde_json::json!({"role": "user", "content": "Hello"});
        for (extra, needle) in [
            (
                serde_json::json!({"messages": [{"role": "user", "content": [
                    {"type": "text", "text": "Hello"},
                    {"type": "image_url", "image_url": {"url": "https://example.com/a.png"}}]}]}),
                "images",
            ),
            (
                serde_json::json!({"response_format": {"type": "json_object"}}),
                "JSON output",
            ),
            (
                serde_json::json!({"response_format": {"type": "json_schema",
                    "json_schema": {"name": "a", "schema": {"type": "object"}, "strict": true}}}),
                "JSON schema",
            ),
            (
                serde_json::json!({"tool_choice": "required", "tools": [{"type": "function",
                    "function": {"name": "f", "parameters": {"type": "object"}}}]}),
                "required tool call",
            ),
            (
                serde_json::json!({"tool_choice": {"type": "function", "function": {"name": "f"}},
                    "tools": [{"type": "function", "function": {"name": "f"}}]}),
                "named tool call",
            ),
            (
                serde_json::json!({"psionic_reasoning": {"parser": "deepseek_r1"}}),
                "reasoning parser",
            ),
            (serde_json::json!({"max_tokens": 4096}), "max_tokens"),
        ] {
            let mut value = serde_json::json!({"messages": [user.clone()]});
            for (key, field) in extra.as_object().into_iter().flatten() {
                value[key] = field.clone();
            }
            let error = prepare_deepseek_v41_chat(&protocol, &request(value), BUDGET)
                .err()
                .map(|error| error.to_string())
                .unwrap_or_default();
            assert!(error.contains(needle), "expected {needle:?} in {error:?}");
        }
    }

    #[test]
    fn capability_flags_are_protocol_only() {
        let capabilities = serde_json::to_value(DEEPSEEK_V41_CAPABILITIES).unwrap_or_default();
        assert_eq!(capabilities["structured_outputs"], false);
        assert_eq!(capabilities["vision"], false);
        assert_eq!(capabilities["native_inference"], false);
        assert_eq!(capabilities["tool_calling"], "protocol_only");
        assert_eq!(
            DEEPSEEK_V41_CAPABILITIES.tool_calling,
            DeepSeekV41ToolCalling::ProtocolOnly
        );
    }
}
