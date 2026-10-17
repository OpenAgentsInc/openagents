//! A fake Computers server whose commands really run in `sh` under a
//! temporary home, a temporary git repository, and fake session files.
use super::*;
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::State,
    http::Request,
    response::{IntoResponse, Response},
};
use base64::Engine as _;
use chat_home::Role;
use openagents_sdk::computers::{CreateOptions, RetryPolicy};
use openagents_sdk::{Client, Deployment, Session};
use serde_json::{Value, json};
use std::process::Command as Std;
use std::sync::{Arc, Mutex};

pub(crate) const ID: &str = "cmp_0123456789abcdef0123";
pub(crate) const CHAT: &str = "0b5c6c3e-4a0e-4c39-9a51-5a2d8f6e1c01";
pub(crate) const SESSION: &str = "5f1d2c3b-0000-4000-8000-00000000abcd";
const CLAUDE_SOURCE: &str = "00000000-0000-0000-0000-0000000000c1";
const CODEX_SOURCE: &str = "00000000-0000-0000-0000-0000000000c2";

#[derive(Default)]
pub(crate) struct Book {
    /// Every request after sign-in, as `label path`.
    pub(crate) seen: Vec<String>,
    pub(crate) created: Vec<String>,
    pub(crate) puts: Vec<String>,
    pub(crate) commands: BTreeMap<String, Value>,
    pub(crate) outputs: BTreeMap<String, (Vec<u8>, Vec<u8>)>,
    pub(crate) runs: usize,
    pub(crate) credentials: BTreeMap<String, Value>,
    /// Labels whose next request is applied but answered with a 503.
    pub(crate) lose: Vec<&'static str>,
    pub(crate) stopped: usize,
    pub(crate) deleted: usize,
}

pub(crate) struct Fake {
    pub(crate) home: PathBuf,
    pub(crate) shims: PathBuf,
    pub(crate) book: Mutex<Book>,
}

fn computer() -> Value {
    json!({"id":ID,"kind":"cloud","name":"Move test","size":"small","state":"awake",
        "desired":"running","online":true})
}

fn decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap();
            out.push(u8::from_str_radix(hex, 16).unwrap());
            i += 3;
        } else {
            out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
            i += 1;
        }
    }
    String::from_utf8(out).unwrap()
}

fn query(uri: &axum::http::Uri) -> BTreeMap<String, String> {
    uri.query()
        .unwrap_or("")
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (k.to_owned(), decode(v)))
        .collect()
}

fn command_json(id: &str, exit: i32) -> Value {
    json!({"id":id,"computer":ID,"state":if exit == 0 {"succeeded"} else {"failed"},
        "exit_code":exit,"digest":"a".repeat(64),"cancel_requested":false,
        "output_limit_bytes":65536,"output_expires_unix":2000000000,
        "stdout_truncated":false,"stderr_truncated":false})
}

async fn serve(State(fake): State<Arc<Fake>>, request: Request<Body>) -> Response {
    let (parts, body) = request.into_parts();
    let path = parts.uri.path().to_owned();
    if path == "/v1/account" {
        return axum::Json(json!({"account":{"id":"acct_fixture","label":"Test"},"workspaces":[]}))
            .into_response();
    }
    let method = parts.method.as_str().to_owned();
    let key = parts
        .headers
        .get("idempotency-key")
        .map(|v| v.to_str().unwrap().to_owned());
    let body = to_bytes(body, 8 * 1024 * 1024).await.unwrap().to_vec();
    let q = query(&parts.uri);
    let segments: Vec<&str> = path.trim_start_matches("/v1/").split('/').collect();
    let label: &'static str = match (method.as_str(), segments.as_slice()) {
        ("POST", ["computers"]) => "create",
        ("GET", ["computers", "capabilities"]) => "capabilities",
        ("GET", ["computers", _]) => "get",
        ("PUT", ["computers", _, "files"]) => "put",
        ("GET", ["computers", _, "files"]) => "read",
        ("POST", ["computers", _, "commands"]) => "command",
        ("GET", ["computers", _, "commands", _]) => "command_get",
        ("GET", ["computers", _, "commands", _, "output"]) => "output",
        ("POST", ["computers", _, "credential-references"]) => "credential",
        ("POST", ["computers", _, "stop"]) => "stop",
        ("DELETE", ["computers", _]) => "delete",
        ("POST", ["computers", _, "commands", _, "cancel"]) => "cancel",
        _ => panic!("unexpected request {method} {path}"),
    };
    fake.book
        .lock()
        .unwrap()
        .seen
        .push(format!("{label} {path}"));
    let (status, reply): (u16, Response) = match label {
        "create" => {
            fake.book.lock().unwrap().created.push(key.unwrap());
            (202, axum::Json(computer()).into_response())
        }
        "capabilities" => (
            200,
            axum::Json(
                json!({"schema":"openagents.computers.capabilities.v1","profiles":["core"],
                "sizes":["small"],"limits":{"max_computers":20},
                "files":{"max_bytes":4194304,"max_path_bytes":1024,"max_concurrent_transfers":2,
                    "requires_awake":true,"timeout_seconds":30}}),
            )
            .into_response(),
        ),
        "get" => (200, axum::Json(computer()).into_response()),
        "stop" => {
            fake.book.lock().unwrap().stopped += 1;
            let mut stopped = computer();
            stopped["state"] = json!("stopped");
            stopped["desired"] = json!("stopped");
            (200, axum::Json(stopped).into_response())
        }
        "delete" => {
            fake.book.lock().unwrap().deleted += 1;
            (204, Body::empty().into_response())
        }
        "cancel" => {
            let book = fake.book.lock().unwrap();
            let value = book
                .commands
                .values()
                .find(|v| v["id"] == segments[3])
                .cloned()
                .unwrap();
            (202, axum::Json(value).into_response())
        }
        "put" => {
            let rel = &q["path"];
            assert!(body.len() <= PIECE_BYTES, "piece over 4 MiB");
            let file = fake.home.join(rel);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(&file, &body).unwrap();
            fake.book.lock().unwrap().puts.push(rel.clone());
            let sha = format!("{:x}", Sha256::digest(&body));
            (
                200,
                axum::Json(json!({"size":body.len(),"sha256":sha})).into_response(),
            )
        }
        "read" => match std::fs::read(fake.home.join(&q["path"])) {
            Ok(bytes) => {
                let sha = format!("{:x}", Sha256::digest(&bytes));
                let reply = Response::builder()
                    .status(200)
                    .header("x-content-sha256", &sha)
                    .header("etag", format!("\"{sha}\""))
                    .header("cache-control", "no-store")
                    .body(Body::from(bytes))
                    .unwrap();
                (200, reply)
            }
            Err(_) => (
                404,
                axum::Json(json!({"error":{"code":"not_found","message":"missing"}}))
                    .into_response(),
            ),
        },
        "command" => {
            let key = key.unwrap();
            let existing = fake.book.lock().unwrap().commands.get(&key).cloned();
            let value = match existing {
                Some(value) => value,
                None => {
                    let options: Value = serde_json::from_slice(&body).unwrap();
                    let mut cmd = tokio::process::Command::new("sh");
                    cmd.arg("-c")
                        .arg(options["command"].as_str().unwrap())
                        .current_dir(&fake.home)
                        .env("HOME", &fake.home)
                        .env(
                            "PATH",
                            format!(
                                "{}:{}",
                                fake.shims.display(),
                                std::env::var("PATH").unwrap_or_default()
                            ),
                        )
                        .env("GIT_CONFIG_NOSYSTEM", "1");
                    if let Some(env) = options["env"].as_object() {
                        for (k, v) in env {
                            cmd.env(k, v.as_str().unwrap());
                        }
                    }
                    let out = cmd.output().await.unwrap();
                    let mut book = fake.book.lock().unwrap();
                    book.runs += 1;
                    let id = format!("trn_{:020x}", book.runs);
                    let value = command_json(&id, out.status.code().unwrap_or(1));
                    book.outputs.insert(id, (out.stdout, out.stderr));
                    book.commands.insert(key, value.clone());
                    value
                }
            };
            (202, axum::Json(value).into_response())
        }
        "command_get" => {
            let book = fake.book.lock().unwrap();
            let value = book
                .commands
                .values()
                .find(|v| v["id"] == segments[3])
                .cloned()
                .unwrap();
            (200, axum::Json(value).into_response())
        }
        "output" => {
            let book = fake.book.lock().unwrap();
            let value = book
                .commands
                .values()
                .find(|v| v["id"] == segments[3])
                .cloned()
                .unwrap();
            let (stdout, stderr) = &book.outputs[segments[3]];
            let from_out: usize = q.get("stdout").map_or(0, |v| v.parse().unwrap());
            let from_err: usize = q.get("stderr").map_or(0, |v| v.parse().unwrap());
            let b64 = base64::engine::general_purpose::STANDARD;
            (
                200,
                axum::Json(json!({"command":value,"encoding":"base64",
                    "gaps":{"stdout":false,"stderr":false},"has_more":false,
                    "next_cursor":{"stdout":stdout.len(),"stderr":stderr.len()},
                    "stdout":b64.encode(&stdout[from_out..]),"stderr":b64.encode(&stderr[from_err..]),
                    "terminal":true}))
                .into_response(),
            )
        }
        "credential" => {
            let request: Value = serde_json::from_slice(&body).unwrap();
            let mut book = fake.book.lock().unwrap();
            let n = book.credentials.len() + 1;
            let row = book
                .credentials
                .entry(key.unwrap())
                .or_insert_with(|| {
                    json!({"id":format!("00000000-0000-4000-8000-{n:012x}"),"computer":ID,
                        "source":request["source"],"rule":request["rule"],
                        "command_key":request["command_key"],"state":"prepared",
                        "created_unix":1,"expires_unix":3601,
                        "egress_profile":format!("model-v1@sha256:{}","a".repeat(64))})
                })
                .clone();
            (201, axum::Json(row).into_response())
        }
        _ => unreachable!(),
    };
    let mut book = fake.book.lock().unwrap();
    if let Some(i) = book.lose.iter().position(|l| *l == label) {
        book.lose.remove(i);
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(json!({"error":{"code":"unavailable","message":"lost"}})),
        )
            .into_response();
    }
    let mut reply = reply;
    *reply.status_mut() = axum::http::StatusCode::from_u16(status).unwrap();
    reply
}

pub(crate) struct World {
    pub(crate) _tmp: tempfile::TempDir,
    pub(crate) project: PathBuf,
    pub(crate) roots: LocalRoots,
    pub(crate) store: MoveStore,
    pub(crate) fake: Arc<Fake>,
    pub(crate) client: Client,
    pub(crate) session: Session,
    pub(crate) task: tokio::task::JoinHandle<()>,
}
impl Drop for World {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub(crate) fn sh(dir: &Path, script: &str) -> String {
    let out = Std::new("sh")
        .arg("-c")
        .arg(script)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{script}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Incompressible bytes, so the bundle really needs several pieces.
fn noise(len: usize) -> Vec<u8> {
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect()
}

pub(crate) const GIT: &str =
    "git -c user.name=t -c user.email=t@example.com -c commit.gpgsign=false";

pub(crate) async fn world(big: usize) -> World {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().canonicalize().unwrap();
    let project = base.join("work/proj");
    std::fs::create_dir_all(&project).unwrap();
    sh(
        &project,
        &format!(
            "git init -q -b main && printf 'one\\n' > tracked.txt && printf '*.log\\n' > .gitignore \
             && {GIT} add -A && {GIT} commit -q -m first && printf 'two\\n' >> tracked.txt \
             && printf 'new\\n' > new.txt && printf 'secret\\n' > ignored.log"
        ),
    );
    if big > 0 {
        std::fs::write(project.join("big.bin"), noise(big)).unwrap();
    }
    let roots = LocalRoots {
        claude: base.join("claude"),
        codex: base.join("codex"),
    };
    let claude_dir = roots.claude.join("projects/-work-proj");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(
        claude_dir.join(format!("{SESSION}.jsonl")),
        b"{\"type\":\"user\",\"message\":\"fix the bug\"}\n",
    )
    .unwrap();
    let codex_dir = roots.codex.join("sessions/2026/10/17");
    std::fs::create_dir_all(&codex_dir).unwrap();
    std::fs::write(
        codex_dir.join(format!("rollout-2026-10-17T00-00-00-{SESSION}.jsonl")),
        b"{\"type\":\"session_meta\"}\n",
    )
    .unwrap();
    let home = base.join("computer-home");
    let shims = base.join("shims");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&shims).unwrap();
    for (name, var) in [
        ("claude", "CLAUDE_CODE_OAUTH_TOKEN"),
        ("codex", "CODEX_API_KEY"),
    ] {
        let shim = shims.join(name);
        std::fs::write(
            &shim,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$HOME/{name}.args\"\nprintf '%s' \"${var}\" > \"$HOME/{name}.login\"\npwd > \"$HOME/{name}.pwd\"\n"
            ),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let fake = Arc::new(Fake {
        home,
        shims,
        book: Mutex::new(Book::default()),
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let router = Router::new().fallback(serve).with_state(fake.clone());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let deployment = Deployment::new(&format!("{origin}/v1"), &origin).unwrap();
    let client = Client::new(deployment).unwrap();
    let session = client
        .authenticate(&format!("sess_{}", "a".repeat(64)))
        .await
        .unwrap();
    World {
        store: MoveStore::open(base.join("moves")).unwrap(),
        _tmp: tmp,
        project,
        roots,
        fake,
        client,
        session,
        task,
    }
}

pub(crate) fn chat(w: &World, harness: HarnessId) -> ChatToMove {
    ChatToMove {
        chat: CHAT.into(),
        harness,
        project: w.project.clone(),
        session: SESSION.into(),
        seed: Seed::new(
            "Fix the bug",
            [
                (Role::User, "Fix the failing test in tracked.txt"),
                (Role::Assistant, "I found it and started the fix."),
            ],
        ),
    }
}

pub(crate) fn logins() -> CloudLogins {
    CloudLogins {
        claude_code: Some(LoginSource {
            source: CLAUDE_SOURCE.into(),
            rule: "messages".into(),
        }),
        codex: Some(LoginSource {
            source: CODEX_SOURCE.into(),
            rule: "responses".into(),
        }),
        git: None,
    }
}

fn status(dir: &Path) -> String {
    sh(dir, "git status --porcelain=v1 --untracked-files=all")
}

#[test]
fn pieces_are_at_most_four_mebibytes() {
    let mib = 1024 * 1024;
    assert_eq!(piece_ranges(0), vec![(0, 0)]);
    assert_eq!(piece_ranges(4 * mib), vec![(0, 4 * mib)]);
    assert_eq!(
        piece_ranges(9 * mib),
        vec![(0, 4 * mib), (4 * mib, 4 * mib), (8 * mib, mib)]
    );
    assert!(
        piece_ranges(123_456_789)
            .iter()
            .all(|(_, len)| *len <= PIECE_BYTES as u64)
    );
}

#[test]
fn restore_keeps_the_path_only_when_the_computer_can_hold_it() {
    let root = Path::new("/Users/me/code/app");
    assert_eq!(
        decide_restore(CHAT, root, &PathState::Available, "/home/oa"),
        RestoreTarget::SamePath
    );
    for state in [PathState::Occupied, PathState::Unavailable] {
        assert_eq!(
            decide_restore(CHAT, root, &state, "/home/oa/"),
            RestoreTarget::Elsewhere {
                root: format!("/home/oa/openagents/moved/{CHAT}/app")
            }
        );
    }
}

#[test]
fn reasons_are_plain_words() {
    for reason in all_reasons() {
        assert!(oa_copy::violations(&reason, &[]).is_empty(), "{reason}");
    }
}

#[tokio::test]
async fn capture_takes_untracked_files_and_leaves_the_index_alone() {
    let w = world(0).await;
    let before = status(&w.project);
    let index_before = std::fs::read(w.project.join(".git/index")).unwrap();
    let blobs = w.store.blobs(CHAT);
    std::fs::create_dir_all(&blobs).unwrap();
    let git_ref = format!("refs/openagents/move/{CHAT}");
    let index = blobs.join("index");
    let (root, commit, _, parent, branch) =
        capture_tree(&w.project, &git_ref, CHAT, 1_760_000_000, &index)
            .await
            .unwrap();
    assert_eq!(root, w.project);
    assert_eq!(branch.as_deref(), Some("main"));
    assert!(parent.is_some());
    let files = sh(&w.project, &format!("git ls-tree -r --name-only {git_ref}"));
    assert!(files.contains("new.txt"), "{files}");
    assert!(files.contains("tracked.txt"));
    assert!(!files.contains("ignored.log"));
    assert_eq!(
        sh(&w.project, &format!("git show {git_ref}:tracked.txt")),
        "one\ntwo\n"
    );
    assert_eq!(status(&w.project), before);
    assert_eq!(
        std::fs::read(w.project.join(".git/index")).unwrap(),
        index_before
    );
    // The same tree captured again makes the same commit.
    let (_, again, ..) = capture_tree(&w.project, &git_ref, CHAT, 1_760_000_000, &index)
        .await
        .unwrap();
    assert_eq!(again, commit);
}

#[tokio::test]
async fn a_missing_login_keeps_the_chat_local_and_sends_nothing() {
    let w = world(0).await;
    let computers = w
        .client
        .computers(&w.session)
        .unwrap()
        .with_retries(RetryPolicy::none())
        .unwrap();
    w.store
        .plan(
            chat(&w, HarnessId::Codex),
            Destination::Existing {
                computer: ID.into(),
            },
            1_760_000_000,
        )
        .unwrap();
    let logins = CloudLogins {
        codex: None,
        ..logins()
    };
    let mover = Mover {
        store: &w.store,
        computers: &computers,
        logins: &logins,
        roots: &w.roots,
        cancel: CancellationToken::new(),
    };
    let outcome = mover.run(CHAT).await.unwrap();
    let Outcome::StaysLocal { reason } = &outcome else {
        panic!("moved without a login");
    };
    assert!(reason.contains("Codex isn't signed in"), "{reason}");
    assert_eq!(mover.run(CHAT).await.unwrap(), outcome);
    assert!(w.fake.book.lock().unwrap().seen.is_empty());
    let record = w.store.load(CHAT).unwrap().unwrap();
    assert!(record.capture.is_none());
    assert!(
        sh(&w.project, "git for-each-ref refs/openagents").is_empty(),
        "nothing captured"
    );
}

#[tokio::test]
async fn same_path_restore_resumes_claude_code_natively_from_a_bundle() {
    let w = world(9 * 1024 * 1024 + 17).await;
    let before = status(&w.project);
    let computers = w
        .client
        .computers(&w.session)
        .unwrap()
        .with_retries(RetryPolicy::none())
        .unwrap();
    w.store
        .plan(
            chat(&w, HarnessId::ClaudeCode),
            Destination::Existing {
                computer: ID.into(),
            },
            1_760_000_000,
        )
        .unwrap();
    let logins = logins();
    let mover = Mover {
        store: &w.store,
        computers: &computers,
        logins: &logins,
        roots: &w.roots,
        cancel: CancellationToken::new(),
    };
    assert!(mover.capture(CHAT).await.unwrap().is_none());
    let big = std::fs::read(w.project.join("big.bin")).unwrap();
    // The cloud computer is another machine: nothing is at the path there.
    std::fs::remove_dir_all(&w.project).unwrap();
    let outcome = mover.run(CHAT).await.unwrap();
    assert!(
        matches!(
            &outcome,
            Outcome::Resumed {
                target: RestoreTarget::SamePath,
                ..
            }
        ),
        "{outcome:?}"
    );
    // Files, the untracked ones included, and the git state are back.
    assert_eq!(std::fs::read(w.project.join("big.bin")).unwrap(), big);
    assert_eq!(
        std::fs::read_to_string(w.project.join("tracked.txt")).unwrap(),
        "one\ntwo\n"
    );
    assert!(!w.project.join("ignored.log").exists());
    assert_eq!(status(&w.project), before);
    assert_eq!(sh(&w.project, "git symbolic-ref --short HEAD"), "main\n");
    // The session file is where Claude Code looks for it.
    let home = &w.fake.home;
    assert_eq!(
        std::fs::read(home.join(format!(".claude/projects/-work-proj/{SESSION}.jsonl"))).unwrap(),
        b"{\"type\":\"user\",\"message\":\"fix the bug\"}\n"
    );
    let args = std::fs::read_to_string(home.join("claude.args")).unwrap();
    assert!(
        args.starts_with(&format!("--resume\n{SESSION}\n-p\n{CONTINUE_PROMPT}\n")),
        "{args}"
    );
    let login = std::fs::read_to_string(home.join("claude.login")).unwrap();
    assert!(
        login.starts_with("oa-ref:"),
        "login goes by reference: {login}"
    );
    let book = w.fake.book.lock().unwrap();
    let bundle_puts = book
        .puts
        .iter()
        .filter(|p| p.contains("workspace.bundle."))
        .count();
    assert_eq!(bundle_puts, 3);
    assert_eq!(
        book.puts
            .iter()
            .filter(|p| p.contains("session.jsonl."))
            .count(),
        1
    );
    assert!(!book.puts.iter().any(|p| p.contains("prompt.txt")));
}

#[tokio::test]
async fn an_occupied_path_continues_codex_from_the_seed_after_a_push() {
    let w = world(0).await;
    let remote = w.project.parent().unwrap().join("remote.git");
    sh(
        w.project.parent().unwrap(),
        &format!(
            "git init -q --bare remote.git && cd proj && git remote add origin {} && git push -q origin main",
            remote.display()
        ),
    );
    let computers = w
        .client
        .computers(&w.session)
        .unwrap()
        .with_retries(RetryPolicy::none())
        .unwrap();
    w.store
        .plan(
            chat(&w, HarnessId::Codex),
            Destination::Existing {
                computer: ID.into(),
            },
            1_760_000_000,
        )
        .unwrap();
    let logins = logins();
    let mover = Mover {
        store: &w.store,
        computers: &computers,
        logins: &logins,
        roots: &w.roots,
        cancel: CancellationToken::new(),
    };
    // The local project is still at the path, so the computer can't use it.
    let outcome = mover.run(CHAT).await.unwrap();
    let elsewhere = w.fake.home.join(format!("openagents/moved/{CHAT}/proj"));
    assert_eq!(
        outcome,
        Outcome::Resumed {
            target: RestoreTarget::Elsewhere {
                root: elsewhere.to_string_lossy().into_owned()
            },
            command: match &outcome {
                Outcome::Resumed { command, .. } => command.clone(),
                _ => unreachable!(),
            },
        }
    );
    let record = w.store.load(CHAT).unwrap().unwrap();
    assert!(matches!(
        record.capture.as_ref().unwrap().workspace,
        Workspace::Pushed { .. }
    ));
    assert!(
        !sh(
            &remote,
            &format!("git for-each-ref refs/openagents/move/{CHAT}")
        )
        .is_empty()
    );
    assert_eq!(
        std::fs::read_to_string(elsewhere.join("new.txt")).unwrap(),
        "new\n"
    );
    let home = &w.fake.home;
    let args = std::fs::read_to_string(home.join("codex.args")).unwrap();
    assert!(
        args.starts_with("exec\n--json\n--dangerously-bypass-approvals-and-sandbox\n"),
        "{args}"
    );
    assert!(!args.contains("resume\n"), "{args}");
    assert!(
        args.contains("Fix the failing test"),
        "the seed rides the prompt: {args}"
    );
    assert!(args.contains(CONTINUE_PROMPT));
    assert!(
        std::fs::read_to_string(home.join("codex.login"))
            .unwrap()
            .starts_with("oa-ref:")
    );
    assert_eq!(
        std::fs::read_to_string(home.join("codex.pwd"))
            .unwrap()
            .trim(),
        elsewhere.canonicalize().unwrap().to_string_lossy()
    );
    let book = w.fake.book.lock().unwrap();
    assert!(
        !book
            .puts
            .iter()
            .any(|p| p.contains("bundle") || p.contains("session"))
    );
    assert_eq!(
        book.puts
            .iter()
            .filter(|p| p.contains("prompt.txt"))
            .count(),
        1
    );
}

#[tokio::test]
async fn crash_and_retry_makes_one_computer_and_one_copy() {
    let w = world(5 * 1024 * 1024).await;
    let computers = w
        .client
        .computers(&w.session)
        .unwrap()
        .with_retries(RetryPolicy::none())
        .unwrap();
    let request = computers.create_request(CreateOptions::default()).unwrap();
    w.store
        .plan(
            chat(&w, HarnessId::ClaudeCode),
            Destination::New { request },
            1_760_000_000,
        )
        .unwrap();
    // Each of these is applied, then its reply is lost.
    w.fake.book.lock().unwrap().lose = vec!["create", "put", "command", "credential"];
    let logins = logins();
    let mut failures = 0;
    let outcome = loop {
        // A fresh mover each time, as after a restart.
        let mover = Mover {
            store: &w.store,
            computers: &computers,
            logins: &logins,
            roots: &w.roots,
            cancel: CancellationToken::new(),
        };
        match mover.run(CHAT).await {
            Ok(outcome) => break outcome,
            Err(_) => {
                failures += 1;
                assert!(failures < 8, "never finished");
            }
        }
    };
    assert_eq!(failures, 4);
    assert!(matches!(outcome, Outcome::Resumed { .. }), "{outcome:?}");
    let book = w.fake.book.lock().unwrap();
    let keys: std::collections::BTreeSet<_> = book.created.iter().collect();
    assert_eq!(book.created.len(), 2, "one retry of the lost create");
    assert_eq!(keys.len(), 1, "one computer");
    let mut puts = book.puts.clone();
    puts.sort();
    let total = puts.len();
    puts.dedup();
    assert_eq!(total, puts.len(), "every piece written once: {puts:?}");
    assert_eq!(book.commands.len(), 3, "probe, restore, and resume");
    assert_eq!(book.runs, 3, "no command ran twice");
    assert_eq!(book.credentials.len(), 1, "one login reference");
    // Running a finished move again sends nothing.
    drop(book);
    let seen = w.fake.book.lock().unwrap().seen.len();
    let mover = Mover {
        store: &w.store,
        computers: &computers,
        logins: &logins,
        roots: &w.roots,
        cancel: CancellationToken::new(),
    };
    assert_eq!(mover.run(CHAT).await.unwrap(), outcome);
    assert_eq!(w.fake.book.lock().unwrap().seen.len(), seen);
}
