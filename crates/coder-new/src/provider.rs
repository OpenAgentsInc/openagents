//! Direct OpenRouter key checks and single-attempt streamed chat requests.

use std::time::Duration;

use openrouter::{ApiKey, ChatRequest, Client, Config, Message, Streamed};
use reqwest::{StatusCode, redirect::Policy};
use serde_json::Value;

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
            .stream(&request, callback)
            .await
            .map_err(stream_error)
    }

    #[cfg(test)]
    pub(crate) fn with_base(key: ApiKey, base_url: &str) -> Result<Self, String> {
        Self::build(key, base_url)
    }
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
        let reply = runtime()
            .block_on(
                provider.stream("", vec![Message::user("hello")], &mut |delta| {
                    received.push(delta.to_owned());
                }),
            )
            .unwrap();
        assert_eq!(received, ["Hello ", "there"]);
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
        let body =
            format!("data: {{\"error\":{{\"code\":429,\"message\":\"{FIXTURE_TOKEN}\"}}}}\n\n");
        let (base, server) = fixture(200, "text/event-stream", &body);
        let provider = Provider::with_base(ApiKey::new(FIXTURE_TOKEN), &base).unwrap();
        let error = runtime()
            .block_on(provider.stream("fixture/model", vec![Message::user("hello")], &mut |_| {}))
            .unwrap_err();
        assert!(error.contains("429"));
        assert!(!error.contains(FIXTURE_TOKEN));
        server.join().unwrap();
    }
}
