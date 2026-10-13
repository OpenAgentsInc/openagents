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

/// What a usage limit says, so the Cloud job pauses until the reset. It
/// names the run's Claude Code conversation (`session`), which is kept, so
/// Continue on your key can pick it up (#11295).
#[must_use]
pub fn limited(resets_at: Option<u64>, session: Option<&str>) -> String {
    let base = match resets_at {
        Some(at) => format!(
            "Claude Code reached a usage limit on this computer's Claude sign-in. Claude AI usage limit reached|{at}"
        ),
        None => "Claude Code reached a usage limit on this computer's Claude sign-in and did not say when it resets.".into(),
    };
    match session.filter(|id| session_id(id)) {
        Some(id) => format!("{base} {SESSION_MARK}{id}."),
        None => base,
    }
}

/// What precedes the conversation id in a usage-limit error.
const SESSION_MARK: &str = "Conversation ";

/// Whether `id` can be a Claude Code session id (a UUID).
#[must_use]
pub fn session_id(id: &str) -> bool {
    id.len() == 36 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

/// The kept conversation a usage-limit error names, if any.
#[must_use]
pub fn limited_session(error: &str) -> Option<String> {
    if !coder_engine_status::limit::says_limited(error) {
        return None;
    }
    let (_, rest) = error.rsplit_once(SESSION_MARK)?;
    let id: String = rest
        .chars()
        .take_while(|c| c.is_ascii_hexdigit() || *c == '-')
        .collect();
    session_id(&id).then_some(id)
}

/// A fresh Claude Code session id (a random UUID), so the run's
/// conversation is kept under a name Coder knows.
#[must_use]
pub fn new_session() -> String {
    let mut bytes = [0u8; 16];
    if getrandom::fill(&mut bytes).is_err() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        bytes = (nanos ^ u128::from(std::process::id()).rotate_left(64)).to_le_bytes();
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

/// The person's own Anthropic API key, from the environment Coder started
/// in: what Continue on your key runs one turn on (#11295). A Claude
/// subscription token (`sk-ant-oat…`) bills the same plan that hit its
/// limit, so it is never one.
#[must_use]
pub fn own_key(env: &dyn Fn(&str) -> Option<String>) -> Option<ApiKey> {
    env("ANTHROPIC_API_KEY")
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty() && !subscription_token(value))
        .map(ApiKey::new)
}

/// Whether `value` is a Claude subscription token.
fn subscription_token(value: &str) -> bool {
    value.trim().starts_with("sk-ant-oat")
}

/// The class of [`own_key`], for [`coder_cloud::claude_task::offered`].
#[must_use]
pub fn saved_class(
    env: &dyn Fn(&str) -> Option<String>,
) -> Option<coder_cloud::claude::OwnCredential> {
    own_key(env).map(|_| coder_cloud::claude::OwnCredential::AnthropicApiKey)
}

/// How the next print-mode run on this thread goes: continue a kept
/// conversation, and on the person's own key for that one turn (#11295).
#[derive(Default)]
pub(crate) struct Turn {
    /// The conversation to continue (`claude --resume`).
    pub resume: Option<String>,
    /// The person's own API key for this turn only; never a subscription
    /// token.
    pub key: Option<ApiKey>,
    /// A read-only task: only reading, searching and listing files
    /// (#11470).
    pub read_only: bool,
}

thread_local! {
    static NEXT: std::cell::RefCell<Turn> = std::cell::RefCell::new(Turn::default());
}

/// Sets how the next run on this thread goes; it applies once.
pub(crate) fn set_next_turn(turn: Turn) {
    NEXT.with(|next| *next.borrow_mut() = turn);
}

/// Takes how the next run on this thread goes.
pub(crate) fn take_next_turn() -> Turn {
    NEXT.with(|next| std::mem::take(&mut *next.borrow_mut()))
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
/// The conversation is kept under a session id Coder chooses (or the one
/// `turn` continues), so a run a usage limit stops can go on later.
pub(super) async fn run(
    program: &Path,
    task: &str,
    cwd: &Path,
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
    turn: Turn,
) -> Result<Value, String> {
    if !crate::approval::tools_allowed() {
        return Err("Claude Code cannot run when tools are disabled.".into());
    }
    if turn
        .key
        .as_ref()
        .is_some_and(|key| subscription_token(key.expose()))
    {
        return Err(
            "A subscription token bills the same Claude plan; use an API key to continue now."
                .into(),
        );
    }
    if turn.resume.as_deref().is_some_and(|id| !session_id(id)) {
        return Err("That Claude Code conversation can't be continued.".into());
    }
    let session = turn.resume.clone().unwrap_or_else(new_session);
    let on_key = turn.key.is_some();
    let mut command = std::process::Command::new(program);
    command
        .args(["-p", "--output-format", "stream-json", "--verbose"])
        .args(match &turn.resume {
            Some(id) => ["--resume", id.as_str()],
            None => ["--session-id", session.as_str()],
        })
        .current_dir(cwd);
    if crate::approval::gated() || turn.read_only {
        command.args([
            "--safe-mode",
            "--restricted",
            "--strict-mcp-config",
            "--tools",
            "Read,Glob,Grep",
            "--allowedTools",
            "Read,Glob,Grep",
            "--permission-mode",
            "dontAsk",
            "--permission-prompts",
            "none",
        ]);
    } else {
        command.args(["--permission-mode", "bypassPermissions"]);
    }
    // The session's hooks before every tool (#11528): the bridge as Claude
    // Code's own `PreToolUse` hook, asking this run's host; with no host,
    // the bridge holds the floors alone.
    let host = hook_host(cwd);
    if let Ok(bridge_program) = super::bridge_program() {
        let bridge = coder_delegate::delegate::HookBridge {
            program: bridge_program,
            args: vec![coder_sdk::bridge::COMMAND.to_owned()],
            socket: host
                .as_ref()
                .map(|host| host.path().to_path_buf())
                .unwrap_or_default(),
        };
        command.args(["--settings", &bridge.claude_settings()]);
        if let Some(host) = &host {
            command.env(coder_sdk::bridge::SOCKET_VAR, host.path());
        }
    }
    let (mark, value) = Agent::ClaudeCode.engine_mark();
    command.env(mark, value);
    let admitted = std::env::var("OA_CODER_CLOUD_CREDENTIAL_NAMES").unwrap_or_default();
    for name in removed_env(std::env::vars_os().map(|(name, _)| name), &admitted) {
        command.env_remove(name);
    }
    crate::bundled_runtime::apply_child_env(&mut command);
    // Their own key, for this turn only: the plan's login and any plan
    // token stay out of it.
    if let Some(key) = &turn.key {
        command
            .env_remove(secret_screen::CLAUDE_CODE_OAUTH_TOKEN)
            .env_remove("ANTHROPIC_AUTH_TOKEN")
            .env("ANTHROPIC_API_KEY", key.expose());
    }
    drop(turn);
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
            line(&record.text, &mut kept, emit, !on_key);
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
        line(&record.text, &mut kept, emit, !on_key);
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
    if !on_key {
        coder_delegate::delegate::remember_engine_notice(ID, &report);
    }
    answer(&report, coder_delegate::limit::now(), &session)
}

/// The host a print run's hook bridge asks: the session's registry
/// ([`crate::sdk_hooks::claude_print`]) on a socket, kept for the run.
fn hook_host(cwd: &Path) -> Option<coder_sdk::bridge::Host> {
    let runner = coder_sdk::Runner::new(
        crate::sdk_hooks::claude_print(cwd),
        coder_sdk::Budget::default(),
    );
    coder_sdk::bridge::Host::start(Arc::new(std::sync::Mutex::new(runner))).ok()
}

/// One line of the stream. `plan` is false on a turn that runs on the
/// person's own key: what it reports is about that key, not the plan.
fn line(text: &str, kept: &mut String, emit: &mut dyn FnMut(RuntimeEvent), plan: bool) {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return;
    };
    match value["type"].as_str() {
        Some("system" | "result" | "rate_limit_event") => {
            match value["type"].as_str() {
                _ if !plan => {}
                Some("rate_limit_event") => {
                    let now = coder_delegate::limit::now();
                    let info = &value["rate_limit_info"];
                    crate::claude_plan::observe_rate_limit(info, now);
                    // The run's cards show where the plan stands (#11292).
                    if let Some(usage) = crate::claude_plan::plan_usage() {
                        emit(RuntimeEvent::Plan(usage));
                    }
                    // Claude Code's own report of where the plan stands is
                    // the usage reading (#11233); nothing reads the login.
                    if let Some(window) = microcoder_loop::usage::rate_limit_window(info, now) {
                        let _ = microcoder_loop::usage::record_reported(
                            &coder::task::local::default_store(),
                            microcoder_loop::capacity::Provider::Claude,
                            window,
                            now,
                        );
                    }
                }
                Some("system") if value["subtype"] == "init" => {
                    crate::claude_plan::observe_credential(credential_type(
                        value["apiKeySource"].as_str(),
                    ));
                }
                _ => {}
            }
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
pub(super) fn answer(report: &Report, now: u64, session: &str) -> Result<Value, String> {
    let summary = &report.summary;
    let engine = json!({
        "engine": ID,
        "version": summary.version.as_deref().filter(|v| v.len() <= 32 && v.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')),
        "credential": credential_type(summary.api_key_source.as_deref()),
    });
    if let Some(limit) = report.limit(ID) {
        let kept = summary
            .session_id
            .as_deref()
            .filter(|id| session_id(id))
            .unwrap_or(session);
        return Err(limited(limit.resets_at, Some(kept)));
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
        "session": summary.session_id.as_deref().filter(|id| session_id(id)).unwrap_or(session),
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

    const SESSION: &str = "0f8a2c4e-1b3d-4e5f-8a6b-7c8d9e0f1a2b";

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
        let answer = answer(&report(&stream, 0, ""), 1, SESSION).unwrap();
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
        let error = answer(&report(&stream, 1, ""), 1_790_163_058, SESSION).unwrap_err();
        // The init event named session s1, which is no UUID: the run's own
        // id is the one kept.
        assert_eq!(error, limited(Some(1_790_164_200), Some(SESSION)));
        assert_eq!(limited_session(&error).as_deref(), Some(SESSION));
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
        assert_eq!(
            answer(&report(expired, 1, ""), 1, SESSION).unwrap_err(),
            SIGN_IN
        );
        let missing = r#"{"type":"result","subtype":"success","is_error":true,"result":"Invalid API key · Please run /login"}"#;
        assert_eq!(
            answer(&report(missing, 1, ""), 1, SESSION).unwrap_err(),
            SIGN_IN
        );
        assert_eq!(
            coder_engine_status::Notice::from_text(SIGN_IN, 5),
            Some(coder_engine_status::Notice::LoginExpired { at: 5 })
        );
    }

    /// #11528: a background Claude agent, started through `acp_briefed`
    /// on a stand-in Claude Code, carries the hook bridge as its own
    /// `PreToolUse` hook, and the run's host holds a merge for the person,
    /// keeps a write in the task's folder, and squeezes a noisy test run.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_background_claude_task_runs_the_floors_through_the_hook_bridge() {
        let _lock = crate::approval::test_lock().lock().unwrap();
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let recorded = dir.path().join("recorded");
        std::fs::create_dir_all(&recorded).unwrap();
        let program = dir.path().join("claude");
        std::fs::write(
            &program,
            format!(
                r#"#!/bin/sh
while [ $# -gt 0 ]; do
  if [ "$1" = --settings ]; then printf '%s' "$2" > '{dir}/settings.json'; fi
  shift
done
printf '%s' "$CODER_HOOK_SOCKET" > '{dir}/socket'
cat > /dev/null
n=0
while [ ! -f '{dir}/go' ] && [ $n -lt 400 ]; do sleep 0.05; n=$((n+1)); done
echo '{INIT}'
echo '{{"type":"result","subtype":"success","is_error":false,"result":"done"}}'
"#,
                dir = recorded.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let agent = AcpAgent {
            id: "claude-code".into(),
            name: "Claude Code".into(),
            program,
            transport: AgentTransport::ClaudeCli,
            arguments: vec![],
            mode: None,
            enabled: true,
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let work = dir.path().join("work");
        std::fs::create_dir_all(&work).unwrap();
        let mut quiet = |_: RuntimeEvent| {};
        let run = super::super::acp_briefed(
            &agent,
            "merge the pull request",
            &work,
            None,
            false,
            None,
            &cancel,
            &mut quiet,
        );
        let asked = async {
            let socket = recorded.join("socket");
            for _ in 0..400 {
                if std::fs::read_to_string(&socket).is_ok_and(|text| !text.is_empty()) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            let socket = PathBuf::from(std::fs::read_to_string(&socket).unwrap());
            let ask = |input: Value| {
                coder_sdk::bridge::ask(
                    &socket,
                    coder_sdk::bridge::Format::Claude,
                    &input,
                    Duration::from_secs(10),
                )
                .unwrap()
            };
            let bash = |command: &str| {
                json!({"hook_event_name": "PreToolUse", "tool_name": "Bash",
                    "tool_input": {"command": command}, "cwd": work})
            };
            let merge = ask(bash("gh pr merge 5 --squash")).unwrap();
            let outside = ask(
                json!({"hook_event_name": "PreToolUse", "tool_name": "Write",
                "tool_input": {"file_path": "/etc/elsewhere", "content": "x"}, "cwd": work}),
            )
            .unwrap();
            let squeezed = ask(bash("cargo test -p coder-new"));
            std::fs::write(recorded.join("go"), "").unwrap();
            (merge, outside, squeezed)
        };
        let (answer, (merge, outside, squeezed)) = tokio::join!(run, asked);
        assert_eq!(answer.unwrap()["reply"], "done");
        let settings: Value =
            serde_json::from_str(&std::fs::read_to_string(recorded.join("settings.json")).unwrap())
                .unwrap();
        let command = settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        assert!(
            command.ends_with(" coder-hook --format claude"),
            "{command}"
        );
        assert_eq!(merge["hookSpecificOutput"]["permissionDecision"], "deny");
        assert!(
            merge["hookSpecificOutput"]["permissionDecisionReason"]
                .as_str()
                .unwrap()
                .starts_with("Held for the person"),
            "{merge}"
        );
        assert_eq!(
            outside["hookSpecificOutput"]["permissionDecisionReason"],
            coder_sdk::worktree::OUTSIDE
        );
        // Token savers is on in a test: the noisy run is squeezed.
        let squeezed = squeezed.expect("the squeezer's rewrite");
        assert!(
            squeezed["hookSpecificOutput"]["updatedInput"]["command"]
                .as_str()
                .unwrap()
                .contains(squeeze::CODER_SUBCOMMAND),
            "{squeezed}"
        );
    }

    /// The fixture stand-in for the binary: never a real account.
    #[cfg(unix)]
    #[tokio::test]
    async fn the_unmodified_binary_runs_in_print_mode_with_the_task_on_stdin() {
        let _lock = crate::approval::test_lock().lock().unwrap();
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
        let agent = AcpAgent {
            id: "claude-code".into(),
            name: "Claude Code".into(),
            program,
            transport: AgentTransport::ClaudeCli,
            arguments: vec![],
            mode: None,
            enabled: true,
        };
        let answer = super::acp(
            &agent,
            "fix the fixture",
            dir.path(),
            None,
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
        // The conversation is kept under an id Coder chose (#11295).
        assert!(!args.contains("--no-session-persistence"), "{args}");
        let kept = args
            .split_once("--session-id ")
            .map(|(_, rest)| rest.split(' ').next().unwrap_or_default().to_owned())
            .unwrap_or_default();
        assert!(session_id(&kept), "{args}");
        assert_eq!(answer["session"], kept);
        crate::approval::install(Some(crate::approval::Gate {
            desk: crate::approval::Desk::new(),
            cancel: Arc::new(AtomicBool::new(false)),
        }));
        let readonly = run(
            &agent.program,
            "read the fixture",
            dir.path(),
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
            Turn::default(),
        )
        .await;
        crate::approval::install(None);
        assert_eq!(readonly.unwrap()["reply"], "read the fixture");
        let args = std::fs::read_to_string(dir.path().join("args")).unwrap();
        assert!(args.contains("--tools Read,Glob,Grep"));
        assert!(args.contains("--permission-mode dontAsk"));
        assert!(!args.contains("bypassPermissions"));
        // A read-only task is held to reading, ungated too (#11470), and
        // names the folder it ran in (#11469).
        let held = super::acp_with(
            &agent,
            "check the fixture",
            dir.path(),
            None,
            true,
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .await
        .unwrap();
        let args = std::fs::read_to_string(dir.path().join("args")).unwrap();
        assert!(args.contains("--tools Read,Glob,Grep"), "{args}");
        assert!(!args.contains("bypassPermissions"), "{args}");
        assert_eq!(held["permissions"]["mode"], "read-only");
        assert_eq!(held["permissions"]["enforced"], true);
        assert_eq!(held["cwd"], dir.path().display().to_string());
    }

    /// #11295: Continue on your key resumes the kept conversation on the
    /// person's own API key for that one turn, with no plan token beside it,
    /// and a subscription token is never one.
    #[cfg(unix)]
    #[tokio::test]
    async fn your_key_resumes_the_kept_conversation_on_that_key_only() {
        let _lock = crate::approval::test_lock().lock().unwrap();
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("claude");
        let seen = dir.path().join("seen");
        std::fs::write(
            &program,
            format!(
                "#!/bin/sh\nprintf '%s |key=%s|oauth=%s' \"$*\" \"$ANTHROPIC_API_KEY\" \"$CLAUDE_CODE_OAUTH_TOKEN\" > {seen}\nread task\necho '{INIT}'\nprintf '{{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"ok\"}}\\n'\n",
                seen = seen.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let turn = Turn {
            resume: Some(SESSION.into()),
            key: Some(ApiKey::new("sk-ant-api03-fixture")),
            read_only: false,
        };
        let answer = run(
            &program,
            "go on",
            dir.path(),
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
            turn,
        )
        .await
        .unwrap();
        assert_eq!(answer["session"], SESSION);
        let seen = std::fs::read_to_string(&seen).unwrap();
        assert!(seen.contains(&format!("--resume {SESSION}")), "{seen}");
        assert!(!seen.contains("--session-id"), "{seen}");
        assert!(seen.contains("|key=sk-ant-api03-fixture|oauth="), "{seen}");
        assert!(
            seen.ends_with("oauth="),
            "no plan token beside the key: {seen}"
        );
        // A subscription token is never your key.
        let token = Turn {
            resume: Some(SESSION.into()),
            key: Some(ApiKey::new("sk-ant-oat01-fixture")),
            read_only: false,
        };
        let refused = run(
            &program,
            "go on",
            dir.path(),
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
            token,
        )
        .await;
        assert!(refused.unwrap_err().contains("subscription token"));
        let env = |value: &'static str| {
            move |name: &str| (name == "ANTHROPIC_API_KEY").then(|| value.to_owned())
        };
        assert!(own_key(&env("sk-ant-oat01-x")).is_none());
        assert!(own_key(&env("  ")).is_none());
        assert_eq!(
            own_key(&env("sk-ant-api03-x")).unwrap().expose(),
            "sk-ant-api03-x"
        );
        assert!(saved_class(&env("sk-ant-oat01-x")).is_none());
    }

    #[test]
    fn a_new_session_is_a_uuid_and_a_limit_names_it() {
        let id = new_session();
        assert!(session_id(&id), "{id}");
        assert_ne!(id, new_session());
        let error = limited(None, Some(&id));
        assert_eq!(limited_session(&error), Some(id));
        assert_eq!(limited_session(&limited(Some(5), None)), None);
        assert_eq!(
            limited_session("Conversation 0f8a2c4e-1b3d-4e5f-8a6b-7c8d9e0f1a2b."),
            None
        );
    }
}
