//! The delegation matrix (#11511), scripted: every parent transport (Claude
//! Code in process, Codex's dynamic tools, MCP stdio for OpenCode and ACP
//! agents, the Coder loop natively) calls `delegate` to every harness.

use std::sync::Arc;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use super::scripted::{Harness, ScriptedHost, every_harness, from_child_env, init_repo};
use super::*;
use crate::admission::Admission;
use crate::registry::Disclosure;
use crate::tools::ToolServer;

/// How a parent harness reaches the Coder tools server.
#[derive(Clone, Copy, Debug)]
enum Transport {
    /// Claude Code: the in-process MCP server.
    InProcess,
    /// Codex: dynamic tools, answered by the server's `call`.
    Dynamic,
    /// OpenCode and ACP agents: `coder coder-tools-mcp` on stdio.
    Stdio,
    /// The Coder loop: the tool itself.
    Native,
}

/// The transport each parent harness uses.
fn transport(harness: &str) -> Transport {
    match harness {
        "claude-code" => Transport::InProcess,
        "codex" => Transport::Dynamic,
        "coder" => Transport::Native,
        _ => Transport::Stdio,
    }
}

struct Fixture {
    _dir: tempfile::TempDir,
    repo: std::path::PathBuf,
    runs: std::path::PathBuf,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    init_repo(&repo).unwrap();
    let runs = dir.path().join("runs");
    Fixture {
        _dir: dir,
        repo,
        runs,
    }
}

fn delegator(fixture: &Fixture, host: ScriptedHost, parent: &str) -> Arc<Delegator> {
    Arc::new(Delegator::new(
        Arc::new(host),
        Lineage::root(&format!("run-{parent}"), parent),
        Authority::default(),
        &fixture.runs,
    ))
}

fn server(delegator: &Arc<Delegator>) -> ToolServer {
    ToolServer::new("coder", "1.0.0").add(delegator.tool())
}

/// One `delegate` call through `transport`: the result's text and whether
/// it was an error.
async fn call(server: &ToolServer, transport: Transport, arguments: Value) -> (String, bool) {
    match transport {
        Transport::InProcess => {
            let reply = server
                .in_process()
                .handle_message(&json!({"jsonrpc":"2.0","id":7,"method":"tools/call",
                    "params":{"name": TOOL, "arguments": arguments}}))
                .await;
            text_of(&reply["result"])
        }
        Transport::Dynamic => {
            let result = server.call(TOOL, arguments).await;
            text_of(&result.to_value())
        }
        Transport::Native => {
            let tool = server.tools().iter().find(|t| t.name == TOOL).unwrap();
            match tool.invoke(arguments).await {
                Ok(result) => text_of(&result.to_value()),
                Err(error) => (error, true),
            }
        }
        Transport::Stdio => {
            let (client, served) = tokio::io::duplex(1 << 20);
            let (served_read, served_write) = tokio::io::split(served);
            let server = server.clone();
            let task = tokio::spawn(async move {
                crate::tools::serve(&server, BufReader::new(served_read), served_write).await
            });
            let (client_read, mut client_write) = tokio::io::split(client);
            let mut lines = BufReader::new(client_read).lines();
            for message in [
                json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"1"}}}),
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":TOOL,"arguments":arguments}}),
            ] {
                client_write
                    .write_all(format!("{message}\n").as_bytes())
                    .await
                    .unwrap();
            }
            let mut answer = Value::Null;
            while let Some(line) = lines.next_line().await.unwrap() {
                let reply: Value = serde_json::from_str(&line).unwrap();
                if reply["id"] == 2 {
                    answer = reply;
                    break;
                }
            }
            drop(client_write);
            task.abort();
            text_of(&answer["result"])
        }
    }
}

fn text_of(result: &Value) -> (String, bool) {
    let text = result["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|block| block["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    (text, result["isError"].as_bool().unwrap_or(false))
}

#[tokio::test(flavor = "multi_thread")]
async fn every_parent_delegates_to_every_harness() {
    let fixture = fixture();
    let harnesses = every_harness();
    let mut pairs = 0;
    for parent in &harnesses {
        let delegator = delegator(
            &fixture,
            ScriptedHost::new(&fixture.repo, &fixture.runs, harnesses.clone()),
            &parent.id,
        );
        let server = server(&delegator);
        for child in &harnesses {
            let (text, error) = call(
                &server,
                transport(&parent.id),
                json!({"to": child.id, "task": format!("write {}.txt", child.id)}),
            )
            .await;
            assert!(!error, "{} -> {}: {text}", parent.id, child.id);
            let report: Report = serde_json::from_str(&text).unwrap();
            assert_eq!(report.outcome, Outcome::Done, "{text}");
            assert_eq!(report.harness, child.id);
            assert_eq!(report.depth, 1);
            assert_eq!(report.run, format!("run-{}", parent.id));
            assert_eq!(report.chain, format!("{}>{}", parent.id, child.id));
            assert_eq!(report.changed_files, vec![format!("{}.txt", child.id)]);
            assert_eq!(report.checks, vec![scripted::passed("true")]);
            let worktree = report.worktree.clone().unwrap();
            assert!(worktree.join(format!("{}.txt", child.id)).is_file());
            assert!(report.transcript.as_ref().unwrap().is_file());
            // Short: the parent never gets the transcript.
            assert!(text.len() < 2_000, "{}", text.len());
            assert!(!text.contains("You are a coding agent"));
            pairs += 1;
        }
    }
    assert_eq!(pairs, harnesses.len() * harnesses.len());
    let records = read_records(&fixture.runs.join("delegations.jsonl"));
    assert_eq!(records.len(), pairs);
    assert!(
        records
            .iter()
            .all(|record| record.outcome == "done" && record.depth == 1)
    );
    assert!(
        records
            .iter()
            .any(|record| record.parent_harness == "codex" && record.harness == "claude-code")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_target_and_model_are_checked_when_the_call_arrives() {
    let fixture = fixture();
    let delegator = delegator(
        &fixture,
        ScriptedHost::new(&fixture.repo, &fixture.runs, every_harness()),
        "claude-code",
    );
    let error = delegator
        .delegate(json!({"to": "cursor", "task": "x"}))
        .await
        .unwrap_err();
    assert!(error.contains("not available"), "{error}");
    assert!(error.contains("codex"), "{error}");
    let error = delegator
        .delegate(json!({"to": "codex", "model": "gpt-3", "task": "x"}))
        .await
        .unwrap_err();
    assert!(error.contains("gpt-6.1-sol"), "{error}");
    let report = delegator
        .delegate(json!({"to": "codex", "model": "gpt-6-astra", "task": "x"}))
        .await
        .unwrap();
    assert_eq!(report.model.as_deref(), Some("gpt-6-astra"));
    let report = delegator
        .delegate(json!({"to": "auto", "task": "x", "read_only": true}))
        .await
        .unwrap();
    assert_eq!(report.harness, "claude-code");
    assert!(report.changed_files.is_empty());
    assert!(report.worktree.is_none());
    let error = delegator
        .delegate(json!({"to": "codex", "task": "x", "extra": 1}))
        .await
        .unwrap_err();
    assert!(error.contains("delegate takes"), "{error}");
    let refused = read_records(&delegator.records())
        .into_iter()
        .filter(|record| record.outcome == "refused")
        .count();
    assert_eq!(refused, 3);
}

/// The parent's tool list is the same bytes whatever is installed or
/// turned on: the harnesses are checked at call time, never listed.
#[tokio::test(flavor = "multi_thread")]
async fn the_tool_list_is_the_same_whatever_harnesses_are_on() {
    let fixture = fixture();
    let all = server(&delegator(
        &fixture,
        ScriptedHost::new(&fixture.repo, &fixture.runs, every_harness()),
        "claude-code",
    ));
    let one = server(&delegator(
        &fixture,
        ScriptedHost::new(
            &fixture.repo,
            &fixture.runs,
            vec![Harness::new("codex", "Codex", &[])],
        ),
        "claude-code",
    ));
    assert_eq!(all.list_bytes().await, one.list_bytes().await);
    assert_eq!(
        crate::codex::dynamic_tools(&all).to_string(),
        crate::codex::dynamic_tools(&one).to_string()
    );
    assert!(!all.list_bytes().await.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn nesting_is_bounded_and_recorded() {
    let fixture = fixture();
    let delegator = delegator(
        &fixture,
        ScriptedHost::new(&fixture.repo, &fixture.runs, every_harness()).nesting(),
        "claude-code",
    );
    let report = delegator
        .delegate(json!({"to": "codex", "task": "start a chain"}))
        .await
        .unwrap();
    assert_eq!(report.outcome, Outcome::Done);
    let records = read_records(&delegator.records());
    // claude-code -> codex (1) -> opencode (2) -> refused at 3.
    let depths: Vec<(u32, String, String)> = records
        .iter()
        .map(|record| (record.depth, record.chain.clone(), record.outcome.clone()))
        .collect();
    assert!(
        depths.contains(&(2, "claude-code>codex>opencode".into(), "done".into())),
        "{depths:?}"
    );
    assert!(
        depths
            .iter()
            .any(|(depth, _, outcome)| *depth == 3 && outcome == "refused"),
        "{depths:?}"
    );
    assert!(records.iter().all(|record| record.run == "run-claude-code"));
}

#[tokio::test(flavor = "multi_thread")]
async fn authority_never_widens() {
    let fixture = fixture();
    let read_only = Arc::new(Delegator::new(
        Arc::new(ScriptedHost::new(
            &fixture.repo,
            &fixture.runs,
            every_harness(),
        )),
        Lineage::root("run-2", "codex"),
        Authority {
            write: false,
            admission: Admission::none(),
        },
        &fixture.runs,
    ));
    let error = read_only
        .delegate(json!({"to": "claude-code", "task": "edit"}))
        .await
        .unwrap_err();
    assert!(error.contains("read_only"), "{error}");
    assert!(
        read_only
            .delegate(json!({"to": "claude-code", "task": "look", "read_only": true}))
            .await
            .is_ok()
    );
    // A child's environment can't give it what its host didn't.
    let decide = Disclosure {
        sends: "the command".into(),
        to: "a decision model".into(),
    };
    let parent = Authority {
        write: true,
        admission: Admission::none().admitting(decide.clone()),
    };
    let child = Delegator::new(
        Arc::new(ScriptedHost::new(
            &fixture.repo,
            &fixture.runs,
            every_harness(),
        )),
        Lineage::root("run-3", "claude-code"),
        parent.clone(),
        &fixture.runs,
    )
    .prepare(json!({"to": "codex", "task": "look", "read_only": true}))
    .unwrap();
    assert!(parent.covers(&child.authority));
    assert!(!child.authority.write);
    let (lineage, inherited) = from_child_env(&child, Authority::default());
    assert_eq!(lineage.depth, 1);
    assert_eq!(lineage.run, "run-3");
    assert_eq!(lineage.chain, vec!["claude-code".to_owned()]);
    // The child's host admits nothing, so the env's disclosure is dropped.
    assert!(inherited.admission.disclosures.is_empty());
    assert!(!inherited.write);
    let widened = Authority {
        write: true,
        admission: Admission::none().admitting(Disclosure {
            sends: "the files".into(),
            to: "elsewhere".into(),
        }),
    };
    assert!(!parent.covers(&widened));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_background_child_answers_at_once() {
    let fixture = fixture();
    let host = ScriptedHost::new(&fixture.repo, &fixture.runs, every_harness());
    let delegator = Arc::new(Delegator::new(
        Arc::new(host.clone()),
        Lineage::root("run-4", "coder"),
        Authority::default(),
        &fixture.runs,
    ));
    let report = delegator
        .delegate(json!({"to": "codex", "task": "long job", "background": true}))
        .await
        .unwrap();
    assert_eq!(report.outcome, Outcome::Running);
    assert_eq!(host.started().len(), 1);
    assert_eq!(read_records(&delegator.records())[0].outcome, "running");
}

#[test]
fn a_report_stays_short() {
    let long = "word ".repeat(1_000);
    let (summary, checks, done) = closing(&format!(
        "{long}\n\n```json\n{{\"summary\":\"did it\",\"done\":true,\"checks\":[{{\"command\":\"cargo test\",\"passed\":false}}]}}\n```"
    ));
    assert_eq!(summary, "did it");
    assert_eq!(checks.len(), 1);
    assert_eq!(done, Some(true));
    let (summary, checks, _) = closing("first\n\nthe last paragraph");
    assert_eq!(summary, "the last paragraph");
    assert!(checks.is_empty());
    let child = Child {
        request: Request {
            to: "codex".into(),
            task: "x".into(),
            ..Request::default()
        },
        target: Target {
            harness: "codex".into(),
            name: "Codex".into(),
            model: None,
        },
        lineage: Lineage::root("r", "coder").child("codex", 1).unwrap(),
        authority: Authority::default(),
        briefing: String::new(),
        run_dir: "/tmp/x".into(),
    };
    let report = Report::for_child(&child, Outcome::Done)
        .summarized(&long)
        .with_files((0..100).map(|n| format!("f{n:03}")).collect());
    assert!(report.summary.len() <= SUMMARY_MAX);
    assert_eq!(report.changed_files.len(), FILES_MAX);
    assert_eq!(report.more_files, 60);
    assert_eq!(
        cache_read_ratio(
            &json!({"input_tokens": 10, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 90})
        ),
        Some(0.9)
    );
}
