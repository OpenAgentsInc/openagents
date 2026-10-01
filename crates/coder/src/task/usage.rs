//! Provider usage windows: how much of a login's allowance is used, and
//! when each window resets, read from the provider before a run starts.
//!
//! [`super::capacity`] learns that a provider is out only from a refusal.
//! When the owner turns usage probes on (`coder host autostart on
//! --probe-usage`), the host also asks each admitted provider's usage
//! endpoint and keeps the typed answer in `usage.json` beside
//! `capacity.json`:
//!
//! | Provider | Endpoint | Windows |
//! | --- | --- | --- |
//! | Claude | `GET https://api.anthropic.com/api/oauth/usage` (`anthropic-beta: oauth-2025-04-20`) | `five_hour`, `seven_day`: `utilization` percent and `resets_at` |
//! | Codex | `GET https://chatgpt.com/backend-api/wham/usage` | `primary_window`, `secondary_window`: `used_percent`, `limit_window_seconds`, `reset_at`; and `limit_reached` |
//!
//! Both endpoints are private and undocumented. A probe result is
//! advisory: routing prefers an admitted route whose provider is below the
//! owner's threshold, and a refusal in the capacity book stays the
//! authority. Any failure (no credential, an expired one, a refused or
//! rate-limited request, a malformed body, no network) is recorded as a
//! typed [`Failure`] and routing falls back to refusal-only capacity.
//!
//! Probes are cached: a provider is asked at most once per
//! [`MIN_INTERVAL`], a failure waits [`FAILURE_BACKOFF`], and a
//! `Retry-After` is honored up to [`MAX_RETRY_AFTER`]. A reading older than
//! [`STALE_AFTER`] is not used.
//!
//! **Credentials.** A probe reads the provider's OAuth access token: the
//! Codex login `codex_transport::codex::Login::load` reads, and Claude Code's
//! `claudeAiOauth.accessToken` from, on macOS, the `Claude Code-credentials`
//! keychain item (read with `/usr/bin/security`, as Claude Code itself
//! reads it), else `$CLAUDE_CONFIG_DIR/.credentials.json` or
//! `~/.claude/.credentials.json`. The keychain comes first on macOS, where
//! Claude Code keeps its current login (#10105). The token is sent
//! only to that provider's own usage endpoint, is never written, logged,
//! or stored in `usage.json`, and no credential store is ever modified.
//!
//! The book's types, parsers, and cache rules are
//! `microcoder_loop::usage`, re-exported here; this module adds the fetch,
//! which reads the credential.

use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;

pub use microcoder_loop::usage::*;

/// Claude Code's OAuth usage endpoint.
pub const CLAUDE_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
/// The beta header value the Claude endpoint requires.
pub const CLAUDE_OAUTH_BETA: &str = "oauth-2025-04-20";
/// The ChatGPT backend's Codex usage endpoint.
pub const CODEX_USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const KEYCHAIN_TIMEOUT: Duration = Duration::from_secs(10);
const KEYCHAIN_SERVICE: &str = "Claude Code-credentials";

use super::capacity::Provider;

/// Ask `provider`'s usage endpoint with its local login. Runs on its own
/// thread with its own runtime, so it can be called from any context.
///
/// # Errors
/// A typed [`Failure`] when there is no usable credential or the request
/// does not complete.
pub fn fetch(provider: Provider) -> Result<Response, Failure> {
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| Failure::Network)?;
        runtime.block_on(fetch_async(provider))
    })
    .join()
    .unwrap_or(Err(Failure::Network))
}

async fn fetch_async(provider: Provider) -> Result<Response, Failure> {
    let http = reqwest::Client::builder()
        .user_agent(concat!("openagents-coder/", env!("CARGO_PKG_VERSION")))
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|_| Failure::Network)?;
    let request = match provider {
        Provider::Codex => {
            let path =
                codex_transport::codex::Login::default_path().ok_or(Failure::NoCredential)?;
            let login =
                codex_transport::codex::Login::load(&path).map_err(|error| match error {
                    codex_transport::codex::LoginError::Expiring { .. } => Failure::Expired,
                    _ => Failure::NoCredential,
                })?;
            login.authorize(http.get(CODEX_USAGE_URL))
        }
        Provider::Claude => {
            let token = claude_token(now_millis())?;
            http.get(CLAUDE_USAGE_URL)
                .bearer_auth(&token.0)
                .header("anthropic-beta", CLAUDE_OAUTH_BETA)
        }
        Provider::Vertex | Provider::Devin | Provider::OpenCode | Provider::Grok => {
            return Err(Failure::Unsupported);
        }
    };
    let response = request.send().await.map_err(|_| Failure::Network)?;
    let status = response.status().as_u16();
    let retry_after = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok());
    let body = response.bytes().await.map_err(|_| Failure::Network)?;
    Ok(Response {
        status,
        retry_after,
        body: body.to_vec(),
    })
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Claude Code's OAuth access token. Its `Debug` output hides it.
struct ClaudeToken(String);

impl std::fmt::Debug for ClaudeToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ClaudeToken(<redacted>)")
    }
}

/// The access token in Claude Code's credential record, checked for
/// expiry at `now_ms`.
fn claude_credential(bytes: &[u8], now_ms: u64) -> Result<ClaudeToken, Failure> {
    #[derive(Deserialize)]
    struct Record {
        #[serde(rename = "claudeAiOauth")]
        oauth: Option<OAuth>,
    }
    #[derive(Deserialize)]
    struct OAuth {
        #[serde(rename = "accessToken")]
        access_token: Option<String>,
        #[serde(default, rename = "expiresAt")]
        expires_at: Option<u64>,
    }
    let record: Record = serde_json::from_slice(bytes).map_err(|_| Failure::NoCredential)?;
    let oauth = record.oauth.ok_or(Failure::NoCredential)?;
    let token = oauth
        .access_token
        .filter(|token| !token.is_empty())
        .ok_or(Failure::NoCredential)?;
    if oauth.expires_at.is_some_and(|at| at <= now_ms) {
        return Err(Failure::Expired);
    }
    Ok(ClaudeToken(token))
}

/// Claude Code's token, from where Claude Code itself keeps the login it
/// signed in last: on macOS the keychain item it writes for this account,
/// else (and on other systems) its credentials file
/// ([`claude_credentials_file`]). The keychain comes first on macOS: a
/// file left from an earlier login must not be read in place of the
/// current one (#10105).
fn claude_token(now_ms: u64) -> Result<ClaudeToken, Failure> {
    let variable = |name: &str| std::env::var_os(name);
    if cfg!(target_os = "macos")
        && std::env::var_os("CLAUDE_CONFIG_DIR").is_none_or(|dir| dir.is_empty())
        && let Some(bytes) = keychain_record()
    {
        return claude_credential(&bytes, now_ms);
    }
    let path = claude_credentials_file(&variable).ok_or(Failure::NoCredential)?;
    let bytes = std::fs::read(path).map_err(|_| Failure::NoCredential)?;
    claude_credential(&bytes, now_ms)
}

/// Claude Code's credentials file: `$CLAUDE_CONFIG_DIR/.credentials.json`,
/// else `~/.claude/.credentials.json`, as Claude Code resolves it.
fn claude_credentials_file(
    variable: &dyn Fn(&str) -> Option<std::ffi::OsString>,
) -> Option<PathBuf> {
    if let Some(dir) = variable("CLAUDE_CONFIG_DIR").filter(|dir| !dir.is_empty()) {
        return Some(PathBuf::from(dir).join(".credentials.json"));
    }
    variable("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".claude/.credentials.json"))
}

/// The `Claude Code-credentials` generic password for this account, read
/// with `/usr/bin/security` and a timeout, so a locked keychain cannot
/// hold the host.
fn keychain_record() -> Option<Vec<u8>> {
    use std::process::{Command, Stdio};
    let account = std::env::var_os("USER")
        .filter(|user| !user.is_empty())
        .or_else(account_name)?;
    let mut child = Command::new("/usr/bin/security")
        .arg("find-generic-password")
        .arg("-s")
        .arg(KEYCHAIN_SERVICE)
        .arg("-a")
        .arg(account)
        .arg("-w")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let deadline = std::time::Instant::now() + KEYCHAIN_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(25));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let bytes = reader.join().ok()?.ok()?;
    status.filter(std::process::ExitStatus::success)?;
    Some(bytes)
}

/// This process's account name.
#[cfg(windows)]
fn account_name() -> Option<std::ffi::OsString> {
    std::env::var_os("USERNAME").filter(|name| !name.is_empty())
}

/// This process's account name from the account database.
#[cfg(unix)]
fn account_name() -> Option<std::ffi::OsString> {
    use std::os::unix::ffi::OsStrExt;
    // SAFETY: getpwuid returns a pointer into static storage or null; the
    // name is copied before any other call that could reuse it.
    unsafe {
        let entry = libc::getpwuid(libc::getuid());
        if entry.is_null() || (*entry).pw_name.is_null() {
            return None;
        }
        let name = std::ffi::CStr::from_ptr((*entry).pw_name);
        Some(std::ffi::OsStr::from_bytes(name.to_bytes()).to_os_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_claude_credential_is_read_typed_and_never_printed() {
        let record = br#"{"claudeAiOauth":{"accessToken":"sk-ant-oat-secret","expiresAt":2000,"refreshToken":"r"},"mcpOAuth":{}}"#;
        let token = claude_credential(record, 1_000).unwrap();
        assert!(!format!("{token:?}").contains("secret"));
        assert_eq!(
            claude_credential(record, 2_000).unwrap_err(),
            Failure::Expired
        );
        assert_eq!(
            claude_credential(br#"{"mcpOAuth":{}}"#, 1_000).unwrap_err(),
            Failure::NoCredential
        );
        assert_eq!(
            claude_credential(b"not json", 1_000).unwrap_err(),
            Failure::NoCredential
        );
    }

    /// The probe reads the credentials file Claude Code writes on sign-in,
    /// `CLAUDE_CONFIG_DIR` included (#10105). Paths only: no file is read.
    #[test]
    fn the_claude_credentials_file_is_where_claude_code_keeps_it() {
        let home = |name: &str| (name == "HOME").then(|| "/home/owner".into());
        assert_eq!(
            claude_credentials_file(&home),
            Some(PathBuf::from("/home/owner/.claude/.credentials.json"))
        );
        let relocated = |name: &str| match name {
            "CLAUDE_CONFIG_DIR" => Some("/srv/claude".into()),
            _ => home(name),
        };
        assert_eq!(
            claude_credentials_file(&relocated),
            Some(PathBuf::from("/srv/claude/.credentials.json"))
        );
        assert_eq!(claude_credentials_file(&|_| None), None);
    }
}
