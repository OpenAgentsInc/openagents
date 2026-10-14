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
            {"tool_call_id": "c3", "function_name": "OpenAgents CLI", "arguments": {"arguments": ["projects", "show", "finances"]}},
            {"tool_call_id": "c4", "function_name": "claude:thinking", "arguments": null},
            {"tool_call_id": "c5", "function_name": "claude:thinking", "arguments": null},
            {"tool_call_id": "route-2", "function_name": "chat_router", "arguments": {"set": "chat-router-v5"},
             "extra": {"schema": "openagents.decision-call.v1"}},
        ], "observation": {"results": [
            {"source_call_id": "c4", "content": "{\"text\":\"I'll check the help first.\"}"},
            {"source_call_id": "c5", "content": "{\"text\":\"  \"}"},
        ]}},
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
            ("tool", "shell cargo test"),
            ("tool", "Asked Reviewer: Check it"),
            ("tool", "OpenAgents CLI projects show finances"),
            ("thinking", "I'll check the help first."),
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
    let loaded = Settings::load(dir.path());
    assert_eq!(loaded.folder, Some(folder_name(dir.path())));
    assert_eq!(
        loaded,
        Settings {
            folder: loaded.folder.clone(),
            ..settings
        }
    );
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

#[test]
fn a_corrupt_settings_file_is_moved_aside_and_reported() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);
    std::fs::write(&file, b"{ this is not json").unwrap();
    let why = Settings::load_checked(dir.path()).unwrap_err();
    assert!(why.contains("not readable settings"), "{why}");
    // The old bytes are preserved, not overwritten or deleted.
    let aside = dir.path().join("sync.json.corrupt");
    assert!(aside.exists(), "the corrupt file should be kept");
    assert_eq!(std::fs::read(&aside).unwrap(), b"{ this is not json");
    // With the bad file gone, the next load is a clean default.
    assert_eq!(Settings::load(dir.path()), Settings::default());
    assert!(!Settings::load_checked(dir.path()).unwrap().on);
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

#[test]
fn many_saves_at_once_in_one_process_all_succeed() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("sync.json");
    let handles: Vec<_> = (0..16)
        .map(|i| {
            let file = file.clone();
            std::thread::spawn(move || {
                for j in 0..50 {
                    super::write_private(&file, format!("{{\"n\":{}}}", i * 100 + j).as_bytes())
                        .unwrap();
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
    assert!(
        std::fs::read_to_string(&file)
            .unwrap()
            .starts_with("{\"n\":")
    );
}

#[test]
fn sends_another_coder_recorded_survive_a_store_but_forgotten_ones_dont() {
    let dir = tempfile::tempdir().unwrap();
    let mut other = Settings {
        on: true,
        chosen: true,
        ..Settings::default()
    };
    for session in ["theirs", "forgotten", "deleting", "kept"] {
        other.sent.insert(session.into(), "d".into());
    }
    other.store(dir.path()).unwrap();

    let mut mine = Settings {
        on: true,
        chosen: true,
        ..Settings::default()
    };
    mine.sent.insert("mine".into(), "m".into());
    mine.to_delete.insert("deleting".into());
    mine.kept_here.insert("kept".into());
    let forgot: std::collections::BTreeSet<String> = ["forgotten".to_owned()].into();
    mine.merge_sent(dir.path(), &forgot);
    mine.store(dir.path()).unwrap();
    let stored = Settings::load(dir.path());
    assert_eq!(
        stored.sent.keys().map(String::as_str).collect::<Vec<_>>(),
        ["mine", "theirs"]
    );
}

#[test]
fn a_stale_window_does_not_undo_another_windows_sync_off() {
    let dir = tempfile::tempdir().unwrap();
    // Window A turns sync on and keeps that copy.
    let mut a = Settings::default();
    a.store_choice(dir.path(), true).unwrap();
    a.store(dir.path()).unwrap();

    // Window B, open at the same time, runs /sync off.
    let mut b = Settings::load(dir.path());
    b.store_choice(dir.path(), false).unwrap();
    b.merge_choice(dir.path());
    b.store(dir.path()).unwrap();

    // Window A still holds on, and saves after sending a chat.
    assert!(a.on);
    a.sent.insert("chat".into(), "d".into());
    a.merge_sent(dir.path(), &std::collections::BTreeSet::new());
    a.merge_choice(dir.path());
    a.store(dir.path()).unwrap();
    let stored = Settings::load(dir.path());
    assert!(!stored.on);
    assert!(stored.chosen);
    assert!(!a.on);
    assert_eq!(stored.sent.get("chat").map(String::as_str), Some("d"));

    // The latest explicit choice wins the other way too: A turns it back on,
    // and B's next save keeps it.
    a.store_choice(dir.path(), true).unwrap();
    a.merge_choice(dir.path());
    a.store(dir.path()).unwrap();
    assert!(!b.on);
    b.merge_choice(dir.path());
    b.store(dir.path()).unwrap();
    assert!(Settings::load(dir.path()).on);
}

/// #11541: a `sync.json` copied into another folder (a test's `--state`)
/// never turns sync on there; only the folder that turned it on sends.
#[test]
fn sync_on_stays_with_the_folder_that_turned_it_on() {
    let mine = tempfile::tempdir().unwrap();
    let copy = tempfile::tempdir().unwrap();
    let mut settings = Settings::default();
    settings.store_choice(mine.path(), true).unwrap();
    assert!(Settings::load(mine.path()).on);
    std::fs::copy(mine.path().join(FILE), copy.path().join(FILE)).unwrap();
    let copied = Settings::load(copy.path());
    assert!(!copied.on && !copied.sends("s1", "d1"));
    // The copy's folder may still turn sync on for itself.
    let mut own = copied.clone();
    own.store_choice(copy.path(), true).unwrap();
    assert!(Settings::load(copy.path()).on);
    // A file from before the folder was kept, outside the person's own
    // Coder folder, is off too.
    let legacy = tempfile::tempdir().unwrap();
    std::fs::write(legacy.path().join(FILE), br#"{"on": true, "chosen": true}"#).unwrap();
    assert!(!Settings::load(legacy.path()).on);
    assert!(Settings::load(legacy.path()).chosen);
}

/// #11541: the chat's repository goes with the upload but doesn't change
/// its digest, and a website that refuses it gets the chat without it.
#[test]
fn the_repository_goes_with_a_chat_and_an_older_site_gets_it_without() {
    let doc = document(json!([{"step_id": 1, "source": "user", "message": "Hello"}]));
    let plain = upload(&doc, "Studio", &Screen::shapes());
    let named = plain
        .clone()
        .in_repository(Some("OpenAgentsInc/openagents"));
    assert_eq!(named.digest, plain.digest);
    assert_eq!(named.body["repository"], "OpenAgentsInc/openagents");
    assert_eq!(plain.clone().in_repository(None), plain);

    let bodies = Arc::new(Mutex::new(Vec::<Value>::new()));
    let seen = bodies.clone();
    let (ready, address) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            // A website from before #11541: `{computer, title, messages}` only.
            let router = Router::new().route(
                "/v1/threads/synced/{session}",
                put(move |Json(body): Json<Value>| async move {
                    seen.lock().unwrap().push(body.clone());
                    if body.get("repository").is_some() {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(json!({"error": {"code": "invalid", "message": "Send {computer, title, messages}."}})),
                        );
                    }
                    (StatusCode::OK, Json(json!({"chat": "c", "changed": true})))
                }),
            );
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            ready.send(listener.local_addr().unwrap()).unwrap();
            axum::serve(listener, router).await.unwrap();
        });
    });
    let origin = format!("http://{}", address.recv().unwrap());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let http = client().unwrap();
    let answer = runtime.block_on(super::put(&http, &saved(&origin), "s1", &named));
    assert_eq!(answer, Answer::Done);
    // Later uploads in this process leave it out at once.
    let again = runtime.block_on(super::put(&http, &saved(&origin), "s2", &named));
    assert_eq!(again, Answer::Done);
    let bodies = bodies.lock().unwrap();
    let named_count = bodies
        .iter()
        .filter(|b| b.get("repository").is_some())
        .count();
    assert_eq!((bodies.len(), named_count), (3, 1), "{bodies:?}");
}

#[test]
fn a_website_that_knows_no_thinking_rows_gets_them_as_tool_rows() {
    let seen: Arc<Mutex<Vec<Vec<String>>>> = Arc::default();
    let kept = seen.clone();
    let (send, address) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                let router = Router::new().route(
                    "/v1/threads/synced/{session}",
                    put(move |Json(body): Json<Value>| {
                        let kept = kept.clone();
                        async move {
                            let roles: Vec<String> = body["messages"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|m| m["role"].as_str().unwrap().to_owned())
                                .collect();
                            let old = roles.iter().any(|role| role == THINKING);
                            kept.lock().unwrap().push(roles);
                            if old {
                                // The website from before #11543.
                                return (
                                    StatusCode::BAD_REQUEST,
                                    Json(json!({"error": {"code": "invalid",
                                    "message": "A message has an unknown role."}})),
                                );
                            }
                            (StatusCode::OK, Json(json!({"chat": "x", "changed": true})))
                        }
                    }),
                );
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                send.send(format!("http://{}", listener.local_addr().unwrap()))
                    .unwrap();
                axum::serve(listener, router).await.unwrap();
            });
    });
    let origin = address.recv().unwrap();
    let doc = document(json!([
        {"step_id": 1, "source": "user", "message": "Look"},
        {"step_id": 2, "source": "agent", "tool_calls": [
            {"tool_call_id": "c1", "function_name": "claude:thinking", "arguments": null},
        ], "observation": {"results": [
            {"source_call_id": "c1", "content": "{\"text\":\"Checking.\"}"},
        ]}},
    ]));
    let sent = upload(&doc, "Studio", &Screen::shapes());
    let answer = tokio::runtime::Runtime::new().unwrap().block_on(super::put(
        &client().unwrap(),
        &saved(&origin),
        "s1",
        &sent,
    ));
    assert_eq!(answer, Answer::Done);
    assert_eq!(
        *seen.lock().unwrap(),
        [vec!["user", "thinking"], vec!["user", "tool"]]
    );
}

/// #11552: Coder now keeps the model's thinking as each step's
/// `reasoning_content`; it syncs as a thinking row ahead of the reply,
/// as the older thinking calls did.
#[test]
fn reasoning_content_syncs_as_a_thinking_row() {
    let doc = document(json!([
        {"step_id": 1, "source": "user", "message": "Is 9991 prime?"},
        {"step_id": 2, "source": "agent", "message": "", "reasoning_content": "Checking 97 × 103."},
        {"step_id": 3, "source": "agent", "message": "No: 97 × 103.", "reasoning_content": "  "},
    ]));
    let sent = upload(&doc, "Studio", &Screen::shapes());
    let messages = sent.body["messages"].as_array().unwrap();
    let rows: Vec<(&str, &str)> = messages
        .iter()
        .map(|m| (m["role"].as_str().unwrap(), m["text"].as_str().unwrap()))
        .collect();
    assert_eq!(
        rows,
        [
            ("user", "Is 9991 prime?"),
            ("thinking", "Checking 97 × 103."),
            ("assistant", "No: 97 × 103."),
        ]
    );
}

/// #11598: a chat goes up with its own last activity, the time of its
/// last step, so the website never lists it by when it arrived.
#[test]
fn a_chat_carries_its_last_steps_time() {
    let doc = document(json!([
        {"step_id": 1, "source": "user", "message": "Hello", "timestamp": "2026-10-07T15:32:00.000Z"},
        {"step_id": 2, "source": "agent", "message": "Hi", "timestamp": "2026-10-07T15:33:10.500Z"},
    ]));
    let sent = upload(&doc, "Studio", &Screen::shapes());
    assert_eq!(sent.body["active_unix"], 1_791_387_190);
    // Without step times, the time it was last saved; without either, none.
    let mut saved_only = document(json!([{"step_id": 1, "source": "user", "message": "Hello"}]));
    saved_only["extra"] = json!({"updated_ms": 1_791_387_190_000_u64});
    assert_eq!(last_active(&saved_only), Some(1_791_387_190));
    let bare = document(json!([{"step_id": 1, "source": "user", "message": "Hello"}]));
    assert!(
        upload(&bare, "Studio", &Screen::shapes())
            .body
            .get("active_unix")
            .is_none()
    );
    // A new time alone is a new upload.
    assert_ne!(
        sent.digest,
        upload(&bare, "Studio", &Screen::shapes()).digest
    );
}

#[test]
fn test_run_marker_reads_the_env_value() {
    assert!(test_run_from(Some("1")));
    assert!(test_run_from(Some(" true ")));
    assert!(!test_run_from(Some("0")));
    assert!(!test_run_from(Some("")));
    assert!(!test_run_from(None));
    assert_eq!(TEST_HEADER, "x-openagents-test");
}

#[test]
fn a_marked_request_carries_the_test_header_only_in_a_test_run() {
    let http = reqwest::Client::new();
    let request = mark(http.get("http://127.0.0.1:9/x")).build().unwrap();
    assert_eq!(
        request.headers().get(TEST_HEADER).is_some(),
        test_run(),
        "the header follows {TEST_RUN_ENV}"
    );
}
