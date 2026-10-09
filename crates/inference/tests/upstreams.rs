//! The upstream adapters against local stub servers: recorded streams,
//! request bodies, privacy fields, errors, rate limits, broken streams,
//! and the attempt records. No test reaches a real provider.

mod common;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use inference::event::{Event, EventBody};
use inference::item::Item;
use inference::meter::{self, Attempt, Collect};
use inference::request::CreateResponse;
use inference::sse::StreamItem;
use inference::stream::{Accumulator, StreamCheck};
use inference::upstream::google::TokenSource;
use inference::upstream::secret::{KeyRef, Secret};
use inference::upstream::{
    AttemptError, ErrorClass, Sent, Stage, Upstream, chat::ChatUpstream, openrouter, pro,
    responses::ResponsesUpstream, vercel, vertex, zai,
};
use inference::{Response, ResponseStatus};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// A key shaped like a real one, so a leak would be visible.
const KEY: &str = "sk-test-0123456789abcdefghijklmnop";

fn key() -> Option<Secret> {
    Secret::new(KEY)
}

fn fixture(name: &str) -> String {
    let path = format!("{}/fixtures/upstream/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path}: {error}"))
}

// ---------------------------------------------------------------------
// The stub server.

#[derive(Clone, Debug)]
struct Reply {
    status: u16,
    headers: Vec<(&'static str, String)>,
    /// Body chunks, written with a short pause between them.
    chunks: Vec<String>,
    /// Pause before the first chunk, in milliseconds.
    delay_ms: u64,
}

impl Reply {
    fn stream(body: &str) -> Self {
        // Split into a few chunks at arbitrary points, as a network would.
        let bytes = body.as_bytes();
        let mut chunks = Vec::new();
        let mut at = 0;
        let mut step = 37;
        while at < bytes.len() {
            let mut end = (at + step).min(bytes.len());
            while !body.is_char_boundary(end) {
                end += 1;
            }
            chunks.push(body[at..end].to_owned());
            at = end;
            step = step * 7 % 211 + 13;
        }
        Self {
            status: 200,
            headers: vec![("content-type", "text/event-stream".to_owned())],
            chunks,
            delay_ms: 0,
        }
    }

    fn json(status: u16, body: Value) -> Self {
        Self {
            status,
            headers: vec![("content-type", "application/json".to_owned())],
            chunks: vec![body.to_string()],
            delay_ms: 0,
        }
    }

    fn header(mut self, name: &'static str, value: &str) -> Self {
        self.headers.push((name, value.to_owned()));
        self
    }
}

#[derive(Clone, Debug)]
struct Seen {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl Seen {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).expect("the request body is JSON")
    }
}

struct Stub {
    url: String,
    seen: Arc<Mutex<Vec<Seen>>>,
}

impl Stub {
    fn seen(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }
}

/// A server answering each connection with the next reply (the last one
/// repeats).
async fn stub(replies: Vec<Reply>) -> Stub {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    tokio::spawn(async move {
        let mut served = 0;
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let reply = replies[served.min(replies.len() - 1)].clone();
            served += 1;
            let log = log.clone();
            tokio::spawn(async move {
                let mut buffer = Vec::new();
                let mut chunk = [0u8; 4096];
                let header_end = loop {
                    let read = socket.read(&mut chunk).await.unwrap_or(0);
                    if read == 0 {
                        return;
                    }
                    buffer.extend_from_slice(&chunk[..read]);
                    if let Some(at) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
                        break at + 4;
                    }
                };
                let head = String::from_utf8_lossy(&buffer[..header_end]).into_owned();
                let mut lines = head.lines();
                let first = lines.next().unwrap_or_default().to_owned();
                let mut parts = first.split_whitespace();
                let method = parts.next().unwrap_or_default().to_owned();
                let path = parts.next().unwrap_or_default().to_owned();
                let headers: HashMap<String, String> = lines
                    .filter_map(|line| line.split_once(':'))
                    .map(|(name, value)| {
                        (name.trim().to_ascii_lowercase(), value.trim().to_owned())
                    })
                    .collect();
                let length: usize = headers
                    .get("content-length")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0);
                while buffer.len() < header_end + length {
                    let read = socket.read(&mut chunk).await.unwrap_or(0);
                    if read == 0 {
                        break;
                    }
                    buffer.extend_from_slice(&chunk[..read]);
                }
                let body = buffer[header_end..].to_vec();
                log.lock().unwrap().push(Seen {
                    method,
                    path,
                    headers,
                    body,
                });
                let mut head = format!("HTTP/1.1 {} Stub\r\nconnection: close\r\n", reply.status);
                for (name, value) in &reply.headers {
                    head.push_str(&format!("{name}: {value}\r\n"));
                }
                if reply.status != 200 {
                    let length: usize = reply.chunks.iter().map(String::len).sum();
                    head.push_str(&format!("content-length: {length}\r\n"));
                }
                head.push_str("\r\n");
                if socket.write_all(head.as_bytes()).await.is_err() {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(reply.delay_ms)).await;
                for piece in &reply.chunks {
                    if socket.write_all(piece.as_bytes()).await.is_err() {
                        return;
                    }
                    let _ = socket.flush().await;
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
                let _ = socket.shutdown().await;
            });
        }
    });
    Stub { url, seen }
}

// ---------------------------------------------------------------------
// Adapters pointed at a stub.

fn zai_at(url: &str, key: Option<Secret>) -> ChatUpstream {
    let mut config = zai::config(key);
    config.url = format!("{url}/api/paas/v4/chat/completions");
    ChatUpstream::new(config)
}

fn pro_at(url: &str) -> ChatUpstream {
    let mut config = pro::config(key());
    config.url = format!("{url}/chat/completions");
    ChatUpstream::new(config)
}

fn openrouter_at(url: &str) -> ResponsesUpstream {
    let mut config = openrouter::config(key());
    config.url = format!("{url}/api/v1/responses");
    ResponsesUpstream::new(config)
}

fn vercel_at(url: &str) -> ResponsesUpstream {
    let mut config = vercel::config(key());
    config.url = format!("{url}/v1/responses");
    ResponsesUpstream::new(config)
}

fn vertex_at(url: &str) -> vertex::Vertex {
    let mut config = vertex::Config::from_env();
    config.project = "test-project".into();
    config.location = "global".into();
    config.token = TokenSource::fixed(Secret::new("ya29.test-token-not-real").unwrap());
    config.base_url = Some(url.to_owned());
    vertex::Vertex::new(config)
}

fn request(value: Value) -> CreateResponse {
    serde_json::from_value(value).expect("a valid request")
}

fn count_request(privacy: &str) -> CreateResponse {
    request(json!({
        "instructions": "You are terse. Answer with the words asked for and nothing else.",
        "input": "Count from one to five, one word per line.",
        "stream": true,
        "openagents": {"privacy": privacy},
    }))
}

/// Reads a whole stream: the events, and the error that ended it, if any.
async fn drain(sent: Sent) -> (Vec<Event>, Option<AttemptError>) {
    let mut events = Vec::new();
    let mut stream = sent.events;
    while let Some(item) = stream.next().await {
        match item {
            Ok(event) => events.push(event),
            Err(error) => return (events, Some(error)),
        }
    }
    (events, None)
}

fn check(events: &[Event]) {
    let mut items: Vec<StreamItem> = events.iter().cloned().map(StreamItem::Event).collect();
    items.push(StreamItem::Done);
    let violations = StreamCheck::run(&items);
    assert!(violations.is_empty(), "spec violations: {violations:#?}");
}

fn folded(events: &[Event]) -> Response {
    let mut accumulator = Accumulator::new();
    for event in events {
        accumulator.push(event);
    }
    accumulator.finish().expect("a terminal event")
}

fn terminal(events: &[Event]) -> &Response {
    events
        .last()
        .and_then(|event| event.body.response())
        .filter(|_| events.last().is_some_and(|e| e.body.is_terminal()))
        .expect("the stream ends with a terminal event")
}

fn template() -> Attempt {
    Attempt::new("req_test", 1, "", "", 0)
}

// ---------------------------------------------------------------------
// Vertex AI.

#[tokio::test]
async fn vertex_recorded_stream_is_reasoning_then_answer() {
    let stub = stub(vec![Reply::stream(&fixture("vertex-gemini-3.8-flash.sse"))]).await;
    let adapter = vertex_at(&stub.url);
    let sent = adapter
        .send(&count_request("strict"), "google/gemini-3.8-flash")
        .await
        .expect("sent");
    let meter = sent.meter.clone();
    let (events, error) = drain(sent).await;
    assert!(error.is_none(), "{error:?}");
    check(&events);
    let response = folded(&events);
    assert_eq!(response.status, ResponseStatus::Completed);
    assert_eq!(response.model, "google/gemini-3.8-flash");
    assert_eq!(response.output_text(), "One\nTwo\nThree\nFour\nFive");
    let Item::Reasoning(reasoning) = &response.output[0] else {
        panic!("first item is reasoning: {:?}", response.output[0]);
    };
    assert!(reasoning.summary_text().starts_with("**Counting Upwards**"));
    let usage = response.usage.expect("usage");
    assert_eq!(usage.input_tokens, 25);
    assert_eq!(usage.output_tokens, 9 + 151);
    assert_eq!(usage.output_tokens_details.reasoning_tokens, 151);

    let seen = stub.seen();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].method, "POST");
    assert_eq!(
        seen[0].path,
        "/v1/projects/test-project/locations/global/publishers/google/models/gemini-3.8-flash:streamGenerateContent?alt=sse"
    );
    assert_eq!(
        seen[0].headers["authorization"],
        "Bearer ya29.test-token-not-real"
    );
    let body = seen[0].json();
    assert_eq!(
        body["systemInstruction"]["parts"][0]["text"],
        "You are terse. Answer with the words asked for and nothing else."
    );
    assert_eq!(body["contents"][0]["role"], "user");
    assert_eq!(
        body["contents"][0]["parts"][0]["text"],
        "Count from one to five, one word per line."
    );
    assert_eq!(
        body["generationConfig"]["thinkingConfig"]["includeThoughts"],
        true
    );

    let measure = meter.snapshot();
    assert_eq!(measure.stage, Stage::Completed);
    assert!(measure.first_token_ms.is_some());
    assert_eq!(measure.tokens.input, 25);
    assert_eq!(measure.tokens.reasoning, 151);
    let record = meter.attempt(template());
    assert_eq!(record.upstream, "vertex");
    assert_eq!(record.account.as_deref(), Some("google-credit"));
    assert_eq!(record.model, "google/gemini-3.8-flash");
    assert_eq!(record.outcome, meter::Outcome::Ok);
    assert_eq!(record.upstream_status, Some(200));
    assert_eq!(record.error, None);
}

#[tokio::test]
async fn vertex_function_call_keeps_its_signature_for_the_next_turn() {
    let stub = stub(vec![Reply::stream(&fixture(
        "vertex-gemini-3.8-flash-tools.sse",
    ))])
    .await;
    let adapter = vertex_at(&stub.url);
    let tools = json!([{"type": "function", "name": "get_weather",
        "description": "Current weather for a city.",
        "parameters": {"type": "object", "properties": {"city": {"type": "string"}}, "required": ["city"]}}]);
    let first = request(json!({
        "input": "What is the weather in Paris? Use the tool.",
        "tools": tools,
        "tool_choice": "auto",
        "reasoning": {"effort": "low"},
    }));
    let sent = adapter
        .send(&first, "google/gemini-3.8-flash")
        .await
        .expect("sent");
    let (events, error) = drain(sent).await;
    assert!(error.is_none(), "{error:?}");
    check(&events);
    let response = folded(&events);
    let call = response
        .output
        .iter()
        .find_map(|item| match item {
            Item::FunctionCall(call) => Some(call.clone()),
            _ => None,
        })
        .expect("a function call");
    assert_eq!(call.name, "get_weather");
    assert_eq!(call.call_id, "call_504386");
    assert_eq!(
        serde_json::from_str::<Value>(&call.arguments).unwrap(),
        json!({"city": "Paris"})
    );
    let signature = response
        .output
        .iter()
        .find_map(|item| match item {
            Item::Reasoning(reasoning) => reasoning.encrypted_content.clone(),
            _ => None,
        })
        .expect("the signature rides as encrypted_content");
    assert!(signature.starts_with("AY89a1/HxVse"));

    let body = stub.seen()[0].json();
    assert_eq!(
        body["tools"][0]["functionDeclarations"][0]["name"],
        "get_weather"
    );
    assert_eq!(
        body["tools"][0]["functionDeclarations"][0]["parametersJsonSchema"]["required"],
        json!(["city"])
    );
    assert_eq!(body["toolConfig"]["functionCallingConfig"]["mode"], "AUTO");
    assert_eq!(
        body["generationConfig"]["thinkingConfig"]["thinkingLevel"],
        "low"
    );

    // A stateless caller sends the output back with the tool's result.
    let mut input: Vec<Value> =
        vec![json!({"role": "user", "content": "What is the weather in Paris? Use the tool."})];
    input.extend(
        response
            .output
            .iter()
            .map(|item| serde_json::to_value(item).unwrap()),
    );
    input.push(
        json!({"type": "function_call_output", "call_id": "call_504386", "output": "18 C, clear"}),
    );
    let second = request(json!({"input": input, "tools": tools}));
    let next = vertex::body(
        &second,
        &vertex::default_models()[0].0,
        vertex::Thinking::Level,
    )
    .unwrap();
    let contents = next["contents"].as_array().unwrap();
    assert_eq!(contents.len(), 3, "{contents:#?}");
    assert_eq!(contents[1]["role"], "model");
    assert_eq!(
        contents[1]["parts"][0]["functionCall"]["name"],
        "get_weather"
    );
    assert_eq!(
        contents[1]["parts"][0]["functionCall"]["args"],
        json!({"city": "Paris"})
    );
    assert_eq!(
        contents[1]["parts"][0]["thoughtSignature"],
        json!(signature)
    );
    assert_eq!(contents[2]["role"], "user");
    assert_eq!(
        contents[2]["parts"][0]["functionResponse"]["name"],
        "get_weather"
    );
    assert_eq!(
        contents[2]["parts"][0]["functionResponse"]["response"]["output"],
        "18 C, clear"
    );

    // Without a signature (a conversation from elsewhere) the call
    // carries Google's placeholder rather than failing.
    let foreign = request(json!({"input": [
        {"role": "user", "content": "Weather in Paris?"},
        {"type": "function_call", "call_id": "c1", "name": "get_weather", "arguments": "{\"city\":\"Paris\"}"},
        {"type": "function_call_output", "call_id": "c1", "output": "18 C"},
    ], "tools": tools}));
    let body = vertex::body(
        &foreign,
        &vertex::default_models()[0].0,
        vertex::Thinking::Level,
    )
    .unwrap();
    assert_eq!(
        body["contents"][1]["parts"][0]["thoughtSignature"],
        vertex::SKIP_SIGNATURE
    );
}

#[tokio::test]
async fn vertex_maps_schema_effort_and_tool_choice() {
    let req = request(json!({
        "input": [
            {"role": "developer", "content": "Answer in JSON."},
            {"role": "user", "content": [{"type": "input_text", "text": "Label this."},
                {"type": "input_image", "image_url": "data:image/png;base64,iVBORw0KGgo="}]},
        ],
        "text": {"format": {"type": "json_schema", "name": "label", "schema": {"type": "object"}}},
        "reasoning": {"effort": "none"},
        "max_output_tokens": 64,
        "temperature": 0.2,
        "stop": ["END"],
        "tools": [{"type": "function", "name": "a"}, {"type": "function", "name": "b"}],
        "tool_choice": {"type": "allowed_tools", "mode": "required", "tools": [{"type": "function", "name": "b"}]},
    }));
    let lite = &vertex::default_models()[1].0;
    let body = vertex::body(&req, lite, vertex::Thinking::Budget).unwrap();
    assert_eq!(
        body["systemInstruction"]["parts"][0]["text"],
        "Answer in JSON."
    );
    assert_eq!(
        body["contents"][0]["parts"][1]["inlineData"]["mimeType"],
        "image/png"
    );
    let generation = &body["generationConfig"];
    assert_eq!(generation["responseMimeType"], "application/json");
    assert_eq!(generation["responseJsonSchema"], json!({"type": "object"}));
    assert_eq!(generation["maxOutputTokens"], 64);
    assert_eq!(generation["stopSequences"], json!(["END"]));
    assert_eq!(generation["thinkingConfig"]["thinkingBudget"], 0);
    assert_eq!(generation["thinkingConfig"]["includeThoughts"], false);
    let declarations = body["tools"][0]["functionDeclarations"].as_array().unwrap();
    assert_eq!(
        declarations.len(),
        1,
        "allowed_tools narrows the declarations"
    );
    assert_eq!(
        body["toolConfig"]["functionCallingConfig"],
        json!({"mode": "ANY", "allowedFunctionNames": ["b"]})
    );
}

#[tokio::test]
async fn vertex_safety_stop_is_incomplete_and_a_quota_error_falls_back() {
    let blocked = "data: {\"candidates\": [{\"content\": {\"role\": \"model\",\"parts\": [{\"text\": \"I can\"}]},\"finishReason\": \"SAFETY\"}],\"usageMetadata\": {\"promptTokenCount\": 5,\"candidatesTokenCount\": 2}}\n\n";
    let quota = json!([{"error": {"code": 429, "message": "Resource exhausted. Please try again later.", "status": "RESOURCE_EXHAUSTED"}}]);
    let stub = stub(vec![Reply::stream(blocked), Reply::json(429, quota)]).await;
    let adapter = vertex_at(&stub.url);
    let sent = adapter
        .send(&count_request("strict"), "google/gemini-3.8-flash")
        .await
        .unwrap();
    let (events, error) = drain(sent).await;
    assert!(error.is_none());
    check(&events);
    let response = terminal(&events);
    assert_eq!(response.status, ResponseStatus::Incomplete);
    assert_eq!(
        response
            .incomplete_details
            .as_ref()
            .unwrap()
            .reason
            .as_str(),
        "content_filter"
    );

    let error = adapter
        .send(&count_request("strict"), "google/gemini-3.8-flash")
        .await
        .unwrap_err();
    assert_eq!(error.class, ErrorClass::RateLimited);
    assert_eq!(error.status, Some(429));
    assert_eq!(error.message, "Resource exhausted. Please try again later.");
    assert!(error.class.falls_back());
}

#[tokio::test]
async fn google_tokens_come_from_the_metadata_server_and_secrets_from_secret_manager() {
    let token = Reply::json(
        200,
        json!({"access_token": "ya29.from-metadata", "expires_in": 3599, "token_type": "Bearer"}),
    );
    let secret = Reply::json(
        200,
        json!({"name": "projects/p/secrets/zai-api-key/versions/3",
        "payload": {"data": "c2stdGVzdC16YWkta2V5LWZyb20tc2VjcmV0LW1hbmFnZXIK"}}),
    );
    let missing = Reply::json(
        404,
        json!({"error": {"code": 404, "message": "Secret not found"}}),
    );
    let stub = stub(vec![token, secret, missing]).await;
    let google = TokenSource::metadata(&format!("{}/computeMetadata/v1/token", stub.url))
        .secret_manager_url(&stub.url);
    assert_eq!(google.token().await.unwrap().expose(), "ya29.from-metadata");
    // Cached: the second call makes no request.
    assert_eq!(google.token().await.unwrap().expose(), "ya29.from-metadata");

    let found = KeyRef::new(&[], Some(("p", "zai-api-key")))
        .resolve(&google)
        .await
        .unwrap();
    assert_eq!(
        found.unwrap().expose(),
        "sk-test-zai-key-from-secret-manager"
    );
    let absent = KeyRef::new(&[], Some(("p", "nothing-here")))
        .resolve(&google)
        .await
        .unwrap();
    assert!(absent.is_none());

    let seen = stub.seen();
    assert_eq!(seen.len(), 3);
    assert_eq!(seen[0].headers["metadata-flavor"], "Google");
    assert_eq!(
        seen[1].path,
        "/v1/projects/p/secrets/zai-api-key/versions/latest:access"
    );
    assert_eq!(
        seen[1].headers["authorization"],
        "Bearer ya29.from-metadata"
    );
}

#[test]
fn keys_come_from_the_environment_or_a_mounted_file_and_never_print() {
    let dir = std::env::temp_dir().join(format!("inference-key-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("key");
    std::fs::write(&file, "sk-from-file-0123456789\n").unwrap();
    let path = file.to_string_lossy().into_owned();
    let refs = KeyRef::new(&["A_KEY", "B_KEY"], None);
    let env = |vars: Vec<(&'static str, String)>| {
        move |name: &str| {
            vars.iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| v.clone())
        }
    };
    assert_eq!(
        refs.local_with(&env(vec![("B_KEY", "sk-b".into())]))
            .unwrap()
            .expose(),
        "sk-b"
    );
    assert_eq!(
        refs.local_with(&env(vec![("A_KEY_FILE", path)]))
            .unwrap()
            .expose(),
        "sk-from-file-0123456789"
    );
    assert!(
        refs.local_with(&env(vec![("A_KEY", "  ".into())]))
            .is_none()
    );
    let secret = Secret::new(KEY).unwrap();
    assert!(!format!("{secret:?} {secret}").contains("sk-test"));
    let adapter = zai_at("http://127.0.0.1:9", key());
    assert!(!format!("{:?}", adapter.config()).contains(KEY));
    std::fs::remove_dir_all(&dir).unwrap();
}

// ---------------------------------------------------------------------
// Z.ai.

#[tokio::test]
async fn zai_streams_its_thinking_as_reasoning_never_as_answer() {
    let stub = stub(vec![Reply::stream(&fixture("zai-glm-5.3-flash.sse"))]).await;
    let adapter = zai_at(&stub.url, key());
    let mut req = count_request("standard");
    req.max_output_tokens = Some(256);
    req.input = Some(inference::Input::Items(vec![
        serde_json::from_value(json!({"role": "developer", "content": "Be brief."})).unwrap(),
        serde_json::from_value(
            json!({"role": "user", "content": "Count from one to five, one word per line."}),
        )
        .unwrap(),
    ]));
    let sent = adapter.send(&req, "zai/glm-5.3-flash").await.expect("sent");
    let meter = sent.meter.clone();
    let (events, error) = drain(sent).await;
    assert!(error.is_none(), "{error:?}");
    check(&events);
    let response = folded(&events);
    assert_eq!(response.model, "zai/glm-5.3-flash");
    assert_eq!(response.output_text(), "One\nTwo\nThree\nFour\nFive");
    let Item::Reasoning(reasoning) = &response.output[0] else {
        panic!("first item is reasoning");
    };
    let thought = format!("{}{}", reasoning.summary_text(), reasoning.content_text());
    assert!(thought.starts_with("The user wants me to count"));
    assert!(!response.output_text().contains("user wants"));
    assert_eq!(response.usage.as_ref().unwrap().input_tokens, 38);

    let seen = stub.seen();
    assert_eq!(seen[0].path, "/api/paas/v4/chat/completions");
    assert_eq!(seen[0].headers["authorization"], format!("Bearer {KEY}"));
    let body = seen[0].json();
    assert_eq!(body["model"], "glm-5.3-flash");
    assert_eq!(body["thinking"], json!({"type": "enabled"}));
    assert_eq!(body["max_tokens"], 256);
    assert!(body.get("max_completion_tokens").is_none());
    assert!(body.get("reasoning_effort").is_none());
    assert!(body.get("openagents").is_none());
    assert_eq!(body["stream"], true);
    assert_eq!(body["stream_options"]["include_usage"], true);
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(
        body["messages"][1]["role"], "system",
        "developer becomes system"
    );
    assert_eq!(body["messages"][2]["role"], "user");

    let record = meter.attempt(template());
    assert_eq!(record.upstream, "zai");
    assert_eq!(record.account.as_deref(), Some("zai-credit"));
    assert_eq!(record.tokens.input, 38);
    assert_eq!(record.tokens.output, 45);
    assert_eq!(record.outcome, meter::Outcome::Ok);
}

#[tokio::test]
async fn zai_refuses_strict_privacy_and_waits_for_its_key_without_calling() {
    let stub = stub(vec![Reply::stream(&fixture("zai-glm-5.3-flash.sse"))]).await;
    let strict = zai_at(&stub.url, key())
        .send(&count_request("strict"), "zai/glm-5.3-flash")
        .await
        .unwrap_err();
    assert_eq!(strict.class, ErrorClass::PrivacyRefused);
    let default = zai_at(&stub.url, key())
        .send(&request(json!({"input": "hi"})), "zai/glm-5.3-flash")
        .await
        .unwrap_err();
    assert_eq!(
        default.class,
        ErrorClass::PrivacyRefused,
        "strict is the default"
    );
    let keyless = zai_at(&stub.url, None);
    assert!(!keyless.configured());
    let unconfigured = keyless
        .send(&count_request("standard"), "zai/glm-5.3-flash")
        .await
        .unwrap_err();
    assert_eq!(unconfigured.class, ErrorClass::Unconfigured);
    assert!(unconfigured.class.benches());
    let unknown = zai_at(&stub.url, key())
        .send(&count_request("standard"), "zai/glm-9")
        .await
        .unwrap_err();
    assert_eq!(unknown.class, ErrorClass::Unsupported);
    assert!(stub.seen().is_empty(), "nothing was sent");
}

#[tokio::test]
async fn an_empty_answer_and_an_error_chunk_before_output_fall_back() {
    let empty = "data: {\"id\":\"x\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":0,\"total_tokens\":3}}\n\ndata: [DONE]\n\n";
    let failing = "data: {\"error\":{\"code\":\"1305\",\"message\":\"The service may be temporarily overloaded, please try again later\"}}\n\n";
    let stub = stub(vec![Reply::stream(empty), Reply::stream(failing)]).await;
    let adapter = zai_at(&stub.url, key());
    let collect = Arc::new(Collect::default());
    for expected in [ErrorClass::Empty, ErrorClass::Upstream] {
        let sent = adapter
            .send(&count_request("standard"), "zai/glm-5.3-flash")
            .await
            .unwrap();
        sent.meter.report_to(collect.clone(), template());
        let (events, error) = drain(sent).await;
        assert!(
            events.is_empty(),
            "nothing goes out before the first token: {events:?}"
        );
        assert_eq!(error.unwrap().class, expected);
    }
    let records = collect.taken();
    assert_eq!(records.len(), 2);
    assert!(
        records
            .iter()
            .all(|r| r.outcome == meter::Outcome::Fallback)
    );
    assert_eq!(records[0].error, Some(meter::ErrorClass::EmptyStream));
    assert_eq!(records[1].error, Some(meter::ErrorClass::Server));
}

// ---------------------------------------------------------------------
// The Pro door.

#[tokio::test]
async fn pro_recorded_stream_and_the_doors_body_rules() {
    let stub = stub(vec![Reply::stream(&fixture("pro-gpt-5.6-luna.sse"))]).await;
    let adapter = pro_at(&stub.url);
    let req = request(json!({
        "input": [{"role": "developer", "content": "Be terse."}, {"role": "user", "content": "Count to five."}],
        "max_output_tokens": 400,
        "temperature": 0,
        "reasoning": {"effort": "low"},
        "store": false,
        "metadata": {"job": "x"},
        "openagents": {"privacy": "standard"},
    }));
    let sent = adapter
        .send(&req, "openai/gpt-5.6-luna")
        .await
        .expect("sent");
    let meter = sent.meter.clone();
    let (events, error) = drain(sent).await;
    assert!(error.is_none(), "{error:?}");
    check(&events);
    let response = folded(&events);
    assert_eq!(response.model, "openai/gpt-5.6-luna");
    assert_eq!(
        response.output_text(),
        "One\n\nTwo\n\nThree\n\nFour\n\nFive"
    );
    let usage = response.usage.unwrap();
    assert_eq!((usage.input_tokens, usage.output_tokens), (35, 12));

    let body = stub.seen()[0].json();
    assert_eq!(body["model"], "openai/gpt-5.6-luna");
    assert_eq!(body["max_completion_tokens"], 400);
    assert!(body.get("max_tokens").is_none());
    assert!(
        body.get("temperature").is_none(),
        "temperature 0 is dropped"
    );
    assert_eq!(body["reasoning_effort"], "low");
    assert_eq!(body["messages"][0]["role"], "developer");
    assert!(body.get("store").is_none() && body.get("metadata").is_none());
    assert_eq!(body["stream_options"]["include_usage"], true);
    assert_eq!(
        meter.attempt(template()).account.as_deref(),
        Some("pro-free-capacity")
    );
}

#[tokio::test]
async fn pro_takes_no_tools_and_never_names_its_upstream() {
    let stub = stub(vec![Reply::json(
        400,
        json!({"error": {"message": "Stripe: invalid model for llm.stripe.com, key sk-live-abcdefghijklmnopqrstuvwxyz", "type": "invalid_request_error"}}),
    )])
    .await;
    let adapter = pro_at(&stub.url);
    let tools = request(json!({
        "input": "Call it.",
        "tools": [{"type": "function", "name": "f"}],
        "openagents": {"privacy": "standard"},
    }));
    let refused = adapter
        .send(&tools, "openai/gpt-5.6-sol")
        .await
        .unwrap_err();
    assert_eq!(refused.class, ErrorClass::Unsupported);
    assert!(stub.seen().is_empty());
    let strict = adapter
        .send(&count_request("strict"), "openai/gpt-5.6-sol")
        .await
        .unwrap_err();
    assert_eq!(strict.class, ErrorClass::PrivacyRefused);

    let error = adapter
        .send(&count_request("standard"), "openai/gpt-5.6-sol")
        .await
        .unwrap_err();
    assert_eq!(error.class, ErrorClass::BadRequest);
    assert!(!error.class.falls_back());
    let text = error.to_string().to_lowercase();
    assert!(!text.contains("stripe"), "{text}");
    assert!(!text.contains("sk-live"), "{text}");
}

// ---------------------------------------------------------------------
// OpenRouter and Vercel.

#[tokio::test]
async fn openrouter_recorded_stream_with_privacy_and_reported_cost() {
    let stub = stub(vec![Reply::stream(&common::recorded(
        "stealth-space-bunny-alpha.sse",
    ))])
    .await;
    let adapter = openrouter_at(&stub.url);
    let mut req = count_request("strict");
    req.reasoning = Some(serde_json::from_value(json!({"effort": "low"})).unwrap());
    req.extra
        .insert("provider".into(), json!({"order": ["chutes"]}));
    let sent = adapter
        .send(&req, "stealth/space-bunny-alpha")
        .await
        .expect("sent");
    let meter = sent.meter.clone();
    let (events, error) = drain(sent).await;
    assert!(error.is_none(), "{error:?}");
    check(&events);
    assert!(
        events
            .iter()
            .any(|e| matches!(e.body, EventBody::ReasoningDelta(_)))
    );
    let response = folded(&events);
    assert_eq!(response.output_text(), "one\ntwo\nthree\nfour\nfive");

    let seen = stub.seen();
    assert_eq!(seen[0].path, "/api/v1/responses");
    assert_eq!(seen[0].headers["x-title"], "OpenAgents");
    let body = seen[0].json();
    assert_eq!(body["model"], "stealth/space-bunny-alpha");
    assert_eq!(body["store"], false);
    assert_eq!(body["stream"], true);
    assert!(body.get("openagents").is_none());
    assert_eq!(
        body["provider"],
        json!({"order": ["chutes"], "data_collection": "deny", "zdr": true}),
        "privacy merges into the caller's provider object"
    );

    let record = meter.attempt(template());
    assert_eq!(record.reported_cost, Some(0));
    assert_eq!(record.tokens.input, 176);
    assert_eq!(record.tokens.cached_input, 140);

    let standard = openrouter::privacy_fields(&inference::openagents::Privacy::Standard);
    assert_eq!(
        Value::Object(standard),
        json!({"provider": {"data_collection": "deny"}})
    );
}

#[tokio::test]
async fn vercel_recorded_streams_with_zero_retention_and_reported_cost() {
    for (file, model, text) in [
        (
            "google-gemini-3.8-flash.sse",
            "google/gemini-3.8-flash",
            "One\nTwo\nThree\nFour\nFive",
        ),
        (
            "zai-glm-5.3-flash.sse",
            "zai/glm-5.3-flash",
            "one\ntwo\nthree\nfour\nfive",
        ),
    ] {
        let stub = stub(vec![Reply::stream(&common::recorded(file))]).await;
        let adapter = vercel_at(&stub.url);
        let sent = adapter
            .send(&count_request("strict"), model)
            .await
            .expect("sent");
        let meter = sent.meter.clone();
        let (events, error) = drain(sent).await;
        assert!(error.is_none(), "{file}: {error:?}");
        check(&events);
        assert_eq!(folded(&events).output_text().trim(), text, "{file}");
        let body = stub.seen()[0].json();
        assert_eq!(
            body["providerOptions"]["gateway"]["zeroDataRetention"],
            true
        );
        assert_eq!(body["model"], model);
        let record = meter.attempt(template());
        assert_eq!(record.upstream, "vercel");
        assert_eq!(record.reported_cost, Some(0), "{file}: BYOK cost is 0");
        assert!(record.first_token_ms.is_some());
    }
    let standard = vercel::privacy_fields(&inference::openagents::Privacy::Standard);
    assert!(standard.is_empty());
}

#[tokio::test]
async fn status_errors_carry_class_status_and_retry_after() {
    let stub = stub(vec![
        Reply::json(429, json!({"error": {"message": "Rate limit exceeded: free-models-per-min", "code": 429}}))
            .header("retry-after", "7"),
        Reply::json(401, json!({"error": {"message": "No auth credentials found", "code": 401}})),
        Reply::json(402, json!({"error": {"message": "Insufficient credits", "code": 402}})),
        Reply::json(503, json!({"error": {"message": "No endpoints found matching your data policy", "code": 503}})),
    ])
    .await;
    let adapter = openrouter_at(&stub.url);
    let collect = Arc::new(Collect::default());
    let mut classes = Vec::new();
    for _ in 0..4 {
        let error = adapter
            .send(&count_request("strict"), "stealth/space-bunny-alpha")
            .await
            .unwrap_err();
        classes.push((error.class, error.status, error.retry_after));
    }
    assert_eq!(
        classes,
        vec![
            (ErrorClass::RateLimited, Some(429), Some(7)),
            (ErrorClass::Auth, Some(401), None),
            (ErrorClass::Payment, Some(402), None),
            (ErrorClass::Upstream, Some(503), None),
        ]
    );
    assert!(ErrorClass::Auth.benches() && ErrorClass::Payment.benches());
    assert!(!ErrorClass::RateLimited.benches());
    drop(collect);
}

#[tokio::test]
async fn an_error_event_before_output_falls_back_and_a_break_after_output_fails_the_response() {
    let early = "event: error\ndata: {\"type\":\"error\",\"sequence_number\":0,\"error\":{\"type\":\"server_error\",\"code\":\"overloaded\",\"param\":null,\"message\":\"Provider overloaded\"}}\n\n";
    let recorded = common::recorded("google-gemini-3.8-flash.sse");
    // Cut the stream after the first text delta.
    let cut = recorded.find("response.output_text.delta").unwrap();
    let cut = cut + recorded[cut..].find("\n\n").unwrap() + 2;
    let broken = recorded[..cut].to_owned();
    let stub = stub(vec![Reply::stream(early), Reply::stream(&broken)]).await;
    let adapter = vercel_at(&stub.url);

    let sent = adapter
        .send(&count_request("strict"), "google/gemini-3.8-flash")
        .await
        .unwrap();
    let meter = sent.meter.clone();
    let (events, error) = drain(sent).await;
    assert!(events.is_empty());
    let error = error.unwrap();
    assert_eq!(error.class, ErrorClass::Upstream);
    assert_eq!(error.message, "Provider overloaded");
    assert_eq!(meter.attempt(template()).outcome, meter::Outcome::Fallback);

    let sent = adapter
        .send(&count_request("strict"), "google/gemini-3.8-flash")
        .await
        .unwrap();
    let meter = sent.meter.clone();
    let (events, error) = drain(sent).await;
    assert!(
        error.is_none(),
        "after the first token a failure is the caller's"
    );
    check(&events);
    let response = terminal(&events);
    assert_eq!(response.status, ResponseStatus::Failed);
    assert_eq!(response.error.as_ref().unwrap().code, "upstream_failed");
    let record = meter.attempt(template());
    assert_eq!(record.outcome, meter::Outcome::Failed);
    assert_eq!(record.error, Some(meter::ErrorClass::StreamFailed));
    assert!(record.first_token_ms.is_some());
}

#[tokio::test]
async fn a_dropped_stream_is_recorded_as_canceled() {
    let mut reply = Reply::stream(&common::recorded("zai-glm-5.3-flash.sse"));
    reply.delay_ms = 0;
    let stub = stub(vec![reply]).await;
    let adapter = vercel_at(&stub.url);
    let collect = Arc::new(Collect::default());
    let sent = adapter
        .send(&count_request("strict"), "zai/glm-5.3-flash")
        .await
        .unwrap();
    sent.meter.report_to(collect.clone(), template());
    let mut events = sent.events;
    let first = events.next().await.unwrap().unwrap();
    assert_eq!(first.type_name(), "response.created");
    drop(events);
    let records = collect.taken();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].outcome, meter::Outcome::Canceled);
    assert_eq!(records[0].request_id, "req_test");
}

#[tokio::test]
async fn the_meter_prices_a_reported_attempt_from_the_adapters_rate_rows() {
    let stub = stub(vec![Reply::stream(&fixture("zai-glm-5.3-flash.sse"))]).await;
    let adapter = zai_at(&stub.url, key());
    let config = meter::Config {
        rates: adapter.rate_rows(),
        ..meter::Config::default()
    };
    let rates = Arc::new(meter::Meter::with_sink(
        &config,
        Box::new(meter::CollectAlerts::default()),
    ));
    let sent = adapter
        .send(&count_request("standard"), "zai/glm-5.3-flash")
        .await
        .unwrap();
    let watched = sent.meter.clone();
    sent.meter.report_to(rates.clone(), template());
    let _ = drain(sent).await;
    assert_eq!(watched.snapshot().stage, Stage::Completed);
    let now = watched.snapshot().at_ms + 60_000;
    let live = rates.rates(3_600_000, now);
    assert_eq!(live.len(), 1, "{live:?}");
    // 38 input at $0.15/M and 45 output at $0.50/M: 5.7 + 22.5 = 28.2 micros.
    let status = rates.status(now).unwrap();
    assert_eq!(status.records.unpriced, 0);
}

#[test]
fn rate_rows_carry_list_prices_and_the_recommended_margin() {
    // The router's view: Pro offers no tools and, terms unconfirmed, no
    // zero retention; Vercel offers zero retention.
    let offers = pro::from_env().offerings();
    assert_eq!(offers.len(), 3);
    assert!(
        offers
            .iter()
            .all(|o| !o.capabilities.tools && !o.zero_retention)
    );
    assert_eq!(offers[0].account.as_deref(), Some("pro-free-capacity"));
    assert!(
        vercel::from_env()
            .offerings()
            .iter()
            .all(|o| o.zero_retention)
    );
    let rows = pro::from_env().rate_rows();
    let luna = rows
        .iter()
        .find(|row| row.model == "openai/gpt-5.6-luna")
        .unwrap();
    assert_eq!(
        (luna.input, luna.output, luna.cached_input),
        (200_000, 1_200_000, Some(20_000))
    );
    assert_eq!(luna.cache_write, Some(250_000));
    assert_eq!(luna.margin_bps, 500);
    assert_eq!(luna.upstream, "pro");
    let glm = zai::from_env().rate_rows();
    let priced = glm[0].price(&meter::Tokens {
        input: 38,
        output: 45,
        ..meter::Tokens::default()
    });
    assert_eq!(
        priced.cost, 28,
        "matches Vercel's recorded marketCost of $0.0000282"
    );
}

#[test]
fn reported_costs_parse_without_floats() {
    use inference::upstream::measure::usd_micros;
    assert_eq!(usd_micros(&json!("0.0000282")), Some(28));
    assert_eq!(usd_micros(&json!("0.00050625")), Some(506));
    assert_eq!(usd_micros(&json!(0)), Some(0));
    assert_eq!(usd_micros(&json!("12.5")), Some(12_500_000));
    assert_eq!(usd_micros(&json!(1.2e-5)), Some(12));
    assert_eq!(usd_micros(&json!("n/a")), None);
}

#[tokio::test]
async fn direct_gemini_uses_the_callers_header_and_native_codec() {
    let server = stub(vec![Reply::stream(&fixture("vertex-gemini-3.8-flash.sse"))]).await;
    let mut upstream =
        inference::upstream::gemini::Gemini::new(Secret::new("stub-credential").unwrap());
    upstream.base_url = server.url.clone();
    let sent = upstream
        .send(&count_request("standard"), "google/gemini-3.8-flash")
        .await
        .unwrap();
    let (events, error) = drain(sent).await;
    assert!(error.is_none());
    check(&events);
    assert!(folded(&events).usage.is_some());
    let seen = server.seen();
    assert_eq!(seen[0].headers["x-goog-api-key"], "stub-credential");
    assert!(!seen[0].headers.contains_key("authorization"));
    assert_eq!(
        seen[0].path,
        "/v1beta/models/gemini-3.8-flash:streamGenerateContent?alt=sse"
    );
    assert!(!seen[0].json().to_string().contains("stub-credential"));
    assert_eq!(
        upstream
            .send(&count_request("strict"), "google/gemini-3.8-flash")
            .await
            .unwrap_err()
            .class,
        ErrorClass::PrivacyRefused
    );
    assert_eq!(server.seen().len(), 1);
}

fn anthropic_stream(chunks: &[Value]) -> String {
    chunks
        .iter()
        .map(|chunk| {
            format!(
                "event: {}\ndata: {chunk}\n\n",
                chunk["type"].as_str().unwrap()
            )
        })
        .collect()
}

#[tokio::test]
async fn anthropic_streams_signed_thinking_tools_and_cumulative_usage() {
    let body = anthropic_stream(&[
        json!({"type":"message_start","message":{"usage":{"input_tokens":10,"cache_read_input_tokens":4,"cache_creation_input_tokens":2,"output_tokens":1}}}),
        json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"Think"}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"signed"}}),
        json!({"type":"content_block_stop","index":0}),
        json!({"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"call_1","name":"weather","input":{}}}),
        json!({"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"city\":\"Paris\"}"}}),
        json!({"type":"content_block_stop","index":1}),
        json!({"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":9}}),
        json!({"type":"message_stop"}),
    ]);
    let server = stub(vec![Reply::stream(&body)]).await;
    let mut upstream =
        inference::upstream::anthropic::Anthropic::new(Secret::new("stub-credential").unwrap());
    upstream.url = format!("{}/v1/messages", server.url);
    let req = request(
        json!({"input":"Weather?","instructions":"Be brief.","max_output_tokens":64,"tools":[{"type":"function","name":"weather","parameters":{"type":"object"}}],"openagents":{"privacy":"standard"}}),
    );
    let sent = upstream
        .send(&req, "anthropic/claude-sonnet-5-5")
        .await
        .unwrap();
    let (events, error) = drain(sent).await;
    assert!(error.is_none());
    check(&events);
    let response = folded(&events);
    assert_eq!(response.usage.as_ref().unwrap().input_tokens, 16);
    assert_eq!(response.usage.as_ref().unwrap().output_tokens, 9);
    assert_eq!(
        response
            .usage
            .as_ref()
            .unwrap()
            .input_tokens_details
            .cached_tokens,
        4
    );
    assert!(
        matches!(&response.output[0], Item::Reasoning(item) if item.encrypted_content.as_deref() == Some("signed"))
    );
    assert!(
        matches!(&response.output[1], Item::FunctionCall(item) if item.arguments == "{\"city\":\"Paris\"}")
    );
    let seen = server.seen();
    assert_eq!(seen[0].headers["x-api-key"], "stub-credential");
    assert_eq!(seen[0].headers["anthropic-version"], "2023-06-01");
    assert_eq!(seen[0].json()["model"], "claude-sonnet-5-5");
    assert_eq!(seen[0].json()["max_tokens"], 64);
    assert!(seen[0].json().get("openagents").is_none());
}

#[tokio::test]
async fn anthropic_broken_streams_fail_before_and_after_output() {
    for output in [false, true] {
        let mut chunks =
            vec![json!({"type":"message_start","message":{"usage":{"input_tokens":1}}})];
        if output {
            chunks.push(json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}));
        }
        chunks.push(
            json!({"type":"error","error":{"type":"overloaded_error","message":"stub-credential"}}),
        );
        let server = stub(vec![Reply::stream(&anthropic_stream(&chunks))]).await;
        let mut upstream =
            inference::upstream::anthropic::Anthropic::new(Secret::new("stub-credential").unwrap());
        upstream.url = server.url;
        let (events, error) = drain(
            upstream
                .send(&count_request("standard"), "anthropic/claude-sonnet-5-5")
                .await
                .unwrap(),
        )
        .await;
        if output {
            assert_eq!(folded(&events).status, ResponseStatus::Failed);
            check(&events);
        } else {
            assert_eq!(error.unwrap().class, ErrorClass::Upstream);
        }
        assert!(!format!("{events:?}").contains("stub-credential"));
    }
}

#[tokio::test]
async fn direct_openai_streams_responses_without_gateway_fields() {
    let req = count_request("standard");
    let mut emitter = inference::upstream::emit::Emitter::new("gpt-5.6-luna", &req);
    let mut events = emitter.text("Hello");
    emitter.usage(inference::Usage::new(3, 0, 1, 0));
    events.extend(emitter.finish(inference::upstream::emit::Finish::Completed));
    let body: String = events
        .iter()
        .map(|event| inference::sse::encode_event(event))
        .collect();
    let server = stub(vec![Reply::stream(&body)]).await;
    let mut config = inference::upstream::openai::config(Secret::new("stub-credential"));
    config.url = format!("{}/v1/responses", server.url);
    let upstream = ResponsesUpstream::new(config);
    let (events, error) = drain(upstream.send(&req, "openai/gpt-5.6-luna").await.unwrap()).await;
    assert!(error.is_none());
    check(&events);
    assert_eq!(folded(&events).model, "openai/gpt-5.6-luna");
    let seen = server.seen();
    assert_eq!(seen[0].headers["authorization"], "Bearer stub-credential");
    assert_eq!(seen[0].json()["model"], "gpt-5.6-luna");
    assert_eq!(seen[0].json()["store"], false);
    assert_eq!(seen[0].json()["stream"], true);
    assert!(seen[0].json().get("openagents").is_none());
}

#[test]
fn anthropic_preserves_tool_history_and_refuses_unsigned_thinking() {
    let upstream =
        inference::upstream::anthropic::Anthropic::new(Secret::new("stub-credential").unwrap());
    let row = &upstream.models()[0];
    let req = request(json!({"input":[
        {"role":"developer","content":"Be brief."},
        {"role":"user","content":"Weather?"},
        {"type":"reasoning","summary":[{"type":"summary_text","text":"Think"}],"encrypted_content":"signed"},
        {"type":"function_call","call_id":"c1","name":"weather","arguments":"{}"},
        {"type":"function_call_output","call_id":"c1","output":"Sunny"}
    ]}));
    let body = inference::upstream::anthropic::body(&req, row).unwrap();
    assert_eq!(body["system"], "Be brief.");
    assert_eq!(body["messages"][1]["role"], "assistant");
    assert_eq!(body["messages"][1]["content"][0]["signature"], "signed");
    assert_eq!(body["messages"][1]["content"][1]["type"], "tool_use");
    assert_eq!(body["messages"][2]["content"][0]["tool_use_id"], "c1");
    let unsigned = request(json!({"input":[{"type":"reasoning","summary":[]}]}));
    assert_eq!(
        inference::upstream::anthropic::body(&unsigned, row)
            .unwrap_err()
            .class,
        ErrorClass::Unsupported
    );
}

#[tokio::test]
async fn anthropic_empty_tool_arguments_and_missing_usage_are_honest() {
    let chunks = [
        json!({"type":"message_start","message":{}}),
        json!({"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"call_empty","name":"ping","input":{}}}),
        json!({"type":"content_block_stop","index":0}),
        json!({"type":"message_delta","delta":{"stop_reason":"tool_use"}}),
        json!({"type":"message_stop"}),
    ];
    let server = stub(vec![Reply::stream(&anthropic_stream(&chunks))]).await;
    let mut upstream =
        inference::upstream::anthropic::Anthropic::new(Secret::new("stub-credential").unwrap());
    upstream.url = server.url;
    let (events, error) = drain(
        upstream
            .send(&count_request("standard"), "anthropic/claude-sonnet-5-5")
            .await
            .unwrap(),
    )
    .await;
    assert!(error.is_none());
    check(&events);
    let response = folded(&events);
    assert!(response.usage.is_none());
    assert!(matches!(&response.output[0], Item::FunctionCall(call) if call.arguments == "{}"));
}
