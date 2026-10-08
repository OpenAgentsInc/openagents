//! Claude Code's sign-in status, read inside the user's computer.
//!
//! The pinned Claude Code release answers `claude auth status` without a
//! session: JSON on standard output, exit 0 when logged in and 1 when not,
//! with `loggedIn`, `authMethod` (`none`, `claude.ai`, `oauth_token`,
//! `api_key`, `api_key_helper`, or `third_party`), and, for an account
//! login, `subscriptionType`. That output also names the account's email
//! and organization; [`classify`] reads only the typed fields above and
//! drops the rest, so nothing it read can leave in a [`Status`].
//!
//! A usage limit, and a login that stopped working, show up only when
//! Claude Code works. The Coder delegate records what Claude Code printed
//! then as a typed [`Notice`] in the computer's own notice directory
//! ([`remember`]), and [`check`] folds the last one in. No credential file
//! is read: the status comes from the binary and from those notices.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::{EXPIRING_WITHIN, Engine, Method, Notice, Plan, State, Status};

/// Where the runtime image installs the pinned, unmodified binary.
pub const PROGRAM: &str = "/usr/local/bin/claude";

/// The binary's own non-interactive status command.
pub const STATUS_ARGS: [&str; 2] = ["auth", "status"];

/// The environment variable that names the computer's notice directory
/// for a Coder run. Unset, the delegate keeps no notice.
pub const NOTICE_DIR_ENV: &str = "OA_ENGINE_NOTICE_DIR";

/// How long the status command may run.
pub const TIMEOUT: Duration = Duration::from_secs(15);

/// The most status output read.
const OUTPUT_MAX: u64 = 64 * 1024;

/// A rate limit with no reported reset is shown this long after the
/// notice: Claude Code's shortest window.
const UNTIMED_LIMIT: u64 = 5 * 3_600;

/// A login-expiring warning without a date counts for this long.
const EXPIRING_NOTICE: u64 = 86_400;

/// The notice directory under a computer's home.
#[must_use]
pub fn notice_dir(home: &Path) -> PathBuf {
    home.join(".openagents").join("engine")
}

fn notice_file(dir: &Path) -> PathBuf {
    dir.join("claude.json")
}

/// What the status command printed and how it exited.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Output {
    pub exit: Option<i32>,
    pub stdout: String,
}

/// Run the binary's status command with standard input closed and
/// standard error discarded, for at most `timeout`.
#[must_use]
pub fn run(program: &Path, timeout: Duration) -> Option<Output> {
    let mut child = Command::new(program)
        .args(STATUS_ARGS)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut bytes = vec![];
        let _ = stdout.take(OUTPUT_MAX).read_to_end(&mut bytes);
        bytes
    });
    let started = Instant::now();
    let exit = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) if started.elapsed() < timeout => {
                std::thread::sleep(Duration::from_millis(25));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    let bytes = reader.join().ok()?;
    Some(Output {
        exit,
        stdout: String::from_utf8_lossy(&bytes).into_owned(),
    })
}

/// The computer's Claude Code sign-in status: run `program`'s status
/// command, then fold in the last notice under `notices`.
#[must_use]
pub fn check(program: &Path, notices: Option<&Path>, now: u64) -> Status {
    let notice = notices.and_then(recall);
    classify(run(program, TIMEOUT).as_ref(), notice, now)
}

/// Fold the status command's answer and the last notice into a status.
#[must_use]
pub fn classify(output: Option<&Output>, notice: Option<Notice>, now: u64) -> Status {
    let Some(output) = output else {
        return Status::unavailable(Engine::Claude, now);
    };
    let mut status = Status::unavailable(Engine::Claude, now);
    let json = serde_json::from_str::<Value>(output.stdout.trim())
        .ok()
        .filter(Value::is_object);
    let Some(json) = json else {
        if output.exit == Some(1) {
            status.state = State::SignedOut;
        }
        return status;
    };
    let logged_in = json["loggedIn"].as_bool().unwrap_or(output.exit == Some(0));
    let method = match json["authMethod"].as_str() {
        Some("none") => None,
        Some("claude.ai") => Some(Method::ClaudeAi),
        Some("oauth_token") => Some(Method::OauthToken),
        Some("api_key") => Some(Method::ApiKey),
        Some("api_key_helper") => Some(Method::ApiKeyHelper),
        Some("third_party") => Some(Method::ThirdParty),
        Some(_) => Some(Method::Other),
        None if logged_in => Some(Method::Other),
        None => None,
    };
    if !logged_in || method.is_none() {
        status.state = State::SignedOut;
        return status;
    }
    status.method = method;
    let account = matches!(method, Some(Method::ClaudeAi | Method::OauthToken));
    if account {
        status.plan = json["subscriptionType"].as_str().map(|plan| {
            match plan.to_ascii_lowercase().as_str() {
                "free" => Plan::Free,
                "pro" => Plan::Pro,
                "max" => Plan::Max,
                "team" => Plan::Team,
                "enterprise" => Plan::Enterprise,
                _ => Plan::Other,
            }
        });
        status.expires_at = expiry(&json);
    }
    let expiry_in_future = status.expires_at.is_some_and(|at| at > now);
    status.state = if status.expires_at.is_some_and(|at| at <= now) {
        State::Expired
    } else if matches!(notice, Some(Notice::LoginExpired { .. })) && !expiry_in_future {
        State::Expired
    } else if let Some(Notice::Limited { resets_at, at }) = notice
        && match resets_at {
            Some(reset) => reset > now,
            None => now.saturating_sub(at) < UNTIMED_LIMIT,
        }
    {
        status.resets_at = resets_at;
        State::RateLimited
    } else if account
        && (status
            .expires_at
            .is_some_and(|at| at.saturating_sub(now) <= EXPIRING_WITHIN)
            || matches!(notice, Some(Notice::LoginExpiring { at })
                if now.saturating_sub(at) < EXPIRING_NOTICE))
    {
        State::Expiring
    } else if account {
        State::SignedIn
    } else {
        State::ApiKey
    };
    status
}

/// The login's expiry, when this release's status names one: Unix seconds
/// or milliseconds, or an ISO 8601 UTC time.
fn expiry(json: &Value) -> Option<u64> {
    ["expiresAt", "tokenExpiresAt", "oauthExpiresAt"]
        .iter()
        .find_map(|key| match &json[*key] {
            Value::Number(number) => number
                .as_u64()
                .map(|at| if at > 100_000_000_000 { at / 1_000 } else { at }),
            Value::String(text) => crate::limit::parse_iso(text),
            _ => None,
        })
}

/// Phrases Claude Code prints when its login no longer works, lowercased.
const EXPIRED: &[&str] = &[
    "oauth token has expired",
    "token has expired",
    "login has expired",
    "please run /login",
    "run /login",
    "invalid api key",
];

/// Phrases Claude Code prints when its login expires soon, lowercased.
const EXPIRING: &[&str] = &[
    "login expires",
    "login is about to expire",
    "login will expire",
];

impl Notice {
    /// The notice error text from a Claude Code run says, if any: a usage
    /// limit with the reset time it states, or a login that expired or
    /// will soon. Only the kind and times are kept.
    #[must_use]
    pub fn from_text(text: &str, now: u64) -> Option<Notice> {
        if crate::limit::says_limited(text) {
            return Some(Notice::Limited {
                resets_at: crate::limit::reset_from_message(text, now),
                at: now,
            });
        }
        let lower = text.to_lowercase();
        if EXPIRED.iter().any(|phrase| lower.contains(phrase)) {
            return Some(Notice::LoginExpired { at: now });
        }
        if EXPIRING.iter().any(|phrase| lower.contains(phrase)) {
            return Some(Notice::LoginExpiring { at: now });
        }
        None
    }
}

/// Keep `notice` as the computer's last Claude Code notice.
///
/// # Errors
/// When the directory or file cannot be written.
pub fn remember(dir: &Path, notice: Notice) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let file = notice_file(dir);
    let staged = dir.join("claude.json.writing");
    std::fs::write(&staged, serde_json::to_vec(&notice)?)?;
    std::fs::rename(staged, file)
}

/// Clear the last notice: a later Claude Code run finished normally.
pub fn forget(dir: &Path) {
    let _ = std::fs::remove_file(notice_file(dir));
}

/// The computer's last Claude Code notice, if one is kept.
#[must_use]
pub fn recall(dir: &Path) -> Option<Notice> {
    let file = notice_file(dir);
    let size = std::fs::metadata(&file).ok()?.len();
    if size > 4096 {
        return None;
    }
    serde_json::from_slice(&std::fs::read(file).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_790_163_058;

    fn output(exit: i32, stdout: &str) -> Output {
        Output {
            exit: Some(exit),
            stdout: stdout.into(),
        }
    }

    #[test]
    fn each_state_comes_from_the_binarys_own_reports() {
        let signed_out = output(1, r#"{"loggedIn":false,"authMethod":"none"}"#);
        assert_eq!(
            classify(Some(&signed_out), None, NOW).state,
            State::SignedOut
        );
        assert_eq!(
            classify(Some(&output(1, "")), None, NOW).state,
            State::SignedOut
        );
        let signed_in = output(
            0,
            r#"{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"firstParty","email":"someone@example.com","orgId":"o","orgName":"Org","subscriptionType":"max"}"#,
        );
        let status = classify(Some(&signed_in), None, NOW);
        assert_eq!(
            (status.state, status.method, status.plan),
            (State::SignedIn, Some(Method::ClaudeAi), Some(Plan::Max))
        );
        let key = output(0, r#"{"loggedIn":true,"authMethod":"api_key"}"#);
        let status = classify(Some(&key), None, NOW);
        assert_eq!(
            (status.state, status.method, status.plan),
            (State::ApiKey, Some(Method::ApiKey), None)
        );
        let cloud = output(0, r#"{"loggedIn":true,"authMethod":"third_party"}"#);
        assert_eq!(classify(Some(&cloud), None, NOW).state, State::ApiKey);
        // Expiry, when the release reports one.
        let soon = output(
            0,
            &format!(
                r#"{{"loggedIn":true,"authMethod":"claude.ai","subscriptionType":"pro","expiresAt":{}}}"#,
                (NOW + 86_400) * 1_000
            ),
        );
        let status = classify(Some(&soon), None, NOW);
        assert_eq!(status.state, State::Expiring);
        assert_eq!(status.expires_at, Some(NOW + 86_400));
        let past = output(
            0,
            r#"{"loggedIn":true,"authMethod":"claude.ai","expiresAt":"2026-09-01T00:00:00Z"}"#,
        );
        assert_eq!(classify(Some(&past), None, NOW).state, State::Expired);
        // A missing or failed binary.
        assert_eq!(classify(None, None, NOW).state, State::Unavailable);
        assert_eq!(
            classify(Some(&output(2, "Segmentation fault")), None, NOW).state,
            State::Unavailable
        );
    }

    #[test]
    fn notices_from_claude_code_runs_mark_limits_and_expired_logins() {
        let signed_in = output(0, r#"{"loggedIn":true,"authMethod":"claude.ai"}"#);
        let limited =
            Notice::from_text("You've hit your session limit · resets 11:50am (UTC)", NOW).unwrap();
        let status = classify(Some(&signed_in), Some(limited), NOW);
        assert_eq!(status.state, State::RateLimited);
        assert_eq!(status.resets_at, Some(1_790_164_200));
        // Once the reset passes, the limit is gone.
        assert_eq!(
            classify(Some(&signed_in), Some(limited), 1_790_164_201).state,
            State::SignedIn
        );
        let expired = Notice::from_text("OAuth token has expired. Please run /login", NOW).unwrap();
        assert_eq!(expired, Notice::LoginExpired { at: NOW });
        assert_eq!(
            classify(Some(&signed_in), Some(expired), NOW).state,
            State::Expired
        );
        let expiring = Notice::from_text("Your login expires in 2 days", NOW).unwrap();
        assert_eq!(
            classify(Some(&signed_in), Some(expiring), NOW).state,
            State::Expiring
        );
        assert_eq!(Notice::from_text("the turn failed", NOW), None);
        // Signed out wins over any notice.
        let out = output(1, r#"{"loggedIn":false,"authMethod":"none"}"#);
        assert_eq!(
            classify(Some(&out), Some(limited), NOW).state,
            State::SignedOut
        );
    }

    #[test]
    fn nothing_the_binary_prints_leaves_in_the_status() {
        let token = format!("sk-ant-oat01-{}", "w4".repeat(40));
        let noisy = output(
            0,
            &format!(
                r#"{{"loggedIn":true,"authMethod":"{token}","subscriptionType":"{token}","email":"someone@example.com","orgName":"{token}","configDirectory":"/home/u/.claude","expiresAt":"{token}"}}"#
            ),
        );
        let status = classify(Some(&noisy), None, NOW);
        let text = serde_json::to_string(&status).unwrap();
        assert!(!text.contains("sk-ant") && !text.contains('@') && !text.contains('/'));
        assert_eq!(status.method, Some(Method::Other));
        assert_eq!(status.plan, None);
        assert!(!status.summary().contains("sk-ant"));
    }

    #[test]
    fn notices_persist_as_typed_records_only() {
        let dir = tempfile::tempdir().unwrap();
        let notices = notice_dir(dir.path());
        assert_eq!(recall(&notices), None);
        let notice = Notice::Limited {
            resets_at: Some(NOW + 60),
            at: NOW,
        };
        remember(&notices, notice).unwrap();
        assert_eq!(recall(&notices), Some(notice));
        let kept = std::fs::read_to_string(notices.join("claude.json")).unwrap();
        assert!(!kept.contains("limit ·"), "{kept}");
        forget(&notices);
        assert_eq!(recall(&notices), None);
        // A file that is not a typed notice is ignored.
        std::fs::write(
            notices.join("claude.json"),
            r#"{"kind":"limited","at":1,"text":"x"}"#,
        )
        .unwrap();
        assert_eq!(recall(&notices), None);
    }

    #[cfg(unix)]
    #[test]
    fn the_status_command_runs_the_binary_with_nothing_added() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("claude");
        let seen = dir.path().join("seen");
        std::fs::write(
            &program,
            format!(
                "#!/bin/sh\nprintf '%s ' \"$@\" > '{}'\necho '{{\"loggedIn\":true,\"authMethod\":\"claude.ai\",\"subscriptionType\":\"pro\"}}'\necho 'login secret on stderr' >&2\nexit 0\n",
                seen.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let status = check(&program, None, NOW);
        assert_eq!(
            (status.state, status.plan),
            (State::SignedIn, Some(Plan::Pro))
        );
        assert_eq!(std::fs::read_to_string(&seen).unwrap(), "auth status ");
        // A missing binary is unavailable; a hung one is stopped.
        assert_eq!(
            check(&dir.path().join("missing"), None, NOW).state,
            State::Unavailable
        );
        std::fs::write(&program, "#!/bin/sh\nexec sleep 30\n").unwrap();
        let started = Instant::now();
        assert_eq!(run(&program, Duration::from_millis(200)), None);
        assert!(started.elapsed() < Duration::from_secs(10));
    }
}
