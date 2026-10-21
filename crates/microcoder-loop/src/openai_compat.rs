//! Generation through any OpenAI-compatible chat completions endpoint:
//! Vertex AI's (`--provider vertex`, [`crate::vertex`]) and a server the
//! operator names (`--provider openai-compatible --base-url URL`), such as
//! Flint served on this computer by `rec` at `http://127.0.0.1:11500/v1`.
//!
//! The action comes back one of two ways ([`Format`]):
//!
//! - **Schema.** The request carries a strict `json_schema` response
//!   format ([`openrouter::Client::structured`]), and the reply is the
//!   action. Vertex's open models honor it.
//! - **Text.** The request carries no response format; the system prompt
//!   gains [`crate::text_action::reminder`], and the reply's text is read
//!   by [`crate::text_action::parse`]. For servers that refuse the
//!   response format or accept it and ignore it, as `rec` does.
//!
//! [`Format::Auto`] asks with the schema first and, when the server
//! refuses the response format or answers outside it, asks the same step
//! again as text and stays on text for the rest of the run.
//!
//! The bearer is read from a file before every call, so another process
//! may keep it fresh. An endpoint without authentication needs no file.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use crate::models::{Basis, Generate, Generated, NextAction, next_action_schema};

/// How the action comes back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// Under a strict `json_schema` response format.
    Schema,
    /// As JSON in the reply's text.
    Text,
    /// Schema until the server refuses or ignores it, then text.
    Auto,
}

impl Format {
    /// `schema`, `text`, or `auto`.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "schema" => Some(Format::Schema),
            "text" => Some(Format::Text),
            "auto" => Some(Format::Auto),
            _ => None,
        }
    }
}

/// How a call's cost is reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pricing {
    /// Vertex's list prices ([`crate::vertex::cost`]).
    Vertex,
    /// A model served on this computer: nothing is billed.
    Local,
}

impl Pricing {
    fn basis(self) -> Basis {
        match self {
            Pricing::Vertex => Basis::ListPrice,
            Pricing::Local => Basis::Local,
        }
    }
}

/// Generation on an OpenAI-compatible endpoint.
pub struct ChatGenerator {
    /// The provider's name in messages: `vertex` or `openai-compatible`.
    pub provider: &'static str,
    pub model: String,
    /// `low`, `medium`, or `high`, or `None` for the model's default.
    pub effort: Option<String>,
    pub base_url: String,
    /// The file holding the bearer, or `None` for an endpoint without
    /// authentication.
    pub token_file: Option<PathBuf>,
    pub pricing: Pricing,
    pub format: Format,
    /// The wire: OpenRouter's fields, which Vertex accepts and ignores, or
    /// plain OpenAI fields for any other server.
    pub dialect: openrouter::Dialect,
    /// Tokens a reply may use, reasoning included.
    pub max_tokens: u32,
    /// Set once [`Format::Auto`] has fallen back to text.
    text: AtomicBool,
}

impl ChatGenerator {
    /// A generator for `model` at `base_url`.
    #[must_use]
    pub fn new(
        provider: &'static str,
        model: &str,
        base_url: &str,
        token_file: Option<PathBuf>,
        pricing: Pricing,
        format: Format,
    ) -> Self {
        ChatGenerator {
            provider,
            model: model.to_string(),
            effort: None,
            base_url: base_url.trim_end_matches('/').to_string(),
            token_file,
            pricing,
            format,
            dialect: openrouter::Dialect::Gemini,
            max_tokens: crate::vertex::MAX_TOKENS,
            text: AtomicBool::new(matches!(format, Format::Text)),
        }
    }

    /// Whether the next call asks for the action as text.
    #[must_use]
    pub fn asks_as_text(&self) -> bool {
        self.text.load(Ordering::Relaxed)
    }

    fn cost(&self, usage: &openrouter::Usage) -> Option<f64> {
        match self.pricing {
            Pricing::Vertex => {
                crate::vertex::cost(&self.model, usage.prompt_tokens, usage.completion_tokens)
            }
            Pricing::Local => Some(0.0),
        }
    }

    fn generated(
        &self,
        action: Result<NextAction, String>,
        usage: &openrouter::Usage,
        milliseconds: u64,
    ) -> Generated {
        let usd = self.cost(usage);
        Generated {
            action,
            model: self.model.clone(),
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            usd,
            known_usd: usd.unwrap_or(0.0),
            cost_unknown: usd
                .is_none()
                .then(|| format!("{} has no list price in microcoder::vertex", self.model)),
            usd_upper: usd,
            cost_basis: self.pricing.basis(),
            milliseconds,
        }
    }

    fn client(&self) -> Result<openrouter::Client, String> {
        let token = match &self.token_file {
            Some(path) => read_token(path)?,
            // The header is required by the client; a server without
            // authentication ignores it.
            None => "none".to_string(),
        };
        let mut config = openrouter::Config::new(openrouter::ApiKey::new(&token));
        config.base_url.clone_from(&self.base_url);
        config.referer = None;
        config.title = None;
        config.dialect = self.dialect;
        openrouter::Client::new(config).map_err(|error| error.to_string())
    }

    fn request(&self, system: &str, prompt: &str) -> openrouter::ChatRequest {
        let mut request = openrouter::ChatRequest::new(
            &self.model,
            vec![
                openrouter::Message::system(system),
                openrouter::Message::user(prompt),
            ],
        )
        .max_tokens(self.max_tokens);
        if let Some(effort) = &self.effort {
            request = request.effort(effort);
        }
        request
    }

    /// One step's call, with the capacity refusal it met: on Vertex, a 429
    /// `RESOURCE_EXHAUSTED` read by [`crate::capacity::Refusal::vertex`].
    pub async fn attempt(
        &self,
        system: &str,
        prompt: &str,
    ) -> (Generated, Option<crate::capacity::Refusal>) {
        let started = Instant::now();
        let client = match self.client() {
            Ok(client) => client,
            // Nothing was sent.
            Err(why) => return (self.failed(why, Some(0.0), &started), None),
        };
        if !self.asks_as_text() {
            let (generated, refusal, fall_back) =
                self.with_schema(&client, system, prompt, &started).await;
            if !(fall_back && self.format == Format::Auto) {
                return (generated, refusal);
            }
            self.text.store(true, Ordering::Relaxed);
            let mut again = self.as_text(&client, system, prompt, &started).await;
            // The schema call's tokens were spent too.
            again.prompt_tokens += generated.prompt_tokens;
            again.completion_tokens += generated.completion_tokens;
            return (again, None);
        }
        (self.as_text(&client, system, prompt, &started).await, None)
    }

    fn failed(&self, why: String, usd: Option<f64>, started: &Instant) -> Generated {
        let usd = match self.pricing {
            Pricing::Local => Some(0.0),
            Pricing::Vertex => usd,
        };
        Generated {
            action: Err(why.clone()),
            model: self.model.clone(),
            prompt_tokens: 0,
            completion_tokens: 0,
            usd,
            known_usd: usd.unwrap_or(0.0),
            cost_unknown: usd.is_none().then_some(why),
            usd_upper: usd,
            cost_basis: self.pricing.basis(),
            milliseconds: elapsed(started),
        }
    }

    /// The call under the strict response format, its capacity refusal,
    /// and whether the server refused or ignored the format.
    async fn with_schema(
        &self,
        client: &openrouter::Client,
        system: &str,
        prompt: &str,
        started: &Instant,
    ) -> (Generated, Option<crate::capacity::Refusal>, bool) {
        let answer = client
            .structured::<NextAction>(
                self.request(system, prompt),
                "next_action",
                next_action_schema(),
            )
            .await;
        let refusal = match (&answer, self.pricing) {
            (
                Err(openrouter::Error::Api {
                    status,
                    retry_after,
                    body,
                    ..
                }),
                Pricing::Vertex,
            ) => crate::capacity::Refusal::vertex(
                *status,
                *retry_after,
                body,
                crate::failover::unix_now(),
            ),
            _ => None,
        };
        match answer {
            Ok(reply) => (
                self.generated(Ok(reply.value), &reply.usage, reply.milliseconds),
                refusal,
                false,
            ),
            Err(openrouter::Error::Schema {
                detail,
                excerpt,
                usage,
            }) => (
                self.generated(
                    Err(format!(
                        "the reply didn't match the schema ({detail}): {excerpt}"
                    )),
                    &usage,
                    elapsed(started),
                ),
                refusal,
                true,
            ),
            Err(
                error @ (openrouter::Error::Connection(_)
                | openrouter::Error::Timeout
                | openrouter::Error::Decode { .. }),
            ) => (
                self.failed(
                    format!("the call failed after it was sent and may have been billed ({error})"),
                    None,
                    started,
                ),
                refusal,
                false,
            ),
            // An error status: refused, so not billed. A 400 or 422 may be
            // the response format itself.
            Err(error) => {
                let format_refused = matches!(
                    &error,
                    openrouter::Error::Api {
                        status: 400 | 422,
                        ..
                    }
                );
                (
                    self.failed(error.to_string(), Some(0.0), started),
                    refusal,
                    format_refused,
                )
            }
        }
    }

    /// The call with the action asked for as JSON in the reply's text.
    async fn as_text(
        &self,
        client: &openrouter::Client,
        system: &str,
        prompt: &str,
        started: &Instant,
    ) -> Generated {
        let system = format!("{system}{}", crate::text_action::reminder());
        match client.chat(&self.request(&system, prompt)).await {
            Ok(response) => {
                let choice = response.choices.first();
                let raw = choice
                    .and_then(|choice| choice.message.content.clone())
                    .unwrap_or_default();
                let action = if raw.trim().is_empty() {
                    Err(format!(
                        "the reply has no text (finish reason {})",
                        choice
                            .and_then(|choice| choice.finish_reason.as_deref())
                            .unwrap_or("unknown")
                    ))
                } else {
                    crate::text_action::parse(&raw)
                };
                self.generated(action, &response.usage, elapsed(started))
            }
            Err(
                error @ (openrouter::Error::Connection(_)
                | openrouter::Error::Timeout
                | openrouter::Error::Decode { .. }),
            ) => self.failed(
                format!("the call failed after it was sent and may have been billed ({error})"),
                None,
                started,
            ),
            Err(error) => self.failed(error.to_string(), Some(0.0), started),
        }
    }
}

impl Generate for ChatGenerator {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        self.attempt(system, prompt).await.0
    }
}

fn elapsed(started: &Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// The token in `path`, trimmed.
///
/// # Errors
///
/// When the file can't be read or is empty. The error never holds the
/// token.
pub fn read_token(path: &std::path::Path) -> Result<String, String> {
    let token = std::fs::read_to_string(path)
        .map_err(|e| format!("can't read the token at {}: {e}", path.display()))?;
    let token = token.trim().to_string();
    if token.is_empty() {
        return Err(format!("the token at {} is empty", path.display()));
    }
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_pricing_costs_nothing_and_says_so() {
        let generator = ChatGenerator::new(
            "openai-compatible",
            "flint",
            "http://127.0.0.1:11500/v1/",
            None,
            Pricing::Local,
            Format::Auto,
        );
        assert_eq!(generator.base_url, "http://127.0.0.1:11500/v1");
        assert!(!generator.asks_as_text());
        let usage = openrouter::Usage {
            prompt_tokens: 1_000,
            completion_tokens: 200,
            ..openrouter::Usage::default()
        };
        let generated = generator.generated(Err("x".into()), &usage, 5);
        assert_eq!(generated.usd, Some(0.0));
        assert_eq!(generated.cost_basis, Basis::Local);
        assert!(generated.cost_unknown.is_none());
        let failed = generator.failed("lost".into(), None, &Instant::now());
        assert_eq!(failed.usd, Some(0.0));
    }

    #[test]
    fn a_text_format_starts_as_text() {
        let generator = ChatGenerator::new(
            "openai-compatible",
            "flint",
            "http://x/v1",
            None,
            Pricing::Local,
            Format::Text,
        );
        assert!(generator.asks_as_text());
        assert_eq!(Format::parse("auto"), Some(Format::Auto));
        assert_eq!(Format::parse("json"), None);
    }

    /// A one-request HTTP server answering `body` with `status`.
    async fn serve_once(status: u16, body: &'static str) -> (String, tokio::task::JoinHandle<()>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/v1", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            for _ in 0..2 {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let mut buffer = vec![0u8; 65_536];
                let _ = socket.read(&mut buffer).await;
                let reply = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes()).await;
            }
        });
        (url, task)
    }

    #[tokio::test]
    async fn auto_falls_back_to_text_when_the_schema_is_ignored() {
        let body = r#"{"model":"flint","choices":[{"message":{"content":"```json\n{\"rationale\":\"look\",\"commands\":[\"ls\"]}\n```"},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":5}}"#;
        let (url, server) = serve_once(200, body).await;
        let generator = ChatGenerator::new(
            "openai-compatible",
            "flint",
            &url,
            None,
            Pricing::Local,
            Format::Auto,
        );
        // The structured call reads the fenced object without the schema's
        // required members as a schema miss, so the step is asked again as
        // text, which fills them.
        let generated = generator.generate("system", "prompt").await;
        server.abort();
        let action = generated.action.unwrap();
        assert_eq!(action.commands, vec!["ls".to_string()]);
        assert!(generator.asks_as_text());
        assert_eq!(generated.usd, Some(0.0));
        assert_eq!(generated.prompt_tokens, 20);
    }
}
