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
//!
//! Every check and every run finds the login the same way,
//! [`Login::default_path`]: `$CODEX_HOME/auth.json` when `CODEX_HOME` is
//! set, else `~/.codex/auth.json`. A launcher that clears a child's
//! environment passes [`Login::home_override`] on as `CODEX_HOME`, so the
//! child finds the login its readiness check found (issue #10083).
//!
//! # Taking the login in a task container
//!
//! Where the model's commands run as the same user as Microluna, as in a
//! Terminal-Bench task container, a login file on disk is a login the
//! model can read. [`Login::take_copy`] marks the process non-dumpable, so
//! another process of the same user can't read its memory, environment,
//! or file descriptors through `/proc`, then reads the run's own copy of
//! the login into memory and removes that copy. [`CodexTransport::holding`]
//! then sends with the login in memory and never opens a file again.
//!
//! The person's own login is never deleted, moved, or rewritten: a take
//! needs `CODEX_HOME` naming the run's private home (never `~/.codex`),
//! removes only a regular file there, and refuses a link, so the file a
//! link names is never touched (issue #10083).

use std::borrow::Cow;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use serde_json::{Value, json};

use crate::transport::{Reply, Request, TokenUsage, Transport, TransportError};

/// The variable that names Codex's home folder, where `auth.json` lives.
pub const HOME_VAR: &str = "CODEX_HOME";

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
    /// The login was read, but its file couldn't be removed, so it would
    /// stay readable to the commands the model runs.
    Unremovable(PathBuf, String),
    /// A take was asked of a link. The run takes only its own copy, never
    /// the file a link names, which may be the person's own login.
    Linked(PathBuf),
    /// A take was asked with no `CODEX_HOME`, or with it at `~/.codex`: the
    /// login would be the person's own, which a run never removes.
    NoPrivateHome,
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
            LoginError::Unremovable(path, why) => write!(
                f,
                "can't remove {} after reading it, so the model's commands could read it: {why}",
                path.display()
            ),
            LoginError::Linked(path) => write!(
                f,
                "{} is a link; a run takes only its own copy of the Codex login, and never \
                 touches the file a link names: place a copy there instead",
                path.display()
            ),
            LoginError::NoPrivateHome => write!(
                f,
                "taking the Codex login needs CODEX_HOME naming the run's own copy, not \
                 ~/.codex: the person's login is never removed"
            ),
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
    /// `~/.codex/auth.json`. Readiness checks and runs both resolve the
    /// login here, so they agree (issue #10083).
    #[must_use]
    pub fn default_path() -> Option<PathBuf> {
        Login::home().map(|home| home.join("auth.json"))
    }

    /// Codex's home folder: [`Login::home_override`], else `~/.codex`.
    #[must_use]
    pub fn home() -> Option<PathBuf> {
        Login::home_override()
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")))
    }

    /// `$CODEX_HOME` as an absolute folder, when it is set and not empty.
    /// A launcher that starts a child with a cleared environment passes
    /// it on as [`HOME_VAR`]; it names a folder, not a secret.
    #[must_use]
    pub fn home_override() -> Option<PathBuf> {
        let home = PathBuf::from(std::env::var_os(HOME_VAR).filter(|home| !home.is_empty())?);
        Some(std::path::absolute(&home).unwrap_or(home))
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

    /// Takes the run's own copy of the login from `$CODEX_HOME/auth.json`
    /// ([`Login::take`]). `CODEX_HOME` must be set, and not to `~/.codex`:
    /// that login is the person's own, which no run removes.
    ///
    /// # Errors
    ///
    /// [`LoginError::NoPrivateHome`] without `CODEX_HOME` or with it at
    /// `~/.codex`, else
    /// [`Login::take`]'s errors.
    pub fn take_copy() -> Result<Login, LoginError> {
        let home = Login::home_override().ok_or(LoginError::NoPrivateHome)?;
        let real = |path: &Path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        if std::env::var_os("HOME")
            .is_some_and(|person| real(&PathBuf::from(person).join(".codex")) == real(&home))
        {
            return Err(LoginError::NoPrivateHome);
        }
        Login::take(&home.join("auth.json"))
    }

    /// Reads the run's own copy of the login at `path` into memory and
    /// removes that copy, so no command this process runs afterward can
    /// read it. The process is marked non-dumpable before the read (see
    /// [`protect_process`]).
    ///
    /// `path` must be a regular file the run placed for itself. A link is
    /// refused and left as it is, with the file it names untouched: that
    /// file may be the person's own login, which is never deleted, moved,
    /// or rewritten (issue #10083). The copy is removed even when it can't
    /// be parsed. The expiry isn't checked here: [`CodexTransport`] checks
    /// it before every request.
    ///
    /// # Errors
    ///
    /// [`LoginError::Linked`] for a link, [`LoginError::Unremovable`] when
    /// the copy stays on disk, either of which the caller must treat as
    /// fatal, or the error of a missing, unreadable, non-ChatGPT, or
    /// malformed login.
    pub fn take(path: &Path) -> Result<Login, LoginError> {
        let _ = protect_process();
        match std::fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(LoginError::Linked(path.to_path_buf()));
            }
            Ok(meta) if !meta.is_file() => {
                return Err(LoginError::Unreadable(
                    path.to_path_buf(),
                    "not a regular file".into(),
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(LoginError::Missing(path.to_path_buf()));
            }
            Err(error) => {
                return Err(LoginError::Unreadable(
                    path.to_path_buf(),
                    error.to_string(),
                ));
            }
        }
        let text = std::fs::read_to_string(path);
        // `remove_file` removes a link itself, never what it names, so
        // even a file swapped for a link since the check above leaves the
        // linked file alone.
        remove(path)?;
        let text = match text {
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
        Login::parse(&text).map_err(|error| match error {
            LoginError::Unreadable(_, why) => LoginError::Unreadable(path.to_path_buf(), why),
            other => other,
        })
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

    /// `request` with this login's bearer token and ChatGPT account header,
    /// the two headers every ChatGPT backend request carries. The secrets
    /// go only into the request, never into a value the caller can print.
    pub fn authorize(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        request
            .bearer_auth(&self.access_token)
            .header("ChatGPT-Account-ID", &self.account_id)
    }
}

/// Removes a file, counting one that is already gone as removed.
fn remove(path: &Path) -> Result<(), LoginError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(LoginError::Unremovable(
            path.to_path_buf(),
            error.to_string(),
        )),
    }
}

/// Marks this process non-dumpable (`PR_SET_DUMPABLE` 0) on Linux, and
/// returns whether it is.
///
/// Reading a non-dumpable process's `/proc/<pid>/mem`, `environ`, `fd`,
/// or `maps` needs `CAP_SYS_PTRACE`, even for a process of the same user,
/// and the process can't be traced or dump core. A child regains the
/// default when it executes a program, so the commands this process runs
/// are unaffected. Elsewhere this does nothing and returns `false`.
#[must_use = "false means the process memory is still readable"]
pub fn protect_process() -> bool {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: PR_SET_DUMPABLE takes an integer, PR_GET_DUMPABLE takes
        // nothing, and neither touches this process's memory.
        unsafe {
            libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) == 0
                && libc::prctl(libc::PR_GET_DUMPABLE, 0, 0, 0, 0) == 0
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
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
        "parallel_tool_calls": request.parallel_tools,
        "store": false,
        "stream": true,
        "include": ["reasoning.encrypted_content"],
        "prompt_cache_key": request.cache_key,
    });
    body["reasoning"] = json!({ "summary": "auto" });
    if let Some(effort) = &request.effort {
        body["reasoning"]["effort"] = json!(effort);
    }
    if let Some(format) = &request.text_format {
        body["text"] = json!({ "format": format });
        // A reply shaped by its format declares no tools.
        if request.tools.is_empty()
            && let Some(fields) = body.as_object_mut()
        {
            for field in ["tools", "tool_choice", "parallel_tool_calls"] {
                fields.remove(field);
            }
        }
    }
    body
}

/// Where a [`CodexTransport`] gets its login.
#[derive(Debug)]
enum Source {
    /// Read from this file before every request.
    File(PathBuf),
    /// Held in memory, taken once by [`Login::take`].
    Held(Login),
    /// An API key for an Open Responses endpoint ([`CodexTransport::with_key`]).
    Key(ApiKey),
}

/// A bearer API key. Its `Debug` output hides it, and its bytes are
/// zeroed when it drops.
struct ApiKey(String);

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKey(<redacted>)")
    }
}

impl Drop for ApiKey {
    fn drop(&mut self) {
        let mut bytes = std::mem::take(&mut self.0).into_bytes();
        bytes.fill(0);
    }
}

/// The Codex-login transport.
#[derive(Debug)]
pub struct CodexTransport {
    http: reqwest::Client,
    login: Source,
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
        CodexTransport::build(Source::File(login_path), session_id)
    }

    /// A transport on a login already in memory, such as one
    /// [`Login::take`] returned. It opens no file, and still checks the
    /// token's expiry before every request.
    ///
    /// # Errors
    ///
    /// [`LoginError::Expiring`] when the token is inside the margin now.
    pub fn holding(login: Login, session_id: &str) -> Result<CodexTransport, LoginError> {
        login.check(now_secs())?;
        CodexTransport::build(Source::Held(login), session_id)
    }

    /// A transport on an Open Responses endpoint (`url`, the full
    /// `.../v1/responses` address, such as the OpenAgents gateway's) with a
    /// bearer API key instead of a Codex login. The request body is the
    /// same; no ChatGPT account header is sent.
    ///
    /// # Errors
    ///
    /// [`LoginError::Unreadable`] for an empty or malformed key, or a URL
    /// that isn't `http(s)://`.
    pub fn with_key(
        url: &str,
        key: String,
        session_id: &str,
    ) -> Result<CodexTransport, LoginError> {
        let key = ApiKey(key.trim().to_owned());
        if key.0.is_empty() || key.0.chars().any(char::is_whitespace) {
            return Err(LoginError::Unreadable(
                PathBuf::new(),
                "the API key is empty or malformed".into(),
            ));
        }
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err(LoginError::Unreadable(
                PathBuf::new(),
                "the responses URL must be http(s)".into(),
            ));
        }
        let mut transport = CodexTransport::build(Source::Key(key), session_id)?;
        transport.url = url.to_owned();
        Ok(transport)
    }

    fn build(login: Source, session_id: &str) -> Result<CodexTransport, LoginError> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("microluna/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| LoginError::Unreadable(PathBuf::new(), error.to_string()))?;
        Ok(CodexTransport {
            http,
            login,
            url: format!("{BASE_URL}/responses"),
            session_id: session_id.to_string(),
        })
    }

    /// The login to send with, checked for expiry now; `None` for an
    /// API-key transport.
    fn login(&self) -> Result<Option<Cow<'_, Login>>, LoginError> {
        match &self.login {
            Source::File(path) => Login::load(path).map(|login| Some(Cow::Owned(login))),
            Source::Held(login) => {
                login.check(now_secs())?;
                Ok(Some(Cow::Borrowed(login)))
            }
            Source::Key(_) => Ok(None),
        }
    }
}

impl Transport for CodexTransport {
    async fn respond(&self, request: &Request) -> Result<Reply, TransportError> {
        self.respond_streaming(request, &mut |_| {}).await
    }

    async fn respond_streaming(
        &self,
        request: &Request,
        text: &mut dyn FnMut(&str),
    ) -> Result<Reply, TransportError> {
        let login = self.login().map_err(TransportError::Login)?;
        let post = self
            .http
            .post(&self.url)
            .timeout(REQUEST_TIMEOUT)
            .header("originator", ORIGINATOR)
            .header("session-id", &self.session_id)
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .json(&body(request));
        let post = match (&login, &self.login) {
            (Some(login), _) => post
                .bearer_auth(&login.access_token)
                .header("ChatGPT-Account-ID", &login.account_id),
            (None, Source::Key(key)) => post.bearer_auth(&key.0),
            (None, _) => post,
        };
        let mut response = post.send().await.map_err(sent_error)?;
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
            events.push_streaming(&chunk, text)?;
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
        self.push_streaming(bytes, &mut |_| {})
    }

    /// [`Events::push`], handing `text` each message-text and
    /// function-call-arguments delta as its event is read.
    ///
    /// # Errors
    ///
    /// The provider's failure, when an event reports one.
    pub fn push_streaming(
        &mut self,
        bytes: &[u8],
        text: &mut dyn FnMut(&str),
    ) -> Result<(), TransportError> {
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
            self.read(&event, text)?;
        }
        Ok(())
    }

    fn read(&mut self, event: &Value, text: &mut dyn FnMut(&str)) -> Result<(), TransportError> {
        match event["type"].as_str().unwrap_or_default() {
            "response.output_text.delta" | "response.function_call_arguments.delta" => {
                if let Some(delta) = event["delta"].as_str() {
                    text(delta);
                }
            }
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
                return Err(reported(
                    TransportError::Failed(
                        error["message"]
                            .as_str()
                            .or(error["code"].as_str())
                            .unwrap_or("no reason given")
                            .to_string(),
                    ),
                    &event["response"]["usage"],
                ));
            }
            "response.incomplete" => {
                return Err(reported(
                    TransportError::Incomplete(
                        event["response"]["incomplete_details"]["reason"]
                            .as_str()
                            .unwrap_or("no reason given")
                            .to_string(),
                    ),
                    &event["response"]["usage"],
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

/// The error for a request that got no response headers. A connection
/// that was never made sent nothing ([`TransportError::Unsent`]); any
/// other failure, a timeout included, may have come after the request
/// went out ([`TransportError::Stream`]).
fn sent_error(error: reqwest::Error) -> TransportError {
    let unsent = error.is_connect();
    let why = error.without_url().to_string();
    if unsent {
        TransportError::Unsent(why)
    } else {
        TransportError::Stream(why)
    }
}

/// A failed or incomplete response, with the usage it reported when it
/// reported any: then its cost is known, not unknown.
fn reported(error: TransportError, usage_value: &Value) -> TransportError {
    if usage_value["input_tokens"].is_u64() {
        TransportError::Reported {
            error: Box::new(error),
            usage: usage(usage_value),
        }
    } else {
        error
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

/// A usage-limit refusal from the Codex backend: HTTP 429 whose body's
/// `error.type` is `usage_limit_reached`. The login's plan has used its
/// allowance for the window, so another attempt before the reset fails the
/// same way. Other 429 bodies are ordinary rate limits and stay transient.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsageLimit {
    /// When the allowance resets, in Unix seconds, as the backend reports.
    pub resets_at: Option<u64>,
    /// Seconds until the reset, as the backend reports.
    pub resets_in_seconds: Option<u64>,
    /// The plan, such as `pro`, when reported.
    pub plan_type: Option<String>,
    /// The allowance window in minutes, when reported.
    pub window_minutes: Option<u64>,
}

/// The error kinds a 429 body can name. Only the usage limit is typed;
/// every other kind is an ordinary rate limit.
#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum LimitKind {
    UsageLimitReached,
    #[serde(other)]
    Other,
}

#[derive(serde::Deserialize)]
struct LimitError {
    #[serde(rename = "type")]
    kind: LimitKind,
    #[serde(default)]
    resets_at: Option<u64>,
    #[serde(default)]
    resets_in_seconds: Option<u64>,
    #[serde(default)]
    plan_type: Option<String>,
    #[serde(default)]
    limit_window_minutes: Option<u64>,
}

#[derive(serde::Deserialize)]
struct LimitBody {
    error: LimitError,
}

impl UsageLimit {
    /// The usage limit an error response reports, or `None` for any other
    /// status or body.
    #[must_use]
    pub fn parse(status: u16, body: &str) -> Option<UsageLimit> {
        if status != 429 {
            return None;
        }
        let body: LimitBody = serde_json::from_str(body.trim()).ok()?;
        match body.error.kind {
            LimitKind::UsageLimitReached => Some(UsageLimit {
                resets_at: body.error.resets_at,
                resets_in_seconds: body.error.resets_in_seconds,
                plan_type: body
                    .error
                    .plan_type
                    .filter(|plan| plan.len() <= 32 && plan.bytes().all(|b| b.is_ascii_graphic())),
                window_minutes: body.error.limit_window_minutes,
            }),
            LimitKind::Other => None,
        }
    }
}

/// At most `max` characters of `text`, trimmed, for an error message.
#[must_use]
pub fn excerpt(text: &str, max: usize) -> String {
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
    fn taking_the_runs_copy_removes_only_that_copy() {
        let dir = tempfile::tempdir().unwrap();
        let copy = dir.path().join("auth.json");
        std::fs::write(&copy, auth(now_secs() + 3_600)).unwrap();

        let login = Login::take(&copy).unwrap();
        assert!(login.check(now_secs()).is_ok());
        assert!(std::fs::symlink_metadata(&copy).is_err());
        // A second take finds nothing; the transport holds the first.
        assert!(matches!(Login::take(&copy), Err(LoginError::Missing(_))));
        let transport = CodexTransport::holding(login, "s-1").unwrap();
        assert!(transport.login().is_ok());
        assert!(!format!("{transport:?}").contains("acct-1"));
    }

    /// A person whose `auth.json` is a link (dotfiles) keeps both the
    /// link and the file it names: a take refuses a link and touches
    /// neither, and the file's bytes stay exactly as they were (#10083).
    #[test]
    #[cfg(unix)]
    fn a_linked_login_is_refused_and_neither_the_link_nor_its_file_is_touched() {
        let dir = tempfile::tempdir().unwrap();
        let dotfiles = dir.path().join("dotfiles");
        let home = dir.path().join("codex-home");
        std::fs::create_dir_all(&dotfiles).unwrap();
        std::fs::create_dir_all(&home).unwrap();
        let real = dotfiles.join("auth.json");
        let bytes = auth(now_secs() + 3_600);
        std::fs::write(&real, &bytes).unwrap();
        let link = home.join("auth.json");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let error = Login::take(&link).unwrap_err();
        assert!(matches!(error, LoginError::Linked(_)), "{error}");
        assert!(!error.to_string().contains("acct-1"));
        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(std::fs::read_to_string(&real).unwrap(), bytes);
        // Reading through the link, as a run without a take does, works.
        assert!(Login::load(&link).is_ok());
    }

    #[test]
    fn a_malformed_login_is_removed_all_the_same() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("auth.json");
        std::fs::write(&file, "not json").unwrap();
        assert!(matches!(
            Login::take(&file),
            Err(LoginError::Unreadable(..))
        ));
        assert!(!file.exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_taken_login_leaves_the_process_non_dumpable() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("auth.json");
        std::fs::write(&file, auth(now_secs() + 3_600)).unwrap();
        Login::take(&file).unwrap();
        // SAFETY: PR_GET_DUMPABLE reads a flag and takes no pointers.
        assert_eq!(unsafe { libc::prctl(libc::PR_GET_DUMPABLE, 0, 0, 0, 0) }, 0);
    }

    #[test]
    fn a_held_login_is_still_refused_near_expiry() {
        let login = Login::parse(&auth(now_secs() + 60)).unwrap();
        assert!(matches!(
            CodexTransport::holding(login, "s-1"),
            Err(LoginError::Expiring { .. })
        ));
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
            parallel_tools: false,
            model: "gpt-6-luna".to_string(),
            instructions: "be brief".to_string(),
            input: vec![json!({"type": "message"})],
            tools: vec![],
            effort: Some("low".to_string()),
            cache_key: "task-1".to_string(),
            text_format: None,
        };
        let body = body(&request);
        assert_eq!(body["store"], false);
        assert_eq!(body["stream"], true);
        assert_eq!(body["prompt_cache_key"], "task-1");
        assert_eq!(body["reasoning"]["effort"], "low");
        assert_eq!(body["reasoning"]["summary"], "auto");
        assert_eq!(body["include"][0], "reasoning.encrypted_content");
    }

    #[test]
    fn a_text_format_replaces_the_tool_fields() {
        let format = json!({"type": "json_schema", "name": "x", "schema": {}, "strict": true});
        let request = Request {
            parallel_tools: false,
            model: "gpt-6-luna".to_string(),
            instructions: "be brief".to_string(),
            input: vec![],
            tools: vec![],
            effort: Some("medium".to_string()),
            cache_key: "task-1".to_string(),
            text_format: Some(format.clone()),
        };
        let body = body(&request);
        assert_eq!(body["text"]["format"], format);
        assert_eq!(body["reasoning"]["effort"], "medium");
        for field in ["tools", "tool_choice", "parallel_tool_calls"] {
            assert!(body.get(field).is_none(), "{field}");
        }
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
    fn text_deltas_stream_in_order_and_the_reply_is_unchanged() {
        let stream = concat!(
            "data: {\"type\":\"response.reasoning_summary_text.delta\",\"delta\":\"thinking\"}\n\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"{\\\"reply\\\":\"}\n\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"\\\"hi\\\"}\"}\n\n",
            "data: {\"type\":\"response.function_call_arguments.delta\",\"delta\":\"{}\"}\n\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r1\",\"model\":\"gpt-6-luna\",",
            "\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}}\n\n",
        );
        let mut events = Events::default();
        let mut text = String::new();
        for piece in stream.as_bytes().chunks(9) {
            events
                .push_streaming(piece, &mut |delta| text.push_str(delta))
                .unwrap();
        }
        assert_eq!(text, "{\"reply\":\"hi\"}{}");
        assert_eq!(events.finish().unwrap().id.as_deref(), Some("r1"));
    }

    #[test]
    fn summaries_are_requested_without_overriding_the_default_effort() {
        let request = Request {
            parallel_tools: false,
            model: "gpt-6-luna".into(),
            instructions: "Review the code.".into(),
            input: vec![],
            tools: vec![],
            effort: None,
            cache_key: "summary-default".into(),
            text_format: None,
        };
        assert_eq!(body(&request)["reasoning"], json!({"summary": "auto"}));
    }

    #[test]
    fn readable_summaries_survive_an_empty_completed_output() {
        let mut events = Events::default();
        let stream = concat!(
            "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"reasoning\",",
            "\"summary\":[{\"type\":\"summary_text\",\"text\":\"Check expiration.\"},",
            "{\"type\":\"summary_text\",\"text\":\"Check replacement.\"}]}}\n\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r1\",",
            "\"model\":\"gpt-6-luna\",\"output\":[],\"usage\":{\"input_tokens\":1,\"output_tokens\":2}}}\n\n",
        );
        for piece in stream.as_bytes().chunks(13) {
            events.push(piece).unwrap();
        }
        assert_eq!(
            events.finish().unwrap().reasoning(),
            "Check expiration.\nCheck replacement."
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

    #[test]
    fn a_failure_that_reports_usage_carries_it() {
        let mut events = Events::default();
        let failed = concat!(
            "data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"message\":\"no\"},",
            "\"usage\":{\"input_tokens\":900,\"output_tokens\":12}}}\n\n"
        );
        match events.push(failed.as_bytes()) {
            Err(TransportError::Reported { error, usage }) => {
                assert!(matches!(*error, TransportError::Failed(ref why) if why == "no"));
                assert_eq!((usage.input, usage.output), (900, 12));
            }
            other => panic!("{other:?}"),
        }
        let mut events = Events::default();
        let incomplete = concat!(
            "data: {\"type\":\"response.incomplete\",\"response\":{\"incomplete_details\":",
            "{\"reason\":\"max_output_tokens\"},\"usage\":null}}\n\n"
        );
        assert!(matches!(
            events.push(incomplete.as_bytes()),
            Err(TransportError::Incomplete(why)) if why == "max_output_tokens"
        ));
    }

    #[test]
    fn a_usage_limit_is_typed_and_not_transient() {
        // The body the Codex backend returned on 2026-09-28.
        let body = r#"{"error":{"type":"usage_limit_reached","message":"The usage limit has been reached","plan_type":"pro","resets_at":1791050823,"eligible_promo":null,"limit_window_minutes":10080,"resets_in_seconds":478613}}"#;
        let limit = UsageLimit::parse(429, body).unwrap();
        assert_eq!(limit.resets_at, Some(1_791_050_823));
        assert_eq!(limit.resets_in_seconds, Some(478_613));
        assert_eq!(limit.plan_type.as_deref(), Some("pro"));
        assert_eq!(limit.window_minutes, Some(10_080));
        let error = TransportError::Http {
            status: 429,
            body: body.into(),
        };
        assert!(!error.transient());
        // Another status, another error kind, or no JSON is not a usage limit.
        assert_eq!(UsageLimit::parse(500, body), None);
        let other = r#"{"error":{"type":"rate_limit_exceeded","message":"slow down"}}"#;
        assert_eq!(UsageLimit::parse(429, other), None);
        assert_eq!(UsageLimit::parse(429, "Too Many Requests"), None);
        let plain = TransportError::Http {
            status: 429,
            body: other.into(),
        };
        assert!(plain.transient());
    }

    /// An API-key transport sends the key as a bearer to the endpoint it
    /// names, without the ChatGPT account header, and reads the same
    /// stream; the key never shows in Debug.
    #[test]
    fn an_api_key_transport_posts_to_its_endpoint_with_the_key() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut seen = Vec::new();
            let mut buffer = [0u8; 8192];
            while !String::from_utf8_lossy(&seen).contains("prompt_cache_key") {
                let n = stream.read(&mut buffer).unwrap();
                if n == 0 {
                    break;
                }
                seen.extend_from_slice(&buffer[..n]);
            }
            let body = concat!(
                "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",",
                "\"call_id\":\"c1\",\"name\":\"finish\",\"arguments\":\"{}\"}}\n\n",
                "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r1\",",
                "\"model\":\"openagents/code\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}}\n\n",
            );
            write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            String::from_utf8_lossy(&seen).into_owned()
        });
        let url = format!("http://{address}/v1/responses");
        let transport =
            CodexTransport::with_key(&url, "oak_test.secret".into(), "environment-1").unwrap();
        assert!(!format!("{transport:?}").contains("oak_test"));
        let request = Request {
            model: "openagents/code".into(),
            instructions: "Set it up.".into(),
            input: vec![],
            tools: vec![],
            effort: None,
            cache_key: "k".into(),
            parallel_tools: false,
            text_format: None,
        };
        let reply = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(transport.respond(&request))
            .unwrap();
        assert_eq!(reply.calls()[0].name, "finish");
        let seen = server.join().unwrap().to_ascii_lowercase();
        assert!(seen.starts_with("post /v1/responses "), "{seen}");
        assert!(
            seen.contains("authorization: bearer oak_test.secret"),
            "{seen}"
        );
        assert!(!seen.contains("chatgpt-account-id"), "{seen}");
        for bad in ["", "two words"] {
            assert!(CodexTransport::with_key(&url, bad.into(), "s").is_err());
        }
        assert!(CodexTransport::with_key("file:///x", "k".into(), "s").is_err());
    }
}
