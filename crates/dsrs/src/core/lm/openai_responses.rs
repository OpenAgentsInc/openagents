use async_openai::{
    Client,
    config::OpenAIConfig,
    traits::EventType,
    types::responses::{
        CreateResponseArgs, CustomToolCall, EasyInputContent, EasyInputMessage, FunctionCallOutput,
        FunctionCallOutputItemParam, FunctionTool, FunctionToolCall, InputItem, InputParam, Item,
        MessageType, OutputItem, OutputMessageContent, OutputStatus, Response, ResponseStreamEvent,
        ResponseUsage, Role, Tool, ToolChoiceAllowed, ToolChoiceAllowedMode, ToolChoiceFunction,
        ToolChoiceOptions, ToolChoiceParam,
    },
};
use futures::StreamExt;
use rig::OneOrMany;
use rig::completion::{CompletionError, CompletionRequest, CompletionResponse, Usage};
use rig::message::{AssistantContent, Message as RigMessage, Text, ToolCall, ToolFunction};
use rig::message::{ToolResultContent, UserContent};
use serde_json::{Value, json};
use std::collections::HashSet;

#[derive(Clone)]
pub struct OpenAiResponsesCompletionModel {
    client: Client<OpenAIConfig>,
    model: String,
}

impl OpenAiResponsesCompletionModel {
    pub fn new(config: OpenAIConfig, model: impl Into<String>) -> Self {
        Self {
            client: Client::with_config(config),
            model: model.into(),
        }
    }

    pub fn from_env(
        model: impl Into<String>,
        api_key: Option<&str>,
        base_url: Option<&str>,
    ) -> Self {
        let mut config = OpenAIConfig::default();
        if let Some(key) = api_key {
            config = config.with_api_key(key);
        }
        if let Some(url) = base_url {
            config = config.with_api_base(url);
        }
        Self::new(config, model)
    }

    pub async fn completion_streaming(
        &self,
        request: CompletionRequest,
        on_token: Option<&(dyn Fn(&str) + Send + Sync)>,
    ) -> Result<CompletionResponse<()>, CompletionError> {
        let (response, _) = self
            .completion_streaming_with_usage_details(request, on_token)
            .await?;
        Ok(response)
    }

    pub async fn completion_streaming_with_usage_details(
        &self,
        request: CompletionRequest,
        on_token: Option<&(dyn Fn(&str) + Send + Sync)>,
    ) -> Result<(CompletionResponse<()>, Option<Value>), CompletionError> {
        let fallback_request = request.clone();
        let create_request = build_create_request(&self.model, &request, true)?;
        let mut stream = self
            .client
            .responses()
            .create_stream(create_request)
            .await
            .map_err(|e| CompletionError::ProviderError(e.to_string()))?;

        let mut final_response = None;
        let mut output_items = Vec::new();
        let mut output_item_ids = HashSet::new();
        let mut streamed_text = String::new();
        let mut saw_text_delta = false;
        let mut last_usage: Option<ResponseUsage> = None;
        let debug_events = debug_enabled();

        while let Some(result) = stream.next().await {
            match result {
                Ok(event) if debug_events => {
                    eprintln!("[openai-responses] event: {}", event.event_type());
                    match event {
                        ResponseStreamEvent::ResponseOutputTextDelta(delta) => {
                            saw_text_delta = true;
                            streamed_text.push_str(&delta.delta);
                            if let Some(cb) = on_token {
                                cb(&delta.delta);
                            }
                        }
                        ResponseStreamEvent::ResponseOutputTextDone(done) => {
                            if !done.text.is_empty() {
                                streamed_text = done.text;
                            }
                        }
                        ResponseStreamEvent::ResponseContentPartAdded(part) => {
                            if !saw_text_delta {
                                append_output_content(&mut streamed_text, &part.part);
                            }
                        }
                        ResponseStreamEvent::ResponseContentPartDone(part) => {
                            if !saw_text_delta {
                                append_output_content(&mut streamed_text, &part.part);
                            }
                        }
                        ResponseStreamEvent::ResponseCompleted(done) => {
                            last_usage = done.response.usage.clone();
                            final_response = Some(done.response);
                        }
                        ResponseStreamEvent::ResponseInProgress(progress) => {
                            last_usage = progress.response.usage.clone();
                        }
                        ResponseStreamEvent::ResponseCreated(created) => {
                            last_usage = created.response.usage.clone();
                        }
                        ResponseStreamEvent::ResponseIncomplete(incomplete) => {
                            if let Some(details) = &incomplete.response.incomplete_details {
                                eprintln!("[openai-responses] incomplete: {}", details.reason);
                            }
                            last_usage = incomplete.response.usage.clone();
                            final_response = Some(incomplete.response);
                        }
                        ResponseStreamEvent::ResponseOutputItemDone(done) => {
                            eprintln!(
                                "[openai-responses] output_item_done: {}",
                                output_item_kind(&done.item)
                            );
                            append_output_item_text(&mut streamed_text, &done.item);
                            push_output_item(&mut output_items, &mut output_item_ids, done.item);
                        }
                        ResponseStreamEvent::ResponseOutputItemAdded(added) => {
                            eprintln!(
                                "[openai-responses] output_item_added: {}",
                                output_item_kind(&added.item)
                            );
                            append_output_item_text(&mut streamed_text, &added.item);
                            push_output_item(&mut output_items, &mut output_item_ids, added.item);
                        }
                        ResponseStreamEvent::ResponseFailed(failed) => {
                            let message = failed
                                .response
                                .error
                                .as_ref()
                                .map(|err| err.message.clone())
                                .unwrap_or_else(|| "OpenAI response failed".to_string());
                            return Err(CompletionError::ProviderError(message));
                        }
                        ResponseStreamEvent::ResponseError(error) => {
                            return Err(CompletionError::ProviderError(error.message));
                        }
                        _ => {}
                    }
                }
                Ok(ResponseStreamEvent::ResponseOutputTextDelta(delta)) => {
                    saw_text_delta = true;
                    streamed_text.push_str(&delta.delta);
                    if let Some(cb) = on_token {
                        cb(&delta.delta);
                    }
                }
                Ok(ResponseStreamEvent::ResponseOutputTextDone(done)) => {
                    if !done.text.is_empty() {
                        streamed_text = done.text;
                    }
                }
                Ok(ResponseStreamEvent::ResponseContentPartAdded(part)) => {
                    if !saw_text_delta {
                        append_output_content(&mut streamed_text, &part.part);
                    }
                }
                Ok(ResponseStreamEvent::ResponseContentPartDone(part)) => {
                    if !saw_text_delta {
                        append_output_content(&mut streamed_text, &part.part);
                    }
                }
                Ok(ResponseStreamEvent::ResponseCompleted(done)) => {
                    last_usage = done.response.usage.clone();
                    final_response = Some(done.response);
                }
                Ok(ResponseStreamEvent::ResponseInProgress(progress)) => {
                    last_usage = progress.response.usage.clone();
                }
                Ok(ResponseStreamEvent::ResponseCreated(created)) => {
                    last_usage = created.response.usage.clone();
                }
                Ok(ResponseStreamEvent::ResponseIncomplete(incomplete)) => {
                    last_usage = incomplete.response.usage.clone();
                    final_response = Some(incomplete.response);
                }
                Ok(ResponseStreamEvent::ResponseOutputItemDone(done)) => {
                    append_output_item_text(&mut streamed_text, &done.item);
                    push_output_item(&mut output_items, &mut output_item_ids, done.item);
                }
                Ok(ResponseStreamEvent::ResponseOutputItemAdded(added)) => {
                    append_output_item_text(&mut streamed_text, &added.item);
                    push_output_item(&mut output_items, &mut output_item_ids, added.item);
                }
                Ok(ResponseStreamEvent::ResponseFailed(failed)) => {
                    let message = failed
                        .response
                        .error
                        .as_ref()
                        .map(|err| err.message.clone())
                        .unwrap_or_else(|| "OpenAI response failed".to_string());
                    return Err(CompletionError::ProviderError(message));
                }
                Ok(ResponseStreamEvent::ResponseError(error)) => {
                    return Err(CompletionError::ProviderError(error.message));
                }
                Ok(_) => {}
                Err(err) => {
                    return Err(CompletionError::ProviderError(err.to_string()));
                }
            }
        }

        let response = final_response.ok_or_else(|| {
            CompletionError::ResponseError("OpenAI stream ended without completion".to_string())
        });

        if let Ok(response) = response {
            let usage_details = response.usage.as_ref().and_then(usage_to_value);
            match response_to_completion(response) {
                Ok(result) => return Ok((result, usage_details)),
                Err(_err) if streamed_text.is_empty() && output_items.is_empty() => {
                    let (fallback, fallback_usage) =
                        self.completion_with_usage_details(fallback_request).await?;
                    if let Some(cb) = on_token {
                        emit_completion_tokens(&fallback, cb);
                    }
                    return Ok((fallback, fallback_usage));
                }
                Err(err) => return Err(err),
            }
        }

        let usage_details = last_usage.as_ref().and_then(usage_to_value);

        if !output_items.is_empty() {
            let completion = completion_from_output_and_usage(output_items, last_usage)?;
            return Ok((completion, usage_details));
        }

        if !streamed_text.is_empty() {
            let completion = completion_from_text(streamed_text, last_usage)?;
            return Ok((completion, usage_details));
        }

        Err(CompletionError::ResponseError(
            "OpenAI stream ended without completion".to_string(),
        ))
    }

    pub async fn completion_with_usage_details(
        &self,
        request: CompletionRequest,
    ) -> Result<(CompletionResponse<()>, Option<Value>), CompletionError> {
        let create_request = build_create_request(&self.model, &request, false)?;
        let response = self
            .client
            .responses()
            .create(create_request)
            .await
            .map_err(|e| CompletionError::ProviderError(e.to_string()))?;

        response_to_completion_with_details(response)
    }
}

impl super::CompletionProvider for OpenAiResponsesCompletionModel {
    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<()>, CompletionError> {
        let (response, _) = self.completion_with_usage_details(request).await?;
        Ok(response)
    }
}

fn build_create_request(
    model: &str,
    request: &CompletionRequest,
    stream: bool,
) -> Result<async_openai::types::responses::CreateResponse, CompletionError> {
    let items = build_input_items(request)?;
    if items.is_empty() {
        return Err(request_error("OpenAI Responses input was empty"));
    }

    let mut builder = CreateResponseArgs::default();
    builder.model(model);
    builder.input(InputParam::Items(items));

    if let Some(preamble) = &request.preamble {
        builder.instructions(preamble.clone());
    }

    if let Some(temperature) = request.temperature
        && temperature_supported(model)
    {
        builder.temperature(temperature as f32);
    }

    if let Some(max_tokens) = request.max_tokens {
        builder.max_output_tokens(max_tokens as u32);
    }

    if stream {
        builder.stream(true);
    }

    if let Some(tools) = build_tools(request) {
        builder.tools(tools);
    }

    if let Some(choice) = request.tool_choice.as_ref().map(map_tool_choice) {
        builder.tool_choice(choice);
    }

    if let Some(store) = extract_store(request.additional_params.as_ref()) {
        builder.store(store);
    }

    builder
        .build()
        .map_err(|e| CompletionError::RequestError(Box::new(e)))
}

fn build_input_items(request: &CompletionRequest) -> Result<Vec<InputItem>, CompletionError> {
    let mut items = Vec::new();

    if let Some(doc_message) = request.normalized_documents() {
        append_message_items(&doc_message, &mut items)?;
    }

    for message in request.chat_history.iter() {
        append_message_items(message, &mut items)?;
    }

    Ok(items)
}

fn append_message_items(
    message: &RigMessage,
    items: &mut Vec<InputItem>,
) -> Result<(), CompletionError> {
    match message {
        RigMessage::User { content } => {
            for content in content.iter() {
                match content {
                    UserContent::Text(text) => {
                        items.push(InputItem::EasyMessage(EasyInputMessage {
                            r#type: MessageType::Message,
                            role: Role::User,
                            content: EasyInputContent::Text(text.text.clone()),
                        }));
                    }
                    UserContent::ToolResult(tool_result) => {
                        let output = tool_result_to_text(tool_result);
                        let call_id = tool_result
                            .call_id
                            .clone()
                            .unwrap_or_else(|| tool_result.id.clone());
                        items.push(InputItem::Item(Item::FunctionCallOutput(
                            FunctionCallOutputItemParam {
                                call_id,
                                output: FunctionCallOutput::Text(output),
                                id: None,
                                status: Some(OutputStatus::Completed),
                            },
                        )));
                    }
                    _ => {
                        return Err(request_error(
                            "OpenAI Responses provider only supports text and tool outputs",
                        ));
                    }
                }
            }
        }
        RigMessage::Assistant { content, .. } => {
            for content in content.iter() {
                match content {
                    AssistantContent::Text(text) => {
                        items.push(InputItem::EasyMessage(EasyInputMessage {
                            r#type: MessageType::Message,
                            role: Role::Assistant,
                            content: EasyInputContent::Text(text.text.clone()),
                        }));
                    }
                    AssistantContent::Reasoning(reasoning) => {
                        let joined = reasoning.reasoning.join("\n");
                        if !joined.is_empty() {
                            items.push(InputItem::EasyMessage(EasyInputMessage {
                                r#type: MessageType::Message,
                                role: Role::Assistant,
                                content: EasyInputContent::Text(joined),
                            }));
                        }
                    }
                    AssistantContent::ToolCall(tool_call) => {
                        let arguments = serde_json::to_string(&tool_call.function.arguments)
                            .unwrap_or_else(|_| tool_call.function.arguments.to_string());
                        let call_id = tool_call
                            .call_id
                            .clone()
                            .unwrap_or_else(|| tool_call.id.clone());
                        items.push(InputItem::Item(Item::FunctionCall(FunctionToolCall {
                            arguments,
                            call_id,
                            name: tool_call.function.name.clone(),
                            id: Some(tool_call.id.clone()),
                            status: None,
                        })));
                    }
                    _ => {
                        return Err(request_error(
                            "OpenAI Responses provider only supports text and tool outputs",
                        ));
                    }
                }
            }
        }
    }

    Ok(())
}

fn request_error(message: &str) -> CompletionError {
    CompletionError::RequestError(Box::new(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        message,
    )))
}

fn temperature_supported(model: &str) -> bool {
    let model = model.trim().to_lowercase();
    let without_prefix = model
        .split_once(':')
        .map(|(_, name)| name.to_string())
        .unwrap_or(model);

    let name = without_prefix.trim();

    if name.starts_with("gpt-5") {
        return false;
    }

    if let Some(stripped) = name.strip_prefix('o') {
        let digits = stripped
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false);
        if digits {
            return false;
        }
    }

    true
}

fn build_tools(request: &CompletionRequest) -> Option<Vec<Tool>> {
    if request.tools.is_empty() {
        return None;
    }

    Some(
        request
            .tools
            .iter()
            .map(|tool| {
                Tool::Function(FunctionTool {
                    name: tool.name.clone(),
                    description: Some(tool.description.clone()),
                    parameters: Some(tool.parameters.clone()),
                    strict: None,
                })
            })
            .collect(),
    )
}

fn map_tool_choice(choice: &rig::message::ToolChoice) -> ToolChoiceParam {
    match choice {
        rig::message::ToolChoice::Auto => ToolChoiceParam::Mode(ToolChoiceOptions::Auto),
        rig::message::ToolChoice::None => ToolChoiceParam::Mode(ToolChoiceOptions::None),
        rig::message::ToolChoice::Required => ToolChoiceParam::Mode(ToolChoiceOptions::Required),
        rig::message::ToolChoice::Specific { function_names } => {
            if function_names.len() == 1 {
                ToolChoiceParam::Function(ToolChoiceFunction {
                    name: function_names[0].clone(),
                })
            } else {
                ToolChoiceParam::AllowedTools(ToolChoiceAllowed {
                    mode: ToolChoiceAllowedMode::Required,
                    tools: function_names
                        .iter()
                        .map(|name| json!({ "type": "function", "name": name }))
                        .collect(),
                })
            }
        }
    }
}

fn extract_store(additional_params: Option<&Value>) -> Option<bool> {
    if let Some(Value::Object(map)) = additional_params
        && let Some(value) = map.get("store").and_then(Value::as_bool)
    {
        return Some(value);
    }

    std::env::var("OPENAI_RESPONSES_STORE")
        .ok()
        .and_then(|value| parse_env_bool(&value))
}

fn parse_env_bool(value: &str) -> Option<bool> {
    match value.trim().to_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn debug_enabled() -> bool {
    std::env::var("OPENAI_RESPONSES_DEBUG")
        .ok()
        .and_then(|value| parse_env_bool(&value))
        .unwrap_or(false)
}

fn tool_result_to_text(tool_result: &rig::message::ToolResult) -> String {
    let mut parts = Vec::new();

    for content in tool_result.content.iter() {
        match content {
            ToolResultContent::Text(text) => parts.push(text.text.clone()),
            ToolResultContent::Image(_) => parts.push("[image output omitted]".to_string()),
        }
    }

    if parts.is_empty() {
        "[empty tool output]".to_string()
    } else {
        parts.join("")
    }
}

fn response_to_completion(response: Response) -> Result<CompletionResponse<()>, CompletionError> {
    completion_from_output_and_usage(response.output, response.usage)
}

fn response_to_completion_with_details(
    response: Response,
) -> Result<(CompletionResponse<()>, Option<Value>), CompletionError> {
    let usage_details = response.usage.as_ref().and_then(usage_to_value);
    let completion = response_to_completion(response)?;
    Ok((completion, usage_details))
}

fn completion_from_output_and_usage(
    output: Vec<OutputItem>,
    usage: Option<ResponseUsage>,
) -> Result<CompletionResponse<()>, CompletionError> {
    let mut content = Vec::new();

    for item in output {
        match item {
            OutputItem::Message(message) => {
                for part in message.content {
                    match part {
                        OutputMessageContent::OutputText(text) => {
                            content.push(AssistantContent::Text(Text { text: text.text }));
                        }
                        OutputMessageContent::Refusal(refusal) => {
                            content.push(AssistantContent::Text(Text {
                                text: refusal.refusal,
                            }));
                        }
                    }
                }
            }
            OutputItem::FunctionCall(call) => {
                content.push(AssistantContent::ToolCall(function_call_to_tool_call(
                    &call,
                )));
            }
            OutputItem::CustomToolCall(call) => {
                content.push(AssistantContent::ToolCall(custom_call_to_tool_call(&call)));
            }
            _ => {}
        }
    }

    if content.is_empty() {
        return Err(CompletionError::ResponseError(
            "OpenAI Responses output was empty".to_string(),
        ));
    }

    let choice = OneOrMany::many(content).map_err(|_| {
        CompletionError::ResponseError("OpenAI Responses output was empty".to_string())
    })?;

    let usage = usage
        .as_ref()
        .map(|usage| Usage {
            input_tokens: usage.input_tokens as u64,
            output_tokens: usage.output_tokens as u64,
            total_tokens: usage.total_tokens as u64,
        })
        .unwrap_or_default();

    Ok(CompletionResponse {
        choice,
        usage,
        raw_response: (),
    })
}

fn completion_from_text(
    text: String,
    usage: Option<ResponseUsage>,
) -> Result<CompletionResponse<()>, CompletionError> {
    if text.is_empty() {
        return Err(CompletionError::ResponseError(
            "OpenAI Responses output was empty".to_string(),
        ));
    }

    let choice = OneOrMany::one(AssistantContent::Text(Text { text }));
    let usage = usage
        .as_ref()
        .map(|usage| Usage {
            input_tokens: usage.input_tokens as u64,
            output_tokens: usage.output_tokens as u64,
            total_tokens: usage.total_tokens as u64,
        })
        .unwrap_or_default();

    Ok(CompletionResponse {
        choice,
        usage,
        raw_response: (),
    })
}

fn usage_to_value(usage: &ResponseUsage) -> Option<Value> {
    serde_json::to_value(usage).ok()
}

fn emit_completion_tokens(
    response: &CompletionResponse<()>,
    on_token: &(dyn Fn(&str) + Send + Sync),
) {
    for choice in response.choice.iter() {
        if let AssistantContent::Text(text) = choice {
            on_token(&text.text);
        }
    }
}

fn function_call_to_tool_call(call: &FunctionToolCall) -> ToolCall {
    let arguments = serde_json::from_str(&call.arguments)
        .unwrap_or_else(|_| Value::String(call.arguments.clone()));

    ToolCall {
        id: call.id.clone().unwrap_or_else(|| call.call_id.clone()),
        call_id: Some(call.call_id.clone()),
        function: ToolFunction {
            name: call.name.clone(),
            arguments,
        },
    }
}

fn custom_call_to_tool_call(call: &CustomToolCall) -> ToolCall {
    ToolCall {
        id: call.id.clone(),
        call_id: Some(call.call_id.clone()),
        function: ToolFunction {
            name: call.name.clone(),
            arguments: Value::String(call.input.clone()),
        },
    }
}

fn append_output_content(text: &mut String, part: &async_openai::types::responses::OutputContent) {
    match part {
        async_openai::types::responses::OutputContent::OutputText(content) => {
            text.push_str(&content.text);
        }
        async_openai::types::responses::OutputContent::Refusal(content) => {
            text.push_str(&content.refusal);
        }
        async_openai::types::responses::OutputContent::ReasoningText(content) => {
            if !content.text.is_empty() {
                text.push_str(&content.text);
            }
        }
    }
}

fn push_output_item(items: &mut Vec<OutputItem>, seen: &mut HashSet<String>, item: OutputItem) {
    if let Some(id) = output_item_id(&item) {
        if seen.insert(id) {
            items.push(item);
        }
    } else {
        items.push(item);
    }
}

fn output_item_id(item: &OutputItem) -> Option<String> {
    match item {
        OutputItem::Message(message) => Some(message.id.clone()),
        OutputItem::FunctionCall(call) => call.id.clone().or_else(|| Some(call.call_id.clone())),
        OutputItem::CustomToolCall(call) => Some(call.id.clone()),
        OutputItem::WebSearchCall(call) => Some(call.id.clone()),
        OutputItem::FileSearchCall(call) => Some(call.id.clone()),
        OutputItem::ComputerCall(call) => Some(call.id.clone()),
        OutputItem::CodeInterpreterCall(call) => Some(call.id.clone()),
        OutputItem::LocalShellCall(call) => Some(call.id.clone()),
        OutputItem::ShellCall(call) => Some(call.id.clone()),
        OutputItem::ApplyPatchCall(call) => Some(call.id.clone()),
        OutputItem::ImageGenerationCall(call) => Some(call.id.clone()),
        OutputItem::ShellCallOutput(call) => Some(call.id.clone()),
        OutputItem::ApplyPatchCallOutput(call) => Some(call.id.clone()),
        OutputItem::McpCall(call) => Some(call.id.clone()),
        OutputItem::McpListTools(call) => Some(call.id.clone()),
        OutputItem::McpApprovalRequest(call) => Some(call.id.clone()),
        OutputItem::Reasoning(reasoning) => Some(reasoning.id.clone()),
        OutputItem::Compaction(compaction) => Some(compaction.id.clone()),
    }
}

fn append_output_item_text(text: &mut String, item: &OutputItem) {
    if let OutputItem::Message(message) = item {
        for part in &message.content {
            match part {
                OutputMessageContent::OutputText(content) => {
                    text.push_str(&content.text);
                }
                OutputMessageContent::Refusal(content) => {
                    text.push_str(&content.refusal);
                }
            }
        }
    }
}

fn output_item_kind(item: &OutputItem) -> &'static str {
    match item {
        OutputItem::Message(_) => "message",
        OutputItem::FileSearchCall(_) => "file_search_call",
        OutputItem::FunctionCall(_) => "function_call",
        OutputItem::WebSearchCall(_) => "web_search_call",
        OutputItem::ComputerCall(_) => "computer_call",
        OutputItem::Reasoning(_) => "reasoning",
        OutputItem::Compaction(_) => "compaction",
        OutputItem::ImageGenerationCall(_) => "image_generation_call",
        OutputItem::CodeInterpreterCall(_) => "code_interpreter_call",
        OutputItem::LocalShellCall(_) => "local_shell_call",
        OutputItem::ShellCall(_) => "shell_call",
        OutputItem::ShellCallOutput(_) => "shell_call_output",
        OutputItem::ApplyPatchCall(_) => "apply_patch_call",
        OutputItem::ApplyPatchCallOutput(_) => "apply_patch_call_output",
        OutputItem::McpCall(_) => "mcp_call",
        OutputItem::McpListTools(_) => "mcp_list_tools",
        OutputItem::McpApprovalRequest(_) => "mcp_approval_request",
        OutputItem::CustomToolCall(_) => "custom_tool_call",
    }
}
