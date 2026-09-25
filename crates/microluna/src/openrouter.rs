//! A backup transport: OpenRouter's Responses API on an OpenRouter key.
//!
//! OpenRouter serves OpenAI's models under an `openai/` prefix and accepts
//! the Responses API request Codex sends, streaming the same server-sent
//! events. So this transport reuses [`crate::codex::body`] and
//! [`crate::codex::Events`] and changes only the endpoint, the key, and the
//! model slug. Issue #9665 records why it exists: the Codex endpoint
//! refused every request on 2026-09-25.
//!
//! The key comes from `OPENROUTER_API_KEY`. It never reaches a log line,
//! an error, or a `Debug` string, and a model's commands don't see the
//! variable, since [`crate::tools::withhold_credentials`] removes every
//! `*_API_KEY`.

use std::fmt;

use serde_json::Value;

use crate::codex::{Events, REQUEST_TIMEOUT, body, excerpt};
use crate::transport::{Reply, Request, Transport, TransportError};

/// OpenRouter's API base URL.
pub const BASE_URL: &str = "https://openrouter.ai/api/v1";

/// The variable that holds the key.
pub const KEY_VAR: &str = "OPENROUTER_API_KEY";

/// The application OpenRouter attributes requests to.
pub const TITLE: &str = "OpenAgents Microluna";

/// The OpenRouter transport.
pub struct OpenRouterTransport {
    http: reqwest::Client,
    key: String,
    url: String,
}

impl fmt::Debug for OpenRouterTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenRouterTransport")
            .field("url", &self.url)
            .finish_non_exhaustive()
    }
}

impl OpenRouterTransport {
    /// A transport on `key`.
    ///
    /// # Errors
    ///
    /// When the key is empty or the HTTP client can't be built.
    pub fn new(key: &str) -> Result<OpenRouterTransport, String> {
        let key = key.trim();
        if key.is_empty() {
            return Err(format!("{KEY_VAR} is empty"));
        }
        let http = reqwest::Client::builder()
            .user_agent(concat!("microluna/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| error.to_string())?;
        Ok(OpenRouterTransport {
            http,
            key: key.to_string(),
            url: format!("{BASE_URL}/responses"),
        })
    }

    /// A transport on the key in `OPENROUTER_API_KEY`.
    ///
    /// # Errors
    ///
    /// When the variable is unset or empty.
    pub fn from_env() -> Result<OpenRouterTransport, String> {
        let key = std::env::var(KEY_VAR).map_err(|_| format!("{KEY_VAR} isn't set"))?;
        OpenRouterTransport::new(&key)
    }
}

/// OpenRouter's slug for a model: `gpt-6-luna` is `openai/gpt-6-luna`, and
/// a slug that already names its vendor is kept.
#[must_use]
pub fn slug(model: &str) -> String {
    if model.contains('/') {
        model.to_string()
    } else {
        format!("openai/{model}")
    }
}

/// The Codex request body with OpenRouter's model slug. Encrypted
/// reasoning isn't requested, since OpenRouter doesn't return it.
#[must_use]
pub fn request_body(request: &Request) -> Value {
    let mut body = body(request);
    body["model"] = Value::String(slug(&request.model));
    if let Some(fields) = body.as_object_mut() {
        fields.remove("include");
    }
    body
}

impl Transport for OpenRouterTransport {
    async fn respond(&self, request: &Request) -> Result<Reply, TransportError> {
        let mut response = self
            .http
            .post(&self.url)
            .timeout(REQUEST_TIMEOUT)
            .bearer_auth(&self.key)
            .header("X-Title", TITLE)
            .header("HTTP-Referer", "https://openagents.com")
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .json(&request_body(request))
            .send()
            .await
            .map_err(|error| TransportError::Stream(error.without_url().to_string()))?;
        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(TransportError::Http {
                status: status.as_u16(),
                body: excerpt(&text, 400),
            });
        }
        let mut events = Events::default();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| TransportError::Stream(error.without_url().to_string()))?
        {
            events.push(&chunk)?;
        }
        let mut reply = events.finish()?;
        // Records and prices key on OpenAI's own slug.
        if let Some(model) = reply.model.strip_prefix("openai/") {
            reply.model = model.to_string();
        }
        Ok(reply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request() -> Request {
        Request {
            model: "gpt-6-luna".to_string(),
            instructions: "Be brief.".to_string(),
            input: vec![json!({"role": "user", "content": "hi"})],
            tools: vec![],
            effort: Some("high".to_string()),
            cache_key: "task".to_string(),
            parallel_tools: true,
        }
    }

    #[test]
    fn the_slug_names_the_vendor_once() {
        assert_eq!(slug("gpt-6-luna"), "openai/gpt-6-luna");
        assert_eq!(slug("openai/gpt-6-luna"), "openai/gpt-6-luna");
    }

    #[test]
    fn the_body_is_the_codex_body_with_the_openrouter_slug() {
        let body = request_body(&request());
        assert_eq!(body["model"], "openai/gpt-6-luna");
        assert_eq!(body["reasoning"]["effort"], "high");
        assert_eq!(body["stream"], true);
        assert!(body.get("include").is_none());
    }

    #[test]
    fn an_empty_key_is_refused_and_a_key_never_prints() {
        assert!(OpenRouterTransport::new("  ").is_err());
        let transport = OpenRouterTransport::new("sk-or-v1-secret").unwrap();
        assert!(!format!("{transport:?}").contains("secret"));
    }

    #[test]
    fn openrouter_events_parse_as_codex_events() {
        let mut events = Events::default();
        let stream = concat!(
            "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"pong\"}]}}\n\n",
            ": OPENROUTER PROCESSING\n\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r1\",\"model\":\"openai/gpt-6-luna\",\"usage\":{\"input_tokens\":12,\"input_tokens_details\":{\"cached_tokens\":4},\"output_tokens\":3,\"output_tokens_details\":{\"reasoning_tokens\":1},\"cost\":0.000003}}}\n\n",
            "data: [DONE]\n\n",
        );
        events.push(stream.as_bytes()).unwrap();
        let reply = events.finish().unwrap();
        assert_eq!(reply.text(), "pong");
        assert_eq!(reply.usage.input, 12);
        assert_eq!(reply.usage.cached, 4);
    }
}
