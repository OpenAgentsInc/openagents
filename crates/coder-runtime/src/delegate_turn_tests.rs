//! Delegate doors preserve conversation permissions and event delivery.

use coder_core::capacity::{self, Connection, Provider, Refusal};
use coder_core::delegate_door::{Agent, DelegateDoor, Target, boundary_available, microcoder};
use coder_core::generate::{Door, StubGenerate};
use coder_core::shell::Status;
use coder_delegate::delegate::{Agent as Cli, Credential};
use microcoder::ProviderState;
use std::path::{Path, PathBuf};

#[path = "../../coder-core/tests/support/delegate.rs"]
mod support;
use support::*;

/// A stand-in Claude Code that tries to write `marker` in its working
/// directory and answers with what happened and the session it was
/// started or resumed under, `s-1` when it was named none.
// Like Claude Code with `--include-partial-messages`, it streams its
// reply as `stream_event` deltas: the shared harness takes the main
// agent's text from those, not from the whole assistant message. It
// reads the session from `--resume=ID` as the harness passes it.
const WRITER: &str = r#"#!/bin/sh
id=s-1
while [ $# -gt 0 ]; do
  case "$1" in
    --session-id|--resume) id=$2; shift ;;
    --session-id=*) id=${1#--session-id=} ;;
    --resume=*) id=${1#--resume=} ;;
  esac
  shift
done
cat > /dev/null
printf x > marker 2>/dev/null && wrote=wrote || wrote=refused
echo "{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"$id\",\"model\":\"stand-in\"}"
echo "{\"type\":\"assistant\",\"message\":{\"id\":\"m1\",\"content\":[{\"type\":\"tool_use\",\"id\":\"toolu_1\",\"name\":\"Bash\",\"input\":{\"command\":\"printf x > marker\"}}],\"usage\":{\"input_tokens\":5}}}"
echo "{\"type\":\"user\",\"message\":{\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"toolu_1\",\"content\":\"$wrote\",\"is_error\":false}]}}"
echo "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"the write was $wrote in $id\"}}}"
echo "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_stop\",\"index\":0}}"
echo "{\"type\":\"assistant\",\"message\":{\"id\":\"m2\",\"content\":[{\"type\":\"text\",\"text\":\"the write was $wrote in $id\"}],\"usage\":{\"input_tokens\":5}}}"
echo "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"num_turns\":1,\"result\":\"the write was $wrote in $id\",\"session_id\":\"$id\",\"total_cost_usd\":0.01,\"usage\":{\"input_tokens\":10,\"output_tokens\":3}}"
"#;

/// A stand-in Claude Code that records the hook settings and the
/// host's socket it was started with, waits for the test to say `go`,
/// and answers.
const HOOKED: &str = r#"#!/bin/sh
while [ $# -gt 0 ]; do
  if [ "$1" = --settings ]; then printf '%s' "$2" > settings.json; fi
  shift
done
printf '%s' "$CODER_HOOK_SOCKET" > socket
cat > /dev/null
n=0
while [ ! -f go ] && [ $n -lt 400 ]; do sleep 0.05; n=$((n+1)); done
echo '{"type":"system","subtype":"init","session_id":"s-1","model":"stand-in"}'
echo '{"type":"result","subtype":"success","is_error":false,"num_turns":1,"result":"done","session_id":"s-1","total_cost_usd":0.01,"usage":{"input_tokens":1,"output_tokens":1}}'
"#;

/// An agent whose delegate door runs [`WRITER`] in a fresh workspace,
/// or `None` on a host that cannot enforce a boundary.
fn writer(dir: &Path) -> Option<(crate::agent::Agent, PathBuf)> {
    let workdir = dir.join("work");
    std::fs::create_dir_all(&workdir).unwrap();
    if let Err(why) = boundary_available(&workdir) {
        eprintln!("skipped: {why}");
        return None;
    }
    let binary = coder_delegate::adapter::standin::install(&dir.join("bin"), "claude", WRITER);
    let target = Target {
        binary: Some(binary),
        ..target(Cli::ClaudeCode, true, Credential::CliLogin)
    };
    let door = DelegateDoor::new(target, None, workdir.clone(), None, String::new())
        .writing_under(dir.join("artifacts"));
    let agent = crate::agent::Agent::new(None, Door::Delegate(std::sync::Arc::new(door)));
    Some((agent, workdir))
}

async fn turn(
    agent: &mut crate::agent::Agent,
    permit: crate::permit::Permit,
    draft: &str,
) -> (crate::agent::Turned, Vec<crate::shell::ShellEvent>, String) {
    agent.push_user(draft);
    let mut shell = Vec::new();
    let mut streamed = String::new();
    let turned = agent
        .turn(
            false,
            permit,
            &mut |delta| streamed.push_str(delta),
            &mut |_| {},
            &mut |event| shell.push(event),
        )
        .await
        .unwrap();
    (turned, shell, streamed)
}

#[tokio::test]
async fn the_permit_decides_whether_the_executor_may_write_and_a_follow_up_resumes() {
    let dir = tempfile::tempdir().unwrap();
    let Some((mut agent, workdir)) = writer(dir.path()) else {
        return;
    };
    let (read, shell, streamed) = turn(
        &mut agent,
        crate::permit::Permit::answering(),
        "what is here?",
    )
    .await;
    let (said, session) = read.text.split_once(" in ").unwrap();
    assert_eq!(said, "the write was refused");
    assert_ne!(session, "s-1", "a new session is started under a named ID");
    assert_eq!(streamed, read.text);
    assert!(!workdir.join("marker").exists(), "a read-only turn wrote");
    assert_eq!(read.cost_usd, Some(0.01));
    assert_eq!(read.commands, 1);
    assert!(matches!(
        &shell[..],
        [crate::shell::ShellEvent::Proposed(proposal), crate::shell::ShellEvent::Ran(_)]
            if proposal.command == "printf x > marker"
    ));

    let (wrote, _, _) = turn(
        &mut agent,
        crate::permit::Permit::executing(),
        "now write the marker",
    )
    .await;
    assert_eq!(
        wrote.text,
        format!("the write was wrote in {session}"),
        "a follow-up resumes the first turn's session"
    );
    assert!(
        workdir.join("marker").exists(),
        "a permitted turn could not write"
    );
    assert_eq!(agent.transcript().len(), 4);
}

/// #11529: the Coder Terminal turn's Claude Code session, started
/// through the delegate door, runs the Coder SDK's hook bridge before
/// every tool, and the host behind it holds a merge for the person and
/// keeps the session's writes in its folder.
#[tokio::test]
async fn a_terminal_turn_runs_the_floors_through_the_hook_bridge() {
    let dir = tempfile::tempdir().unwrap();
    let workdir = dir.path().join("work");
    std::fs::create_dir_all(&workdir).unwrap();
    if let Err(why) = boundary_available(&workdir) {
        eprintln!("skipped: {why}");
        return;
    }
    let binary =
        coder_delegate::adapter::standin::install(&dir.path().join("bin"), "claude", HOOKED);
    let target = Target {
        binary: Some(binary),
        ..target(Cli::ClaudeCode, true, Credential::CliLogin)
    };
    let door = DelegateDoor::new(target, None, workdir.clone(), None, String::new())
        .writing_under(dir.path().join("artifacts"));
    let mut agent = crate::agent::Agent::new(None, Door::Delegate(std::sync::Arc::new(door)));
    let asked = async {
        let socket = workdir.join("socket");
        for _ in 0..400 {
            if std::fs::read_to_string(&socket).is_ok_and(|text| !text.is_empty()) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        let socket = PathBuf::from(std::fs::read_to_string(&socket).unwrap());
        let ask = |input: serde_json::Value| {
            coder_sdk::bridge::ask(
                &socket,
                coder_sdk::bridge::Format::Claude,
                &input,
                std::time::Duration::from_secs(10),
            )
            .unwrap()
        };
        let merge = ask(
            serde_json::json!({"hook_event_name": "PreToolUse", "tool_name": "Bash",
            "tool_input": {"command": "gh pr merge 5 --squash"}}),
        )
        .unwrap();
        let outside = ask(
            serde_json::json!({"hook_event_name": "PreToolUse", "tool_name": "Write",
            "tool_input": {"file_path": "/etc/elsewhere", "content": "x"}}),
        )
        .unwrap();
        let plain = ask(
            serde_json::json!({"hook_event_name": "PreToolUse", "tool_name": "Bash",
            "tool_input": {"command": "cargo test"}}),
        );
        std::fs::write(workdir.join("go"), "").unwrap();
        (merge, outside, plain)
    };
    let ((turned, _, _), (merge, outside, plain)) = tokio::join!(
        turn(&mut agent, crate::permit::Permit::executing(), "merge it"),
        asked
    );
    assert_eq!(turned.text, "done");
    let settings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(workdir.join("settings.json")).unwrap())
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
    assert_eq!(plain, None);
}

/// #11529: a Microcoder turn (the issue flow's sessions are these) runs
/// each command through the session's floors: a merge is held, and the
/// turn goes on without it.
#[tokio::test]
async fn a_microcoder_turn_holds_a_floored_command_for_the_person() {
    let dir = tempfile::tempdir().unwrap();
    let script = vec![(
        Provider::Codex,
        vec![
            Ok(running("gh pr merge 5 --squash")),
            Ok(finish("Left the merge to you.")),
        ],
    )];
    let Some((door, _workdir)) = microcoder_door(dir.path(), providers(true, false, false), script)
    else {
        return;
    };
    let mut agent = crate::agent::Agent::new(None, Door::Delegate(std::sync::Arc::clone(&door)));
    let (done, shell, _) = turn(
        &mut agent,
        crate::permit::Permit::executing(),
        "merge the pull request",
    )
    .await;
    assert_eq!(done.text, "Left the merge to you.");
    let held = shell
        .iter()
        .find_map(|event| match event {
            crate::shell::ShellEvent::Ran(outcome) => Some(outcome.output.clone()),
            _ => None,
        })
        .expect("the merge's outcome");
    assert!(held.starts_with("Held for the person"), "{held}");
}

#[tokio::test]
async fn a_microcoder_turn_streams_as_shell_events_and_the_permit_bounds_its_writes() {
    let dir = tempfile::tempdir().unwrap();
    let script = vec![(
        Provider::Codex,
        vec![
            Ok(running("printf x > marker")),
            Ok(finish("Here is the answer.")),
            Ok(running("printf x > marker")),
            Ok(finish("Wrote it.")),
        ],
    )];
    let Some((door, workdir)) = microcoder_door(dir.path(), providers(true, false, false), script)
    else {
        return;
    };
    assert_eq!(door.label(), "microcoder/gpt-6.1-sol");
    let mut agent = crate::agent::Agent::new(None, Door::Delegate(std::sync::Arc::clone(&door)));
    let (read, shell, streamed) = turn(
        &mut agent,
        crate::permit::Permit::answering(),
        "what is here?",
    )
    .await;
    assert_eq!(read.text, "Here is the answer.");
    assert_eq!(streamed, read.text);
    assert!(!workdir.join("marker").exists(), "a read-only turn wrote");
    assert_eq!(read.commands, 1);
    assert!(
        matches!(
            &shell[..],
            [crate::shell::ShellEvent::Proposed(proposal), crate::shell::ShellEvent::Ran(outcome)]
                if proposal.command == "printf x > marker" && !matches!(outcome.status, Status::Exit(0))
        ),
        "{shell:?}"
    );
    assert_eq!(
        door.session(),
        None,
        "Microcoder keeps no session to resume"
    );

    let (wrote, _, _) = turn(
        &mut agent,
        crate::permit::Permit::executing(),
        "now write the marker",
    )
    .await;
    assert_eq!(wrote.text, "Wrote it.");
    assert!(
        workdir.join("marker").exists(),
        "a permitted turn could not write"
    );
}

#[tokio::test]
async fn a_session_with_no_model_left_ends_every_turn_with_the_sentence() {
    let stub = Door::Stub(StubGenerate::refusing("No model can answer now: x."));
    let mut agent = crate::agent::Agent::new(None, stub);
    agent.push_user("hello");
    let error = agent
        .turn(
            false,
            crate::permit::Permit::answering(),
            &mut |_| {},
            &mut |_| {},
            &mut |_| {},
        )
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "No model can answer now: x.");
    assert_eq!(error.cause(), "no_capacity");
}
