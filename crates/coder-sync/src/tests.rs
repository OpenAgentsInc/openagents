use std::sync::{Arc, Mutex};

use axum::Json;
use axum::Router;
use axum::extract::Path as UrlPath;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post, put};

use super::*;

const TOKEN: &str = "sess_0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn document(steps: Value) -> Value {
    json!({"schema_version": "ATIF-v1.8", "session_id": "s1", "steps": steps})
}

fn saved(origin: &str) -> Saved {
    serde_json::from_value(json!({
        "origin": origin, "account": "acct_1", "label": "Octo",
        "expires_at": u64::MAX, "token": TOKEN,
    }))
    .unwrap()
}

#[test]
fn an_upload_carries_messages_and_tool_lines_and_leaves_secrets_out() {
    // Assembled at run time so no credential-shaped literal sits here.
    let key = format!("sk-ant-{}", "a1".repeat(20));
    let doc = document(json!([
        {"step_id": 1, "source": "user", "message": "Fix the  build"},
        {"step_id": 2, "source": "agent", "message": "Looking.", "tool_calls": [
            {"tool_call_id": "c1", "function_name": "shell", "arguments": {"command": "cargo test"}},
            {"tool_call_id": "c2", "function_name": "delegate", "arguments": {"agent": "Reviewer", "task": "Check it"},
             "extra": {"schema": "openagents.delegation.v1"}},
        ]},
        {"step_id": 3, "source": "user", "message": format!("use {key}")},
        {"step_id": 4, "source": "system", "message": "ignored"},
    ]));
    let sent = upload(&doc, "Studio\n", &Screen::shapes());
    assert_eq!(sent.left_out, 1);
    assert_eq!(sent.body["computer"], "Studio");
    assert_eq!(sent.body["title"], "Fix the build");
    let texts: Vec<(&str, &str)> = sent.body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| (m["role"].as_str().unwrap(), m["text"].as_str().unwrap()))
        .collect();
    assert_eq!(
        texts,
        [
            ("user", "Fix the  build"),
            ("assistant", "Looking."),
            ("tool", "shell: cargo test"),
            ("tool", "Asked Reviewer: Check it"),
            ("user", LEFT_OUT),
        ]
    );
    assert!(!sent.body.to_string().contains(&key));
    // The same chat has the same digest; a change changes it.
    assert_eq!(
        sent.digest,
        upload(&doc, "Studio", &Screen::shapes()).digest
    );
    assert_ne!(sent.digest, upload(&doc, "Other", &Screen::shapes()).digest);
}

#[test]
fn a_host_credential_is_left_out_too_and_long_text_is_cut() {
    let mut screen = Screen::shapes();
    screen.add("plain-looking-value-123");
    let long = "x".repeat(MAX_TEXT + 10);
    let doc = document(json!([
        {"step_id": 1, "source": "user", "message": "the plain-looking-value-123 is here"},
        {"step_id": 2, "source": "agent", "message": long},
    ]));
    let sent = upload(&doc, "Studio", &screen);
    assert_eq!(sent.body["messages"][0]["text"], LEFT_OUT);
    assert_eq!(sent.body["title"], "Coder chat");
    assert!(sent.body["messages"][1]["text"].as_str().unwrap().len() <= MAX_TEXT + 4);
}

#[test]
fn settings_are_off_by_default_private_and_remember_what_was_sent() {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = Settings::load(dir.path());
    assert!(!settings.on);
    assert!(!settings.sends("s1", "d1"));
    settings.on = true;
    assert!(settings.sends("s1", "d1"));
    settings.sent.insert("s1".into(), "d1".into());
    assert!(!settings.sends("s1", "d1"));
    assert!(settings.sends("s1", "d2"));
    settings.kept_here.insert("s2".into());
    assert!(!settings.sends("s2", "d1"));
    settings.store(dir.path()).unwrap();
    assert_eq!(Settings::load(dir.path()), settings);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(dir.path().join(FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

/// A website that keeps what it is sent; session "gone" was deleted there.
fn site() -> (String, Arc<Mutex<Vec<String>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let (ready, address) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let authorized = |headers: &HeaderMap| {
                headers.get("authorization").and_then(|v| v.to_str().ok())
                    == Some(&format!("Bearer {TOKEN}"))
            };
            let (a, b, c, d) = (log.clone(), log.clone(), log.clone(), log.clone());
            let router = Router::new()
                .route(
                    "/coder/sessions",
                    get(move |headers: HeaderMap| async move {
                        assert!(authorized(&headers));
                        a.lock().unwrap().push("list".into());
                        Json(json!({"sessions": [
                            {"session": "s1", "deleted": false},
                            {"session": "gone", "deleted": true},
                        ]}))
                    }),
                )
                .route(
                    "/coder/sessions/{session}",
                    put(
                        move |UrlPath(session): UrlPath<String>,
                              headers: HeaderMap,
                              Json(body): Json<Value>| async move {
                            assert!(authorized(&headers));
                            b.lock().unwrap().push(format!("put {session} {}", body["title"]));
                            if session == "gone" {
                                return (
                                    StatusCode::GONE,
                                    Json(json!({"error": {"code": "deleted"}})),
                                );
                            }
                            (StatusCode::OK, Json(json!({"chat": "x", "changed": true})))
                        },
                    )
                    .delete(move |UrlPath(session): UrlPath<String>| async move {
                        c.lock().unwrap().push(format!("delete {session}"));
                        Json(json!({"deleted": true}))
                    }),
                )
                .route(
                    "/coder/sessions/{session}/status",
                    post(
                        move |UrlPath(session): UrlPath<String>, Json(body): Json<Value>| async move {
                            d.lock()
                                .unwrap()
                                .push(format!("status {session} {}", body["working"]));
                            Json(json!({"chat": "x", "changed": true}))
                        },
                    ),
                );
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            ready
                .send(format!("http://{}", listener.local_addr().unwrap()))
                .unwrap();
            axum::serve(listener, router).await.unwrap();
        });
    });
    (address.recv().unwrap(), seen)
}

fn wait_for(worker: &Worker, count: usize) -> Vec<Event> {
    let mut events = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(10);
    while events.len() < count && Instant::now() < deadline {
        events.extend(worker.drain());
        std::thread::sleep(Duration::from_millis(20));
    }
    events
}

#[test]
fn the_worker_sends_chats_heartbeats_and_deletes_and_hears_of_web_deletes() {
    let (origin, seen) = site();
    let worker = Worker::start(saved(&origin));
    // The first round asks which chats were deleted on the website.
    let events = wait_for(&worker, 1);
    assert_eq!(
        events,
        [Event::Gone {
            session: "gone".into()
        }]
    );
    let doc = document(json!([{"step_id": 1, "source": "user", "message": "Hello"}]));
    let sent = upload(&doc, "Studio", &Screen::shapes());
    worker.send(Job::Upload {
        session: "s1".into(),
        upload: sent.clone(),
    });
    worker.send(Job::Status {
        session: "s1".into(),
        working: true,
    });
    worker.send(Job::Upload {
        session: "gone".into(),
        upload: sent.clone(),
    });
    worker.send(Job::Delete {
        session: "old".into(),
    });
    let mut events = wait_for(&worker, 3);
    events.sort_by_key(|e| format!("{e:?}"));
    assert_eq!(
        events,
        [
            Event::Gone {
                session: "gone".into()
            },
            Event::Removed {
                session: "old".into()
            },
            Event::Saved {
                session: "s1".into(),
                digest: sent.digest.clone()
            },
        ]
    );
    let log = seen.lock().unwrap().clone();
    assert!(log.contains(&"put s1 \"Hello\"".to_string()), "{log:?}");
    assert!(log.contains(&"status s1 true".to_string()), "{log:?}");
    assert!(log.contains(&"delete old".to_string()), "{log:?}");
}

#[test]
fn an_unreachable_site_is_retried_quietly() {
    // Nothing listens here: the worker keeps the work and reports nothing.
    let worker = Worker::start(saved("http://127.0.0.1:9"));
    worker.send(Job::Delete {
        session: "s1".into(),
    });
    std::thread::sleep(Duration::from_millis(300));
    assert!(worker.drain().is_empty());
    assert!(!delete_now(&saved("http://127.0.0.1:9"), "s1"));
}
