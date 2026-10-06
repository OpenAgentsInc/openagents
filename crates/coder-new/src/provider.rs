//! Direct OpenRouter key checks and single-attempt streamed chat requests.

use std::collections::BTreeSet;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use openrouter::{ApiKey, ChatRequest, Client, Config, Message, Streamed};
use reqwest::{StatusCode, redirect::Policy};
use serde_json::{Value, json};

use crate::bundled_runtime::RuntimeEvent;
use crate::plugin_tools::{ExecutionSettings, GenerationProvider, redact_value};

const CHECK_TIMEOUT: Duration = Duration::from_secs(15);
const STREAM_TIMEOUT: Duration = Duration::from_secs(180);
const KEY_BODY_LIMIT: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyInfo {
    pub status: &'static str,
    /// The remaining per-key spending allowance, not the account balance.
    pub limit_remaining: Option<f64>,
}

#[derive(Clone)]
pub struct Provider {
    key: ApiKey,
    check_http: reqwest::Client,
    chat: Client,
    base_url: String,
}

impl Provider {
    pub fn new(key: ApiKey) -> Result<Self, String> {
        Self::build(key, openrouter::BASE_URL)
    }

    fn build(key: ApiKey, base_url: &str) -> Result<Self, String> {
        if key.expose().is_empty()
            || !key.expose().is_ascii()
            || key
                .expose()
                .chars()
                .any(|character| character.is_whitespace() || character.is_control())
        {
            return Err("Enter an OpenRouter API key without spaces or control characters.".into());
        }
        let check_http = reqwest::Client::builder()
            .timeout(CHECK_TIMEOUT)
            .connect_timeout(CHECK_TIMEOUT)
            .redirect(Policy::none())
            .build()
            .map_err(|_| "The OpenRouter connection could not start.".to_owned())?;
        let mut config = Config::new(key.clone()).base_url(base_url);
        config.timeout = STREAM_TIMEOUT;
        config.retries = 0;
        let chat = Client::new(config).map_err(|_| "The OpenRouter connection could not start.")?;
        Ok(Self {
            key,
            check_http,
            chat,
            base_url: base_url.trim_end_matches('/').to_owned(),
        })
    }

    /// Checks the key without returning its label or any response-body text.
    pub async fn check(&self) -> Result<KeyInfo, String> {
        let mut response = self
            .check_http
            .get(format!("{}/key", self.base_url))
            .bearer_auth(self.key.expose())
            .send()
            .await
            .map_err(check_error)?;
        if !response.status().is_success() {
            return Err(status_error(response.status().as_u16()));
        }
        if response
            .content_length()
            .is_some_and(|length| length > KEY_BODY_LIMIT as u64)
        {
            return Err("OpenRouter returned an invalid key response.".into());
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(check_error)? {
            if body.len().saturating_add(chunk.len()) > KEY_BODY_LIMIT {
                return Err("OpenRouter returned an invalid key response.".into());
            }
            body.extend_from_slice(&chunk);
        }
        let value: Value = serde_json::from_slice(&body)
            .map_err(|_| "OpenRouter returned an invalid key response.".to_owned())?;
        let data = value
            .get("data")
            .and_then(Value::as_object)
            .ok_or_else(|| "OpenRouter returned an invalid key response.".to_owned())?;
        let limit_remaining = match data.get("limit_remaining") {
            Some(Value::Null) => None,
            Some(value) => Some(
                value
                    .as_f64()
                    .filter(|amount| amount.is_finite())
                    .ok_or_else(|| "OpenRouter returned an invalid key response.".to_owned())?,
            ),
            None => return Err("OpenRouter returned an invalid key response.".into()),
        };
        Ok(KeyInfo {
            status: "Verified",
            limit_remaining,
        })
    }

    /// Streams one request. A blank model uses the free router.
    pub async fn stream(
        &self,
        model: &str,
        messages: Vec<Message>,
        callback: &mut (dyn FnMut(&str) + Send),
    ) -> Result<Streamed, String> {
        self.stream_with_options(
            model,
            &crate::models::GenerationOptions::default(),
            messages,
            callback,
        )
        .await
    }

    pub async fn stream_with_options(
        &self,
        model: &str,
        options: &crate::models::GenerationOptions,
        messages: Vec<Message>,
        callback: &mut (dyn FnMut(&str) + Send),
    ) -> Result<Streamed, String> {
        self.stream_with_options_and_model(model, options, messages, callback, &mut |_| {})
            .await
    }

    pub async fn stream_with_options_and_model(
        &self,
        model: &str,
        options: &crate::models::GenerationOptions,
        messages: Vec<Message>,
        callback: &mut (dyn FnMut(&str) + Send),
        model_callback: &mut (dyn FnMut(&str) + Send),
    ) -> Result<Streamed, String> {
        if messages.is_empty() {
            return Err("Add a message before requesting a reply.".into());
        }
        if !options.valid() {
            return Err("The model settings are invalid.".into());
        }
        let model = model.trim();
        let mut request = ChatRequest::new(
            if model.is_empty() {
                crate::models::DEFAULT_MODEL
            } else {
                model
            },
            messages,
        );
        if let Some(effort) = &options.reasoning {
            request = request.effort(effort);
        }
        if let Some(limit) = options.max_tokens {
            request = request.max_tokens(limit);
        }
        self.chat
            .stream_with_model(&request, callback, model_callback)
            .await
            .map_err(stream_error)
    }

    /// Execute each complete call once and continue under a bounded turn snapshot.
    pub async fn chat_with_plugins(
        &self,
        model: &str,
        options: &crate::models::GenerationOptions,
        messages: Vec<Message>,
        execution: &ExecutionSettings,
        callback: &mut (dyn FnMut(&str) + Send),
        model_callback: &mut (dyn FnMut(&str) + Send),
        event_callback: &mut (dyn FnMut(RuntimeEvent) + Send),
        cancel: &Arc<AtomicBool>,
    ) -> Result<Streamed, String> {
        if messages.is_empty() {
            return Err("Add a message before requesting a reply.".into());
        }
        if !options.valid() {
            return Err("The model settings are invalid.".into());
        }
        let definitions = execution.defs();
        if definitions.is_empty() {
            return tokio::select! {
                result = self.stream_with_options_and_model(model, options, messages, callback, model_callback) => result,
                () = canceled(cancel) => Err("The OpenRouter request was canceled; whether it was billed is unknown.".into()),
            };
        }
        let model = if model.trim().is_empty() {
            crate::models::DEFAULT_MODEL
        } else {
            model.trim()
        };
        let mut request = ChatRequest::new(model, vec![]);
        if let Some(effort) = &options.reasoning {
            request = request.effort(effort);
        }
        if let Some(limit) = options.max_tokens {
            request = request.max_tokens(limit);
        }
        let mut history = vec![json!({"role":"system","content":execution.instructions()})];
        history.extend(
            messages
                .iter()
                .map(|message| json!({"role":message.role,"content":message.content})),
        );
        let started = Instant::now();
        let mut joined = String::new();
        let mut aggregate = Streamed::default();
        let mut seen = BTreeSet::new();
        let mut calls_used = 0usize;
        let provider = GenerationProvider {
            client: self.chat.clone(),
            model: model.into(),
            effort: options.reasoning.clone(),
        };
        for round in 0..8 {
            if cancel.load(Ordering::Relaxed) {
                return Err("The reply was canceled.".into());
            }
            let mut first = true;
            let mut sink = |delta: &str| {
                if delta.is_empty() {
                    return;
                }
                if first && !joined.is_empty() {
                    callback("\n\n");
                    joined.push_str("\n\n");
                }
                first = false;
                callback(delta);
                joined.push_str(delta);
            };
            let streamed = tokio::select! {
                result = self.chat.stream_tools(&request, &history, &definitions, &mut sink, model_callback) => result.map_err(stream_error)?,
                () = canceled(cancel) => return Err("The OpenRouter request was canceled; whether it was billed is unknown.".into()),
            };
            aggregate_usage(&mut aggregate.usage, &streamed.reply.usage, round == 0);
            if aggregate.first_text_ms.is_none() {
                aggregate.first_text_ms = streamed.reply.first_text_ms.map(|millis| {
                    u64::try_from(started.elapsed().as_millis())
                        .unwrap_or(u64::MAX)
                        .saturating_sub(streamed.reply.milliseconds)
                        .saturating_add(millis)
                });
            }
            if !streamed.reply.model.is_empty() {
                aggregate.model = streamed.reply.model.clone();
            }
            aggregate.finish_reason = streamed.reply.finish_reason.clone();
            if streamed.calls.is_empty() {
                aggregate.text = joined;
                aggregate.milliseconds =
                    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                return Ok(aggregate);
            }
            if round == 7 || calls_used.saturating_add(streamed.calls.len()) > 32 {
                return Err("The turn reached its limit of 8 model rounds or 32 plugin calls. Pending calls were not executed.".into());
            }
            for call in &streamed.calls {
                if !seen.insert(call.id.clone()) {
                    return Err(
                        "OpenRouter reused a tool call ID. The repeated call was not executed."
                            .into(),
                    );
                }
            }
            history.push(json!({"role":"assistant","content":streamed.reply.text,"tool_calls":streamed.calls.iter().map(|call|json!({"id":call.id,"type":"function","function":{"name":call.name,"arguments":call.arguments}})).collect::<Vec<_>>()}));
            for call in streamed.calls {
                if cancel.load(Ordering::Relaxed) {
                    return Err("The reply was canceled before its next plugin call.".into());
                }
                calls_used += 1;
                let arguments: Value = serde_json::from_str(&call.arguments)
                    .map_err(|_| "OpenRouter supplied invalid tool arguments.")?;
                let has_credential =
                    !self.key.expose().is_empty() && call.arguments.contains(self.key.expose());
                let mut safe_input = arguments.clone();
                execution.redact(&mut safe_input);
                redact_value(&mut safe_input, self.key.expose());
                event_callback(RuntimeEvent::Tool {
                    name: call.name.clone(),
                    input: safe_input.clone(),
                    output: Value::Null,
                    running: true,
                });
                let mut child_events = |event| {
                    if let RuntimeEvent::Tool {
                        name,
                        input,
                        output,
                        running,
                    } = event
                    {
                        let mut input = input;
                        let mut output = output;
                        execution.redact(&mut input);
                        execution.redact(&mut output);
                        redact_value(&mut input, self.key.expose());
                        redact_value(&mut output, self.key.expose());
                        event_callback(RuntimeEvent::Tool {
                            name: execution
                                .redact_text(&name)
                                .replace(self.key.expose(), "[redacted]"),
                            input,
                            output,
                            running,
                        });
                    }
                };
                let result = if has_credential {
                    Err("Keep API keys in plugin settings, outside tool arguments.".into())
                } else {
                    execution
                        .execute(
                            &call.name,
                            arguments,
                            Some(provider.clone()),
                            cancel,
                            &mut child_events,
                        )
                        .await
                };
                let mut output = match result {
                    Ok(value) => value,
                    Err(error) => json!({"error":error}),
                };
                execution.redact(&mut output);
                redact_value(&mut output, self.key.expose());
                event_callback(RuntimeEvent::Tool {
                    name: call.name.clone(),
                    input: safe_input,
                    output: output.clone(),
                    running: false,
                });
                if cancel.load(Ordering::Relaxed) {
                    return Err("The reply was canceled after its plugin call; completed effects were not replayed.".into());
                }
                history.push(
                    json!({"role":"tool","tool_call_id":call.id,"content":output.to_string()}),
                );
            }
        }
        Err("The turn reached its model-round limit.".into())
    }

    #[cfg(test)]
    pub(crate) fn with_base(key: ApiKey, base_url: &str) -> Result<Self, String> {
        Self::build(key, base_url)
    }
}

async fn canceled(cancel: &AtomicBool) {
    while !cancel.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn aggregate_usage(total: &mut openrouter::Usage, next: &openrouter::Usage, first: bool) {
    if first {
        *total = next.clone();
        return;
    }
    total.prompt_tokens = total.prompt_tokens.saturating_add(next.prompt_tokens);
    total.completion_tokens = total
        .completion_tokens
        .saturating_add(next.completion_tokens);
    total.total_tokens = total.total_tokens.saturating_add(next.total_tokens);
    total.cost = total.cost.zip(next.cost).map(|(left, right)| left + right);
    total.completion_tokens_details = total
        .completion_tokens_details
        .as_ref()
        .and_then(|details| details.reasoning_tokens)
        .zip(
            next.completion_tokens_details
                .as_ref()
                .and_then(|details| details.reasoning_tokens),
        )
        .map(|(left, right)| openrouter::CompletionDetails {
            reasoning_tokens: Some(left.saturating_add(right)),
        });
}

fn check_error(error: reqwest::Error) -> String {
    if error.is_timeout() {
        "The OpenRouter key check timed out.".into()
    } else {
        "The OpenRouter key check could not connect.".into()
    }
}

fn stream_error(error: openrouter::Error) -> String {
    match error {
        openrouter::Error::Api { status, .. } => status_error(status),
        openrouter::Error::Timeout => {
            "The OpenRouter request timed out. It was not retried.".into()
        }
        openrouter::Error::Connection(_) => {
            "The OpenRouter connection failed. The request was not retried.".into()
        }
        openrouter::Error::Decode { .. } | openrouter::Error::Schema { .. } => {
            "OpenRouter returned an incomplete or invalid reply. The request was not retried."
                .into()
        }
        openrouter::Error::NoKey | openrouter::Error::Client(_) => {
            "The OpenRouter connection could not start.".into()
        }
    }
}

fn status_error(status: u16) -> String {
    match StatusCode::from_u16(status).ok() {
        Some(StatusCode::UNAUTHORIZED) => "OpenRouter rejected the API key (HTTP 401).".into(),
        Some(StatusCode::PAYMENT_REQUIRED) => {
            "OpenRouter's credit or request budget is exhausted (HTTP 402).".into()
        }
        Some(StatusCode::TOO_MANY_REQUESTS) => {
            "OpenRouter is rate limited (HTTP 429). Try again later.".into()
        }
        Some(StatusCode::FORBIDDEN) => "OpenRouter denied this request (HTTP 403).".into(),
        Some(StatusCode::NOT_FOUND) => {
            "The OpenRouter model or endpoint is unavailable (HTTP 404).".into()
        }
        _ => format!("OpenRouter could not complete this request (HTTP {status})."),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread::{self, JoinHandle},
    };

    use super::*;

    const FIXTURE_TOKEN: &str = "local-http-fixture-token";

    fn fixture(status: u16, content_type: &str, body: &str) -> (String, JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let response = format!(
            "HTTP/1.1 {status} Fixture\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 4096];
            loop {
                let count = socket.read(&mut buffer).unwrap();
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..count]);
                if let Some(header_end) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&request[..header_end]);
                    let length = header
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if request.len() >= header_end + 4 + length {
                        break;
                    }
                }
            }
            // Divide the response so stream decoding handles network chunk boundaries.
            let middle = response.len() / 2;
            socket.write_all(&response.as_bytes()[..middle]).unwrap();
            socket.write_all(&response.as_bytes()[middle..]).unwrap();
            String::from_utf8(request).unwrap()
        });
        (format!("http://{address}/api/v1"), server)
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    fn sequence(bodies: Vec<String>) -> (String, JoinHandle<Vec<Value>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let mut requests = vec![];
            for body in bodies {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut request = vec![];
                let mut bytes = [0; 4096];
                let header_end = loop {
                    let count = socket.read(&mut bytes).unwrap();
                    request.extend_from_slice(&bytes[..count]);
                    if let Some(end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                        let header = String::from_utf8_lossy(&request[..end]);
                        let length = header
                            .lines()
                            .find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if request.len() >= end + 4 + length {
                            break end;
                        }
                    }
                    assert!(
                        count > 0,
                        "The HTTP fixture closed before the request ended."
                    );
                };
                requests.push(serde_json::from_slice(&request[header_end + 4..]).unwrap());
                write!(socket,"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",body.len()).unwrap();
            }
            requests
        });
        (format!("http://{address}/api/v1"), server)
    }

    fn jev_settings(endpoint: String, key: Option<model_access::ApiKey>) -> ExecutionSettings {
        ExecutionSettings {
            microcoder: false,
            cli: false,
            acp: false,
            jev_enabled: true,
            jev_key: key,
            redaction_keys: vec![],
            jev_model: "jev-fixture".into(),
            jev_endpoint: endpoint,
            agents: vec![],
            cwd: std::path::PathBuf::from("/unused"),
        }
    }

    fn tool_reply(id: &str, arguments: Value) -> String {
        format!(
            "data: {}\n\ndata: [DONE]\n\n",
            json!({"model":"fixture/first","choices":[{"delta":{"content":"Checking.","tool_calls":[{"index":0,"id":id,"function":{"name":"jev","arguments":arguments.to_string()}}]},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":2,"completion_tokens":1,"total_tokens":3,"cost":0.001}})
        )
    }

    #[test]
    fn plugin_turn_dispatches_jev_and_returns_its_result_to_the_next_model_round() {
        let args = json!({"state":{"message":"I want a refund."},"questions":{"refund":{"type":"noul","instructions":"Does the customer ask for a refund?"}}});
        let final_reply = format!(
            "data: {}\n\ndata: [DONE]\n\n",
            json!({"model":"fixture/served","choices":[{"delta":{"content":"Refund is requested."},"finish_reason":"stop"}],"usage":{"prompt_tokens":3,"completion_tokens":1,"total_tokens":4,"cost":0.002}})
        );
        let (base, model_server) = sequence(vec![tool_reply("call-jev", args), final_reply]);
        let jev_body = json!({"model":"jev-fixture","answers":{"refund":{"type":"noul","noul":0.9}},"usage":{"input_tokens":12,"output_tokens":1}}).to_string();
        let (endpoint, jev_server) = fixture(200, "application/json", &jev_body);
        let execution = jev_settings(
            endpoint.trim_end_matches("/api/v1").into(),
            Some(model_access::ApiKey::new("jev-fixture-credential")),
        );
        let provider = Provider::with_base(ApiKey::new(FIXTURE_TOKEN), &base).unwrap();
        let mut text = String::new();
        let mut events = vec![];
        let result = runtime()
            .block_on(provider.chat_with_plugins(
                "openrouter/free",
                &crate::models::GenerationOptions::default(),
                vec![Message::user("Does this customer want a refund?")],
                &execution,
                &mut |delta| text.push_str(delta),
                &mut |_| {},
                &mut |event| events.push(event),
                &Arc::new(AtomicBool::new(false)),
            ))
            .unwrap();
        assert_eq!(text, "Checking.\n\nRefund is requested.");
        assert_eq!(result.text, text);
        assert_eq!(result.model, "fixture/served");
        assert_eq!(result.usage.total_tokens, 7);
        assert_eq!(result.usage.cost, Some(0.003));
        assert_eq!(events.len(), 2);
        assert!(matches!(&events[0],RuntimeEvent::Tool {name,running:true,..} if name == "jev"));
        assert!(
            matches!(&events[1],RuntimeEvent::Tool {output,running:false,..} if output["answers"]["refund"]["noul"] == 0.9)
        );
        let requests = model_server.join().unwrap();
        assert_eq!(requests[0]["tools"][0]["function"]["name"], "jev");
        assert!(
            requests[0]["messages"][0]["content"]
                .as_str()
                .unwrap()
                .contains("Batch independent questions")
        );
        assert_eq!(
            requests[1]["messages"][2]["tool_calls"][0]["id"],
            "call-jev"
        );
        assert_eq!(requests[1]["messages"][3]["tool_call_id"], "call-jev");
        let output: Value =
            serde_json::from_str(requests[1]["messages"][3]["content"].as_str().unwrap()).unwrap();
        assert_eq!(output["answers"]["refund"]["noul"], 0.9);
        assert!(
            !requests[1]["messages"]
                .to_string()
                .contains("jev-fixture-credential")
        );
        let jev_request = jev_server.join().unwrap();
        assert!(jev_request.starts_with("POST /v1/systemone "));
        assert_eq!(
            serde_json::from_str::<Value>(jev_request.split_once("\r\n\r\n").unwrap().1).unwrap()["model"],
            "jev-fixture"
        );
    }

    #[test]
    fn duplicate_call_ids_stop_before_redispatch() {
        let reply = tool_reply("same-call", json!({"state":"text","questions":{}}));
        let (base, server) = sequence(vec![reply.clone(), reply]);
        let provider = Provider::with_base(ApiKey::new(FIXTURE_TOKEN), &base).unwrap();
        let mut events = vec![];
        let error = runtime()
            .block_on(provider.chat_with_plugins(
                "openrouter/free",
                &crate::models::GenerationOptions::default(),
                vec![Message::user("Check")],
                &jev_settings(jev_plugin_endpoint(), None),
                &mut |_| {},
                &mut |_| {},
                &mut |event| events.push(event),
                &Arc::new(AtomicBool::new(false)),
            ))
            .unwrap_err();
        assert!(error.contains("reused a tool call ID"));
        assert_eq!(events.len(), 2);
        assert_eq!(server.join().unwrap().len(), 2);
    }

    fn jev_plugin_endpoint() -> String {
        crate::jev_plugin::DEFAULT_ENDPOINT.into()
    }

    #[test]
    fn malformed_tokens_are_rejected_without_echoing_them() {
        for token in ["", "bad token", "bad\ntoken", "bad\u{7f}token", "badétoken"] {
            let error = Provider::new(ApiKey::new(token)).err().unwrap();
            assert_eq!(
                error,
                "Enter an OpenRouter API key without spaces or control characters."
            );
        }
    }

    #[test]
    fn key_check_returns_only_fixed_status_and_numeric_allowance() {
        let body = format!(r#"{{"data":{{"label":"{FIXTURE_TOKEN}","limit_remaining":12.5}}}}"#);
        let (base, server) = fixture(200, "application/json", &body);
        let provider = Provider::with_base(ApiKey::new(FIXTURE_TOKEN), &base).unwrap();
        let info = runtime().block_on(provider.check()).unwrap();
        assert_eq!(info.status, "Verified");
        assert_eq!(info.limit_remaining, Some(12.5));
        assert!(!format!("{info:?}").contains(FIXTURE_TOKEN));
        assert!(
            server
                .join()
                .unwrap()
                .starts_with("GET /api/v1/key HTTP/1.1")
        );
    }

    #[test]
    fn key_check_errors_and_malformed_responses_do_not_echo_body_text() {
        for status in [401, 402, 429] {
            let body = format!(r#"{{"error":{{"message":"{FIXTURE_TOKEN}"}}}}"#);
            let (base, server) = fixture(status, "application/json", &body);
            let provider = Provider::with_base(ApiKey::new(FIXTURE_TOKEN), &base).unwrap();
            let error = runtime().block_on(provider.check()).unwrap_err();
            assert!(error.contains(&status.to_string()));
            assert!(!error.contains(FIXTURE_TOKEN));
            server.join().unwrap();
        }
        for body in [
            r#"{"data":{}}"#,
            r#"{"data":{"limit_remaining":"bad"}}"#,
            "not json",
        ] {
            let (base, server) = fixture(200, "application/json", body);
            let provider = Provider::with_base(ApiKey::new(FIXTURE_TOKEN), &base).unwrap();
            assert_eq!(
                runtime().block_on(provider.check()).unwrap_err(),
                "OpenRouter returned an invalid key response."
            );
            server.join().unwrap();
        }
    }

    #[test]
    fn streaming_delivers_deltas_usage_and_explicit_free_router_request() {
        let body = concat!(
            ": keep-alive\n\n",
            "data: {\"model\":\"fixture/model\",\"choices\":[{\"delta\":{\"content\":\"Hello \"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"there\"},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":2,\"total_tokens\":4,\"cost\":0.001}}\n\n",
            "data: [DONE]\n\n"
        );
        let (base, server) = fixture(200, "text/event-stream", body);
        let provider = Provider::with_base(ApiKey::new(FIXTURE_TOKEN), &base).unwrap();
        let mut received = Vec::new();
        let mut models = Vec::new();
        let reply = runtime()
            .block_on(provider.stream_with_options_and_model(
                "",
                &crate::models::GenerationOptions::default(),
                vec![Message::user("hello")],
                &mut |delta| {
                    received.push(delta.to_owned());
                },
                &mut |model| models.push(model.to_owned()),
            ))
            .unwrap();
        assert_eq!(received, ["Hello ", "there"]);
        assert_eq!(models, ["fixture/model"]);
        assert_eq!(reply.text, "Hello there");
        assert_eq!(reply.model, "fixture/model");
        assert_eq!(reply.usage.total_tokens, 4);
        assert_eq!(reply.usage.cost, Some(0.001));
        let request = server.join().unwrap();
        assert!(request.starts_with("POST /api/v1/chat/completions HTTP/1.1"));
        assert!(request.contains(r#""model":"openrouter/free""#));
        assert!(request.contains(r#""stream":true"#));
    }

    #[test]
    fn streaming_sends_selected_model_reasoning_and_output_limit() {
        let body = "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";
        let (base, server) = fixture(200, "text/event-stream", body);
        let provider = Provider::with_base(ApiKey::new(FIXTURE_TOKEN), &base).unwrap();
        let options = crate::models::GenerationOptions {
            reasoning: Some("high".into()),
            max_tokens: Some(8192),
        };
        runtime()
            .block_on(provider.stream_with_options(
                "openai/gpt-6-luna",
                &options,
                vec![Message::user("hello")],
                &mut |_| {},
            ))
            .unwrap();
        let request = server.join().unwrap();
        let (_, body) = request.split_once("\r\n\r\n").unwrap();
        let body: Value = serde_json::from_str(body).unwrap();
        assert_eq!(body["model"], "openai/gpt-6-luna");
        assert_eq!(body["reasoning"]["effort"], "high");
        assert_eq!(body["max_tokens"], 8192);
    }

    #[test]
    fn streaming_http_and_midstream_errors_are_sanitized() {
        for status in [401, 402, 429] {
            let body = format!(r#"{{"error":{{"code":{status},"message":"{FIXTURE_TOKEN}"}}}}"#);
            let (base, server) = fixture(status, "application/json", &body);
            let provider = Provider::with_base(ApiKey::new(FIXTURE_TOKEN), &base).unwrap();
            let error = runtime()
                .block_on(provider.stream(
                    "fixture/model",
                    vec![Message::user("hello")],
                    &mut |_| {},
                ))
                .unwrap_err();
            assert!(error.contains(&status.to_string()));
            assert!(!error.contains(FIXTURE_TOKEN));
            server.join().unwrap();
        }
        let body = format!(
            "data: {{\"model\":\"fixture/served-model\",\"choices\":[{{\"delta\":{{\"content\":\"Partial\"}}}}]}}\n\ndata: {{\"error\":{{\"code\":429,\"message\":\"{FIXTURE_TOKEN}\"}}}}\n\n"
        );
        let (base, server) = fixture(200, "text/event-stream", &body);
        let provider = Provider::with_base(ApiKey::new(FIXTURE_TOKEN), &base).unwrap();
        let mut models = Vec::new();
        let mut text = String::new();
        let error = runtime()
            .block_on(provider.stream_with_options_and_model(
                "openrouter/free",
                &crate::models::GenerationOptions::default(),
                vec![Message::user("hello")],
                &mut |delta| text.push_str(delta),
                &mut |model| models.push(model.to_owned()),
            ))
            .unwrap_err();
        assert_eq!(models, ["fixture/served-model"]);
        assert_eq!(text, "Partial");
        assert!(error.contains("429"));
        assert!(!error.contains(FIXTURE_TOKEN));
        server.join().unwrap();
    }
}
