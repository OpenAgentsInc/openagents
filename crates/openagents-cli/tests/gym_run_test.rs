//! `gym run` lifecycle and stub-server tests.
//!
//! These tests stand up a local HTTP/1.1 stub for the Gym lifecycle routes and
//! assert exactly what the Rust client puts on the wire.

use openagents_cli::gym::run::{
    ExecuteArgs, GymClient, HostPlan, RunAction, api_origin, catalog_model,
    coder_api_url_for_container, container_docker_args, container_env_vars, finalize_job_dir,
    host_env_vars, infer_lane, preflight_refusal, run, suite_script_args,
};
use openagents_cli::gym::schemas::{RUN_STATUS_SCHEMA, RunStatus};
use openagents_cli::gym::suite::{ResolvedSuite, ResolvedTask, resolve_for_run_in};
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

#[derive(Debug, Clone)]
struct SeenRequest {
    method: String,
    path: String,
    authorization: Option<String>,
    body: Value,
}

struct StubGymApi {
    base: String,
    seen: Receiver<SeenRequest>,
}

/// Start a stub server that accepts `count` requests and always replies with
/// `body` over a `Connection: close` stream.
fn stub_gym_api(count: usize, body: Value) -> StubGymApi {
    stub_gym_api_status(count, 200, body)
}

/// Same stub, answering every request with `status`.
fn stub_gym_api_status(count: usize, status: u16, body: Value) -> StubGymApi {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (sender, seen) = channel();
    let response_text = body.to_string();

    std::thread::spawn(move || {
        for _ in 0..count {
            let Ok((stream, _)) = listener.accept() else {
                return;
            };
            let mut reader = BufReader::new(stream);
            let mut request_line = String::new();
            if reader.read_line(&mut request_line).is_err() {
                return;
            }
            let mut parts = request_line.split_whitespace();
            let method = parts.next().unwrap_or_default().to_string();
            let path = parts.next().unwrap_or_default().to_string();

            let mut content_length = 0usize;
            let mut authorization = None;
            loop {
                let mut header = String::new();
                match reader.read_line(&mut header) {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(_) => return,
                }
                let trimmed = header.trim_end();
                if trimmed.is_empty() {
                    break;
                }
                if let Some((name, value)) = trimmed.split_once(':') {
                    if name.eq_ignore_ascii_case("content-length") {
                        content_length = value.trim().parse().unwrap_or(0);
                    }
                    if name.eq_ignore_ascii_case("authorization") {
                        authorization = Some(value.trim().to_string());
                    }
                }
            }

            let mut raw = vec![0u8; content_length];
            if content_length > 0 && reader.read_exact(&mut raw).is_err() {
                return;
            }
            let parsed = if raw.is_empty() {
                Value::Null
            } else {
                serde_json::from_slice(&raw).unwrap_or(Value::Null)
            };
            let _ = sender.send(SeenRequest {
                method,
                path,
                authorization,
                body: parsed,
            });

            let reason = match status {
                200 => "OK",
                401 => "Unauthorized",
                403 => "Forbidden",
                _ => "Status",
            };
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_text.len(),
                response_text
            );
            let stream = reader.get_mut();
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });

    StubGymApi {
        base: format!("http://127.0.0.1:{port}/api/v1"),
        seen,
    }
}

fn seen(stub: &StubGymApi) -> SeenRequest {
    stub.seen
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("the client never sent a request")
}

#[tokio::test]
async fn start_run_posts_the_expected_payload_and_auth() {
    let stub = stub_gym_api(1, serde_json::json!({"run": {"id": "run-2026-001"}}));
    let client = GymClient::new(&stub.base, Some("oa_pat_test".to_string()));

    let started = client
        .start_run("tb2-quick", "gpt-5.6-luna", "proxy", 2)
        .await
        .expect("the stub answered 200");

    assert_eq!(started.run_id, "run-2026-001");

    let req = seen(&stub);
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, "/api/v1/gym/runs/start");
    assert_eq!(req.authorization.as_deref(), Some("Bearer oa_pat_test"));
    assert_eq!(req.body["suite"], "tb2-quick");
    assert_eq!(req.body["agent"], "openagents-coder");
    assert_eq!(req.body["model"], "gpt-5.6-luna");
    assert_eq!(req.body["lane"], "proxy");
    assert_eq!(req.body["tasks_total"], 2);
}

#[tokio::test]
async fn finalize_patches_abandoned_when_no_verifier_graded() {
    let stub = stub_gym_api(3, serde_json::json!({"run": {"id": "run-2026-001"}}));
    let client = GymClient::new(&stub.base, Some("oa_pat_test".to_string()));

    let suite = ResolvedSuite {
        id: "tb2-quick".to_string(),
        tasks: vec![
            openagents_cli::gym::suite::ResolvedTask {
                id: "regex-log".to_string(),
                dataset: "terminal-bench@2.0".to_string(),
            },
            openagents_cli::gym::suite::ResolvedTask {
                id: "openssl-selfsigned-cert".to_string(),
                dataset: "terminal-bench@2.0".to_string(),
            },
        ],
    };

    let (job_dir, _tmp) = make_job_dir(&[
        ("regex-log__a1", "regex-log", None),
        (
            "openssl-selfsigned-cert__b2",
            "openssl-selfsigned-cert",
            None,
        ),
    ]);

    let status = finalize_job_dir(
        &client,
        Some("run-2026-001"),
        &suite,
        "proxy",
        "openai/gpt-5.6-luna",
        &job_dir,
    )
    .await
    .expect("finalize succeeds");

    // Two trial upserts, then the abandon patch.
    let req1 = seen(&stub);
    let req2 = seen(&stub);
    let req3 = seen(&stub);

    for req in [&req1, &req2] {
        assert_eq!(req.method, "POST");
        assert_eq!(req.path, "/api/v1/gym/runs/run-2026-001/trials");
        assert_eq!(req.body["state"], "ungraded");
    }

    assert_eq!(req3.method, "PATCH");
    assert_eq!(req3.path, "/api/v1/gym/runs/run-2026-001");
    assert_eq!(req3.body["status"], "abandoned");

    assert_eq!(status.state, "abandoned");
    assert_eq!(status.ungraded, 2);
    assert_eq!(status.graded, 0);
}

#[tokio::test]
async fn finalize_upserts_graded_trials_and_patches_graded() {
    let stub = stub_gym_api(3, serde_json::json!({"run": {"id": "run-2026-002"}}));
    let client = GymClient::new(&stub.base, Some("oa_pat_test".to_string()));

    let suite = ResolvedSuite {
        id: "tb2-quick".to_string(),
        tasks: vec![
            openagents_cli::gym::suite::ResolvedTask {
                id: "regex-log".to_string(),
                dataset: "terminal-bench@2.0".to_string(),
            },
            openagents_cli::gym::suite::ResolvedTask {
                id: "openssl-selfsigned-cert".to_string(),
                dataset: "terminal-bench@2.0".to_string(),
            },
        ],
    };

    let (job_dir, _tmp) = make_job_dir(&[
        (
            "regex-log__a1",
            "regex-log",
            Some(serde_json::json!({"rewards": 1.0})),
        ),
        (
            "openssl-selfsigned-cert__b2",
            "openssl-selfsigned-cert",
            Some(serde_json::json!({"rewards": -1.0})),
        ),
    ]);

    let status = finalize_job_dir(
        &client,
        Some("run-2026-002"),
        &suite,
        "proxy",
        "openai/gpt-5.6-luna",
        &job_dir,
    )
    .await
    .expect("finalize succeeds");

    let req1 = seen(&stub);
    let req2 = seen(&stub);
    let req3 = seen(&stub);

    // The trials are upserted before the run is patched.
    assert_eq!(req1.method, "POST");
    assert_eq!(req2.method, "POST");
    assert_eq!(req3.method, "PATCH");
    assert_eq!(req3.path, "/api/v1/gym/runs/run-2026-002");
    assert_eq!(req3.body["status"], "graded");

    let mut by_task: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    for req in [&req1, &req2] {
        by_task.insert(
            req.body["task"].as_str().unwrap().to_string(),
            req.body["state"].as_str().unwrap().to_string(),
        );
    }

    assert_eq!(by_task.get("regex-log").map(String::as_str), Some("passed"));
    assert_eq!(
        by_task.get("openssl-selfsigned-cert").map(String::as_str),
        Some("failed")
    );

    assert_eq!(status.state, "graded");
    assert_eq!(status.accepted, 1);
    assert_eq!(status.rejected, 1);
    assert_eq!(status.graded, 2);
}

#[test]
fn drifted_suite_refuses_before_any_network_call() {
    let tmp = tempfile::tempdir().unwrap();
    let suites = tmp.path().join("bench").join("suites");
    let results = tmp.path().join("bench-results");
    std::fs::create_dir_all(&suites).unwrap();
    std::fs::create_dir_all(&results).unwrap();

    std::fs::write(
        suites.join("drifted.suite.json"),
        r#"{
  "schema": "openagents.effectiveness_suite.v1",
  "id": "drifted",
  "tier": "smoke",
  "description": "Drifted",
  "tasks": [
    {
      "id": "regex-log",
      "pin": {
        "kind": "harbor-registry",
        "dataset": "terminal-bench@2.0",
        "gitUrl": "https://github.com/laude-institute/terminal-bench-2.git",
        "commit": "69671fbaac6d67a7ef0dfec016cc38a64ef7a77c",
        "path": "regex-log"
      },
      "environmentAvailable": true
    }
  ]
}"#,
    )
    .unwrap();

    std::fs::write(
        results.join("drifted.jsonl"),
        r#"{"suiteId":"drifted","suiteDigest":"suite-manifest:0000000000000000000000000000000000000000000000000000000000000000"}"#,
    )
    .unwrap();

    let result = resolve_for_run_in(&suites, "drifted");
    assert!(result.is_err(), "drifted suite must be refused");
    let text = result.unwrap_err().to_string();
    assert!(
        text.contains("drifted"),
        "refusal must name drift; got {text}"
    );
}

#[tokio::test]
async fn run_status_json_matches_golden_shape() {
    let value = RunStatus {
        schema: RUN_STATUS_SCHEMA.to_string(),
        run_id: "run-2026-001".to_string(),
        suite_id: "tb2-quick".to_string(),
        lane: "proxy".to_string(),
        model: Some("openai/gpt-5.6-luna".to_string()),
        state: "graded".to_string(),
        started_at: Some("2026-08-27T10:00:00Z".to_string()),
        updated_at: Some("2026-08-27T10:05:12Z".to_string()),
        tasks_total: 2,
        accepted: 1,
        rejected: 0,
        ungraded: 1,
        graded: 1,
        summary: "1 accepted, 0 rejected, 1 ungraded; 1 of 2 tasks graded".to_string(),
        trials: vec![
            openagents_cli::gym::schemas::RunTrial {
                task: "regex-log".to_string(),
                state: "accepted".to_string(),
                outcome: Some("accepted".to_string()),
                started_at: Some("2026-08-27T10:00:01Z".to_string()),
                finished_at: Some("2026-08-27T10:02:30Z".to_string()),
                transcript_ref: Some("/trace/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string()),
                cost_usd: Some(0.0042),
            },
            openagents_cli::gym::schemas::RunTrial {
                task: "openssl-selfsigned-cert".to_string(),
                state: "ungraded".to_string(),
                outcome: None,
                started_at: Some("2026-08-27T10:00:01Z".to_string()),
                finished_at: Some("2026-08-27T10:05:12Z".to_string()),
                transcript_ref: Some("/trace/bbbbbbbb-cccc-dddd-eeee-ffffffffffff".to_string()),
                cost_usd: None,
            },
        ],
    };
    let json = serde_json::to_string(&value).unwrap();
    let golden = std::fs::read_to_string("tests/fixtures/gym/run_status.golden.json").unwrap();
    assert_eq!(
        json, golden,
        "run_status.v1 JSON shape must match the golden fixture"
    );
}

#[test]
fn lane_and_catalog_model_match_shell_inference() {
    assert_eq!(infer_lane("ollama/qwen3.8:27b"), "local");
    assert_eq!(infer_lane("openai/gpt-5.6-luna"), "proxy");
    assert_eq!(catalog_model("openai/gpt-5.6-luna"), "gpt-5.6-luna");
    assert_eq!(catalog_model("ollama/qwen3.8:27b"), "ollama:qwen3.8:27b");
}

#[tokio::test]
async fn dry_run_resolves_suite_and_does_not_contact_api() {
    let action = RunAction::Run(ExecuteArgs {
        suite_id: "tb2-quick".to_string(),
        model: "openai/gpt-5.6-luna".to_string(),
        lane: None,
        n_concurrent: 1,
        jobs_dir: None,
        timeout_multiplier: None,
        env: None,
        dry_run: true,
    });
    // A closed port proves no network call was made: dry-run only resolves and prints.
    let result = run(action, "http://127.0.0.1:1/api/v1", None, false).await;
    assert!(
        result.is_ok(),
        "dry-run must succeed without registering or executing"
    );
}

#[test]
fn container_docker_args_mount_jobs_and_pin_the_image() {
    let plan = HostPlan {
        suite: ResolvedSuite {
            id: "tb2-quick".to_string(),
            tasks: vec![ResolvedTask {
                id: "regex-log".to_string(),
                dataset: "terminal-bench@2.0".to_string(),
            }],
        },
        model: "openai/gpt-5.6-luna".to_string(),
        lane: "proxy".to_string(),
        n_concurrent: 2,
        jobs_dir: PathBuf::from("/tmp/gym-jobs-test"),
        timeout_multiplier: Some(1.5),
        env_provider: Some("docker".to_string()),
        run_id: Some("run-1".to_string()),
        api_origin: "http://127.0.0.1:4000".to_string(),
        token: Some("token".to_string()),
    };
    let args = container_docker_args(&plan, &plan.jobs_dir);
    let text = args.join(" ");
    assert!(text.starts_with("run --rm "), "{text}");
    assert!(text.contains("/tmp/gym-jobs-test:/jobs"), "{text}");
    assert!(
        text.contains("/var/run/docker.sock:/var/run/docker.sock"),
        "{text}"
    );
    assert!(text.contains("harbor-runner:0.1.0"), "{text}");
    assert!(text.contains("--jobs-dir /jobs"), "{text}");
    assert!(text.contains("-i regex-log"), "{text}");
    assert!(text.contains("--n-concurrent 2"), "{text}");
    assert!(text.contains("--timeout-multiplier 1.5"), "{text}");
    assert!(text.contains("-e OPENAGENTS_TOKEN"), "{text}");
    assert!(text.contains("-e OPENAGENTS_GYM_RUN_ID"), "{text}");
    assert!(
        !text.contains("/tmp/gym-jobs-test --n-concurrent"),
        "{text}"
    );
}

#[test]
fn api_origin_reduces_an_api_v1_base_to_the_bare_origin() {
    assert_eq!(
        api_origin("https://openagents.com/api/v1"),
        "https://openagents.com"
    );
    assert_eq!(
        api_origin("http://localhost:4000/api/v1"),
        "http://localhost:4000"
    );
    assert_eq!(
        api_origin("http://127.0.0.1:4000/api/v1/"),
        "http://127.0.0.1:4000"
    );
    // An already-bare origin passes through unchanged.
    assert_eq!(
        api_origin("https://staging.openagents.com"),
        "https://staging.openagents.com"
    );
    assert_eq!(
        api_origin("http://localhost:4000/"),
        "http://localhost:4000"
    );
}

#[test]
fn suite_script_receives_a_bare_origin_api_url() {
    let plan = host_plan_with_origin(api_origin("https://openagents.com/api/v1"));
    let args = suite_script_args(&plan);
    let position = args
        .iter()
        .position(|a| a == "--api-url")
        .expect("--api-url is passed to bench/run-suite.sh");
    assert_eq!(
        args[position + 1],
        "https://openagents.com",
        "the script appends /api/v1 itself; handing it an /api/v1 base doubles the path"
    );
}

#[test]
fn container_env_carries_a_bare_origin_in_coder_api_url() {
    let plan = host_plan_with_origin(api_origin("http://127.0.0.1:4000/api/v1"));
    let vars: std::collections::BTreeMap<_, _> = container_env_vars(&plan).into_iter().collect();
    // `openagents coder --api-url` refuses any URL with a path, so the
    // container env must carry the bare (loopback-rewritten) origin.
    assert_eq!(
        vars.get("OPENAGENTS_CODER_API_URL").map(String::as_str),
        Some("http://host.docker.internal:4000")
    );
    assert_eq!(
        vars.get("OPENAGENTS_GYM_API_URL").map(String::as_str),
        Some("http://host.docker.internal:4000")
    );
    assert_eq!(
        vars.get("OPENAGENTS_GYM_RUN_ID").map(String::as_str),
        Some("run-1")
    );
}

#[test]
fn host_env_carries_the_bare_origin_unrewritten() {
    let plan = host_plan_with_origin(api_origin("https://openagents.com/api/v1"));
    let vars: std::collections::BTreeMap<_, _> = host_env_vars(&plan).into_iter().collect();
    assert_eq!(
        vars.get("OPENAGENTS_CODER_API_URL").map(String::as_str),
        Some("https://openagents.com")
    );
    assert_eq!(
        vars.get("OPENAGENTS_GYM_API_URL").map(String::as_str),
        Some("https://openagents.com")
    );
}

#[tokio::test]
async fn client_paths_carry_a_single_api_v1_when_rebuilt_from_the_origin() {
    let stub = stub_gym_api(1, serde_json::json!({"run": {"id": "run-2026-003"}}));
    // The same derivation execute() performs: origin out of the base, client
    // back onto `{origin}/api/v1`. The wire path must carry /api/v1 once.
    let base = format!("{}/api/v1", api_origin(&stub.base));
    let client = GymClient::new(&base, Some("oa_pat_test".to_string()));
    client
        .start_run("tb2-quick", "gpt-5.6-luna", "proxy", 1)
        .await
        .expect("the stub answered 200");
    let req = seen(&stub);
    assert_eq!(req.path, "/api/v1/gym/runs/start");
}

#[tokio::test]
async fn preflight_refusal_on_403_names_the_scope_and_the_login_command() {
    let stub = stub_gym_api_status(
        1,
        403,
        serde_json::json!({
            "code": "not_operator",
            "message": "operator standing required",
        }),
    );
    let client = GymClient::new(&stub.base, Some("oa_pat_test".to_string()));
    let error = client.preflight().await.expect_err("403 must refuse");
    let refusal = preflight_refusal(error, "https://openagents.com");
    let text = refusal.to_string();
    assert!(text.contains("forge:write"), "{text}");
    assert!(text.contains("operator"), "{text}");
    assert!(text.contains("chat:account"), "{text}");
    assert!(
        text.contains("openagents auth login --scope forge:write"),
        "{text}"
    );
    assert!(text.contains("not_operator"), "{text}");

    let req = seen(&stub);
    assert_eq!(req.method, "GET");
    assert_eq!(req.path, "/api/v1/gym/runs?limit=1");
}

#[tokio::test]
async fn status_malformed_answer_surfaces_the_server_body() {
    // 200 with a body the RunStatus shape cannot absorb: the error must quote
    // what the server actually said, not just a bare parse failure.
    let stub = stub_gym_api(1, serde_json::json!({"error": "run row gone missing"}));
    let client = GymClient::new(&stub.base, Some("oa_pat_test".to_string()));
    let error = client
        .get_run("run-2026-004")
        .await
        .expect_err("an unparseable body must refuse");
    let text = error.to_string();
    assert!(text.contains("run row gone missing"), "{text}");
}

#[test]
fn container_api_url_rewrites_loopback() {
    assert_eq!(
        coder_api_url_for_container("http://127.0.0.1:4000/api/v1"),
        "http://host.docker.internal:4000/api/v1"
    );
    assert_eq!(
        coder_api_url_for_container("http://localhost:4000/api/v1"),
        "http://host.docker.internal:4000/api/v1"
    );
    assert_eq!(
        coder_api_url_for_container("https://openagents.com/api/v1"),
        "https://openagents.com/api/v1"
    );
}

fn host_plan_with_origin(api_origin: String) -> HostPlan {
    HostPlan {
        suite: ResolvedSuite {
            id: "tb2-quick".to_string(),
            tasks: vec![ResolvedTask {
                id: "regex-log".to_string(),
                dataset: "terminal-bench@2.0".to_string(),
            }],
        },
        model: "openai/gpt-5.6-luna".to_string(),
        lane: "proxy".to_string(),
        n_concurrent: 1,
        jobs_dir: PathBuf::from("/tmp/gym-jobs-test"),
        timeout_multiplier: None,
        env_provider: None,
        run_id: Some("run-1".to_string()),
        api_origin,
        token: Some("token".to_string()),
    }
}

fn make_job_dir(trials: &[(&str, &str, Option<Value>)]) -> (PathBuf, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    std::fs::write(root.join("result.json"), r#"{"id":"job-1"}"#).unwrap();
    std::fs::write(root.join("config.json"), r#"{}"#).unwrap();

    for (dir_name, task_name, verifier) in trials {
        let dir = root.join(dir_name);
        let agent = dir.join("agent");
        std::fs::create_dir_all(&agent).unwrap();
        let mut result = serde_json::json!({
            "task_name": task_name,
            "agent_execution": {
                "started_at": "2026-08-27T10:00:01Z",
                "finished_at": "2026-08-27T10:02:30Z",
            },
        });
        if let Some(v) = verifier {
            result["verifier_result"] = v.clone();
        }
        std::fs::write(dir.join("result.json"), result.to_string()).unwrap();
    }
    (root, tmp)
}
