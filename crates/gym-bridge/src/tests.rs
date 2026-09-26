use super::*;
use crate::host::{Config, Host, RecipeConfig, Source, SourceKind};
use nostr::domain::Event;
use secp256k1::SecretKey;
use serde_json::json;
use std::os::unix::fs::PermissionsExt;
use std::{path::Path, time::Duration};
#[path = "../../coder-control/src/tests/relay.rs"]
mod relay;

fn key() -> SecretKey {
    SecretKey::new(&mut secp256k1::rand::rng())
}
fn source(root: &Path) -> Source {
    Source {
        root: root.into(),
        label: "Synthetic Microcoder".into(),
        kind: SourceKind::Microcoder,
    }
}
fn config(root: &Path) -> Config {
    Config {
        sources: vec![source(root)],
        recipes: vec![],
    }
}
fn recipe(root: &Path, body: &str) -> RecipeConfig {
    let program = root.join("fixture.sh");
    std::fs::write(&program, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
    RecipeConfig {
        id: "fixture".into(),
        title: "Harmless fixture".into(),
        detail: "Writes a synthetic count file".into(),
        program,
        args: vec![],
        cwd: root.into(),
        wall_ms: 3000,
        max_starts: 2,
        environment: vec![],
    }
}
fn request(code: &Connection, secret: &SecretKey, query: Query, now: u64) -> Event {
    let body = Request {
        v: REQUEST_SCHEMA.into(),
        request: random_id(),
        grant: code.grant.clone(),
        authorization: code.authorization.id.clone(),
        issued_at: now,
        expires_at: (now + 60).min(code.expires_at),
        query,
    };
    seal(
        &body,
        REQUEST_SCHEMA,
        secret,
        &code.host,
        &body.request,
        now,
        body.expires_at,
    )
    .unwrap()
}
fn response(
    host: &Host,
    code: &Connection,
    secret: &SecretKey,
    query: Query,
    now: u64,
) -> Response {
    let event = request(code, secret, query, now);
    let reply = host.handle(&event, &code.relay, now).unwrap();
    open::<Reply>(&reply, secret, &code.host, &code.client, REPLY_SCHEMA)
        .unwrap()
        .response
}
fn board(host: &Host, code: &Connection, secret: &SecretKey) -> Snapshot {
    match response(host, code, secret, Query::Snapshot, unix_time().unwrap()) {
        Response::Snapshot(s) => *s,
        _ => panic!("expected board"),
    }
}
fn write_summary(root: &Path, name: &str, value: serde_json::Value) {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("summary.json"), value.to_string()).unwrap();
}
#[test]
fn paired_connection_is_device_bound_signed_and_expiring() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("runs");
    std::fs::create_dir(&root).unwrap();
    let host = Host::new(t.path().join("host"), RelayPolicy::Production);
    let secret = key();
    let now = unix_time().unwrap();
    let code = host
        .pair(
            &pubkey(&secret),
            "wss://example.invalid",
            config(&root),
            now,
            now + 300,
        )
        .unwrap();
    let parsed = Connection::parse(&code.encode().unwrap()).unwrap();
    parsed
        .verify(&secret, now, RelayPolicy::Production)
        .unwrap();
    assert_eq!(
        parsed
            .verify(&key(), now, RelayPolicy::Production)
            .unwrap_err()
            .code,
        ErrorCode::Forbidden
    );
    assert_eq!(
        parsed
            .verify(&secret, now + 300, RelayPolicy::Production)
            .unwrap_err()
            .code,
        ErrorCode::Expired
    );
    let mut altered = parsed;
    altered.relay = "wss://other.invalid".into();
    assert!(
        altered
            .verify(&secret, now, RelayPolicy::Production)
            .is_err()
    );
}
#[test]
fn microcoder_cost_unknowns_and_incomplete_records_never_become_zero_or_complete() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("runs");
    std::fs::create_dir(&root).unwrap();
    write_summary(
        &root,
        "known",
        json!({"task":"known","cost_basis":"list_price","outcome":{"ending":{"reason":"finished"},"seconds":2.5,"steps":4,"model_usd":0.1,"jev_usd":0.02,"embedding_usd":0.003,"cost_unknown":[]}}),
    );
    write_summary(
        &root,
        "unknown",
        json!({"task":"unknown","outcome":{"ending":{"reason":"time_limit"},"seconds":3,"model_usd":null,"jev_usd":0.02,"embedding_usd":0}}),
    );
    write_summary(
        &root,
        "null-component",
        json!({"task":"null-component","outcome":{"ending":{"reason":"finished"},"seconds":3,"model_usd":0.1,"jev_usd":null}}),
    );
    write_summary(
        &root,
        "unpriced",
        json!({"task":"unpriced","outcome":{"ending":{"reason":"finished"},"seconds":3,"model_usd":0.1,"cost_unknown":[{"why":"lost"}]}}),
    );
    write_summary(&root, "empty", json!({}));
    write_summary(
        &root,
        "omitted",
        json!({"task":"omitted","outcome":{"ending":{"reason":"finished"},"seconds":1,"model_usd":0.1}}),
    );
    let host = Host::new(t.path().join("host"), RelayPolicy::Production);
    let secret = key();
    let now = unix_time().unwrap();
    let code = host
        .pair(
            &pubkey(&secret),
            "wss://example.invalid",
            config(&root),
            now,
            now + 300,
        )
        .unwrap();
    let rows = board(&host, &code, &secret).runs;
    let known = rows.iter().find(|r| r.title == "known").unwrap();
    assert!((known.cost_usd.unwrap() - 0.123).abs() < 1e-9);
    assert_eq!(known.elapsed_ms, Some(2500));
    assert_eq!(known.status, Status::Completed);
    for name in ["unknown", "null-component", "unpriced", "empty", "omitted"] {
        assert!(
            rows.iter()
                .find(|r| r.title == name)
                .unwrap()
                .cost_usd
                .is_none()
        );
    }
    assert_eq!(
        rows.iter().find(|r| r.title == "empty").unwrap().status,
        Status::Unknown
    );
}
#[test]
fn live_series_are_bounded_ordered_and_do_not_convert_missing_cost() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("runs");
    let run = root.join("active");
    std::fs::create_dir_all(&run).unwrap();
    let events=(0..90).map(|step|json!({"event":"generated","step":step,"generated":{"usd":if step%2==0{Some(0.01)}else{None},"milliseconds":step+1}}).to_string()).collect::<Vec<_>>().join("\n");
    std::fs::write(run.join("events.jsonl"), events).unwrap();
    let roots = vec![crate::sources::Root::admit(source(&root)).unwrap()];
    let snapshot = crate::sources::snapshot(&roots, unix_time().unwrap()).unwrap();
    let row = &snapshot.runs[0];
    assert!(row.cost_usd.is_none());
    assert_eq!(row.status, Status::Running);
    assert_eq!(row.metrics.len(), 2);
    assert_eq!(row.metrics[0].points.len(), 45);
    assert_eq!(row.metrics[1].points.len(), 64);
    assert!(row.metrics[0].points.iter().all(|p| p.step % 2 == 0));
}
#[test]
fn directory_symlinks_cannot_disclose_other_sources() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("runs");
    std::fs::create_dir(&root).unwrap();
    let outside = t.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    write_summary(
        &outside,
        "private",
        json!({"task":"not admitted","outcome":{}}),
    );
    std::os::unix::fs::symlink(outside.join("private"), root.join("link")).unwrap();
    let roots = vec![crate::sources::Root::admit(source(&root)).unwrap()];
    assert!(
        crate::sources::snapshot(&roots, unix_time().unwrap())
            .unwrap()
            .runs
            .is_empty()
    );
    let run = root.join("run");
    std::fs::create_dir(&run).unwrap();
    std::os::unix::fs::symlink(
        outside.join("private/summary.json"),
        run.join("summary.json"),
    )
    .unwrap();
    assert!(
        crate::sources::snapshot(&roots, unix_time().unwrap())
            .unwrap()
            .runs
            .is_empty()
    );
}
#[test]
fn terminal_bench_reads_normalized_attempts_and_retained_episode_without_inventing_cost() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("jobs");
    let attempts = root.join("job/tbench/attempts");
    std::fs::create_dir_all(&attempts).unwrap();
    std::fs::write(attempts.join("one.json"),json!({"schema":"openagents.tbench.attempt.v1","attempt":{"trial":"one"},"task":{"name":"synthetic-task"},"outcome":{"terminal_status":"passed","reward":1.0},"cost":{"amount_usd":0.4,"provenance":"provider"},"timing":{"total_ms":3200}}).to_string()).unwrap();
    let episode = root.join("retained/two.episode");
    std::fs::create_dir_all(&episode).unwrap();
    std::fs::write(episode.join("harbor-result.json"),json!({"started_at":"2026-09-26T00:00:00Z","finished_at":"2026-09-26T00:00:02.500Z","agent_result":{"cost_usd":null},"verifier_result":{"rewards":{"reward":0.0}}}).to_string()).unwrap();
    let malformed = root.join("job/empty");
    std::fs::create_dir(&malformed).unwrap();
    std::fs::write(malformed.join("result.json"), "{}").unwrap();
    let roots = vec![
        crate::sources::Root::admit(Source {
            root,
            label: "Synthetic TB".into(),
            kind: SourceKind::TerminalBench,
        })
        .unwrap(),
    ];
    let snapshot = crate::sources::snapshot(&roots, unix_time().unwrap()).unwrap();
    assert_eq!(snapshot.runs.len(), 3);
    let a = snapshot
        .runs
        .iter()
        .find(|r| r.title == "synthetic-task")
        .unwrap();
    assert_eq!(a.cost_usd, Some(0.4));
    assert_eq!(a.elapsed_ms, Some(3200));
    let b = snapshot
        .runs
        .iter()
        .find(|r| r.title == "two.episode")
        .unwrap();
    assert_eq!(b.elapsed_ms, Some(2500));
    assert!(b.cost_usd.is_none());
    let c = snapshot.runs.iter().find(|r| r.title == "empty").unwrap();
    assert_eq!(c.status, Status::Unknown);
    assert_eq!(c.completed, None);
}
#[tokio::test]
async fn exact_launch_retry_runs_once_changed_semantics_refuses_and_reopen_retains_result() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("work");
    std::fs::create_dir(&root).unwrap();
    let host = Host::new(t.path().join("host"), RelayPolicy::Production);
    let secret = key();
    let now = unix_time().unwrap();
    let cfg = Config {
        sources: vec![],
        recipes: vec![recipe(&root, "echo ran >> count")],
    };
    let code = host
        .pair(
            &pubkey(&secret),
            "wss://example.invalid",
            cfg,
            now,
            now + 300,
        )
        .unwrap();
    let granted = code.verify(&secret, now, RelayPolicy::Production).unwrap();
    let revision = &granted.recipes[0].revision;
    let query = Query::Launch {
        request_id: random_id(),
        recipe_id: "fixture".into(),
        revision: revision.clone(),
    };
    let Response::Launch(first) = response(&host, &code, &secret, query.clone(), now) else {
        panic!("not launched")
    };
    let Response::Launch(second) = response(&host, &code, &secret, query.clone(), now) else {
        panic!("not retained")
    };
    assert_eq!(first, second);
    for _ in 0..100 {
        if board(&host, &code, &secret).runs[0].status == Status::Completed {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        std::fs::read_to_string(root.join("count")).unwrap(),
        "ran\n"
    );
    drop(host);
    let host = Host::new(t.path().join("host"), RelayPolicy::Production);
    host.recover().unwrap();
    let Response::Launch(done) =
        response(&host, &code, &secret, query.clone(), unix_time().unwrap())
    else {
        panic!("missing retained result")
    };
    assert_eq!(done.status, Status::Completed);
    assert_eq!(done.run_id, first.run_id);
    let Query::Launch { request_id, .. } = query else {
        unreachable!()
    };
    assert!(matches!(
        response(
            &host,
            &code,
            &secret,
            Query::Launch {
                request_id,
                recipe_id: "other".into(),
                revision: revision.clone()
            },
            unix_time().unwrap()
        ),
        Response::Refused(ErrorCode::Conflict)
    ));
    assert_eq!(
        std::fs::read_to_string(root.join("count")).unwrap(),
        "ran\n"
    );
}
#[tokio::test]
async fn changed_executable_and_ungranted_recipe_refuse_before_dispatch() {
    let t = tempfile::tempdir().unwrap();
    let host = Host::new(t.path().join("host"), RelayPolicy::Production);
    let secret = key();
    let now = unix_time().unwrap();
    let cfg = recipe(t.path(), "echo harmless");
    let program = cfg.program.clone();
    let code = host
        .pair(
            &pubkey(&secret),
            "wss://example.invalid",
            Config {
                sources: vec![],
                recipes: vec![cfg],
            },
            now,
            now + 300,
        )
        .unwrap();
    let grant = code.verify(&secret, now, RelayPolicy::Production).unwrap();
    std::fs::write(program, "#!/bin/sh\necho changed\n").unwrap();
    assert!(matches!(
        response(
            &host,
            &code,
            &secret,
            Query::Launch {
                request_id: random_id(),
                recipe_id: "fixture".into(),
                revision: grant.recipes[0].revision.clone()
            },
            now
        ),
        Response::Refused(ErrorCode::SourceChanged)
    ));
    assert!(board(&host, &code, &secret).runs.is_empty());
}
#[test]
fn revocation_and_root_replacement_refuse_current_observation() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("runs");
    std::fs::create_dir(&root).unwrap();
    let host = Host::new(t.path().join("host"), RelayPolicy::Production);
    let secret = key();
    let now = unix_time().unwrap();
    let code = host
        .pair(
            &pubkey(&secret),
            "wss://example.invalid",
            config(&root),
            now,
            now + 300,
        )
        .unwrap();
    std::fs::rename(&root, t.path().join("old")).unwrap();
    std::fs::create_dir(&root).unwrap();
    assert!(matches!(
        response(&host, &code, &secret, Query::Snapshot, now),
        Response::Refused(ErrorCode::SourceChanged)
    ));
    host.revoke(&code.grant).unwrap();
    assert!(matches!(
        response(&host, &code, &secret, Query::Snapshot, now),
        Response::Refused(ErrorCode::Revoked)
    ));
}
#[tokio::test]
async fn actual_authenticated_socket_snapshot_launch_and_revoke() {
    let (relay, relay_task, _) = relay::start().await;
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("runs");
    std::fs::create_dir(&root).unwrap();
    write_summary(
        &root,
        "sample",
        json!({"task":"fixture","outcome":{"ending":{"reason":"finished"},"seconds":1,"model_usd":null}}),
    );
    let host = Host::new(t.path().join("host"), RelayPolicy::LoopbackTest);
    let secret = key();
    let now = unix_time().unwrap();
    let cfg = Config {
        sources: vec![source(&root)],
        recipes: vec![recipe(t.path(), "echo socket >> count")],
    };
    let code = host
        .pair(&pubkey(&secret), &relay, cfg, now, now + 300)
        .unwrap();
    let mut receiver =
        transport::Receiver::connect(&relay, &host.key().unwrap(), RelayPolicy::LoopbackTest)
            .await
            .unwrap();
    let serving = host.clone();
    let address = relay.clone();
    let task = tokio::spawn(async move {
        loop {
            let Ok(event) = receiver.next_request().await else {
                break;
            };
            if let Ok(reply) = serving.handle_current(&event, &address) {
                receiver.publish(&reply).await.unwrap();
            }
        }
    });
    let client = Client::new_with_policy(code.clone(), secret, RelayPolicy::LoopbackTest).unwrap();
    let snapshot = client.snapshot().await.unwrap();
    assert_eq!(snapshot.runs[0].title, "fixture");
    assert!(snapshot.runs[0].cost_usd.is_none());
    let r = &snapshot.recipes[0];
    let request_id = random_id();
    let receipt = client
        .launch(&request_id, &r.id, &r.revision)
        .await
        .unwrap();
    assert_eq!(receipt.status, Status::Running);
    let repeat = client
        .launch(&request_id, &r.id, &r.revision)
        .await
        .unwrap();
    assert_eq!(receipt.run_id, repeat.run_id);
    for _ in 0..100 {
        if t.path().join("count").exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        std::fs::read_to_string(t.path().join("count")).unwrap(),
        "socket\n"
    );
    host.revoke(&code.grant).unwrap();
    let refused = client.snapshot().await.unwrap_err();
    assert_eq!(refused.code, ErrorCode::Revoked);
    assert!(confirmed_refusal(&refused));
    task.abort();
    relay_task.abort();
}
#[test]
fn dishonest_metric_and_monetary_limits_refuse() {
    let mut run = Run {
        id: random_id(),
        title: "r".into(),
        category: Category::Agent,
        status: Status::Unknown,
        completed: Some(3),
        total: Some(2),
        cost_usd: None,
        elapsed_ms: None,
        metrics: vec![],
        source: "fixture".into(),
        provenance: "synthetic".into(),
    };
    assert!(run.validate().is_err());
    run.completed = None;
    run.cost_usd = Some(f64::NAN);
    assert!(run.validate().is_err());
    let recipe = Recipe {
        id: "x".into(),
        title: "x".into(),
        revision: format!("sha256:{}", random_id()),
        detail: "x".into(),
        budget: Budget {
            wall_ms: 100,
            max_starts: 1,
            spend_limit_usd: Some(1.0),
            spend_enforced: true,
        },
    };
    assert!(recipe.validate().is_err());
    assert!(!confirmed_refusal(&error(
        ErrorCode::Forbidden,
        "reply signature failed"
    )));
}

#[tokio::test]
async fn dropped_host_retains_unknown_intent_and_never_replays_after_restart() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("work");
    std::fs::create_dir(&root).unwrap();
    let host = Host::new(t.path().join("host"), RelayPolicy::Production);
    let secret = key();
    let now = unix_time().unwrap();
    let code = host
        .pair(
            &pubkey(&secret),
            "wss://example.invalid",
            Config {
                sources: vec![],
                recipes: vec![recipe(&root, "sleep 2; echo ran >> count")],
            },
            now,
            now + 300,
        )
        .unwrap();
    let granted = code.verify(&secret, now, RelayPolicy::Production).unwrap();
    let query = Query::Launch {
        request_id: random_id(),
        recipe_id: "fixture".into(),
        revision: granted.recipes[0].revision.clone(),
    };
    assert!(matches!(
        response(&host, &code, &secret, query.clone(), now),
        Response::Launch(_)
    ));
    tokio::time::sleep(Duration::from_millis(20)).await;
    drop(host);
    tokio::time::sleep(Duration::from_millis(350)).await;
    let host = Host::new(t.path().join("host"), RelayPolicy::Production);
    host.recover().unwrap();
    let Response::Launch(old) = response(&host, &code, &secret, query, unix_time().unwrap()) else {
        panic!("lost unknown")
    };
    assert_eq!(old.status, Status::Unknown);
    assert!(matches!(
        response(
            &host,
            &code,
            &secret,
            Query::Launch {
                request_id: random_id(),
                recipe_id: "fixture".into(),
                revision: granted.recipes[0].revision.clone()
            },
            unix_time().unwrap()
        ),
        Response::Refused(ErrorCode::Conflict)
    ));
    assert!(!root.join("count").exists());
}
#[tokio::test]
async fn expiry_during_admission_leaves_no_dispatch_intent() {
    let t = tempfile::tempdir().unwrap();
    let host = Host::new(t.path().join("host"), RelayPolicy::Production);
    let secret = key();
    let now = unix_time().unwrap();
    let code = host
        .pair(
            &pubkey(&secret),
            "wss://example.invalid",
            Config {
                sources: vec![],
                recipes: vec![recipe(t.path(), "echo ran >> count")],
            },
            now,
            now + 300,
        )
        .unwrap();
    let grant = code.verify(&secret, now, RelayPolicy::Production).unwrap();
    let event = request(
        &code,
        &secret,
        Query::Launch {
            request_id: random_id(),
            recipe_id: "fixture".into(),
            revision: grant.recipes[0].revision.clone(),
        },
        now,
    );
    let calls = std::cell::Cell::new(0);
    let e = host
        .handle_with_clock(&event, &code.relay, || {
            let n = calls.get();
            calls.set(n + 1);
            Ok(if n == 0 { now } else { now + 61 })
        })
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Expired);
    assert!(board(&host, &code, &secret).runs.is_empty());
    assert!(!t.path().join("count").exists());
}
#[tokio::test]
async fn fifo_replacement_refuses_without_waiting_for_writer() {
    let t = tempfile::tempdir().unwrap();
    let host = Host::new(t.path().join("host"), RelayPolicy::Production);
    let secret = key();
    let now = unix_time().unwrap();
    let cfg = recipe(t.path(), "echo safe");
    let program = cfg.program.clone();
    let code = host
        .pair(
            &pubkey(&secret),
            "wss://example.invalid",
            Config {
                sources: vec![],
                recipes: vec![cfg],
            },
            now,
            now + 300,
        )
        .unwrap();
    let grant = code.verify(&secret, now, RelayPolicy::Production).unwrap();
    std::fs::remove_file(&program).unwrap();
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(program.as_os_str().as_bytes()).unwrap();
    // SAFETY: c is a live NUL-terminated temporary fixture path.
    assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
    let start = std::time::Instant::now();
    let response = response(
        &host,
        &code,
        &secret,
        Query::Launch {
            request_id: random_id(),
            recipe_id: "fixture".into(),
            revision: grant.recipes[0].revision.clone(),
        },
        now,
    );
    assert!(matches!(response, Response::Refused(ErrorCode::Forbidden)));
    assert!(start.elapsed() < Duration::from_secs(1));
}
