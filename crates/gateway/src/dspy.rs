//! DSPy integration for Gateway inference.

use std::sync::Arc;

use async_trait::async_trait;
use dsrs::{
    Chat, ChatAdapter, CompletionProvider, Example, LmUsage, Message as DspyMessage,
    MetaSignature, Prediction, get_callback,
};
use rig::completion::{CompletionError, CompletionRequest, CompletionResponse, Usage};
use rig::message::{AssistantContent, Message as RigMessage, Text, UserContent};
use rig::OneOrMany;
use uuid::Uuid;

use crate::error::{GatewayError, Result as GatewayResult};
use crate::inference::types::{ChatRequest, Message as GatewayMessage};
use crate::traits::InferenceGateway;

/// DSPy configuration for Gateway-backed structured queries.
#[derive(Debug, Clone)]
pub struct GatewayDspyConfig {
    pub model: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

impl GatewayDspyConfig {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            temperature: None,
            max_tokens: None,
        }
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }
}

/// Result from a Gateway-backed DSPy completion.
pub struct GatewayCompletion {
    pub response: CompletionResponse<()>,
    pub usage: LmUsage,
}

/// Gateway-backed DSPy LM wrapper.
#[derive(Clone)]
pub struct GatewayLM {
    gateway: Arc<dyn InferenceGateway>,
    model: String,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
}

impl GatewayLM {
    pub fn new(gateway: Arc<dyn InferenceGateway>, model: impl Into<String>) -> Self {
        Self {
            gateway,
            model: model.into(),
            temperature: None,
            max_tokens: None,
        }
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub async fn completion_with_signature(
        &self,
        _signature: Option<&dyn MetaSignature>,
        request: CompletionRequest,
    ) -> std::result::Result<GatewayCompletion, CompletionError> {
        let chat_request = completion_request_to_chat_request(
            &request,
            &self.model,
            self.temperature,
            self.max_tokens,
        );

        let callback = get_callback();
        let call_id = Uuid::new_v4();
        let prompt_tokens_est = estimate_prompt_tokens(&chat_request.messages);
        callback.on_lm_start(call_id, &self.model, prompt_tokens_est);

        let response = match self.gateway.chat(chat_request).await {
            Ok(response) => response,
            Err(err) => {
                let err_any = anyhow::Error::new(err);
                callback.on_lm_end(call_id, Err(&err_any), &LmUsage::default());
                return Err(CompletionError::ProviderError(err_any.to_string()));
            }
        };

        let usage = gateway_usage_to_dsrs(&response);
        callback.on_lm_end(call_id, Ok(()), &usage);

        let text = response
            .content()
            .unwrap_or_default()
            .to_string();
        if text.is_empty() {
            return Err(CompletionError::ProviderError(
                "Gateway response missing text content".to_string(),
            ));
        }

        let completion = CompletionResponse {
            choice: OneOrMany::one(AssistantContent::Text(Text { text })),
            usage: Usage {
                input_tokens: usage.prompt_tokens,
                output_tokens: usage.completion_tokens,
                total_tokens: usage.total_tokens,
            },
            raw_response: (),
        };

        Ok(GatewayCompletion {
            response: completion,
            usage,
        })
    }
}

#[async_trait]
impl CompletionProvider for GatewayLM {
    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> std::result::Result<CompletionResponse<()>, CompletionError> {
        Ok(self
            .completion_with_signature(None, request)
            .await?
            .response)
    }
}

/// Run a structured DSPy query through a Gateway inference backend.
pub async fn query_with_signature(
    gateway: Arc<dyn InferenceGateway>,
    config: GatewayDspyConfig,
    signature: &dyn MetaSignature,
    inputs: Example,
) -> GatewayResult<Prediction> {
    let adapter = ChatAdapter::default();
    let chat = adapter.format(signature, inputs);
    let completion_request = completion_request_from_chat(
        &chat,
        config.temperature,
        config.max_tokens,
    );

    let mut lm = GatewayLM::new(gateway, config.model);
    if let Some(temp) = config.temperature {
        lm = lm.with_temperature(temp);
    }
    if let Some(max_tokens) = config.max_tokens {
        lm = lm.with_max_tokens(max_tokens);
    }

    let completion = lm
        .completion_with_signature(Some(signature), completion_request)
        .await
        .map_err(|err| GatewayError::Dspy(err.to_string()))?;

    let content = completion_choice_text(&completion.response)
        .ok_or_else(|| GatewayError::Dspy("Gateway response missing text".to_string()))?;

    let output = adapter.parse_response(signature, DspyMessage::assistant(content));
    Ok(Prediction {
        data: output,
        lm_usage: completion.usage,
        node_id: None,
    })
}

/// Convenience extension for inference gateways that support cloning.
#[async_trait]
pub trait GatewayDspyExt: InferenceGateway + Clone + Send + Sync + 'static {
    async fn query_with_signature(
        &self,
        config: GatewayDspyConfig,
        signature: &dyn MetaSignature,
        inputs: Example,
    ) -> GatewayResult<Prediction> {
        query_with_signature(Arc::new(self.clone()), config, signature, inputs).await
    }

    fn dspy_lm(&self, model: impl Into<String>) -> GatewayLM {
        GatewayLM::new(Arc::new(self.clone()), model)
    }
}

impl<T> GatewayDspyExt for T where T: InferenceGateway + Clone + Send + Sync + 'static {}

fn completion_request_to_chat_request(
    request: &CompletionRequest,
    model: &str,
    default_temperature: Option<f32>,
    default_max_tokens: Option<u32>,
) -> ChatRequest {
    let mut messages = Vec::new();

    if let Some(preamble) = &request.preamble {
        messages.push(GatewayMessage::system(preamble.clone()));
    }

    for msg in request.chat_history.iter() {
        if let Some(message) = rig_message_to_gateway_message(msg) {
            messages.push(message);
        }
    }

    let temperature = request
        .temperature
        .map(|t| t as f32)
        .or(default_temperature);
    let max_tokens = request
        .max_tokens
        .map(|t| t.min(u32::MAX as u64) as u32)
        .or(default_max_tokens);

    let mut chat_request = ChatRequest::new(model.to_string(), messages);
    chat_request.temperature = temperature;
    chat_request.max_tokens = max_tokens;
    chat_request
}

fn rig_message_to_gateway_message(message: &RigMessage) -> Option<GatewayMessage> {
    match message {
        RigMessage::User { content } => collect_user_text(content).map(GatewayMessage::user),
        RigMessage::Assistant { content, .. } => {
            collect_assistant_text(content).map(GatewayMessage::assistant)
        }
    }
}

fn collect_user_text(content: &OneOrMany<UserContent>) -> Option<String> {
    let mut parts = Vec::new();
    for item in content.iter() {
        if let UserContent::Text(text) = item {
            parts.push(text.text.clone());
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

fn collect_assistant_text(content: &OneOrMany<AssistantContent>) -> Option<String> {
    let mut parts = Vec::new();
    for item in content.iter() {
        match item {
            AssistantContent::Text(text) => parts.push(text.text.clone()),
            AssistantContent::Reasoning(reasoning) => {
                parts.push(reasoning.reasoning.join("\n"));
            }
            _ => {}
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

fn estimate_prompt_tokens(messages: &[GatewayMessage]) -> usize {
    let chars: usize = messages.iter().map(|m| m.content.len()).sum();
    chars / 4
}

fn gateway_usage_to_dsrs(response: &crate::inference::types::ChatResponse) -> LmUsage {
    LmUsage {
        prompt_tokens: response.usage.prompt_tokens as u64,
        completion_tokens: response.usage.completion_tokens as u64,
        total_tokens: response.usage.total_tokens as u64,
        cost_msats: 0,
    }
}

fn completion_request_from_chat(
    chat: &Chat,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
) -> CompletionRequest {
    let request_messages = chat.get_rig_messages();
    let mut chat_history = request_messages.conversation;
    chat_history.push(request_messages.prompt);

    CompletionRequest {
        preamble: Some(request_messages.system.clone()),
        chat_history: if chat_history.len() == 1 {
            OneOrMany::one(chat_history.clone().into_iter().next().unwrap())
        } else {
            OneOrMany::many(chat_history.clone()).expect("chat_history should not be empty")
        },
        documents: Vec::new(),
        tools: Vec::new(),
        temperature: temperature.map(|t| t as f64),
        max_tokens: max_tokens.map(|t| t as u64),
        tool_choice: None,
        additional_params: None,
    }
}

fn completion_choice_text(response: &CompletionResponse<()>) -> Option<String> {
    match response.choice.first() {
        AssistantContent::Text(text) => Some(text.text.clone()),
        AssistantContent::Reasoning(reasoning) => Some(reasoning.reasoning.join("\n")),
        AssistantContent::ToolCall(_) => None,
        AssistantContent::Image(_) => None,
    }
}
