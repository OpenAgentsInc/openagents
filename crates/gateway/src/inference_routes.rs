//! The inference routes for our own services (`docs/inference/gateway.md`,
//! sections 3, 7, and 13, P0): `POST /v1/responses` (Open Responses) and
//! `POST /v1/chat/completions` (OpenAI Chat Completions, translated onto
//! the same request).
//!
//! P0 admits only service keys: an `oak_` key whose tenant is one of
//! `inference.service_tenants` (a house tenant). Calls are metered, every
//! attempt recorded into the meter, and not charged. A key scoped to
//! actions must include `inference`. Anyone else gets `401` or `403`.
//!
//! Each answer carries `x-request-id`, `x-openagents-model`, and
//! `x-openagents-upstream`; a non-streaming answer also carries
//! `x-openagents-cost-usd`. A streaming answer is server-sent events ending
//! with `[DONE]`.
//!
//! `openagents/auto` is judged by [`JevClass`]: one typed System One
//! choice over the six task classes, read from the request's last user
//! turn and its shape. No keyword matching; without a TypeSafe key, or
//! without an answer in time, the class is `chat`.

use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{MethodRouter, post};
use futures_util::StreamExt;
use inference::chat::{
    ChatRequest, ChunkWriter, completion_from_response, encode_chunk, to_responses_request,
};
use inference::error::{ApiError, ErrorType};
use inference::item::{ContentPart, Item, MessageContent, Role};
use inference::meter::Api;
use inference::request::{CreateResponse, Input};
use inference::router::TaskClass;
use inference::run::{Caller, Gateway, PickClass, Routed, collect};
use inference::sse::{DONE_FRAME, encode_event};
use inference::upstream::{BoxFuture, Upstream};
use serde_json::json;
use tenancy::{Registry, keys};

use crate::serve::ServeState;

pub const RESPONSES: &str = "/v1/responses";
pub const CHAT: &str = "/v1/chat/completions";

pub fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    vec![(RESPONSES, post(responses)), (CHAT, post(chat))]
}

/// The adapters the gateway holds, keys from the environment or mounted
/// files (`*_FILE`): Vertex AI, Z.ai, the Pro door, OpenRouter, and the
/// Vercel AI Gateway. An adapter without its key stays out of routing.
#[must_use]
pub fn upstreams() -> Vec<Arc<dyn Upstream>> {
    use inference::upstream::{openrouter, pro, vercel, vertex, zai};
    vec![
        Arc::new(vertex::Vertex::new(vertex::Config::from_env())),
        Arc::new(zai::from_env()),
        Arc::new(pro::from_env()),
        Arc::new(openrouter::from_env()),
        Arc::new(vercel::from_env()),
    ]
}

/// The gateway over `upstreams` (normally [`upstreams`]), judging `openagents/auto` with Jev when
/// a TypeSafe key is configured.
#[must_use]
pub fn gateway(
    config: &crate::config::Inference,
    meter: Arc<inference::meter::Meter>,
    upstreams: Vec<Arc<dyn Upstream>>,
) -> Gateway {
    let mut gateway = Gateway::new(upstreams, meter);
    if let Some(classes) = &config.classes {
        gateway = gateway.with_classes(classes.clone());
    }
    if let Some(judge) = JevClass::from_env() {
        gateway = gateway.with_judge(Arc::new(judge), inference::run::JUDGE_BUDGET);
    }
    gateway
}

/// The `openagents/auto` judgment: one System One Choice over the task
/// classes.
pub struct JevClass {
    client: jev::Client,
}

/// The longest slice of the conversation the judgment reads.
const JUDGED_CHARS: usize = 2_000;

impl JevClass {
    /// A judge on the TypeSafe key in the environment, if one is set.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        std::env::var("TYPESAFE_API_KEY")
            .ok()
            .filter(|key| !key.trim().is_empty())?;
        jev::Client::from_env().ok().map(|client| Self { client })
    }

    /// The state the judgment reads: the request's shape and its last
    /// user turn, clipped. Structure only, never a keyword list.
    fn state(request: &CreateResponse) -> String {
        let mut last_user = String::new();
        let mut turns = 0usize;
        let mut chars = request.instructions.as_ref().map_or(0, String::len);
        match &request.input {
            Some(Input::Text(text)) => {
                last_user.clone_from(text);
                turns = 1;
                chars += text.len();
            }
            Some(Input::Items(items)) => {
                for item in items {
                    if let Item::Message(message) = item {
                        turns += 1;
                        let text = match &message.content {
                            MessageContent::Text(text) => text.clone(),
                            MessageContent::Parts(parts) => parts
                                .iter()
                                .filter_map(ContentPart::text)
                                .collect::<Vec<_>>()
                                .join(" "),
                        };
                        chars += text.len();
                        if message.role == Role::User {
                            last_user = text;
                        }
                    }
                }
            }
            None => {}
        }
        let tools = request.tools.as_ref().map_or(0, Vec::len);
        let clipped: String = last_user.chars().take(JUDGED_CHARS).collect();
        format!(
            "A request to a language model. Conversation turns: {turns}. Approximate input \
             characters: {chars}. Tools offered: {tools}.\nThe latest user message:\n{clipped}"
        )
    }
}

impl PickClass for JevClass {
    fn pick<'a>(&'a self, request: &'a CreateResponse) -> BoxFuture<'a, Option<TaskClass>> {
        Box::pin(async move {
            let choice = jev::Choice::default()
                .option("classify", "Labels, extraction, or a short yes or no")
                .option("fast", "A short reply, an opener, or a summary")
                .option("chat", "General conversation")
                .option(
                    "code",
                    "Writing, fixing, or reasoning about code, or using tools",
                )
                .option("long", "Reading a very long input (hundreds of pages)")
                .option(
                    "reason",
                    "A hard multi-step problem that needs careful reasoning",
                );
            let questions = jev::Questions::new().with(
                "class",
                jev::Choice {
                    instructions: Some("Which kind of model work does this request need?".into()),
                    ..choice
                },
            );
            let answer = self
                .client
                .system_one(jev::SystemOneRequest::new(Self::state(request), questions))
                .await
                .ok()?;
            let picked = answer.choice("class").ok()?.choice.clone();
            TaskClass::ALL
                .into_iter()
                .find(|class| class.as_str() == picked)
        })
    }
}

static REQUESTS: AtomicU64 = AtomicU64::new(0);

fn request_id() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|span| span.as_micros())
        .unwrap_or_default();
    format!(
        "req_{now:x}{:04x}",
        REQUESTS.fetch_add(1, Ordering::Relaxed) & 0xffff
    )
}

fn error(error: &ApiError, request_id: &str) -> Response {
    let status = StatusCode::from_u16(error.kind.status()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut response = (status, axum::Json(json!({"error": error}))).into_response();
    if let Ok(value) = HeaderValue::from_str(request_id) {
        response.headers_mut().insert("x-request-id", value);
    }
    response
}

/// The service caller, or the refusal.
fn admit(state: &ServeState, headers: &HeaderMap) -> Result<Caller, ApiError> {
    let unauthorized = |message: &str| ApiError::new(ErrorType::Unauthorized, message);
    let Some(config) = &state.config.inference else {
        return Err(ApiError::new(
            ErrorType::NotFound,
            "Inference is not set up here.",
        ));
    };
    let token = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or_else(|| unauthorized("Send a service key in the `Authorization: Bearer` header."))?;
    let registry = Registry::open(&state.dir).map_err(|_| {
        ApiError::new(
            ErrorType::ServerError,
            "The key registry can't be read right now.",
        )
    })?;
    let authenticated = keys::authenticate(&state.dir, registry.manifest(), token)
        .map_err(|_| unauthorized("Your API key was rejected."))?;
    if !config
        .service_tenants
        .iter()
        .any(|tenant| *tenant == authenticated.tenant)
    {
        return Err(ApiError::new(
            ErrorType::LimitReached,
            "Inference is open to OpenAgents services only for now.",
        ));
    }
    if authenticated
        .scopes
        .as_ref()
        .is_some_and(|scopes| !scopes.permits_action("inference"))
    {
        return Err(ApiError {
            param: Some("scopes.actions".into()),
            ..ApiError::new(
                ErrorType::LimitReached,
                "This key is not allowed to run inference.",
            )
        });
    }
    Ok(Caller {
        request_id: request_id(),
        tenant: Some(authenticated.tenant),
        key_id: Some(authenticated.key_id),
        api: Api::Responses,
        limits: inference::router::PriceLimit::default(),
    })
}

fn headers(response: &mut Response, request_id: &str, routed: &Routed) {
    let headers = response.headers_mut();
    for (name, value) in [
        ("x-request-id", request_id),
        ("x-openagents-model", routed.model.as_str()),
        ("x-openagents-upstream", routed.upstream.as_str()),
    ] {
        if let Ok(value) = HeaderValue::from_str(value) {
            headers.insert(name, value);
        }
    }
}

fn cost_header(response: &mut Response, info: Option<&inference::openagents::ResponseInfo>) {
    if let Some(cost) = info.and_then(|info| info.cost.as_ref())
        && let Ok(value) = HeaderValue::from_str(&cost.price_usd)
    {
        response
            .headers_mut()
            .insert("x-openagents-cost-usd", value);
    }
}

fn sse(body: impl futures_util::Stream<Item = String> + Send + 'static) -> Response {
    let stream = body.map(|frame| Ok::<_, Infallible>(Bytes::from(frame)));
    let mut response = Response::new(Body::from_stream(stream));
    let headers = response.headers_mut();
    headers.insert(
        "content-type",
        HeaderValue::from_static("text/event-stream"),
    );
    headers.insert("cache-control", HeaderValue::from_static("no-cache"));
    response
}

fn engine(state: &ServeState) -> Result<&Arc<Gateway>, ApiError> {
    state
        .inference
        .as_ref()
        .ok_or_else(|| ApiError::new(ErrorType::NotFound, "Inference is not set up here."))
}

async fn responses(
    State(state): State<Arc<ServeState>>,
    headers_in: HeaderMap,
    body: Bytes,
) -> Response {
    let caller = match admit(&state, &headers_in) {
        Ok(caller) => caller,
        Err(refusal) => return error(&refusal, &request_id()),
    };
    let id = caller.request_id.clone();
    let request: CreateResponse = match serde_json::from_slice(&body) {
        Ok(request) => request,
        Err(why) => {
            return error(
                &ApiError::invalid_request(
                    "body",
                    format!("The request body isn't a valid request: {why}"),
                ),
                &id,
            );
        }
    };
    if let Err(refusal) = request.require_stateless() {
        return error(&refusal, &id);
    }
    let gateway = match engine(&state) {
        Ok(gateway) => gateway.clone(),
        Err(refusal) => return error(&refusal, &id),
    };
    let mut routed = match gateway.run(&request, &caller).await {
        Ok(routed) => routed,
        Err(refusal) => return error(&refusal, &id),
    };
    let events = std::mem::replace(&mut routed.events, Box::pin(futures_util::stream::empty()));
    if request.stream == Some(true) {
        let frames = events
            .map(|event| encode_event(&event))
            .chain(futures_util::stream::once(async { DONE_FRAME.to_owned() }));
        let mut response = sse(frames);
        headers(&mut response, &id, &routed);
        return response;
    }
    let Some(folded) = collect(events).await else {
        return error(
            &ApiError::new(ErrorType::UpstreamFailed, "The model sent no answer."),
            &id,
        );
    };
    let mut response = axum::Json(&folded).into_response();
    headers(&mut response, &id, &routed);
    cost_header(&mut response, folded.openagents.as_ref());
    response
}

async fn chat(
    State(state): State<Arc<ServeState>>,
    headers_in: HeaderMap,
    body: Bytes,
) -> Response {
    let mut caller = match admit(&state, &headers_in) {
        Ok(caller) => caller,
        Err(refusal) => return error(&refusal, &request_id()),
    };
    caller.api = Api::Chat;
    let id = caller.request_id.clone();
    let chat: ChatRequest = match serde_json::from_slice(&body) {
        Ok(chat) => chat,
        Err(why) => {
            return error(
                &ApiError::invalid_request(
                    "body",
                    format!("The request body isn't a valid request: {why}"),
                ),
                &id,
            );
        }
    };
    let (request, shape) = match to_responses_request(chat) {
        Ok(translated) => translated,
        Err(refusal) => return error(&refusal, &id),
    };
    let gateway = match engine(&state) {
        Ok(gateway) => gateway.clone(),
        Err(refusal) => return error(&refusal, &id),
    };
    let mut routed = match gateway.run(&request, &caller).await {
        Ok(routed) => routed,
        Err(refusal) => return error(&refusal, &id),
    };
    let events = std::mem::replace(&mut routed.events, Box::pin(futures_util::stream::empty()));
    if request.stream == Some(true) {
        let mut writer = ChunkWriter::new(shape.include_usage);
        let frames = events
            .flat_map(move |event| {
                futures_util::stream::iter(
                    writer
                        .push(&event)
                        .iter()
                        .map(encode_chunk)
                        .collect::<Vec<_>>(),
                )
            })
            .chain(futures_util::stream::once(async { DONE_FRAME.to_owned() }));
        let mut response = sse(frames);
        headers(&mut response, &id, &routed);
        return response;
    }
    let Some(folded) = collect(events).await else {
        return error(
            &ApiError::new(ErrorType::UpstreamFailed, "The model sent no answer."),
            &id,
        );
    };
    let completion = completion_from_response(&folded);
    let mut response = axum::Json(&completion).into_response();
    headers(&mut response, &id, &routed);
    cost_header(&mut response, folded.openagents.as_ref());
    response
}
