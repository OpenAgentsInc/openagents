//! The real transport: the ChatGPT Codex Responses endpoint on the
//! operator's Codex login.
//!
//! The endpoint, the request fields, and the headers follow OpenAI's Codex
//! (Apache-2.0), reimplemented here: `model-provider-info` for the base
//! URL, `core/src/client.rs` for the body, and
//! `model-provider/src/bearer_auth_provider.rs` for the auth headers.
//! `docs/coder/design/microluna.md` has the file references.
//!
//! # The login is read, never written
//!
//! The login lives in `~/.codex/auth.json`, or `$CODEX_HOME/auth.json`.
//! Its refresh token rotates, so a second process that refreshes the same
//! login independently can invalidate it for Codex. [`Login::load`]
//! therefore only reads, and refuses with [`LoginError::Expiring`] when the
//! access token has less than [`REFRESH_MARGIN_SECS`] left. Any Codex
//! command refreshes it the way Codex does.
//!
//! No token reaches a log line, an error, or a `Debug` string.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use serde_json::{Value, json};

use crate::transport::{Reply, Request, TokenUsage, Transport, TransportError};

/// The Codex backend's base URL for ChatGPT-login access.
pub const BASE_URL: &str = "https://chatgpt.com/backend-api/codex";

/// The client identity Microluna sends as `originator`. It names
/// Microluna rather than presenting itself as Codex.
pub const ORIGINATOR: &str = "openagents_microluna";

/// How long before expiry the access token counts as expiring. Codex
/// refreshes inside a few minutes of expiry; this margin keeps a session
/// from starting on a token Codex is about to rotate.
pub const REFRESH_MARGIN_SECS: u64 = 10 * 60;

/// The longest a single request may take, stream included.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

/// Why the Codex login can't be used.
#[derive(Debug)]
pub enum LoginError {
    /// There is no login file.
    Missing(PathBuf),
    /// The login file couldn't be read or parsed.
    Unreadable(PathBuf, String),
    /// The login isn't a ChatGPT login, so it has no access token.
    NotChatgpt(String),
    /// A field the transport needs is absent.
    Malformed(&'static str),
    /// The access token has expired or is about to.
    Expiring {
        /// Seconds of validity left, zero when already expired.
        seconds_left: u64,
    },
}

impl fmt::Display for LoginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoginError::Missing(path) => write!(
                f,
                "no Codex login at {}; sign in with Codex first",
                path.display()
            ),
            LoginError::Unreadable(path, why) => {
                write!(f, "can't read {}: {why}", path.display())
            }
            LoginError::NotChatgpt(mode) => {
                write!(f, "the Codex login uses {mode}, not a ChatGPT sign-in")
            }
            LoginError::Malformed(field) => write!(f, "the Codex login has no {field}"),
            LoginError::Expiring { seconds_left } => write!(
                f,
                "the Codex access token expires in {seconds_left} s; run any Codex command \
                 to refresh it, then retry"
            ),
        }
    }
}

impl std::error::Error for LoginError {}

/// A loaded Codex login. Its `Debug` output hides both secrets.
#[derive(Clone)]
pub struct Login {
    access_token: String,
    account_id: String,
    expires_at: Option<u64>,
}

impl fmt::Debug for Login {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Login")
            .field("access_token", &"<redacted>")
            .field("account_id", &"<redacted>")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

impl Login {
    /// The login file Codex uses: `$CODEX_HOME/auth.json`, or
    /// `~/.codex/auth.json`.
    #[must_use]
    pub fn default_path() -> Option<PathBuf> {
        if let Some(home) = std::env::var_os("CODEX_HOME").filter(|home| !home.is_empty()) {
            return Some(PathBuf::from(home).join("auth.json"));
        }
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex/auth.json"))
    }

    /// Reads the login at `path` and checks that its access token has
    /// more than [`REFRESH_MARGIN_SECS`] left.
    ///
    /// # Errors
    ///
    /// A [`LoginError`] for a missing, unreadable, non-ChatGPT, malformed,
    /// or expiring login.
    pub fn load(path: &Path) -> Result<Login, LoginError> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(LoginError::Missing(path.to_path_buf()));
            }
            Err(error) => {
                return Err(LoginError::Unreadable(
                    path.to_path_buf(),
                    error.to_string(),
                ));
            }
        };
        let login = Login::parse(&text).map_err(|error| match error {
            LoginError::Unreadable(_, why) => LoginError::Unreadable(path.to_path_buf(), why),
            other => other,
        })?;
        login.check(now_secs())?;
        Ok(login)
    }

    /// Parses the contents of an `auth.json`, without checking expiry.
    ///
    /// # Errors
    ///
    /// A [`LoginError`] for text that isn't a usable ChatGPT login.
    pub fn parse(text: &str) -> Result<Login, LoginError> {
        let value: Value = serde_json::from_str(text)
            .map_err(|error| LoginError::Unreadable(PathBuf::new(), error.to_string()))?;
        if let Some(mode) = value["auth_mode"].as_str()
            && !mode.eq_ignore_ascii_case("chatgpt")
        {
            return Err(LoginError::NotChatgpt(mode.to_string()));
        }
        let tokens = &value["tokens"];
        let access_token = tokens["access_token"]
            .as_str()
            .filter(|token| !token.is_empty())
            .ok_or(LoginError::Malformed("access token"))?
            .to_string();
        let account_id = tokens["account_id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or(LoginError::Malformed("account ID"))?
            .to_string();
        let expires_at = jwt_expiry(&access_token);
        Ok(Login {
            access_token,
            account_id,
            expires_at,
        })
    }

    /// Refuses a token with [`REFRESH_MARGIN_SECS`] or less left at `now`.
    /// A token whose expiry can't be read is let through: the provider's
    /// refusal is then the authority.
    ///
    /// # Errors
    ///
    /// [`LoginError::Expiring`] when the token is inside the margin.
    pub fn check(&self, now: u64) -> Result<(), LoginError> {
        match self.expires_at {
            Some(expires_at) if expires_at <= now + REFRESH_MARGIN_SECS => {
                Err(LoginError::Expiring {
                    seconds_left: expires_at.saturating_sub(now),
                })
            }
            _ => Ok(()),
        }
    }

    /// When the access token expires, in seconds since the epoch.
    #[must_use]
    pub fn expires_at(&self) -> Option<u64> {
        self.expires_at
    }
}

/// The `exp` claim of a JWT, without verifying it: the provider verifies.
fn jwt_expiry(token: &str) -> Option<u64> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()?;
    let claims: Value = serde_json::from_slice(&bytes).ok()?;
    claims["exp"].as_u64()
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default()
}

/// The request body Codex sends, for Microluna's fields.
#[must_use]
pub fn body(request: &Request) -> Value {
    let mut body = json!({
        "model": request.model,
        "instructions": request.instructions,
        "input": request.input,
        "tools": request.tools,
        "tool_choice": "auto",
        "parallel_tool_calls": false,
        "store": false,
        "stream": true,
        "include": ["reasoning.encrypted_content"],
        "prompt_cache_key": request.cache_key,
    });
    if let Some(effort) = &request.effort {
        body["reasoning"] = json!({ "effort": effort, "summary": "auto" });
    }
    body
}

/// The Codex-login transport.
#[derive(Debug)]
pub struct CodexTransport {
    http: reqwest::Client,
    login_path: PathBuf,
    url: String,
    session_id: String,
}

impl CodexTransport {
    /// A transport on the login at `login_path`, reporting `session_id` as
    /// its session. The login is read again before every request, so a
    /// refresh Codex makes in between is picked up.
    ///
    /// # Errors
    ///
    /// The login's error, when it can't be used now.
    pub fn new(login_path: PathBuf, session_id: &str) -> Result<CodexTransport, LoginError> {
        Login::load(&login_path)?;
        let http = reqwest::Client::builder()
            .user_agent(concat!("microluna/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| LoginError::Unreadable(login_path.clone(), error.to_string()))?;
        Ok(CodexTransport {
            http,
            login_path,
            url: format!("{BASE_URL}/responses"),
            session_id: session_id.to_string(),
        })
    }
}

impl Transport for CodexTransport {
    async fn respond(&self, request: &Request) -> Result<Reply, TransportError> {
        let login = Login::load(&self.login_path).map_err(TransportError::Login)?;
        let mut response = self
            .http
            .post(&self.url)
            .timeout(REQUEST_TIMEOUT)
            .bearer_auth(&login.access_token)
            .header("ChatGPT-Account-ID", &login.account_id)
            .header("originator", ORIGINATOR)
            .header("session-id", &self.session_id)
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .json(&body(request))
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
        events.finish()
    }
}

/// A server-sent event reader for the five event kinds a reply needs.
#[derive(Debug, Default)]
pub struct Events {
    buffer: Vec<u8>,
    reply: Reply,
    completed: bool,
}

impl Events {
    /// Feeds bytes as they arrive; an event may span chunks.
    ///
    /// # Errors
    ///
    /// The provider's failure, when an event reports one.
    pub fn push(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        self.buffer.extend_from_slice(bytes);
        while let Some(end) = find(&self.buffer, b"\n\n") {
            let block: Vec<u8> = self.buffer.drain(..end + 2).collect();
            let block = String::from_utf8_lossy(&block).replace('\r', "");
            let data: Vec<&str> = block
                .lines()
                .filter_map(|line| line.strip_prefix("data:"))
                .map(str::trim_start)
                .collect();
            if data.is_empty() {
                continue;
            }
            let Ok(event) = serde_json::from_str::<Value>(&data.join("\n")) else {
                continue;
            };
            self.read(&event)?;
        }
        Ok(())
    }

    fn read(&mut self, event: &Value) -> Result<(), TransportError> {
        match event["type"].as_str().unwrap_or_default() {
            "response.output_item.done" => self.reply.items.push(event["item"].clone()),
            "response.completed" => {
                let response = &event["response"];
                self.reply.id = response["id"].as_str().map(str::to_string);
                self.reply.model = response["model"].as_str().unwrap_or_default().to_string();
                self.reply.usage = usage(&response["usage"]);
                self.completed = true;
            }
            "response.failed" => {
                let error = &event["response"]["error"];
                return Err(TransportError::Failed(
                    error["message"]
                        .as_str()
                        .or(error["code"].as_str())
                        .unwrap_or("no reason given")
                        .to_string(),
                ));
            }
            "response.incomplete" => {
                return Err(TransportError::Incomplete(
                    event["response"]["incomplete_details"]["reason"]
                        .as_str()
                        .unwrap_or("no reason given")
                        .to_string(),
                ));
            }
            "error" => {
                return Err(TransportError::Failed(
                    event["message"]
                        .as_str()
                        .or(event["error"]["message"].as_str())
                        .unwrap_or("no reason given")
                        .to_string(),
                ));
            }
            _ => {}
        }
        Ok(())
    }

    /// The reply, once the stream has ended.
    ///
    /// # Errors
    ///
    /// [`TransportError::Stream`] when the stream ended without
    /// `response.completed`.
    pub fn finish(self) -> Result<Reply, TransportError> {
        if self.completed {
            Ok(self.reply)
        } else {
            Err(TransportError::Stream(
                "the stream ended without response.completed".to_string(),
            ))
        }
    }
}

/// The usage object of a completed response.
#[must_use]
pub fn usage(value: &Value) -> TokenUsage {
    TokenUsage {
        input: value["input_tokens"].as_u64().unwrap_or_default(),
        cached: value["input_tokens_details"]["cached_tokens"]
            .as_u64()
            .unwrap_or_default(),
        output: value["output_tokens"].as_u64().unwrap_or_default(),
        reasoning: value["output_tokens_details"]["reasoning_tokens"]
            .as_u64()
            .unwrap_or_default(),
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn excerpt(text: &str, max: usize) -> String {
    let text = text.trim();
    match text.char_indices().nth(max) {
        Some((at, _)) => format!("{}…", &text[..at]),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(exp: u64) -> String {
        let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        format!(
            "{}.{}.sig",
            engine.encode(br#"{"alg":"none"}"#),
            engine.encode(format!(r#"{{"exp":{exp}}}"#))
        )
    }

    fn auth(exp: u64) -> String {
        json!({
            "auth_mode": "chatgpt",
            "tokens": { "access_token": token(exp), "account_id": "acct-1" },
        })
        .to_string()
    }

    #[test]
    fn a_login_reads_its_expiry_and_hides_its_secrets() {
        let login = Login::parse(&auth(5_000)).unwrap();
        assert_eq!(login.expires_at(), Some(5_000));
        let shown = format!("{login:?}");
        assert!(!shown.contains("acct-1"));
        assert!(!shown.contains(&token(5_000)));
    }

    #[test]
    fn a_token_inside_the_margin_is_refused() {
        let login = Login::parse(&auth(5_000)).unwrap();
        assert!(login.check(5_000 - REFRESH_MARGIN_SECS - 1).is_ok());
        assert!(matches!(
            login.check(5_000 - 60),
            Err(LoginError::Expiring { seconds_left: 60 })
        ));
        assert!(matches!(
            login.check(9_000),
            Err(LoginError::Expiring { seconds_left: 0 })
        ));
    }

    #[test]
    fn an_api_key_login_is_not_a_chatgpt_login() {
        let text = json!({ "auth_mode": "apikey", "OPENAI_API_KEY": "x" }).to_string();
        assert!(matches!(
            Login::parse(&text),
            Err(LoginError::NotChatgpt(mode)) if mode == "apikey"
        ));
    }

    #[test]
    fn the_body_is_stateless_and_carries_the_cache_key() {
        let request = Request {
            model: "gpt-6-luna".to_string(),
            instructions: "be brief".to_string(),
            input: vec![json!({"type": "message"})],
            tools: vec![],
            effort: Some("low".to_string()),
            cache_key: "task-1".to_string(),
        };
        let body = body(&request);
        assert_eq!(body["store"], false);
        assert_eq!(body["stream"], true);
        assert_eq!(body["prompt_cache_key"], "task-1");
        assert_eq!(body["reasoning"]["effort"], "low");
        assert_eq!(body["include"][0], "reasoning.encrypted_content");
    }

    #[test]
    fn events_split_across_chunks_become_one_reply() {
        let stream = concat!(
            "event: response.output_item.done\n",
            "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",",
            "\"call_id\":\"c1\",\"name\":\"finish\",\"arguments\":\"{}\"}}\n\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r1\",\"model\":\"gpt-6-luna\",",
            "\"usage\":{\"input_tokens\":100,\"input_tokens_details\":{\"cached_tokens\":40},",
            "\"output_tokens\":7,\"output_tokens_details\":{\"reasoning_tokens\":2}}}}\n\n",
        );
        let mut events = Events::default();
        for piece in stream.as_bytes().chunks(17) {
            events.push(piece).unwrap();
        }
        let reply = events.finish().unwrap();
        assert_eq!(reply.calls()[0].name, "finish");
        assert_eq!(
            reply.usage,
            TokenUsage {
                input: 100,
                cached: 40,
                output: 7,
                reasoning: 2
            }
        );
    }

    #[test]
    fn a_stream_without_completion_or_with_failure_is_an_error() {
        let mut events = Events::default();
        events
            .push(b"data: {\"type\":\"response.created\"}\n\n")
            .unwrap();
        assert!(matches!(events.finish(), Err(TransportError::Stream(_))));
        let mut events = Events::default();
        let failed = b"data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"message\":\"no\"}}}\n\n";
        assert!(matches!(events.push(failed), Err(TransportError::Failed(why)) if why == "no"));
    }
}
