//! Shared pieces for the runner tests: a fake Open Responses door, the
//! fake agent binary, a fixture extension, and a small suite.

#![allow(dead_code)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use ext_eval::arms::{AgentPin, Program, Skill, Subject};
use ext_eval::artifact::{ArtifactRef, JSON};
use ext_eval::case::Grant;
use ext_eval::proxy::Secret;
use ext_eval::run::{Author, Door, Options};
use ext_eval::{LoadOptions, Suite};
use serde_json::{Value, json};

/// The suite author and evaluator, fixed test keys.
pub const AUTHOR: &str = "5be6446aef0a9a6b1f2c3d4e5f6071829a4b5c6d7e8f90112233445566778899";
/// The answer the door gives when the skill is in the instructions.
pub const RIGHT: &str = "The callers of parse_case are load and parse_all.";
/// The line the skill carries; the fake door looks for it.
pub const MARKER: &str = "SKILL-MARKER";

/// A fake Open Responses door on a loopback port. It answers only the
/// real key, and it answers the callers question right only when the
/// instructions carry the skill.
pub struct FakeDoor {
    pub url: String,
    pub key: String,
    pub requests: Arc<AtomicU64>,
    _stop: tokio::sync::oneshot::Sender<()>,
    _thread: std::thread::JoinHandle<()>,
}

pub fn fake_door() -> FakeDoor {
    use axum::Json;
    use axum::http::{HeaderMap, StatusCode};
    let key = format!("real-door-key-{}", std::process::id());
    let requests = Arc::new(AtomicU64::new(0));
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let expected = format!("Bearer {key}");
    let counter = Arc::clone(&requests);
    let thread = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            let app = axum::Router::new().route(
                "/v1/responses",
                axum::routing::post(move |headers: HeaderMap, Json(body): Json<Value>| {
                    let expected = expected.clone();
                    let counter = Arc::clone(&counter);
                    async move {
                        counter.fetch_add(1, Ordering::SeqCst);
                        let auth = headers
                            .get("authorization")
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or_default();
                        if auth != expected {
                            return (StatusCode::UNAUTHORIZED, Json(json!({"error": "key"})));
                        }
                        let instructions = body["instructions"].as_str().unwrap_or_default();
                        let prompt = body["input"].to_string();
                        let text = if prompt.contains("callers") {
                            if instructions.contains(MARKER) {
                                RIGHT
                            } else {
                                "I would have to look."
                            }
                        } else if prompt.contains("hello") {
                            "Hello."
                        } else {
                            "PASS"
                        };
                        (
                            StatusCode::OK,
                            Json(json!({"output": [{"type": "message", "content": [
                                {"type": "output_text", "text": text}
                            ]}]})),
                        )
                    }
                }),
            );
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = stopped.await;
                })
                .await;
        });
    });
    FakeDoor {
        url,
        key,
        requests,
        _stop: stop,
        _thread: thread,
    }
}

impl FakeDoor {
    pub fn door(&self) -> Door {
        Door {
            name: "fake".into(),
            url: self.url.clone(),
            key: Secret::new(self.key.clone()),
            model: "fake/model".into(),
        }
    }
}

/// The fake agent binary, built once per test process so a filtered
/// `cargo test --test runner` never runs a stale one.
pub fn agent() -> AgentPin {
    static BUILT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    let path = BUILT.get_or_init(|| {
        let exe = std::env::current_exe().unwrap();
        let profile = exe.parent().unwrap().parent().unwrap().to_path_buf();
        let target = profile.parent().unwrap();
        let status = std::process::Command::new(env!("CARGO"))
            .args([
                "build",
                "--quiet",
                "-p",
                "ext-eval",
                "--example",
                "fake_agent",
            ])
            .env("CARGO_TARGET_DIR", target)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .status()
            .unwrap();
        assert!(status.success(), "the fake agent builds");
        profile.join("examples").join("fake_agent")
    });
    AgentPin::of(path.clone()).unwrap()
}

/// The fixture extension: one program with a Wasm-less module step and
/// one skill.
pub fn subject() -> Subject {
    let program = json!({
        "definition": {
            "v": 1,
            "id": format!("{AUTHOR}:map-it/map-it"),
            "steps": [{"name": "repo_map", "kind": "module"}],
        },
        "binding": {"steps": {"repo_map": {"module": {"operation": "map"}}}},
    });
    let record = json!({"v": 1, "slug": "map-it", "program": {"name": "map-it"}});
    let record_bytes = serde_json::to_vec(&record).unwrap();
    Subject {
        slug: "map-it".into(),
        definition: json!({
            "id": format!("{AUTHOR}:map-it/map-it"),
            "artifact": ArtifactRef::of(&record_bytes, JSON, Some("openagents.coder-package.v1")).value(),
        }),
        package_lock: json!({"v": 1, "slug": "map-it"}),
        programs: vec![Program {
            slug: "map-it".into(),
            bytes: serde_json::to_vec(&program).unwrap(),
        }],
        skills: vec![Skill {
            name: "callers".into(),
            bytes: format!("{MARKER}: name the callers from the map.\n").into_bytes(),
        }],
    }
}

pub fn author() -> Author {
    Author {
        author: AUTHOR.into(),
        package: "map-it-tests".into(),
        component: "suite".into(),
        evaluator: AUTHOR.into(),
        suite_release: None,
        requester: None,
    }
}

/// Writes a case under `dir/evals/<name>/`.
pub fn case(dir: &Path, name: &str, prompt: &str, graders: &[(&str, &str)]) {
    let case = dir.join("evals").join(name);
    std::fs::create_dir_all(case.join("graders")).unwrap();
    std::fs::write(case.join("prompt.md"), prompt).unwrap();
    for (file, text) in graders {
        std::fs::write(case.join("graders").join(file), text).unwrap();
    }
}

/// The two-case suite: the callers question (should fire) and a greeting
/// (should not).
pub fn suite(dir: &Path, runs: u32, extra: &str) -> Suite {
    case(
        dir,
        "find-callers",
        &format!(
            "+++\nv = \"openagents.eval-case.v1\"\nkind = \"should-fire\"\nruns = {runs}\n{extra}+++\n\nWho are the callers of parse_case?\n"
        ),
        &[
            (
                "answer.md",
                "+++\ntype = \"regex\"\ntarget = \"last_message\"\n+++\n\nload and parse_all\n",
            ),
            (
                "used-map.md",
                "+++\ntype = \"operation_used\"\noperation = \"repo_map\"\n+++\n",
            ),
        ],
    );
    case(
        dir,
        "greet",
        &format!(
            "+++\nv = \"openagents.eval-case.v1\"\nkind = \"should-not-fire\"\nruns = {runs}\n+++\n\nSay hello.\n"
        ),
        &[(
            "polite.md",
            "+++\ntype = \"regex\"\ntarget = \"last_message\"\n+++\n\nHello\n",
        )],
    );
    Suite::load(&dir.join("evals"), LoadOptions::default()).unwrap()
}

pub fn options(temp: &Path) -> Options {
    Options {
        runs: None,
        baseline: true,
        concurrency: 2,
        keep_temp: false,
        grants: BTreeSet::from([Grant::Read]),
        temp_root: temp.to_path_buf(),
        backend: None,
    }
}

/// Every file under `dir`, recursively.
pub fn files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out
}
