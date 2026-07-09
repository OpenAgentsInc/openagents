use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Serializes tests that mutate the process-global GitHub-token env vars so they
/// do not race when the integration suite runs multithreaded.
static GIT_TOKEN_ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn codex_run_executes_captures_artifacts_scrubs_auth_and_redacts_events() -> TestResult {
    let state_dir = unique_state_dir("codex-run");
    fs::create_dir_all(&state_dir)?;
    let grant_file = state_dir.join("grant.json");
    let auth_file = state_dir.join("broker-auth-cache.json");
    let assignment_file = state_dir.join("assignment.json");
    let fake_codex = state_dir.join("fake-codex.sh");
    let now = now_ms();
    let auth_cache = "{\"kind\":\"test-codex-auth-cache\",\"value\":\"canary-auth-cache\"}\n";

    fs::write(
        &grant_file,
        format!(
            r#"{{
  "contract_version": "openagents.codex_auth_grant.v1",
  "workroom_id": "workroom.codex.run",
  "user_ref": "user.test",
  "organization_ref": "org.test",
  "project_ref": "project.test",
  "provider_account_ref": "provider-account_codex_run",
  "grant_ref": "codex-auth-grant_run",
  "provider_secret_ref": "secret://codex/account/run",
  "requested_mode": "exec",
  "issued_at_ms": {now},
  "expires_at_ms": {},
  "audit_context": "vortex.issue.85"
}}"#,
            now + 1000 * 60 * 30,
        ),
    )?;
    fs::write(&auth_file, auth_cache)?;
    fs::write(
        &assignment_file,
        format!(
            r#"{{
  "contract_version": "openagents.codex_workroom_assignment.v1",
  "assignment_id": "assignment.codex.run",
  "workroom_id": "workroom.codex.run",
  "target_node_id": "oa-gcp-shc-katy-01",
  "user_ref": "user.test",
  "organization_ref": "org.test",
  "project_ref": "project.test",
  "provider_account_ref": "provider-account_codex_run",
  "auth_grant_ref": "codex-auth-grant_run",
  "repo_ref": "OpenAgentsInc/cloud",
  "prompt": "Create the required summary artifact.",
  "required_artifacts": ["summary"],
  "sandbox": "workspace_write",
  "timeout_ms": 10000,
  "wallet_authority": false,
  "created_at_ms": {now},
  "audit_context": "vortex.issue.85"
}}"#
        ),
    )?;
    write_fake_codex(&fake_codex)?;

    let materialized = run_workroomd_json(&[
        "codex",
        "auth",
        "materialize",
        "--grant-file",
        state_path(&grant_file)?,
        "--auth-json-file",
        state_path(&auth_file)?,
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    let auth_json_path = materialized
        .pointer("/state/auth_json_path")
        .and_then(Value::as_str)
        .ok_or("missing auth_json_path")?
        .to_string();

    let run = run_workroomd_json(&[
        "codex",
        "run",
        "--assignment-file",
        state_path(&assignment_file)?,
        "--codex-bin",
        state_path(&fake_codex)?,
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        run.pointer("/state/status").and_then(Value::as_str),
        Some("completed")
    );
    assert_eq!(
        run.pointer("/state/artifact_refs")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert!(run
        .pointer("/state/artifact_refs/0/content_digest")
        .and_then(Value::as_str)
        .is_some_and(|digest| digest.starts_with("sha256:")));
    assert!(run
        .pointer("/state/events")
        .and_then(Value::as_array)
        .is_some_and(|events| {
            events.iter().any(|event| {
                event.pointer("/event_kind").and_then(Value::as_str) == Some("redacted")
            }) && events.iter().any(|event| {
                event.pointer("/event_kind").and_then(Value::as_str) == Some("completed")
            })
        }));
    assert!(run
        .pointer("/auth_receipts")
        .and_then(Value::as_array)
        .is_some_and(|receipts| receipts.len() >= 2));
    assert!(run
        .pointer("/runner_events")
        .and_then(Value::as_array)
        .is_some_and(|events| {
            has_runner_event(events, "redacted")
                && has_runner_event(events, "artifact.created")
                && has_runner_event(events, "receipt.created")
                && has_runner_event(events, "run.heartbeat")
                && has_runner_event(events, "turn.completed")
                && has_runner_event(events, "resource.usage.captured")
                && !has_runner_event(events, "usage.unavailable")
                && has_runner_event(events, "run.completed")
        }));
    let runner_events = run
        .pointer("/runner_events")
        .and_then(Value::as_array)
        .ok_or("missing runner events")?;
    let usage_event =
        runner_event_by_type(runner_events, "turn.completed").ok_or("missing turn usage event")?;
    assert!(usage_event
        .pointer("/raw_payload_json")
        .and_then(Value::as_str)
        .is_some_and(|payload| payload.contains("\"usage\"")));

    assert!(!Path::new(&auth_json_path).exists());
    assert!(!state_dir
        .join("codex-workspaces/assignment.codex.run")
        .exists());
    assert!(state_dir.join("closeout-manifest.json").exists());
    let resource_receipts = fs::read_to_string(state_dir.join("resource-usage-receipts.jsonl"))?;
    let resource_receipt: Value = serde_json::from_str(
        resource_receipts
            .lines()
            .find(|line| !line.trim().is_empty())
            .ok_or("missing resource receipt line")?,
    )?;
    assert_eq!(
        resource_receipt
            .pointer("/schema_version")
            .and_then(Value::as_str),
        Some("openagents.resource_usage_receipt.v1")
    );
    assert!(resource_receipt
        .pointer("/host/logical_cpu_count")
        .and_then(Value::as_u64)
        .is_some_and(|count| count > 0));
    assert_eq!(
        resource_receipt
            .pointer("/model_usage/0/count_source")
            .and_then(Value::as_str),
        Some("codex_reported")
    );
    assert_eq!(
        resource_receipt
            .pointer("/model_usage/0/input_tokens")
            .and_then(Value::as_u64),
        Some(42)
    );
    assert_eq!(
        resource_receipt
            .pointer("/model_usage/0/cached_input_tokens")
            .and_then(Value::as_u64),
        Some(12)
    );
    assert_eq!(
        resource_receipt
            .pointer("/model_usage/0/output_tokens")
            .and_then(Value::as_u64),
        Some(5)
    );
    assert_eq!(
        resource_receipt
            .pointer("/model_usage/0/reasoning_tokens")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert!(resource_receipt
        .pointer("/model_usage/0/unavailable_reason")
        .is_some_and(Value::is_null));

    let event_log = fs::read_to_string(state_dir.join("codex-run-events.jsonl"))?;
    assert!(event_log.contains("artifact"));
    assert!(event_log.contains("cleanup"));
    assert!(!event_log.contains("canary-auth-cache"));
    assert!(!event_log.to_ascii_lowercase().contains("access_token"));
    assert!(!event_log.to_ascii_lowercase().contains("wallet_seed"));

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn codex_run_closes_when_required_artifacts_are_stable_before_process_exit() -> TestResult {
    let state_dir = unique_state_dir("codex-run-artifact-complete");
    fs::create_dir_all(&state_dir)?;
    let grant_file = state_dir.join("grant.json");
    let auth_file = state_dir.join("broker-auth-cache.json");
    let assignment_file = state_dir.join("assignment.json");
    let fake_codex = state_dir.join("fake-hanging-codex.sh");
    let now = now_ms();

    fs::write(
        &grant_file,
        format!(
            r#"{{
  "contract_version": "openagents.codex_auth_grant.v1",
  "workroom_id": "workroom.codex.artifact_complete",
  "user_ref": "user.test",
  "organization_ref": "org.test",
  "project_ref": "project.test",
  "provider_account_ref": "provider-account_codex_artifact_complete",
  "grant_ref": "codex-auth-grant_artifact_complete",
  "provider_secret_ref": "secret://codex/account/artifact-complete",
  "requested_mode": "exec",
  "issued_at_ms": {now},
  "expires_at_ms": {},
  "audit_context": "vortex.issue.artifact-complete"
}}"#,
            now + 1000 * 60 * 30,
        ),
    )?;
    fs::write(
        &auth_file,
        "{\"kind\":\"test-codex-auth-cache\",\"value\":\"artifact-complete\"}\n",
    )?;
    fs::write(
        &assignment_file,
        format!(
            r#"{{
  "contract_version": "openagents.codex_workroom_assignment.v1",
  "assignment_id": "assignment.codex.artifact_complete",
  "workroom_id": "workroom.codex.artifact_complete",
  "target_node_id": "oa-gcp-shc-katy-01",
  "user_ref": "user.test",
  "organization_ref": "org.test",
  "project_ref": "project.test",
  "provider_account_ref": "provider-account_codex_artifact_complete",
  "auth_grant_ref": "codex-auth-grant_artifact_complete",
  "repo_ref": "OpenAgentsInc/cloud",
  "prompt": "Create the required summary artifact and keep running.",
  "required_artifacts": ["summary"],
  "sandbox": "workspace_write",
  "timeout_ms": 10000,
  "wallet_authority": false,
  "created_at_ms": {now},
  "audit_context": "vortex.issue.artifact-complete"
}}"#
        ),
    )?;
    write_fake_hanging_codex(&fake_codex)?;

    run_workroomd_json(&[
        "codex",
        "auth",
        "materialize",
        "--grant-file",
        state_path(&grant_file)?,
        "--auth-json-file",
        state_path(&auth_file)?,
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;

    let run = run_workroomd_json(&[
        "codex",
        "run",
        "--assignment-file",
        state_path(&assignment_file)?,
        "--codex-bin",
        state_path(&fake_codex)?,
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;

    assert_eq!(
        run.pointer("/state/status").and_then(Value::as_str),
        Some("completed")
    );
    assert!(run
        .pointer("/runner_events")
        .and_then(Value::as_array)
        .is_some_and(|events| {
            has_runner_event(events, "artifact_set.completed")
                && has_runner_event(events, "artifact.created")
                && has_runner_event(events, "usage.unavailable")
                && has_runner_event(events, "run.completed")
        }));

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn codex_session_preserves_workspace_across_turns_and_scrubs_auth() -> TestResult {
    let state_dir = unique_state_dir("codex-session");
    fs::create_dir_all(&state_dir)?;
    let assignment_file = state_dir.join("assignment-session.json");
    let grant_one = state_dir.join("grant-one.json");
    let grant_two = state_dir.join("grant-two.json");
    let auth_file = state_dir.join("broker-auth-cache.json");
    let fake_codex = state_dir.join("fake-session-codex.sh");
    let now = now_ms();

    fs::write(
        &auth_file,
        "{\"kind\":\"test-codex-auth-cache\",\"value\":\"session\"}\n",
    )?;
    fs::write(
        &assignment_file,
        format!(
            r#"{{
  "contract_version": "openagents.codex_workroom_assignment.v1",
  "assignment_id": "assignment.codex.session",
  "workroom_id": "workroom.codex.session",
  "target_node_id": "oa-shc-katy-01",
  "user_ref": "user.test",
  "organization_ref": "org.test",
  "project_ref": "project.test",
  "provider_account_ref": "provider-account_codex_session",
  "auth_grant_ref": "codex-auth-grant_one",
  "repo_ref": "OpenAgentsInc/cloud",
  "prompt": "Create the first session artifact.",
  "required_artifacts": ["summary"],
  "sandbox": "danger_full_access",
  "timeout_ms": 10000,
  "wallet_authority": false,
  "created_at_ms": {now},
  "audit_context": "vortex.issue.72"
}}"#
        ),
    )?;
    write_codex_grant(&grant_one, "codex-auth-grant_one", now)?;
    write_codex_grant(&grant_two, "codex-auth-grant_two", now)?;
    write_fake_session_codex(&fake_codex)?;

    let created = run_workroomd_json(&[
        "codex",
        "session",
        "create",
        "--assignment-file",
        state_path(&assignment_file)?,
        "--ttl-ms",
        "600000",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        created.pointer("/session/status").and_then(Value::as_str),
        Some("created")
    );

    let first = run_workroomd_json(&[
        "codex",
        "session",
        "start-turn",
        "--grant-file",
        state_path(&grant_one)?,
        "--auth-json-file",
        state_path(&auth_file)?,
        "--codex-bin",
        state_path(&fake_codex)?,
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        first.pointer("/session/status").and_then(Value::as_str),
        Some("idle")
    );
    assert_eq!(
        first.pointer("/session/turn_index").and_then(Value::as_u64),
        Some(1)
    );
    let workspace = state_dir.join("codex-workspaces/assignment.codex.session");
    assert!(workspace.exists());
    assert!(workspace.join("turn-marker").exists());
    assert!(workspace.join("summary").exists());

    let auth_state: Value = serde_json::from_str(&fs::read_to_string(
        state_dir.join("codex-auth-state.json"),
    )?)?;
    assert!(auth_state
        .pointer("/scrubbed_at_ms")
        .and_then(Value::as_u64)
        .is_some());
    let materialized_auth = auth_state
        .pointer("/auth_json_path")
        .and_then(Value::as_str)
        .ok_or("missing auth_json_path")?;
    assert!(!Path::new(materialized_auth).exists());

    let second = run_workroomd_json(&[
        "codex",
        "session",
        "continue-turn",
        "--prompt",
        "Continue from the first turn and preserve the marker.",
        "--grant-file",
        state_path(&grant_two)?,
        "--auth-json-file",
        state_path(&auth_file)?,
        "--codex-bin",
        state_path(&fake_codex)?,
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        second.pointer("/session/status").and_then(Value::as_str),
        Some("idle")
    );
    assert_eq!(
        second
            .pointer("/session/turn_index")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert!(second
        .pointer("/runner_events")
        .and_then(Value::as_array)
        .is_some_and(|events| {
            has_runner_event(events, "shell.command.started")
                && has_runner_event(events, "shell.command.completed")
                && has_runner_event(events, "tool.call.started")
                && has_runner_event(events, "tool.call.completed")
                && has_runner_event(events, "message.completed")
                && has_runner_event(events, "reasoning.completed")
                && has_runner_event(events, "file.edit")
                && has_runner_event(events, "run.heartbeat")
                && has_runner_event(events, "ThreadTokenUsageUpdated")
                && has_runner_event(events, "resource.usage.captured")
                && !has_runner_event(events, "usage.unavailable")
                && !has_runner_event(events, "message.part.delta")
                && !has_runner_event(events, "shell.output.delta")
                && !has_runner_event(events, "tool.call.delta")
        }));
    let second_events = second
        .pointer("/runner_events")
        .and_then(Value::as_array)
        .ok_or("missing second-turn runner events")?;
    let shell_started = runner_event_by_type(second_events, "shell.command.started")
        .ok_or("missing shell command started event")?;
    assert_eq!(
        shell_started
            .pointer("/raw_payload_json")
            .and_then(Value::as_str),
        Some("{\"cmd\":\"printf session\",\"type\":\"exec_command_begin\"}")
    );
    let tool_started = runner_event_by_type(second_events, "tool.call.started")
        .ok_or("missing tool call event")?;
    assert!(tool_started
        .pointer("/raw_payload_json")
        .and_then(Value::as_str)
        .is_some_and(|payload| payload.contains("\"name\":\"apply_patch\"")));
    let assistant_text = runner_event_by_type(second_events, "message.completed")
        .ok_or("missing assistant text event")?;
    assert!(assistant_text
        .pointer("/raw_payload_json")
        .and_then(Value::as_str)
        .is_some_and(|payload| payload.contains("\"message.part.updated\"")));
    let reasoning = runner_event_by_type(second_events, "reasoning.completed")
        .ok_or("missing reasoning event")?;
    assert_eq!(
        reasoning.pointer("/detail_excerpt").and_then(Value::as_str),
        Some("checking workspace")
    );
    let usage_event = runner_event_by_type(second_events, "ThreadTokenUsageUpdated")
        .ok_or("missing session token usage event")?;
    assert!(usage_event
        .pointer("/raw_payload_json")
        .and_then(Value::as_str)
        .is_some_and(|payload| payload.contains("\"tokenUsage\"")));
    let summary = fs::read_to_string(workspace.join("summary"))?;
    assert!(summary.contains("Continued from preserved workspace"));

    let events = run_workroomd_json(&[
        "codex",
        "session",
        "events",
        "--cursor",
        "1",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert!(events
        .pointer("/events")
        .and_then(Value::as_array)
        .is_some_and(|events| events.len() >= 4));

    let closeout = run_workroomd_json(&[
        "codex",
        "session",
        "closeout",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        closeout.pointer("/session/status").and_then(Value::as_str),
        Some("completed")
    );
    assert_eq!(
        closeout
            .pointer("/session/artifact_refs")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert!(closeout
        .pointer("/runner_events")
        .and_then(Value::as_array)
        .is_some_and(|events| {
            has_runner_event(events, "artifact.created")
                && has_runner_event(events, "receipt.created")
                && has_runner_event(events, "run.completed")
        }));

    let archived = run_workroomd_json(&[
        "codex",
        "session",
        "archive",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        archived.pointer("/session/status").and_then(Value::as_str),
        Some("archived")
    );

    let destroyed = run_workroomd_json(&[
        "codex",
        "session",
        "destroy",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        destroyed.pointer("/session/status").and_then(Value::as_str),
        Some("destroyed")
    );
    assert!(!workspace.exists());
    assert!(!state_dir.join("codex-auth").exists());

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn codex_session_cancel_marks_session_without_auth_material() -> TestResult {
    let state_dir = unique_state_dir("codex-session-cancel");
    fs::create_dir_all(&state_dir)?;
    let assignment_file = state_dir.join("assignment-session.json");
    let now = now_ms();
    fs::write(
        &assignment_file,
        format!(
            r#"{{
  "contract_version": "openagents.codex_workroom_assignment.v1",
  "assignment_id": "assignment.codex.cancel",
  "workroom_id": "workroom.codex.cancel",
  "target_node_id": "oa-shc-katy-01",
  "user_ref": "user.test",
  "organization_ref": "org.test",
  "project_ref": "project.test",
  "provider_account_ref": "provider-account_codex_session",
  "auth_grant_ref": "codex-auth-grant_cancel",
  "repo_ref": "OpenAgentsInc/cloud",
  "prompt": "Create the first session artifact.",
  "required_artifacts": ["summary"],
  "sandbox": "danger_full_access",
  "timeout_ms": 10000,
  "wallet_authority": false,
  "created_at_ms": {now},
  "audit_context": "vortex.issue.72"
}}"#
        ),
    )?;
    run_workroomd_json(&[
        "codex",
        "session",
        "create",
        "--assignment-file",
        state_path(&assignment_file)?,
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    let canceled = run_workroomd_json(&[
        "codex",
        "session",
        "cancel-turn",
        "--reason",
        "operator requested",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;
    assert_eq!(
        canceled.pointer("/session/status").and_then(Value::as_str),
        Some("canceled")
    );
    assert!(!state_dir.join("codex-auth").exists());
    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn codex_run_git_writeback_commits_and_pushes_when_token_present() -> TestResult {
    let state_dir = unique_state_dir("codex-writeback");
    fs::create_dir_all(&state_dir)?;
    let grant_file = state_dir.join("grant.json");
    let auth_file = state_dir.join("broker-auth-cache.json");
    let assignment_file = state_dir.join("assignment.json");
    let fake_codex = state_dir.join("fake-writeback-codex.sh");
    let remote_dir = state_dir.join("remote.git");
    let now = now_ms();

    // Local bare repo acts as the push target ("origin"). The fake Codex
    // initializes the workspace as a git repo pointed at this remote and writes
    // changes, so writeback can commit + push without touching github.com.
    init_bare_remote(&remote_dir)?;

    write_run_fixtures(&grant_file, &auth_file, &assignment_file, "writeback", now)?;
    write_fake_writeback_codex(&fake_codex, &remote_dir)?;

    let _env_guard = GIT_TOKEN_ENV_LOCK.lock().unwrap();

    run_workroomd_json(&[
        "codex",
        "auth",
        "materialize",
        "--grant-file",
        state_path(&grant_file)?,
        "--auth-json-file",
        state_path(&auth_file)?,
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;

    // OA_CODEX_GITHUB_TOKEN gates writeback on; its value never reaches events
    // or the receipt.
    std::env::set_var("OA_CODEX_GITHUB_TOKEN", "ghp_fake_writeback_token");
    let run = run_workroomd_json(&[
        "codex",
        "run",
        "--assignment-file",
        state_path(&assignment_file)?,
        "--codex-bin",
        state_path(&fake_codex)?,
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ]);
    std::env::remove_var("OA_CODEX_GITHUB_TOKEN");
    let run = run?;

    assert_eq!(
        run.pointer("/state/status").and_then(Value::as_str),
        Some("completed")
    );
    let runner_events = run
        .pointer("/runner_events")
        .and_then(Value::as_array)
        .ok_or("missing runner events")?;
    assert!(
        has_runner_event(runner_events, "git.writeback.completed"),
        "expected git.writeback.completed event"
    );
    let writeback = runner_event_by_type(runner_events, "git.writeback.completed")
        .ok_or("missing writeback event")?;
    let payload = writeback
        .pointer("/raw_payload_json")
        .and_then(Value::as_str)
        .ok_or("missing writeback payload")?;
    assert!(payload.contains("\"commitSha\""));
    assert!(payload.contains("\"branchRef\":\"main\""));
    assert!(payload.contains("\"pushed\":true"));

    // The refs-only receipt was written and carries a commit sha + branch ref.
    let receipt: Value =
        serde_json::from_str(&fs::read_to_string(state_dir.join("git-writeback.json"))?)?;
    let commit_sha = receipt
        .pointer("/commit_sha")
        .and_then(Value::as_str)
        .ok_or("missing receipt commit sha")?;
    assert!(!commit_sha.is_empty());
    assert_eq!(
        receipt.pointer("/branch_ref").and_then(Value::as_str),
        Some("main")
    );

    // The pushed commit is now present in the bare remote.
    let remote_log = Command::new("git")
        .args(["--git-dir"])
        .arg(&remote_dir)
        .args(["log", "--format=%H", "main"])
        .output()?;
    assert!(remote_log.status.success(), "remote git log failed");
    assert!(String::from_utf8_lossy(&remote_log.stdout).contains(commit_sha));

    // The token never leaks into any state/event/receipt file.
    assert_no_token_leak(&state_dir, "ghp_fake_writeback_token")?;

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

#[test]
fn codex_run_git_writeback_skipped_without_token() -> TestResult {
    let state_dir = unique_state_dir("codex-writeback-skip");
    fs::create_dir_all(&state_dir)?;
    let grant_file = state_dir.join("grant.json");
    let auth_file = state_dir.join("broker-auth-cache.json");
    let assignment_file = state_dir.join("assignment.json");
    let fake_codex = state_dir.join("fake-writeback-codex.sh");
    let remote_dir = state_dir.join("remote.git");
    let now = now_ms();

    init_bare_remote(&remote_dir)?;
    write_run_fixtures(
        &grant_file,
        &auth_file,
        &assignment_file,
        "writeback-skip",
        now,
    )?;
    write_fake_writeback_codex(&fake_codex, &remote_dir)?;

    let _env_guard = GIT_TOKEN_ENV_LOCK.lock().unwrap();

    run_workroomd_json(&[
        "codex",
        "auth",
        "materialize",
        "--grant-file",
        state_path(&grant_file)?,
        "--auth-json-file",
        state_path(&auth_file)?,
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;

    // No token in the environment -> writeback must be skipped, behaving exactly
    // as the pre-writeback runner did.
    std::env::remove_var("OA_CODEX_GITHUB_TOKEN");
    std::env::remove_var("GITHUB_TOKEN");
    std::env::remove_var("GH_TOKEN");
    let run = run_workroomd_json(&[
        "codex",
        "run",
        "--assignment-file",
        state_path(&assignment_file)?,
        "--codex-bin",
        state_path(&fake_codex)?,
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;

    assert_eq!(
        run.pointer("/state/status").and_then(Value::as_str),
        Some("completed")
    );
    let runner_events = run
        .pointer("/runner_events")
        .and_then(Value::as_array)
        .ok_or("missing runner events")?;
    assert!(
        has_runner_event(runner_events, "git.writeback.skipped"),
        "expected git.writeback.skipped event"
    );
    assert!(
        !has_runner_event(runner_events, "git.writeback.completed"),
        "writeback must not run without a token"
    );
    assert!(!state_dir.join("git-writeback.json").exists());

    // Nothing was pushed to the remote.
    let remote_log = Command::new("git")
        .args(["--git-dir"])
        .arg(&remote_dir)
        .args(["log", "--format=%H", "main"])
        .output()?;
    assert!(
        !remote_log.status.success()
            || String::from_utf8_lossy(&remote_log.stdout)
                .trim()
                .is_empty(),
        "no commit should have reached the remote"
    );

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

fn init_bare_remote(remote_dir: &Path) -> TestResult {
    let init = Command::new("git")
        .args(["init", "--bare", "--initial-branch=main"])
        .arg(remote_dir)
        .output()?;
    if !init.status.success() {
        // Older git without --initial-branch: fall back.
        let init = Command::new("git")
            .args(["init", "--bare"])
            .arg(remote_dir)
            .output()?;
        if !init.status.success() {
            return Err(format!(
                "git init --bare failed: {}",
                String::from_utf8_lossy(&init.stderr)
            )
            .into());
        }
    }
    Ok(())
}

fn write_run_fixtures(
    grant_file: &Path,
    auth_file: &Path,
    assignment_file: &Path,
    slug: &str,
    now: u128,
) -> TestResult {
    fs::write(
        grant_file,
        format!(
            r#"{{
  "contract_version": "openagents.codex_auth_grant.v1",
  "workroom_id": "workroom.codex.{slug}",
  "user_ref": "user.test",
  "organization_ref": "org.test",
  "project_ref": "project.test",
  "provider_account_ref": "provider-account_codex_{slug}",
  "grant_ref": "codex-auth-grant_{slug}",
  "provider_secret_ref": "secret://codex/account/{slug}",
  "requested_mode": "exec",
  "issued_at_ms": {now},
  "expires_at_ms": {expires},
  "audit_context": "vortex.issue.96"
}}"#,
            expires = now + 1000 * 60 * 30,
        ),
    )?;
    fs::write(
        auth_file,
        format!("{{\"kind\":\"test-codex-auth-cache\",\"value\":\"{slug}\"}}\n"),
    )?;
    fs::write(
        assignment_file,
        format!(
            r#"{{
  "contract_version": "openagents.codex_workroom_assignment.v1",
  "assignment_id": "assignment.codex.{slug}",
  "workroom_id": "workroom.codex.{slug}",
  "target_node_id": "oa-shc-katy-01",
  "user_ref": "user.test",
  "organization_ref": "org.test",
  "project_ref": "project.test",
  "provider_account_ref": "provider-account_codex_{slug}",
  "auth_grant_ref": "codex-auth-grant_{slug}",
  "repo_ref": "OpenAgentsInc/cloud@main",
  "prompt": "Create the required summary artifact.",
  "required_artifacts": ["summary"],
  "sandbox": "danger_full_access",
  "timeout_ms": 10000,
  "wallet_authority": false,
  "created_at_ms": {now},
  "audit_context": "vortex.issue.96"
}}"#
        ),
    )?;
    Ok(())
}

fn write_fake_writeback_codex(path: &Path, remote_dir: &Path) -> TestResult {
    // The fake Codex initializes the workspace as a git repo pointed at the
    // local bare remote, writes the required artifact and a code change, and
    // leaves them uncommitted so the runner's writeback step commits + pushes.
    let remote = remote_dir
        .to_str()
        .ok_or("remote path is not utf-8")?
        .to_string();
    fs::write(
        path,
        format!(
            r#"#!/bin/sh
if [ -z "$CODEX_HOME" ]; then exit 9; fi
if [ "$1" = "login" ] && [ "$2" = "status" ]; then exit 0; fi
if [ "$1" = "exec" ]; then
  git init --initial-branch=main >/dev/null 2>&1 || git init >/dev/null 2>&1
  git checkout -B main >/dev/null 2>&1 || true
  git remote add origin "{remote}" >/dev/null 2>&1 || git remote set-url origin "{remote}"
  printf '# Summary\n\nDone from fake Codex.\n' > summary
  printf 'changed by codex run\n' > code-change.txt
  exit 0
fi
exit 2
"#
        ),
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

fn assert_no_token_leak(state_dir: &Path, token: &str) -> TestResult {
    for name in [
        "git-writeback.json",
        "codex-run-events.jsonl",
        "openagents-runner-events.jsonl",
        "codex-run-state.json",
        "resource-usage-receipts.jsonl",
    ] {
        let path = state_dir.join(name);
        if let Ok(contents) = fs::read_to_string(&path) {
            assert!(!contents.contains(token), "token leaked into {name}");
        }
    }
    Ok(())
}

fn run_workroomd_json(args: &[&str]) -> Result<Value, Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_oa-workroomd"))
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "oa-workroomd failed: status={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn write_codex_grant(path: &Path, grant_ref: &str, now: u128) -> TestResult {
    fs::write(
        path,
        format!(
            r#"{{
  "contract_version": "openagents.codex_auth_grant.v1",
  "workroom_id": "workroom.codex.session",
  "user_ref": "user.test",
  "organization_ref": "org.test",
  "project_ref": "project.test",
  "provider_account_ref": "provider-account_codex_session",
  "grant_ref": "{grant_ref}",
  "provider_secret_ref": "secret://codex/account/session",
  "requested_mode": "exec",
  "issued_at_ms": {now},
  "expires_at_ms": {},
  "audit_context": "vortex.issue.72"
}}"#,
            now + 1000 * 60 * 30,
        ),
    )?;
    Ok(())
}

fn write_fake_codex(path: &Path) -> TestResult {
    fs::write(
        path,
        r#"#!/bin/sh
if [ -z "$CODEX_HOME" ]; then exit 9; fi
if [ "$1" = "login" ] && [ "$2" = "status" ]; then exit 0; fi
if [ "$1" = "exec" ]; then
  printf '%s\n' "$@" | grep -qx -- '--skip-git-repo-check' || exit 11
  echo '{"type":"turn.completed","model":"gpt-5-codex","usage":{"cached_input_tokens":12,"input_tokens":42,"output_tokens":5,"reasoning_output_tokens":0}}'
  echo 'stdout has access_token canary that must be redacted'
  printf '# Summary\n\nDone from fake Codex.\n' > summary
  exit 0
fi
exit 2
"#,
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

fn write_fake_hanging_codex(path: &Path) -> TestResult {
    fs::write(
        path,
        "#!/bin/sh\nif [ -z \"$CODEX_HOME\" ]; then exit 9; fi\nif [ \"$1\" = \"login\" ] && [ \"$2\" = \"status\" ]; then exit 0; fi\nif [ \"$1\" = \"exec\" ]; then printf '# Summary\\n\\nDone before final message.\\n' > summary; sleep 30; exit 0; fi\nexit 2\n",
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

fn write_fake_session_codex(path: &Path) -> TestResult {
    fs::write(
        path,
        "#!/bin/sh\nif [ -z \"$CODEX_HOME\" ]; then exit 9; fi\nif [ \"$1\" = \"login\" ] && [ \"$2\" = \"status\" ]; then exit 0; fi\nif [ \"$1\" = \"exec\" ]; then echo '{\"type\":\"exec_command_begin\",\"cmd\":\"printf session\"}'; echo '{\"type\":\"exec_command_output_delta\",\"delta\":\"session\"}'; echo '{\"type\":\"exec_command_end\",\"cmd\":\"printf session\"}'; echo '{\"type\":\"tool_call\",\"name\":\"apply_patch\",\"arguments\":{\"cmd\":\"patch summary\"}}'; echo '{\"type\":\"tool_call_delta\",\"name\":\"apply_patch\",\"delta\":\"patch chunk\"}'; echo '{\"type\":\"tool_call_completed\",\"name\":\"apply_patch\",\"output\":\"ok\"}'; echo '{\"type\":\"message.part.delta\",\"properties\":{\"sessionID\":\"s\",\"messageID\":\"m\",\"partID\":\"p-text\",\"field\":\"text\",\"delta\":\"partial\"}}'; echo '{\"type\":\"message.part.updated\",\"properties\":{\"sessionID\":\"s\",\"part\":{\"id\":\"p-reasoning\",\"type\":\"reasoning\",\"messageID\":\"m\",\"sessionID\":\"s\",\"text\":\"checking workspace\",\"time\":{\"start\":1,\"end\":2}}}}'; echo '{\"type\":\"message.part.updated\",\"properties\":{\"sessionID\":\"s\",\"part\":{\"id\":\"p-text\",\"type\":\"text\",\"messageID\":\"m\",\"sessionID\":\"s\",\"text\":\"session response\",\"time\":{\"start\":1,\"end\":2}}}}'; echo '{\"type\":\"message.part.updated\",\"properties\":{\"sessionID\":\"s\",\"part\":{\"id\":\"p-tool\",\"type\":\"tool\",\"tool\":\"bash\",\"messageID\":\"m\",\"sessionID\":\"s\",\"state\":{\"status\":\"completed\",\"input\":{\"command\":\"bun --version\"},\"output\":\"1.2.3\"}}}}'; echo '{\"method\":\"thread/tokenUsage/updated\",\"params\":{\"threadId\":\"thread-test\",\"turnId\":\"turn-test\",\"tokenUsage\":{\"last\":{\"cachedInputTokens\":7,\"inputTokens\":70,\"outputTokens\":9,\"reasoningOutputTokens\":3,\"totalTokens\":82},\"total\":{\"cachedInputTokens\":70,\"inputTokens\":700,\"outputTokens\":90,\"reasoningOutputTokens\":30,\"totalTokens\":820}}}}'; echo '{\"type\":\"file_edit\",\"content\":\"summary\"}'; if [ -f turn-marker ]; then echo 'stdout second turn'; printf '# Summary\\n\\nContinued from preserved workspace.\\n' > summary; else echo 'stdout first turn'; printf 'first\\n' > turn-marker; printf '# Summary\\n\\nFirst turn complete.\\n' > summary; fi; exit 0; fi\nexit 2\n",
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

fn has_runner_event(events: &[Value], event_type: &str) -> bool {
    events
        .iter()
        .any(|event| event.pointer("/type").and_then(Value::as_str) == Some(event_type))
}

fn runner_event_by_type<'a>(events: &'a [Value], event_type: &str) -> Option<&'a Value> {
    events
        .iter()
        .find(|event| event.pointer("/type").and_then(Value::as_str) == Some(event_type))
}

fn unique_state_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "oa-workroomd-{label}-{}-{}",
        std::process::id(),
        unique_suffix()
    ))
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn state_path(path: &Path) -> Result<&str, Box<dyn std::error::Error>> {
    path.to_str().ok_or_else(|| "path is not utf-8".into())
}
