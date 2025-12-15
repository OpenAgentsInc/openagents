use crate::api::auth::AuthProvider;
use crate::api::common::Prompt as ApiPrompt;
use crate::api::common::Reasoning;
use crate::api::common::ResponseStream;
use crate::api::common::TextControls;
use crate::api::endpoint::streaming::StreamingClient;
use crate::api::error::ApiError;
use crate::api::provider::Provider;
use crate::api::provider::WireApi;
use crate::api::requests::ResponsesRequest;
use crate::api::requests::ResponsesRequestBuilder;
use crate::api::sse::spawn_response_stream;
use crate::api::telemetry::SseTelemetry;
use crate::client::HttpTransport;
use crate::client::RequestTelemetry;
use crate::protocol::protocol::SessionSource;
use http::HeaderMap;
use serde_json::Value;
use std::sync::Arc;
use tracing::instrument;

pub struct ResponsesClient<T: HttpTransport, A: AuthProvider> {
    streaming: StreamingClient<T, A>,
}

#[derive(Default)]
pub struct ResponsesOptions {
    pub reasoning: Option<Reasoning>,
    pub include: Vec<String>,
    pub prompt_cache_key: Option<String>,
    pub text: Option<TextControls>,
    pub store_override: Option<bool>,
    pub conversation_id: Option<String>,
    pub session_source: Option<SessionSource>,
}

impl<T: HttpTransport, A: AuthProvider> ResponsesClient<T, A> {
    pub fn new(transport: T, provider: Provider, auth: A) -> Self {
        Self {
            streaming: StreamingClient::new(transport, provider, auth),
        }
    }

    pub fn with_telemetry(
        self,
        request: Option<Arc<dyn RequestTelemetry>>,
        sse: Option<Arc<dyn SseTelemetry>>,
    ) -> Self {
        Self {
            streaming: self.streaming.with_telemetry(request, sse),
        }
    }

    pub async fn stream_request(
        &self,
        request: ResponsesRequest,
    ) -> Result<ResponseStream, ApiError> {
        self.stream(request.body, request.headers).await
    }

    #[instrument(skip_all, err)]
    pub async fn stream_prompt(
        &self,
        model: &str,
        prompt: &ApiPrompt,
        options: ResponsesOptions,
    ) -> Result<ResponseStream, ApiError> {
        let ResponsesOptions {
            reasoning,
            include,
            prompt_cache_key,
            text,
            store_override,
            conversation_id,
            session_source,
        } = options;

        let request = ResponsesRequestBuilder::new(model, &prompt.instructions, &prompt.input)
            .tools(&prompt.tools)
            .parallel_tool_calls(prompt.parallel_tool_calls)
            .reasoning(reasoning)
            .include(include)
            .prompt_cache_key(prompt_cache_key)
            .text(text)
            .conversation(conversation_id)
            .session_source(session_source)
            .store_override(store_override)
            .build(self.streaming.provider())?;

        self.stream_request(request).await
    }

    fn path(&self) -> &'static str {
        match self.streaming.provider().wire {
            WireApi::Responses | WireApi::Compact => "responses",
            WireApi::Chat => "chat/completions",
        }
    }

    pub async fn stream(
        &self,
        body: Value,
        extra_headers: HeaderMap,
    ) -> Result<ResponseStream, ApiError> {
        self.streaming
            .stream(self.path(), body, extra_headers, spawn_response_stream)
            .await
    }
}
