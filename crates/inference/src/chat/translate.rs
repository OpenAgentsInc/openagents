//! Request and response translation between Chat Completions and Open
//! Responses. The crate README has the mapping table.

use crate::chat::types::{
    AssistantMessage, ChatContent, ChatFunction, ChatFunctionCall, ChatJsonSchema, ChatMessage,
    ChatPart, ChatRequest, ChatStreamOptions, ChatTool, ChatToolCall, ChatToolChoice, ChatUsage,
    Choice, Completion, CompletionTokensDetails, FileRef, FinishReason, ImageUrl,
    PromptTokensDetails, ResponseFormat,
};
use crate::error::{ApiError, ErrorType};
use crate::item::{
    ContentPart, FunctionCall, FunctionCallOutput, InputFile, InputImage, Item, ItemStatus,
    Message, MessageContent, OutputText, Reasoning, Refusal, Role, TextPart, ToolOutput,
};
use crate::request::{
    CreateResponse, Empty, FunctionTool, Include, Input, JsonSchemaFormat, ReasoningConfig,
    TextConfig, TextFormat, Tool, ToolChoice,
};
use crate::response::{IncompleteDetails, IncompleteReason, Response, ResponseStatus, Usage};
use crate::wire::Extra;

/// How a Chat Completions caller shaped its request, beyond what the
/// internal request holds: what the reply must look like for it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChatShape {
    /// `stream_options.include_usage`: send a usage chunk before `[DONE]`.
    pub include_usage: bool,
    /// The caller sent `max_tokens` rather than `max_completion_tokens`.
    pub legacy_max_tokens: bool,
    /// `stream_options` fields other than `include_usage`.
    pub stream_options_extra: Extra,
}

/// Translates a Chat Completions request onto the internal (Open
/// Responses) request. Refuses what has no equivalent: `n > 1`, audio
/// input, uploaded file ids, and the legacy `function` role.
pub fn to_responses_request(chat: ChatRequest) -> Result<(CreateResponse, ChatShape), ApiError> {
    if chat.n.is_some_and(|n| n > 1) {
        return Err(ApiError::invalid_request(
            "n",
            "Only one choice per request is supported. Send `n: 1` or leave it out.",
        ));
    }
    let shape = ChatShape {
        include_usage: chat
            .stream_options
            .as_ref()
            .and_then(|options| options.include_usage)
            .unwrap_or(false),
        legacy_max_tokens: chat.max_tokens.is_some() && chat.max_completion_tokens.is_none(),
        stream_options_extra: chat
            .stream_options
            .as_ref()
            .map(|options| options.extra.clone())
            .unwrap_or_default(),
    };

    let mut instructions = None;
    let mut items = Vec::new();
    for (index, message) in chat.messages.into_iter().enumerate() {
        let at = |field: &str| format!("messages[{index}].{field}");
        match message {
            ChatMessage::System { content, .. }
                if instructions.is_none()
                    && items.is_empty()
                    && matches!(content, ChatContent::Text(_)) =>
            {
                if let ChatContent::Text(text) = content {
                    instructions = Some(text);
                }
            }
            ChatMessage::System { content, .. } => {
                items.push(message_item(
                    Role::System,
                    input_content(content, &at("content"))?,
                ));
            }
            ChatMessage::Developer { content, .. } => {
                items.push(message_item(
                    Role::Developer,
                    input_content(content, &at("content"))?,
                ));
            }
            ChatMessage::User { content, .. } => {
                items.push(message_item(
                    Role::User,
                    input_content(content, &at("content"))?,
                ));
            }
            ChatMessage::Assistant {
                content,
                refusal,
                tool_calls,
                reasoning,
                ..
            } => {
                if let Some(text) = reasoning {
                    items.push(Item::Reasoning(Reasoning {
                        summary: vec![ContentPart::SummaryText(TextPart::new(text))],
                        ..Reasoning::default()
                    }));
                }
                let content = match (content, refusal) {
                    (None, None) => None,
                    (Some(ChatContent::Text(text)), None) => Some(MessageContent::Text(text)),
                    (content, refusal) => {
                        let mut parts = match content {
                            None => Vec::new(),
                            Some(ChatContent::Text(text)) => {
                                vec![ContentPart::OutputText(OutputText::new(text))]
                            }
                            Some(ChatContent::Parts(parts)) => parts
                                .into_iter()
                                .enumerate()
                                .map(|(part_index, part)| match part {
                                    ChatPart::Text { text } => {
                                        Ok(ContentPart::OutputText(OutputText::new(text)))
                                    }
                                    ChatPart::Refusal { refusal } => {
                                        Ok(ContentPart::Refusal(Refusal {
                                            refusal,
                                            extra: Extra::new(),
                                        }))
                                    }
                                    _ => Err(ApiError::invalid_request(
                                        at(&format!("content[{part_index}]")),
                                        "Assistant messages may hold only text and refusal parts.",
                                    )),
                                })
                                .collect::<Result<_, _>>()?,
                        };
                        if let Some(refusal) = refusal {
                            parts.push(ContentPart::Refusal(Refusal {
                                refusal,
                                extra: Extra::new(),
                            }));
                        }
                        Some(MessageContent::Parts(parts))
                    }
                };
                if let Some(content) = content {
                    items.push(message_item(Role::Assistant, content));
                }
                for call in tool_calls.unwrap_or_default() {
                    items.push(Item::FunctionCall(FunctionCall {
                        id: None,
                        status: None,
                        call_id: call.id,
                        name: call.function.name,
                        arguments: call.function.arguments,
                        extra: Extra::new(),
                    }));
                }
            }
            ChatMessage::Tool {
                content,
                tool_call_id,
            } => {
                let output = match content {
                    ChatContent::Text(text) => ToolOutput::Text(text),
                    ChatContent::Parts(parts) => {
                        match input_content(ChatContent::Parts(parts), &at("content"))? {
                            MessageContent::Parts(parts) => ToolOutput::Parts(parts),
                            MessageContent::Text(text) => ToolOutput::Text(text),
                        }
                    }
                };
                items.push(Item::FunctionCallOutput(FunctionCallOutput {
                    id: None,
                    status: None,
                    call_id: tool_call_id,
                    output,
                    extra: Extra::new(),
                }));
            }
        }
    }

    let tools = chat
        .tools
        .map(|tools| {
            tools
                .into_iter()
                .enumerate()
                .map(|(index, tool)| {
                    if tool.kind != "function" {
                        return Err(ApiError::invalid_request(
                            format!("tools[{index}].type"),
                            "Only function tools are supported.",
                        ));
                    }
                    Ok(Tool::Function(FunctionTool {
                        name: tool.function.name,
                        description: tool.function.description,
                        parameters: tool.function.parameters,
                        strict: tool.function.strict,
                        extra: Extra::new(),
                    }))
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;

    let format = chat.response_format.map(|format| match format {
        ResponseFormat::Text => TextFormat::Text(Empty::default()),
        ResponseFormat::JsonObject => TextFormat::JsonObject(Empty::default()),
        ResponseFormat::JsonSchema { json_schema } => TextFormat::JsonSchema(JsonSchemaFormat {
            name: json_schema.name,
            description: json_schema.description,
            schema: json_schema.schema.unwrap_or(serde_json::Value::Null),
            strict: json_schema.strict,
            extra: Extra::new(),
        }),
    });
    let text = (format.is_some() || chat.verbosity.is_some()).then(|| TextConfig {
        format,
        verbosity: chat.verbosity,
        extra: Extra::new(),
    });

    let mut include = Vec::new();
    if chat.logprobs == Some(true) {
        include.push(Include::OutputTextLogprobs);
    }

    let request = CreateResponse {
        model: chat.model,
        input: Some(Input::Items(items)),
        instructions,
        include,
        tools,
        tool_choice: chat.tool_choice.map(|choice| match choice {
            ChatToolChoice::Mode(mode) => ToolChoice::Mode(mode),
            ChatToolChoice::Function(name) => ToolChoice::Function { name },
            ChatToolChoice::AllowedTools { mode, tools } => {
                ToolChoice::AllowedTools { mode, tools }
            }
        }),
        parallel_tool_calls: chat.parallel_tool_calls,
        metadata: chat.metadata,
        text,
        temperature: chat.temperature,
        top_p: chat.top_p,
        presence_penalty: chat.presence_penalty,
        frequency_penalty: chat.frequency_penalty,
        top_logprobs: chat.top_logprobs,
        max_output_tokens: chat.max_completion_tokens.or(chat.max_tokens),
        reasoning: chat.reasoning_effort.map(|effort| ReasoningConfig {
            effort: Some(effort),
            ..ReasoningConfig::default()
        }),
        stream: chat.stream,
        store: chat.store,
        service_tier: chat.service_tier,
        safety_identifier: chat.safety_identifier,
        prompt_cache_key: chat.prompt_cache_key,
        stop: chat.stop,
        seed: chat.seed,
        user: chat.user,
        openagents: chat.openagents,
        extra: chat.extra,
        ..CreateResponse::default()
    };
    Ok((request, shape))
}

fn message_item(role: Role, content: MessageContent) -> Item {
    Item::Message(Message {
        id: None,
        status: None,
        role,
        content,
        phase: None,
        extra: Extra::new(),
    })
}

fn input_content(content: ChatContent, at: &str) -> Result<MessageContent, ApiError> {
    let parts = match content {
        ChatContent::Text(text) => return Ok(MessageContent::Text(text)),
        ChatContent::Parts(parts) => parts,
    };
    parts
        .into_iter()
        .enumerate()
        .map(|(index, part)| match part {
            ChatPart::Text { text } => Ok(ContentPart::InputText(TextPart::new(text))),
            ChatPart::ImageUrl { image_url } => Ok(ContentPart::InputImage(InputImage {
                image_url: Some(image_url.url),
                detail: image_url.detail,
                extra: Extra::new(),
            })),
            ChatPart::File { file } => {
                if file.file_id.is_some() {
                    return Err(ApiError::invalid_request(
                        format!("{at}[{index}].file.file_id"),
                        "Uploaded file ids are not supported. Send the file inline as `file_data`.",
                    ));
                }
                Ok(ContentPart::InputFile(InputFile {
                    filename: file.filename,
                    file_data: file.file_data,
                    file_url: None,
                    extra: Extra::new(),
                }))
            }
            ChatPart::InputAudio { .. } => Err(ApiError::invalid_request(
                format!("{at}[{index}]"),
                "Audio input is not supported.",
            )),
            ChatPart::Refusal { .. } => Err(ApiError::invalid_request(
                format!("{at}[{index}]"),
                "Only assistant messages may hold refusals.",
            )),
        })
        .collect::<Result<_, _>>()
        .map(MessageContent::Parts)
}

/// Translates the internal request into a Chat Completions request, for
/// an upstream that speaks only Chat Completions. Refuses what Chat
/// Completions cannot carry: `previous_response_id`, item references,
/// compaction items, hosted tools, file URLs, and video.
pub fn from_responses_request(
    request: &CreateResponse,
    shape: &ChatShape,
) -> Result<ChatRequest, ApiError> {
    if request.previous_response_id.is_some() {
        return Err(ApiError::invalid_request(
            "previous_response_id",
            "Chat Completions has no stored responses. Send the whole conversation.",
        ));
    }
    let mut messages = Vec::new();
    if let Some(instructions) = &request.instructions {
        messages.push(ChatMessage::System {
            content: ChatContent::Text(instructions.clone()),
            name: None,
        });
    }
    let mut pending_reasoning: Option<String> = None;
    for (index, item) in request.input_items().into_iter().enumerate() {
        let at = |field: &str| format!("input[{index}]{field}");
        match item {
            Item::Message(message) => {
                if message.role == Role::Assistant {
                    let (content, refusal) = assistant_content(message.content);
                    messages.push(ChatMessage::Assistant {
                        content,
                        refusal,
                        tool_calls: None,
                        reasoning: pending_reasoning.take(),
                        name: None,
                    });
                    continue;
                }
                let content = chat_content(message.content, &at(".content"))?;
                messages.push(match message.role {
                    Role::System => ChatMessage::System {
                        content,
                        name: None,
                    },
                    Role::Developer => ChatMessage::Developer {
                        content,
                        name: None,
                    },
                    Role::User | Role::Assistant => ChatMessage::User {
                        content,
                        name: None,
                    },
                });
            }
            Item::FunctionCall(call) => {
                let tool_call = ChatToolCall {
                    id: call.call_id,
                    kind: "function".to_owned(),
                    function: ChatFunctionCall {
                        name: call.name,
                        arguments: call.arguments,
                    },
                };
                // Calls attach to the assistant message just before them;
                // with none there, they start one.
                if pending_reasoning.is_none()
                    && let Some(ChatMessage::Assistant { tool_calls, .. }) = messages.last_mut()
                {
                    tool_calls.get_or_insert_with(Vec::new).push(tool_call);
                } else {
                    messages.push(ChatMessage::Assistant {
                        content: None,
                        refusal: None,
                        tool_calls: Some(vec![tool_call]),
                        reasoning: pending_reasoning.take(),
                        name: None,
                    });
                }
            }
            Item::FunctionCallOutput(output) => {
                let content = match output.output {
                    ToolOutput::Text(text) => ChatContent::Text(text),
                    ToolOutput::Parts(parts) => {
                        chat_content(MessageContent::Parts(parts), &at(".output"))?
                    }
                };
                messages.push(ChatMessage::Tool {
                    content,
                    tool_call_id: output.call_id,
                });
            }
            Item::Reasoning(reasoning) => {
                // Summary text rides on the next assistant message;
                // encrypted reasoning has nowhere to go and is dropped.
                let text = match reasoning.summary_text() {
                    summary if !summary.is_empty() => summary,
                    _ => reasoning.content_text(),
                };
                if !text.is_empty() {
                    pending_reasoning = Some(match pending_reasoning.take() {
                        Some(before) => format!("{before}\n\n{text}"),
                        None => text,
                    });
                }
            }
            Item::ItemReference(_) | Item::Compaction(_) | Item::Unknown(_) => {
                return Err(ApiError::invalid_request(
                    at(""),
                    "This kind of input item cannot be sent to a Chat Completions model.",
                ));
            }
        }
    }
    if let Some(reasoning) = pending_reasoning {
        messages.push(ChatMessage::Assistant {
            content: None,
            refusal: None,
            tool_calls: None,
            reasoning: Some(reasoning),
            name: None,
        });
    }

    let tools = request
        .tools
        .as_ref()
        .map(|tools| {
            tools
                .iter()
                .enumerate()
                .map(|(index, tool)| match tool {
                    Tool::Function(function) => Ok(ChatTool {
                        kind: "function".to_owned(),
                        function: ChatFunction {
                            name: function.name.clone(),
                            description: function.description.clone(),
                            parameters: function.parameters.clone(),
                            strict: function.strict,
                        },
                    }),
                    Tool::Unknown(_) => Err(ApiError::invalid_request(
                        format!("tools[{index}]"),
                        "Hosted tools cannot be sent to a Chat Completions model.",
                    )),
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;

    let text = request.text.as_ref();
    let response_format = match text.and_then(|text| text.format.as_ref()) {
        None => None,
        Some(TextFormat::Text(_)) => Some(ResponseFormat::Text),
        Some(TextFormat::JsonObject(_)) => Some(ResponseFormat::JsonObject),
        Some(TextFormat::JsonSchema(schema)) => Some(ResponseFormat::JsonSchema {
            json_schema: ChatJsonSchema {
                name: schema.name.clone(),
                description: schema.description.clone(),
                schema: (!schema.schema.is_null()).then(|| schema.schema.clone()),
                strict: schema.strict,
            },
        }),
        Some(TextFormat::Unknown(_)) => {
            return Err(ApiError::invalid_request(
                "text.format",
                "Unknown output format.",
            ));
        }
    };

    let stream_options =
        (shape.include_usage || !shape.stream_options_extra.is_empty()).then(|| {
            ChatStreamOptions {
                include_usage: shape.include_usage.then_some(true),
                extra: shape.stream_options_extra.clone(),
            }
        });

    Ok(ChatRequest {
        model: request.model.clone(),
        messages,
        tools,
        tool_choice: request.tool_choice.clone().map(|choice| match choice {
            ToolChoice::Mode(mode) => ChatToolChoice::Mode(mode),
            ToolChoice::Function { name } => ChatToolChoice::Function(name),
            ToolChoice::AllowedTools { mode, tools } => {
                ChatToolChoice::AllowedTools { mode, tools }
            }
        }),
        parallel_tool_calls: request.parallel_tool_calls,
        response_format,
        verbosity: text.and_then(|text| text.verbosity),
        max_tokens: request
            .max_output_tokens
            .filter(|_| shape.legacy_max_tokens),
        max_completion_tokens: request
            .max_output_tokens
            .filter(|_| !shape.legacy_max_tokens),
        reasoning_effort: request
            .reasoning
            .as_ref()
            .and_then(|reasoning| reasoning.effort.clone()),
        temperature: request.temperature,
        top_p: request.top_p,
        presence_penalty: request.presence_penalty,
        frequency_penalty: request.frequency_penalty,
        stop: request.stop.clone(),
        seed: request.seed,
        user: request.user.clone(),
        stream: request.stream,
        stream_options,
        n: None,
        logprobs: request
            .include
            .contains(&Include::OutputTextLogprobs)
            .then_some(true),
        top_logprobs: request.top_logprobs,
        metadata: request.metadata.clone(),
        store: request.store,
        service_tier: request.service_tier.clone(),
        safety_identifier: request.safety_identifier.clone(),
        prompt_cache_key: request.prompt_cache_key.clone(),
        openagents: request.openagents.clone(),
        extra: request.extra.clone(),
    })
}

/// An assistant message's content as Chat Completions content and refusal.
fn assistant_content(content: MessageContent) -> (Option<ChatContent>, Option<String>) {
    let parts = match content {
        MessageContent::Text(text) => return (Some(ChatContent::Text(text)), None),
        MessageContent::Parts(parts) => parts,
    };
    let mut texts = Vec::new();
    let mut refusal: Option<String> = None;
    for part in parts {
        match part {
            ContentPart::Refusal(part) => refusal
                .get_or_insert_with(String::new)
                .push_str(&part.refusal),
            other => {
                if let Some(text) = other.text() {
                    texts.push(ChatPart::Text {
                        text: text.to_owned(),
                    });
                }
            }
        }
    }
    let content = (!texts.is_empty()).then_some(ChatContent::Parts(texts));
    (content, refusal)
}

fn chat_content(content: MessageContent, at: &str) -> Result<ChatContent, ApiError> {
    let parts = match content {
        MessageContent::Text(text) => return Ok(ChatContent::Text(text)),
        MessageContent::Parts(parts) => parts,
    };
    parts
        .into_iter()
        .enumerate()
        .map(|(index, part)| match part {
            ContentPart::InputText(part) | ContentPart::Text(part) => {
                Ok(ChatPart::Text { text: part.text })
            }
            ContentPart::OutputText(part) => Ok(ChatPart::Text { text: part.text }),
            ContentPart::InputImage(image) => match image.image_url {
                Some(url) => Ok(ChatPart::ImageUrl {
                    image_url: ImageUrl {
                        url,
                        detail: image.detail,
                    },
                }),
                None => Err(ApiError::invalid_request(
                    format!("{at}[{index}].image_url"),
                    "An image needs `image_url`.",
                )),
            },
            ContentPart::InputFile(file) if file.file_url.is_none() => Ok(ChatPart::File {
                file: FileRef {
                    file_data: file.file_data,
                    file_id: None,
                    filename: file.filename,
                },
            }),
            _ => Err(ApiError::invalid_request(
                format!("{at}[{index}]"),
                "This kind of content cannot be sent to a Chat Completions model.",
            )),
        })
        .collect::<Result<_, _>>()
        .map(ChatContent::Parts)
}

/// The finish reason Chat Completions reports for a response.
#[must_use]
pub fn finish_reason(response: &Response) -> Option<FinishReason> {
    match &response.status {
        ResponseStatus::Completed => Some(
            if response
                .output
                .iter()
                .any(|item| matches!(item, Item::FunctionCall(_)))
            {
                FinishReason::ToolCalls
            } else {
                FinishReason::Stop
            },
        ),
        ResponseStatus::Incomplete => Some(
            match response
                .incomplete_details
                .as_ref()
                .map(|details| &details.reason)
            {
                Some(IncompleteReason::ContentFilter) => FinishReason::ContentFilter,
                _ => FinishReason::Length,
            },
        ),
        ResponseStatus::Failed => Some(FinishReason::Error),
        _ => None,
    }
}

/// Usage in Chat Completions' shape.
#[must_use]
pub fn chat_usage(usage: &Usage) -> ChatUsage {
    ChatUsage {
        prompt_tokens: usage.input_tokens,
        completion_tokens: usage.output_tokens,
        total_tokens: usage.total_tokens,
        prompt_tokens_details: Some(PromptTokensDetails {
            cached_tokens: usage.input_tokens_details.cached_tokens,
            extra: Extra::new(),
        }),
        completion_tokens_details: Some(CompletionTokensDetails {
            reasoning_tokens: usage.output_tokens_details.reasoning_tokens,
            extra: Extra::new(),
        }),
        extra: usage.extra.clone(),
    }
}

/// Usage in Open Responses' shape.
#[must_use]
pub fn responses_usage(usage: &ChatUsage) -> Usage {
    let mut out = Usage::new(
        usage.prompt_tokens,
        usage
            .prompt_tokens_details
            .as_ref()
            .map_or(0, |details| details.cached_tokens),
        usage.completion_tokens,
        usage
            .completion_tokens_details
            .as_ref()
            .map_or(0, |details| details.reasoning_tokens),
    );
    out.total_tokens = usage.total_tokens;
    out.extra = usage.extra.clone();
    out
}

/// The reasoning text Chat Completions shows for a reasoning item: its
/// summary, or its raw text when it has no summary.
pub(crate) fn reasoning_text(reasoning: &Reasoning) -> String {
    match reasoning.summary_text() {
        summary if !summary.is_empty() => summary,
        _ => reasoning.content_text(),
    }
}

/// Translates a response into a Chat Completions reply: one choice whose
/// message flattens every output item, in order.
#[must_use]
pub fn completion_from_response(response: &Response) -> Completion {
    let mut message = AssistantMessage::default();
    let mut tool_calls = Vec::new();
    let mut reasoning = Vec::new();
    for item in &response.output {
        match item {
            Item::Message(item) if item.role == Role::Assistant => {
                let (content, refusal) = assistant_content(item.content.clone());
                if let Some(content) = content {
                    let text = match content {
                        ChatContent::Text(text) => text,
                        ChatContent::Parts(parts) => parts
                            .into_iter()
                            .filter_map(|part| match part {
                                ChatPart::Text { text } => Some(text),
                                _ => None,
                            })
                            .collect(),
                    };
                    message
                        .content
                        .get_or_insert_with(String::new)
                        .push_str(&text);
                }
                if let Some(refusal) = refusal {
                    message
                        .refusal
                        .get_or_insert_with(String::new)
                        .push_str(&refusal);
                }
            }
            Item::FunctionCall(call) => tool_calls.push(ChatToolCall {
                id: call.call_id.clone(),
                kind: "function".to_owned(),
                function: ChatFunctionCall {
                    name: call.name.clone(),
                    arguments: call.arguments.clone(),
                },
            }),
            Item::Reasoning(item) => {
                let text = reasoning_text(item);
                if !text.is_empty() {
                    reasoning.push(text);
                }
            }
            _ => {}
        }
    }
    if !tool_calls.is_empty() {
        message.tool_calls = Some(tool_calls);
    }
    if !reasoning.is_empty() {
        message.reasoning = Some(reasoning.join("\n\n"));
    }
    Completion {
        id: response.id.clone(),
        object: "chat.completion".to_owned(),
        created: response.created_at,
        model: response.model.clone(),
        choices: vec![Choice {
            index: 0,
            message,
            finish_reason: finish_reason(response),
            logprobs: None,
            extra: Extra::new(),
        }],
        usage: response.usage.as_ref().map(chat_usage),
        service_tier: Some(response.service_tier.clone()),
        system_fingerprint: None,
        openagents: response.openagents.clone(),
        extra: Extra::new(),
    }
}

/// Item ids for a response built from a Chat Completions reply, derived
/// from the reply's id so the same reply always gives the same ids.
pub(crate) fn derived_id(prefix: &str, completion_id: &str, index: usize) -> String {
    format!("{prefix}_{completion_id}_{index}")
}

/// Translates a Chat Completions reply (from an upstream that speaks only
/// Chat Completions) into a response to the internal request. Only the
/// first choice is read.
#[must_use]
pub fn response_from_completion(completion: &Completion, request: &CreateResponse) -> Response {
    let mut response = Response::from_request(
        completion.id.clone(),
        completion.created,
        completion.model.clone(),
        request,
    );
    let choice = completion.choices.first();
    let finish = choice.and_then(|choice| choice.finish_reason.clone());
    let (status, incomplete) = status_from_finish(finish.as_ref());
    if let Some(choice) = choice {
        let message = &choice.message;
        if let Some(reasoning) = message.reasoning.as_ref().filter(|text| !text.is_empty()) {
            response.output.push(Item::Reasoning(Reasoning {
                id: Some(derived_id("rs", &completion.id, response.output.len())),
                status: Some(ItemStatus::Completed),
                summary: vec![ContentPart::SummaryText(TextPart::new(reasoning.clone()))],
                ..Reasoning::default()
            }));
        }
        if message.content.is_some() || message.refusal.is_some() {
            let mut parts = Vec::new();
            if let Some(content) = &message.content {
                parts.push(ContentPart::OutputText(OutputText::new(content.clone())));
            }
            if let Some(refusal) = &message.refusal {
                parts.push(ContentPart::Refusal(Refusal {
                    refusal: refusal.clone(),
                    extra: Extra::new(),
                }));
            }
            response.output.push(Item::Message(Message {
                id: Some(derived_id("msg", &completion.id, response.output.len())),
                status: Some(ItemStatus::Completed),
                role: Role::Assistant,
                content: MessageContent::Parts(parts),
                phase: None,
                extra: Extra::new(),
            }));
        }
        for call in message.tool_calls.iter().flatten() {
            response.output.push(Item::FunctionCall(FunctionCall {
                id: Some(derived_id("fc", &completion.id, response.output.len())),
                status: Some(ItemStatus::Completed),
                call_id: call.id.clone(),
                name: call.function.name.clone(),
                arguments: call.function.arguments.clone(),
                extra: Extra::new(),
            }));
        }
    }
    finish_response(&mut response, status, incomplete, completion.created);
    response.usage = completion.usage.as_ref().map(responses_usage);
    if completion.openagents.is_some() {
        response.openagents = completion.openagents.clone();
    }
    response
}

/// The response status a Chat Completions finish reason means.
pub(crate) fn status_from_finish(
    finish: Option<&FinishReason>,
) -> (ResponseStatus, Option<IncompleteReason>) {
    match finish {
        Some(FinishReason::Length) => (
            ResponseStatus::Incomplete,
            Some(IncompleteReason::MaxOutputTokens),
        ),
        Some(FinishReason::ContentFilter) => (
            ResponseStatus::Incomplete,
            Some(IncompleteReason::ContentFilter),
        ),
        Some(FinishReason::Error) => (ResponseStatus::Failed, None),
        _ => (ResponseStatus::Completed, None),
    }
}

/// Sets a finished response's status, details, and the last item's
/// status (`incomplete` when the response is).
pub(crate) fn finish_response(
    response: &mut Response,
    status: ResponseStatus,
    incomplete: Option<IncompleteReason>,
    completed_at: u64,
) {
    if status == ResponseStatus::Incomplete
        && let Some(last) = response.output.last_mut()
    {
        last.set_status(ItemStatus::Incomplete);
    }
    if status == ResponseStatus::Failed && response.error.is_none() {
        response.error = Some(
            ApiError::new(ErrorType::ModelError, "The model failed while answering.")
                .response_error(),
        );
    }
    response.completed_at = (status == ResponseStatus::Completed).then_some(completed_at);
    response.incomplete_details = incomplete.map(IncompleteDetails::new);
    response.status = status;
}
