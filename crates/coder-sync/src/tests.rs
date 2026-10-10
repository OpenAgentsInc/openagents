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
/// It serves the `/v1` paths, or, when `older`, only the paths of a
/// website from before #11158.
fn site(older: bool) -> (String, Arc<Mutex<Vec<String>>>) {
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
            let (e, f, g) = (log.clone(), log.clone(), log.clone());
            let (synced, check_in) = if older {
                ("/coder/sessions", "/coder/check-in")
            } else {
                ("/v1/threads/synced", "/v1/computers/check-in")
            };
            let router = Router::new();
            // The computer's choice (#11089): "Studio" chose to keep its
            // chats; others haven't been asked.
            let router = if older {
                router.route(
                    "/coder/sync",
                    get(
                        |axum::extract::RawQuery(query): axum::extract::RawQuery| async move {
                            let studio = query.as_deref() == Some("computer=Studio");
                            Json(json!({"choice": if studio { json!("local") } else { Value::Null }}))
                        },
                    )
                    .put(move |Json(body): Json<Value>| async move {
                        g.lock().unwrap().push(format!(
                            "choose {} {}",
                            body["computer"], body["choice"]
                        ));
                        Json(json!({"choice": body["choice"]}))
                    }),
                )
            } else {
                router.route(
                    "/v1/computers/{name}/sync",
                    get(|UrlPath(name): UrlPath<String>| async move {
                        let studio = name == "Studio";
                        Json(json!({"choice": if studio { json!("local") } else { Value::Null }}))
                    })
                    .put(
                        move |UrlPath(name): UrlPath<String>, Json(body): Json<Value>| async move {
                            // The `/v1` path takes only the choice.
                            assert_eq!(body.as_object().map(|o| o.len()), Some(1));
                            g.lock()
                                .unwrap()
                                .push(format!("choose {} {}", json!(name), body["choice"]));
                            Json(json!({"choice": body["choice"]}))
                        },
                    ),
                )
            };
            let router = router
                .route(
                    synced,
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
                    &format!("{synced}/{{session}}"),
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
                    &format!("{synced}/{{session}}/status"),
                    post(
                        move |UrlPath(session): UrlPath<String>, Json(body): Json<Value>| async move {
                            d.lock()
                                .unwrap()
                                .push(format!("status {session} {}", body["working"]));
                            Json(json!({"chat": "x", "changed": true}))
                        },
                    ),
                )
                // Replies typed on the website (#11048): "s1" has one.
                .route(
                    check_in,
                    post(move |headers: HeaderMap, Json(body): Json<Value>| async move {
                        assert!(authorized(&headers));
                        e.lock()
                            .unwrap()
                            .push(format!("check-in {}", body["computer"]));
                        Json(json!({"waiting": ["s1", "../bad"]}))
                    }),
                )
                .route(
                    &format!("{synced}/{{session}}/replies"),
                    post(move |UrlPath(session): UrlPath<String>| async move {
                        f.lock().unwrap().push(format!("take {session}"));
                        if session == "gone" {
                            return (
                                StatusCode::GONE,
                                Json(json!({"error": {"code": "deleted"}})),
                            );
                        }
                        (
                            StatusCode::OK,
                            Json(json!({"replies": [
                                {"id": "r1", "text": " Now the tests "},
                                {"id": "r2", "text": "  "},
                            ], "continued": [
                                {"role": "user", "text": "Go on"},
                                {"role": "tool", "text": "ignored"},
                                {"role": "assistant", "text": "Fixed on a Cloud computer."},
                            ]})),
                        )
                    }),
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
    sends_and_hears(false);
}

#[test]
fn a_website_from_before_v1_is_asked_at_the_older_paths() {
    sends_and_hears(true);
    checks_in_and_takes(true);
    reads_and_tells_the_choice(true);
}

fn sends_and_hears(older: bool) {
    let (origin, seen) = site(older);
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

#[test]
fn the_worker_checks_in_and_takes_replies_typed_on_the_website() {
    checks_in_and_takes(false);
}

fn checks_in_and_takes(older: bool) {
    let (origin, seen) = site(older);
    let worker = Worker::start(saved(&origin));
    // The first round's delete check.
    assert_eq!(wait_for(&worker, 1).len(), 1);
    worker.send(Job::Listen {
        computer: Some("Studio".into()),
    });
    // Only well-formed session ids come back.
    assert_eq!(
        wait_for(&worker, 1),
        [Event::Waiting {
            sessions: vec!["s1".into()]
        }]
    );
    worker.send(Job::Take {
        session: "s1".into(),
    });
    worker.send(Job::Take {
        session: "gone".into(),
    });
    let mut events = wait_for(&worker, 2);
    events.sort_by_key(|e| format!("{e:?}"));
    assert_eq!(
        events,
        [
            Event::Gone {
                session: "gone".into()
            },
            Event::Replies {
                session: "s1".into(),
                replies: vec![Reply {
                    id: "r1".into(),
                    text: "Now the tests".into()
                }],
                added: vec![
                    Added {
                        user: true,
                        text: "Go on".into()
                    },
                    Added {
                        user: false,
                        text: "Fixed on a Cloud computer.".into()
                    },
                ]
            },
        ]
    );
    // Stopped: no more check-ins.
    worker.send(Job::Listen { computer: None });
    std::thread::sleep(Duration::from_millis(200));
    let checks = |log: &[String]| log.iter().filter(|l| l.starts_with("check-in")).count();
    let before = checks(&seen.lock().unwrap());
    assert!(before >= 1);
    assert!(
        seen.lock()
            .unwrap()
            .contains(&"check-in \"Studio\"".to_string())
    );
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(checks(&seen.lock().unwrap()), before);
}

#[test]
fn the_choice_is_read_and_told_to_the_website() {
    reads_and_tells_the_choice(false);
}

fn reads_and_tells_the_choice(older: bool) {
    let (origin, seen) = site(older);
    let saved = saved(&origin);
    assert_eq!(choice_now(&saved, "Studio"), Some(Choice::Local));
    assert_eq!(choice_now(&saved, "Studio Mac"), None);
    assert!(choose_now(&saved, "Studio Mac", Choice::All));
    assert!(
        seen.lock()
            .unwrap()
            .contains(&"choose \"Studio Mac\" \"all\"".to_string())
    );
    // A website that can't be reached has no choice.
    assert_eq!(choice_now(&saved_at("http://127.0.0.1:9"), "Studio"), None);
    assert_eq!(
        Choice::of(&Settings {
            on: true,
            chosen: true,
            ..Settings::default()
        }),
        Some(Choice::All)
    );
    assert_eq!(Choice::of(&Settings::default()), None);
    let sent = send_now(
        &saved,
        vec![(
            "s1".into(),
            upload(&document(json!([])), "Studio", &Screen::host()),
        )],
    );
    assert_eq!(sent.len(), 1);
}

fn saved_at(origin: &str) -> Saved {
    saved(origin)
}

#[test]
fn a_take_from_a_site_without_added_messages_brings_only_replies() {
    assert_eq!(taken(&json!({"replies": []})), Taken::default());
    let only = taken(&json!({"replies": [{"id": "r", "text": "Hi"}]}));
    assert_eq!(only.replies.len(), 1);
    assert!(only.added.is_empty());
}

#[test]
fn asks_from_the_website_are_read_and_bad_ones_left_out() {
    let id = "0b3c1a5e-8f0d-4c5e-9a7b-2d4e6f8a0b1c";
    let body = json!({"replies": [], "asks": [
        {"id": id, "action": {"kind": "screenshot"}},
        {"id": id, "action": {"kind": "pull", "path": "~/notes.txt"}},
        {"id": id, "action": {"kind": "pull", "path": "notes.txt"}},
        {"id": "not-an-id", "action": {"kind": "screenshot"}},
        {"id": id, "action": {"kind": "run"}},
    ]});
    assert_eq!(
        taken(&body).asks,
        vec![
            Ask {
                id: id.into(),
                action: AskAction::Screenshot
            },
            Ask {
                id: id.into(),
                action: AskAction::Pull {
                    path: "~/notes.txt".into()
                }
            },
        ]
    );
}
