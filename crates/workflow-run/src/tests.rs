use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::{EndResult, Outcome, RunRequest, Session, ThreadCall, ThreadResult, ThreadRunner};

fn script(body: &str) -> String {
    format!("export const meta = {{ name: \"Test\", description: \"A test run.\" }};\n{body}")
}

fn request(body: &str) -> RunRequest {
    RunRequest::new(script(body)).lifetime(Duration::from_secs(10))
}

fn text_runner(calls: Arc<AtomicU32>) -> ThreadRunner {
    Arc::new(move |call: ThreadCall| {
        calls.fetch_add(1, Ordering::Relaxed);
        Box::pin(async move {
            ThreadResult {
                value: Some(Value::String(call.prompt)),
                cost_sats: 0,
            }
        })
    })
}

fn kinds(outcome: &Outcome) -> Vec<String> {
    outcome
        .events
        .iter()
        .filter_map(|event| event["type"].as_str().map(str::to_string))
        .collect()
}

#[tokio::test]
async fn args_round_trip() {
    let session = Session::new(None);
    let outcome = session
        .start(
            request("return args;").args(json!({"n": 1, "name": "ada"})),
            text_runner(Arc::new(AtomicU32::new(0))),
        )
        .await;
    assert_eq!(
        outcome.result.as_ref().map(|result| result.kind.as_str()),
        Some("completed")
    );
    assert_eq!(outcome.value, json!({"n": 1, "name": "ada"}));
    assert!(outcome.id.as_deref().unwrap().starts_with("wrun_"));
    assert_eq!(kinds(&outcome)[0], "workflow_run.created");
    assert_eq!(kinds(&outcome)[1], "workflow_run.status_running");
    assert_eq!(
        kinds(&outcome).last().map(String::as_str),
        Some("workflow_run.status_ended")
    );
}

#[tokio::test]
async fn numeric_separator_parses() {
    let outcome = Session::new(None)
        .start(
            request("return 1_000;"),
            text_runner(Arc::new(AtomicU32::new(0))),
        )
        .await;
    assert_eq!(outcome.error_type(), None, "{}", outcome.notice);
    assert_eq!(outcome.value, json!(1000));
}

#[tokio::test]
async fn pipeline_streams_and_isolates_a_throw() {
    let order = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&order);
    let runner: ThreadRunner = Arc::new(move |call: ThreadCall| {
        let order = Arc::clone(&order);
        Box::pin(async move {
            order.lock().unwrap().push(format!("start {}", call.prompt));
            if call.prompt == "slow" {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            order.lock().unwrap().push(format!("end {}", call.prompt));
            ThreadResult {
                value: Some(Value::String(call.prompt)),
                cost_sats: 0,
            }
        })
    });
    let session = Session::new(None);
    let streamed = session
        .start(
            request(
                "return await pipeline([0, 1], async (prev, item) => { if (item === 0) await agent('slow'); return item; }, async (prev, item) => { await agent('late-' + item); return item; });",
            ),
            runner.clone(),
        )
        .await;
    assert_eq!(streamed.value, json!([0, 1]), "{}", streamed.notice);
    let order = seen.lock().unwrap().clone();
    let late = order
        .iter()
        .position(|line| line == "start late-1")
        .unwrap();
    let slow_end = order.iter().position(|line| line == "end slow").unwrap();
    assert!(late < slow_end, "{order:?}");

    let isolated = session
        .start(
            request(
                "return await pipeline(['a', 'b'], async (prev) => { if (prev === 'a') throw new Error('no'); return prev; }, async (prev) => prev + '!');",
            ),
            runner,
        )
        .await;
    assert_eq!(isolated.value, json!([null, "b!"]), "{}", isolated.notice);
}

#[tokio::test]
async fn parallel_is_a_barrier_and_a_throw_is_null() {
    let outcome = Session::new(None)
        .start(
            request(
                "const values = await parallel([async () => 'ok', async () => { throw new Error('x'); }, async () => null]); return values.filter(Boolean);",
            ),
            text_runner(Arc::new(AtomicU32::new(0))),
        )
        .await;
    assert_eq!(outcome.value, json!(["ok"]), "{}", outcome.notice);
    let calls = Arc::new(AtomicU32::new(0));
    let calls2 = Arc::clone(&calls);
    let runner: ThreadRunner = Arc::new(move |call: ThreadCall| {
        calls2.fetch_add(1, Ordering::Relaxed);
        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(40)).await;
            ThreadResult {
                value: Some(Value::String(call.prompt)),
                cost_sats: 0,
            }
        })
    });
    let started = Instant::now();
    let both = Session::new(None)
        .start(
            request(
                "const values = await parallel([async () => agent('a'), async () => agent('b')]); return values;",
            ),
            runner,
        )
        .await;
    assert_eq!(both.value, json!(["a", "b"]), "{}", both.notice);
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    assert!(
        started.elapsed() < Duration::from_millis(400),
        "parallel waited for the sum"
    );
}

#[tokio::test]
async fn clock_and_random_throw() {
    for (body, word) in [
        ("Date.now(); return 1;", "clock"),
        ("Math.random(); return 1;", "random"),
        ("new Date(); return 1;", "clock"),
    ] {
        let outcome = Session::new(None)
            .start(request(body), text_runner(Arc::new(AtomicU32::new(0))))
            .await;
        assert_eq!(outcome.error_type(), Some("program_error"), "{body}");
        assert!(outcome.notice.contains(word), "{}", outcome.notice);
        assert!(outcome.id.is_some());
    }
    let dated = Session::new(None)
        .start(
            request("return new Date(0).getTime();"),
            text_runner(Arc::new(AtomicU32::new(0))),
        )
        .await;
    assert_eq!(dated.value.as_f64(), Some(0.0), "{}", dated.notice);
}

#[tokio::test]
async fn sixty_four_at_once_and_the_life_cap() {
    let current = Arc::new(AtomicU32::new(0));
    let max = Arc::new(AtomicU32::new(0));
    let calls = Arc::new(AtomicU32::new(0));
    let runner: ThreadRunner = {
        let current = Arc::clone(&current);
        let max = Arc::clone(&max);
        let calls = Arc::clone(&calls);
        Arc::new(move |_call: ThreadCall| {
            let current = Arc::clone(&current);
            let max = Arc::clone(&max);
            let calls = Arc::clone(&calls);
            Box::pin(async move {
                calls.fetch_add(1, Ordering::Relaxed);
                let now = current.fetch_add(1, Ordering::Relaxed) + 1;
                max.fetch_max(now, Ordering::Relaxed);
                tokio::time::sleep(Duration::from_millis(40)).await;
                current.fetch_sub(1, Ordering::Relaxed);
                ThreadResult {
                    value: Some(json!(1)),
                    cost_sats: 0,
                }
            })
        })
    };
    let wide = Session::new(None)
        .start(
            request(
                "const ids = []; for (let i = 0; i < 80; i += 1) ids.push(i); return await parallel(ids.map((i) => () => agent('t' + i)));",
            ),
            runner.clone(),
        )
        .await;
    assert_eq!(wide.error_type(), None, "{}", wide.notice);
    assert_eq!(calls.load(Ordering::Relaxed), 80);
    let peak = max.load(Ordering::Relaxed);
    assert!(peak <= 64, "peak {peak}");
    assert!(peak > 1, "peak {peak}");

    let limited_calls = Arc::new(AtomicU32::new(0));
    let limited_runner: ThreadRunner = {
        let calls = Arc::clone(&limited_calls);
        Arc::new(move |call: ThreadCall| {
            calls.fetch_add(1, Ordering::Relaxed);
            Box::pin(async move {
                ThreadResult {
                    value: Some(Value::String(call.prompt)),
                    cost_sats: 0,
                }
            })
        })
    };
    let capped = Session::new(None)
        .start(
            request("const a = await agent('one'); const b = await agent('two'); return [a, b];")
                .max_agents(1),
            limited_runner,
        )
        .await;
    assert_eq!(
        capped.error_type(),
        Some("thread_limit_error"),
        "{}",
        capped.notice
    );
    assert_eq!(limited_calls.load(Ordering::Relaxed), 1);
    assert!(capped.notice.contains("1 thread did not start"));
}

#[tokio::test]
async fn a_step_with_the_same_inputs_is_reused() {
    let calls = Arc::new(AtomicU32::new(0));
    let session = Session::new(None);
    let body = "const a = await agent('one'); const b = await agent('two'); return [a, b];";
    let first = session
        .start(request(body), text_runner(Arc::clone(&calls)))
        .await;
    assert_eq!(first.value, json!(["one", "two"]), "{}", first.notice);
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    let second = session
        .start(request(body), text_runner(Arc::clone(&calls)))
        .await;
    assert_eq!(second.value, json!(["one", "two"]));
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    assert_ne!(first.id, second.id);
    let changed = session
        .start(
            request("return await agent('three');"),
            text_runner(Arc::clone(&calls)),
        )
        .await;
    assert_eq!(changed.value, json!("three"));
    assert_eq!(calls.load(Ordering::Relaxed), 3);
}

#[tokio::test]
async fn the_session_budget_pauses_and_resumes() {
    let calls = Arc::new(AtomicU32::new(0));
    let calls2 = Arc::clone(&calls);
    let runner: ThreadRunner = Arc::new(move |call: ThreadCall| {
        calls2.fetch_add(1, Ordering::Relaxed);
        Box::pin(async move {
            ThreadResult {
                value: Some(Value::String(call.prompt)),
                cost_sats: 10,
            }
        })
    });
    let session = Session::new(Some(10));
    let first_body = "const a = await agent('a'); const b = await agent('b'); return [a, b];";
    let first = session.start(request(first_body), runner.clone()).await;
    assert_eq!(first.status, "idle", "{}", first.notice);
    assert!(first.result.is_none());
    assert!(first.notice.contains("paused"));
    assert!(kinds(&first).contains(&"workflow_run.status_idle".into()));
    assert!(
        !kinds(&first)
            .iter()
            .any(|kind| kind == "workflow_run.status_ended")
    );
    assert_eq!(session.spent(), 10);
    assert_eq!(session.open_count(), 1);
    assert_eq!(calls.load(Ordering::Relaxed), 1);

    let other = session
        .start(request("return await agent('c');"), runner.clone())
        .await;
    assert_eq!(other.status, "idle");
    assert_eq!(
        calls.load(Ordering::Relaxed),
        1,
        "a second run sees the spent budget"
    );
    assert_eq!(session.open_count(), 2);

    session.set_budget(Some(100));
    let resumed = session.start(request(first_body), runner.clone()).await;
    assert_eq!(resumed.id, first.id);
    assert_eq!(
        resumed.result.as_ref().map(|result| result.kind.as_str()),
        Some("completed")
    );
    assert_eq!(resumed.value, json!(["a", "b"]), "{}", resumed.notice);
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    assert!(kinds(&resumed).contains(&"workflow_run.status_idle".into()));
    let other_again = session
        .start(request("return await agent('c');"), runner)
        .await;
    assert_eq!(other_again.id, other.id);
    assert_eq!(other_again.value, json!("c"), "{}", other_again.notice);
    assert_eq!(calls.load(Ordering::Relaxed), 3);
    assert_eq!(session.open_count(), 0);
    assert_eq!(session.spent(), 30);
}

#[tokio::test]
async fn the_eleventh_open_run_is_refused() {
    let go = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let notify = Arc::new(tokio::sync::Notify::new());
    let entered = Arc::new(AtomicU32::new(0));
    let runner: ThreadRunner = {
        let go = Arc::clone(&go);
        let notify = Arc::clone(&notify);
        let entered = Arc::clone(&entered);
        Arc::new(move |_call: ThreadCall| {
            let go = Arc::clone(&go);
            let notify = Arc::clone(&notify);
            let entered = Arc::clone(&entered);
            Box::pin(async move {
                entered.fetch_add(1, Ordering::SeqCst);
                loop {
                    let notified = notify.notified();
                    tokio::pin!(notified);
                    if go.load(Ordering::SeqCst) {
                        break;
                    }
                    notified.await;
                }
                ThreadResult {
                    value: Some(json!(1)),
                    cost_sats: 0,
                }
            })
        })
    };
    let session = Session::new(None);
    let mut joins = tokio::task::JoinSet::new();
    for index in 0..10 {
        let session = session.clone();
        let runner = runner.clone();
        joins.spawn(async move {
            session
                .start(
                    request(&format!("return await agent('hold-{index}');")),
                    runner,
                )
                .await
        });
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    while entered.load(Ordering::SeqCst) < 10 {
        if Instant::now() > deadline {
            go.store(true, Ordering::SeqCst);
            notify.notify_waiters();
            panic!("the ten runs did not reach their threads");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(session.open_count(), 10);
    let extra = session
        .start(request("return await agent('hold-extra');"), runner)
        .await;
    assert!(extra.id.is_none());
    assert_eq!(extra.error_type(), Some("max_workflow_runs_error"));
    go.store(true, Ordering::SeqCst);
    notify.notify_waiters();
    while let Some(joined) = joins.join_next().await {
        let outcome = joined.unwrap();
        assert_eq!(
            outcome.result.as_ref().map(|result| result.kind.as_str()),
            Some("completed"),
            "{}",
            outcome.notice
        );
    }
    assert_eq!(session.open_count(), 0);
}

#[tokio::test]
async fn a_run_does_not_start_another_run() {
    let session = Session::new(None);
    let nested = session.clone();
    let runner: ThreadRunner = Arc::new(move |_call: ThreadCall| {
        let nested = nested.clone();
        Box::pin(async move {
            let outcome = nested
                .start(
                    request("return 1;"),
                    Arc::new(|_call: ThreadCall| Box::pin(async { ThreadResult::dead() })),
                )
                .await;
            ThreadResult {
                value: Some(json!({
                    "id": outcome.id,
                    "message": outcome.notice,
                })),
                cost_sats: 0,
            }
        })
    });
    let outcome = session
        .start(request("return await agent('nest');"), runner)
        .await;
    assert_eq!(outcome.value["id"], Value::Null, "{}", outcome.notice);
    assert!(
        outcome.value["message"]
            .as_str()
            .unwrap()
            .contains("does not start another run")
    );
    let missing = Session::new(None)
        .start(
            request("return workflow();"),
            text_runner(Arc::new(AtomicU32::new(0))),
        )
        .await;
    assert_eq!(missing.error_type(), Some("program_error"));
    assert!(missing.id.is_some());
}

#[tokio::test]
async fn meta_phases_and_log() {
    let bad = Session::new(None)
        .start(
            RunRequest::new("const name = 'x';\nexport const meta = { name: name, description: 'd' };\nreturn 1;")
                .lifetime(Duration::from_secs(5)),
            text_runner(Arc::new(AtomicU32::new(0))),
        )
        .await;
    assert!(bad.id.is_none());
    assert_eq!(bad.error_type(), Some("program_error"));

    let spread = Session::new(None)
        .start(
            RunRequest::new(
                "export const meta = { ...{ name: 'n', description: 'd' } };\nreturn 1;",
            )
            .lifetime(Duration::from_secs(5)),
            text_runner(Arc::new(AtomicU32::new(0))),
        )
        .await;
    assert!(spread.id.is_none());

    let dots = Session::new(None)
        .start(
            RunRequest::new(
                "export const meta = { name: 'n...', description: 'has ... inside' };\nreturn 1;",
            )
            .lifetime(Duration::from_secs(5)),
            text_runner(Arc::new(AtomicU32::new(0))),
        )
        .await;
    assert_eq!(dots.value, json!(1), "{}", dots.notice);

    let long_name = "n".repeat(80);
    let named = Session::new(None)
        .start(
            RunRequest::new(format!(
                "export const meta = {{ name: '{long_name}', description: 'd' }};\nlog('hello');\nphase('Scan');\nphase('Scan');\nphase('Fix');\nreturn 1;"
            ))
            .lifetime(Duration::from_secs(5)),
            text_runner(Arc::new(AtomicU32::new(0))),
        )
        .await;
    assert_eq!(
        named.events[0]["name"].as_str().unwrap().chars().count(),
        64
    );
    assert!(named.notice.contains("hello"));
    let started: Vec<_> = named
        .events
        .iter()
        .filter(|event| event["type"] == "workflow_run.phase_started")
        .collect();
    let ended: Vec<_> = named
        .events
        .iter()
        .filter(|event| event["type"] == "workflow_run.phase_ended")
        .collect();
    assert_eq!(started.len(), 2);
    assert_eq!(ended.len(), 2);
    assert_eq!(ended[0]["phase_started_id"], started[0]["id"]);
    assert_eq!(ended[1]["phase_started_id"], started[1]["id"]);
    assert_eq!(
        named.events.last().unwrap()["type"],
        "workflow_run.status_ended"
    );

    let declared = Session::new(None)
        .start(
            RunRequest::new(
                "export const meta = { name: 'Scan', description: 'Look', phases: [{ title: 'Scan', detail: 'read' }, { title: 'Fix' }] };\nphase('Scan');\nphase('Fix');\nreturn 1;",
            )
            .lifetime(Duration::from_secs(5)),
            text_runner(Arc::new(AtomicU32::new(0))),
        )
        .await;
    assert_eq!(declared.events[0]["phases"][0]["id"], "wph_0");
    assert_eq!(declared.events[0]["phases"][0]["name"], "Scan");
    assert_eq!(declared.events[0]["phases"][0]["description"], "read");
    assert_eq!(declared.events[0]["phases"][1]["description"], Value::Null);
    let phase_id = declared
        .events
        .iter()
        .find(|event| event["type"] == "workflow_run.phase_started")
        .unwrap()["workflow_run_phase_id"]
        .as_str()
        .unwrap();
    assert_eq!(phase_id, "wph_0");
}

#[tokio::test]
async fn lifetime_and_stop() {
    let runner: ThreadRunner = Arc::new(|call: ThreadCall| {
        Box::pin(async move {
            loop {
                if call.cancel.load(Ordering::Relaxed) {
                    return ThreadResult::dead();
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
    });
    let timed = Session::new(None)
        .start(
            request("return await agent('hold');").lifetime(Duration::from_millis(300)),
            runner.clone(),
        )
        .await;
    assert_eq!(
        timed.error_type(),
        Some("timeout_error"),
        "{}",
        timed.notice
    );

    let session = Session::new(None);
    let stopping = session.clone();
    let handle = tokio::spawn(async move {
        stopping
            .start(request("return await agent('hold');"), runner)
            .await
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while session.open_ids().is_empty() {
        if Instant::now() > deadline {
            panic!("the run did not open");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let id = session.open_ids()[0].clone();
    assert!(session.stop(&id));
    let stopped = handle.await.unwrap();
    assert_eq!(
        stopped.result.as_ref().map(|result| result.kind.as_str()),
        Some("stopped"),
        "{}",
        stopped.notice
    );
    assert_eq!(session.open_count(), 0);
    let _ = EndResult::stopped();
}

#[tokio::test]
async fn schema_null_and_siblings() {
    let thrown = Session::new(None)
        .start(
            request(
                "await agent('x', { schema: { type: 'object', properties: { a: { type: 'string' } }, required: ['missing'] } });",
            ),
            text_runner(Arc::new(AtomicU32::new(0))),
        )
        .await;
    assert_eq!(
        thrown.error_type(),
        Some("program_error"),
        "{}",
        thrown.notice
    );
    assert!(thrown.notice.contains("schema"));

    let runner: ThreadRunner = Arc::new(|call: ThreadCall| {
        Box::pin(async move {
            if call.prompt == "die" {
                return ThreadResult::dead();
            }
            if call.prompt == "bad" {
                return ThreadResult {
                    value: Some(json!({"nope": 1})),
                    cost_sats: 0,
                };
            }
            ThreadResult {
                value: Some(json!({"ok": true})),
                cost_sats: 0,
            }
        })
    });
    let schema = "{ type: 'object', properties: { ok: { type: 'boolean' } }, required: ['ok'] }";
    let outcome = Session::new(None)
        .start(
            request(&format!(
                "const values = await parallel([async () => agent('die', {{ schema: {schema} }}), async () => agent('bad', {{ schema: {schema} }}), async () => agent('live', {{ schema: {schema} }})]); return values;"
            )),
            runner,
        )
        .await;
    assert_eq!(
        outcome.value,
        json!([null, null, {"ok": true}]),
        "{}",
        outcome.notice
    );

    let both = Session::new(None)
        .start(
            request(
                "await agent('x', { agentType: 'scout', agent: { name: 'n', description: 'd', system: 's' } });",
            ),
            text_runner(Arc::new(AtomicU32::new(0))),
        )
        .await;
    assert!(both.notice.contains("not both"), "{}", both.notice);
    let mut blocked =
        request("await agent('x', { agent: { name: 'n', description: 'd', system: 's' } });");
    blocked.inline_agents = false;
    let inline = Session::new(None)
        .start(blocked, text_runner(Arc::new(AtomicU32::new(0))))
        .await;
    assert!(
        inline.notice.contains("cannot define an agent"),
        "{}",
        inline.notice
    );
}
