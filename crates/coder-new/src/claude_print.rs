//! Claude Code in print mode, on the login the computer holds (BYO-03).
//!
//! A Cloud computer's background task names the engine `claude`
//! ([`coder_cloud::claude::ENGINE`]). This bridge runs the pinned,
//! unmodified binary as `claude -p` with the task on standard input, inside
//! the user's own computer, on whatever sign-in that computer holds: the
//! login made there through Anthropic's own flow, or the user's own API key
//! or subscription token when their run admitted one
//! (`docs/cloud/claude-code-byo.md`). It passes `CLAUDE_CODE_OAUTH_TOKEN`
//! only when the run admitted it, and never reads a login file.
//!
//! A run that ends on a usage limit, or on a login that is missing or
//! expired, fails with a sentence the Cloud job reads back with
//! [`coder_engine_status::Notice::from_text`]: the limit carries the reset
//! Claude Code reported as `Claude AI usage limit reached|SECONDS`, and the
//! sign-in failure asks the person to sign in inside the computer. The
//! computer's own sign-in status keeps the same typed notice (BYO-02).
//! The answer carries the engine, its reported version, and the credential
//! type, never the credential.

use super::*;
use coder_delegate::delegate::{Agent, Report, Status, Summary};

/// The id a Cloud computer's background task uses for this engine.
pub const ID: &str = coder_cloud::claude::ENGINE;

/// What a usage limit says, so the Cloud job pauses until the reset.
#[must_use]
pub fn limited(resets_at: Option<u64>) -> String {
    match resets_at {
        Some(at) => format!(
            "Claude Code reached a usage limit on this computer's Claude sign-in. Claude AI usage limit reached|{at}"
        ),
        None => "Claude Code reached a usage limit on this computer's Claude sign-in and did not say when it resets.".into(),
    }
}

/// What a missing or expired login says, so the Cloud job stops with a
/// sign-in prompt. Sign-in happens only inside the computer.
pub const SIGN_IN: &str = "Claude Code is not signed in on this computer, or its login expired. Please run /login: use Sign in to Claude for this computer, then continue the task.";

/// The built-in print-mode agent, when the computer has the engine: the
/// pinned install first, else `claude` on `PATH`.
#[must_use]
pub fn agent() -> Option<AcpAgent> {
    let pinned = PathBuf::from(coder_engine_status::claude::PROGRAM);
    let program = acp_client::process::first_executable([pinned]).or_else(|| {
        acp_client::process::first_executable(acp_client::process::on_path(
            "claude",
            std::env::var_os("PATH").as_deref(),
        ))
    })?;
    Some(AcpAgent {
        id: ID.into(),
        name: "Claude Code".into(),
        program,
        transport: AgentTransport::ClaudeCli,
        arguments: vec![],
        mode: None,
        enabled: true,
    })
}

/// The credential type a run's `init` event names. Closed words only: the
/// source names where Claude Code took its credential from, and nothing
/// it printed is passed on.
#[must_use]
pub fn credential_type(source: Option<&str>) -> &'static str {
    match source.map(str::trim) {
        None => "unknown",
        Some(s) if s.is_empty() || s == "none" || s.contains("/login") => "claude_ai_login",
        Some("ANTHROPIC_API_KEY") => "anthropic_api_key",
        Some("ANTHROPIC_AUTH_TOKEN") => "anthropic_auth_token",
        Some("CLAUDE_CODE_OAUTH_TOKEN") => "claude_subscription_token",
        Some("apiKeyHelper") => "api_key_helper",
        Some(_) => "other",
    }
}

/// The variables Claude Code must not see. Only credentials the run
/// admitted pass (`OA_CODER_CLOUD_CREDENTIAL_NAMES`: the user's own API
/// key, subscription token, or cloud credential); every other key, token,
/// or secret is removed. A subscription token passes only when admitted, so
/// without one a plan runs on the login inside this computer.
fn removed_env(
    names: impl Iterator<Item = std::ffi::OsString>,
    admitted: &str,
) -> Vec<std::ffi::OsString> {
    let allowed = |name: &str| admitted.split(',').any(|allowed| allowed == name);
    let mut out: Vec<_> = names
        .filter(|name| {
            let text = name.to_string_lossy();
            let secret =
                text.ends_with("_API_KEY") || text.ends_with("_TOKEN") || text.ends_with("_SECRET");
            secret && !allowed(&text)
        })
        .collect();
    if !allowed(secret_screen::CLAUDE_CODE_OAUTH_TOKEN) {
        out.push(secret_screen::CLAUDE_CODE_OAUTH_TOKEN.into());
    }
    out
}

/// Drive `claude -p` once and answer, or fail with a typed sentence.
pub(super) async fn run(
    program: &Path,
    task: &str,
    cwd: &Path,
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    let mut command = std::process::Command::new(program);
    command
        .args([
            "-p",
            "--output-format",
            "stream-json",
            "--verbose",
            "--permission-mode",
            "bypassPermissions",
        ])
        .current_dir(cwd);
    let (mark, value) = Agent::ClaudeCode.engine_mark();
    command.env(mark, value);
    let admitted = std::env::var("OA_CODER_CLOUD_CREDENTIAL_NAMES").unwrap_or_default();
    for name in removed_env(std::env::vars_os().map(|(name, _)| name), &admitted) {
        command.env_remove(name);
    }
    crate::bundled_runtime::apply_child_env(&mut command);
    let started = Instant::now();
    let mut live = supervise::Job::from_command(command)
        .bounded(supervise::Limits::until_stopped().keeping(TEXT_MAX))
        .start(supervise::Input::Piped)?;
    if let Err(error) = live.send(task.as_bytes()).await {
        let stopped = live.stop().await;
        return Err(format!(
            "Claude Code could not read the task: {error}.{}",
            crate::bundled_runtime::still_running(stopped.group_clear)
        ));
    }
    live.close_input();
    let mut reader = coder_delegate::tail::Reader::new(TEXT_MAX);
    // Only the events the summary reads are kept: init, rate-limit, and
    // the result. Assistant text streams out as it comes.
    let mut kept = String::new();
    let mut keep = |delivery: supervise::Delivery,
                    reader: &mut coder_delegate::tail::Reader,
                    emit: &mut dyn FnMut(RuntimeEvent)| {
        for gap in delivery.gaps {
            reader.dropped(gap.offset, gap.bytes);
        }
        for record in reader.feed(delivery.offset, &delivery.bytes) {
            line(&record.text, &mut kept, emit);
        }
    };
    let stopped = loop {
        keep(live.take(), &mut reader, emit);
        if live.finished() {
            break live.wait().await;
        }
        if cancel.load(Ordering::Relaxed) {
            break live.stop().await;
        }
        tokio::time::sleep(POLL).await;
    };
    keep(stopped.rest.clone(), &mut reader, emit);
    if let Some(record) = reader.finish() {
        line(&record.text, &mut kept, emit);
    }
    if stopped.requested || cancel.load(Ordering::Relaxed) {
        return Err(format!(
            "The Claude Code task was canceled.{}",
            crate::bundled_runtime::still_running(stopped.group_clear)
        ));
    }
    let summary = Summary::parse(&kept);
    let stderr = stopped.stderr.marked();
    let report = Report {
        status: coder_delegate::delegate::classify(&stopped.ending, &summary, &stderr),
        summary,
        milliseconds: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        stderr,
        stream: None,
    };
    // The computer's sign-in status remembers what Claude Code reported.
    coder_delegate::delegate::remember_engine_notice(ID, &report);
    answer(&report, coder_delegate::limit::now())
}

fn line(text: &str, kept: &mut String, emit: &mut dyn FnMut(RuntimeEvent)) {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return;
    };
    match value["type"].as_str() {
        Some("system" | "result" | "rate_limit_event") => {
            kept.push_str(text);
            kept.push('\n');
        }
        Some("assistant") => {
            for part in value["message"]["content"].as_array().into_iter().flatten() {
                if part["type"] == "text" {
                    if let Some(text) = part["text"].as_str() {
                        emit(RuntimeEvent::Text(text.to_owned()));
                    }
                }
            }
        }
        _ => {}
    }
}

/// The answer a finished run gives, or the sentence it fails with.
pub(super) fn answer(report: &Report, now: u64) -> Result<Value, String> {
    let summary = &report.summary;
    let engine = json!({
        "engine": ID,
        "version": summary.version.as_deref().filter(|v| v.len() <= 32 && v.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')),
        "credential": credential_type(summary.api_key_source.as_deref()),
    });
    if let Some(limit) = report.limit(ID) {
        return Err(limited(limit.resets_at));
    }
    let notice = coder_delegate::delegate::engine_notice(ID, report, now);
    if matches!(
        notice,
        Some(coder_engine_status::Notice::LoginExpired { .. })
    ) || report.status == Status::Refused("not_logged_in".into())
    {
        return Err(SIGN_IN.into());
    }
    if report.status != Status::Answered {
        return Err(format!(
            "Claude Code task failed ({}).",
            report.status.word()
        ));
    }
    let tokens = ["input_tokens", "output_tokens"]
        .iter()
        .filter_map(|k| summary.usage.as_ref().and_then(|u| u[k].as_u64()))
        .sum::<u64>();
    Ok(json!({
        "session": summary.session_id,
        "reply": summary.result.clone().unwrap_or_default(),
        "model": summary.model,
        "stop_reason": "end_turn",
        "usage": summary.usage,
        "tokens": tokens,
        "cost_usd": summary.total_cost_usd,
        "transport": "claude-cli",
        "engine": engine,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const INIT: &str = r#"{"type":"system","subtype":"init","model":"claude-opus-5-5","claude_code_version":"2.1.295","apiKeySource":"none","session_id":"s1"}"#;

    fn report(stream: &str, code: i32, stderr: &str) -> Report {
        let summary = Summary::parse(stream);
        let ending = supervise::Ending::Exited(Some(code));
        Report {
            status: coder_delegate::delegate::classify(&ending, &summary, stderr),
            summary,
            milliseconds: 1,
            stderr: stderr.into(),
            stream: None,
        }
    }

    #[test]
    fn an_answer_carries_engine_version_and_credential_type_only() {
        let stream = format!(
            "{INIT}\n{}",
            r#"{"type":"result","subtype":"success","is_error":false,"result":"done","session_id":"s1","usage":{"input_tokens":3,"output_tokens":4}}"#
        );
        let answer = answer(&report(&stream, 0, ""), 1).unwrap();
        assert_eq!(answer["reply"], "done");
        assert_eq!(answer["tokens"], 7);
        assert_eq!(
            answer["engine"],
            json!({"engine":"claude","version":"2.1.295","credential":"claude_ai_login"})
        );
        assert_eq!(
            credential_type(Some("ANTHROPIC_API_KEY")),
            "anthropic_api_key"
        );
        assert_eq!(credential_type(Some("sk-ant-api03-xyz")), "other");
        assert_eq!(
            credential_type(Some("CLAUDE_CODE_OAUTH_TOKEN")),
            "claude_subscription_token"
        );
    }

    #[test]
    fn only_the_admitted_credential_reaches_claude_code() {
        let names = || {
            [
                "CLAUDE_CODE_OAUTH_TOKEN",
                "ANTHROPIC_API_KEY",
                "GH_TOKEN",
                "PATH",
            ]
            .map(std::ffi::OsString::from)
            .into_iter()
        };
        let removed = |admitted: &str| -> Vec<String> {
            let mut out: Vec<String> = removed_env(names(), admitted)
                .into_iter()
                .map(|n| n.to_string_lossy().into_owned())
                .collect();
            out.sort();
            out.dedup();
            out
        };
        // No admitted credential: the computer's own login, nothing else.
        assert_eq!(
            removed(""),
            ["ANTHROPIC_API_KEY", "CLAUDE_CODE_OAUTH_TOKEN", "GH_TOKEN"]
        );
        // The user's own subscription token: it alone, never an API key.
        assert_eq!(
            removed("CLAUDE_CODE_OAUTH_TOKEN"),
            ["ANTHROPIC_API_KEY", "GH_TOKEN"]
        );
        // The user's own API key: unchanged, and never a subscription token.
        assert_eq!(
            removed("ANTHROPIC_API_KEY"),
            ["CLAUDE_CODE_OAUTH_TOKEN", "GH_TOKEN"]
        );
    }

    #[test]
    fn a_usage_limit_fails_with_the_reset_claude_code_reported() {
        let stream = format!(
            "{INIT}\n{}",
            r#"{"type":"result","subtype":"success","is_error":true,"api_error_status":429,"result":"Claude AI usage limit reached|1790164200"}"#
        );
        let error = answer(&report(&stream, 1, ""), 1_790_163_058).unwrap_err();
        assert_eq!(error, limited(Some(1_790_164_200)));
        assert_eq!(
            coder_engine_status::Notice::from_text(&error, 5),
            Some(coder_engine_status::Notice::Limited {
                resets_at: Some(1_790_164_200),
                at: 5
            })
        );
    }

    #[test]
    fn a_missing_or_expired_login_fails_with_a_sign_in_prompt() {
        let expired = r#"{"type":"result","subtype":"error_during_execution","is_error":true,"result":"OAuth token has expired. Please run /login"}"#;
        assert_eq!(answer(&report(expired, 1, ""), 1).unwrap_err(), SIGN_IN);
        let missing = r#"{"type":"result","subtype":"success","is_error":true,"result":"Invalid API key · Please run /login"}"#;
        assert_eq!(answer(&report(missing, 1, ""), 1).unwrap_err(), SIGN_IN);
        assert_eq!(
            coder_engine_status::Notice::from_text(SIGN_IN, 5),
            Some(coder_engine_status::Notice::LoginExpired { at: 5 })
        );
    }

    /// The fixture stand-in for the binary: never a real account.
    #[cfg(unix)]
    #[tokio::test]
    async fn the_unmodified_binary_runs_in_print_mode_with_the_task_on_stdin() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("claude");
        let args = dir.path().join("args");
        std::fs::write(
            &program,
            format!(
                "#!/bin/sh\nprintf '%s ' \"$@\" > {args}\nread task\necho '{INIT}'\necho '{{\"type\":\"assistant\",\"message\":{{\"content\":[{{\"type\":\"text\",\"text\":\"working\"}}]}}}}'\nprintf '{{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"%s\"}}\\n' \"$task\"\n",
                args = args.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut texts = vec![];
        let answer = run(
            &program,
            "fix the fixture",
            dir.path(),
            &Arc::new(AtomicBool::new(false)),
            &mut |event| {
                if let RuntimeEvent::Text(text) = event {
                    texts.push(text);
                }
            },
        )
        .await
        .unwrap();
        assert_eq!(answer["reply"], "fix the fixture");
        assert_eq!(answer["engine"]["credential"], "claude_ai_login");
        assert_eq!(texts, ["working"]);
        let args = std::fs::read_to_string(args).unwrap();
        assert!(args.starts_with("-p --output-format stream-json"), "{args}");
    }
}
