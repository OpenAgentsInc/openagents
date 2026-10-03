//! `openagents ext eval` as a process: its help, a stop by `SIGINT`, and
//! a result published to a local relay and checked by a second trainer.
//!
//! The agent is `ext-eval`'s fake agent and the door a fake Open
//! Responses server, so nothing here reaches a model. The relay test needs
//! a disposable Postgres: set `NOSTR_RELAY_TEST_DATABASE_URL` (and
//! `NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE=1`), as the relay's own suites do.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const OPENAGENTS: &str = env!("CARGO_BIN_EXE_openagents");
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../ext-eval/fixtures/repo-map-brief"
);

/// The fake agent, built once per test process.
fn fake_agent() -> PathBuf {
    static BUILT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    BUILT
        .get_or_init(|| {
            let profile = Path::new(OPENAGENTS).parent().unwrap().to_path_buf();
            let target = profile.parent().unwrap();
            let status = Command::new(env!("CARGO"))
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
        })
        .clone()
}

/// A copy of the fixture extension a test may run and write results into.
fn extension(dir: &Path) -> PathBuf {
    let root = dir.join("repo-map-brief");
    copy(Path::new(FIXTURE), &root);
    root
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let path = entry.path();
        let target = to.join(entry.file_name());
        if path.is_dir() {
            if entry.file_name() != "results" {
                copy(&path, &target);
            }
        } else {
            std::fs::copy(&path, &target).unwrap();
        }
    }
}

/// A fake Open Responses door on a loopback port: the overview answer
/// only when the skill is in the instructions, a greeting for a greeting.
struct Door {
    url: String,
    key: String,
}

fn door() -> Door {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let key = format!("door-key-{}", std::process::id());
    let expected = format!("bearer {key}");
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let expected = expected.clone();
            std::thread::spawn(move || answer(stream, &expected));
        }
    });
    Door { url, key }
}

fn answer(mut stream: std::net::TcpStream, expected: &str) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut length = 0;
    let mut authorized = false;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        let lower = line.trim().to_ascii_lowercase();
        if lower.is_empty() {
            break;
        }
        if let Some(value) = lower.strip_prefix("content-length:") {
            length = value.trim().parse().unwrap_or(0);
        }
        if let Some(value) = lower.strip_prefix("authorization:") {
            authorized = value.trim() == expected;
        }
    }
    let mut body = vec![0; length];
    let _ = reader.read_exact(&mut body);
    let request: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let prompt = request["input"].to_string().to_ascii_lowercase();
    let skilled = request["instructions"]
        .as_str()
        .unwrap_or_default()
        .contains("name its main language");
    let text = if prompt.contains("repository") {
        if skilled {
            "It is a Rust crate of six files; the largest is src/tables.rs."
        } else {
            "I would have to look at the files."
        }
    } else {
        "Welcome aboard, glad you are here!"
    };
    let (status, body) = if authorized {
        (
            "200 OK",
            json!({"output": [{"type": "message", "content": [{"type": "output_text", "text": text}]}]})
                .to_string(),
        )
    } else {
        ("401 Unauthorized", "{}".to_string())
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
}

/// `openagents` with a clean identity, trust store, and door.
fn openagents(home: &Path, profile_dir: &Path, door: &Door) -> Command {
    let mut command = Command::new(OPENAGENTS);
    command
        .env("VERSE_HOME", profile_dir)
        .env("OPENAGENTS_HOME", home)
        .env("CODER_DOOR_URL", &door.url)
        .env("CODER_DOOR_KEY", &door.key)
        .env("CODER_MODEL", "fake/model")
        .env_remove("TYPESAFE_API_KEY")
        .env_remove("CODER_AI_GATEWAY_KEY");
    command
}

#[test]
fn the_help_names_every_quick_start_command() {
    let out = Command::new(OPENAGENTS)
        .args(["ext", "eval", "--help"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let help = String::from_utf8(out.stdout).unwrap();
    // docs/extensions/evaluation.md, Quick start.
    for form in [
        "init [TARGET] [--bare]",
        "run TARGET [--runs N] [--case GLOB]...",
        "publish REPORT",
        "check EVENT [TARGET]",
        "--json",
        "130 or 143 on a signal",
    ] {
        assert!(help.contains(form), "{form} is missing from:\n{help}");
    }
    // `plugin test` is the name; `ext eval` stays a working alias.
    let plugin = Command::new(OPENAGENTS)
        .args(["plugin", "test", "--help"])
        .output()
        .unwrap();
    assert!(plugin.status.success());
    assert_eq!(String::from_utf8(plugin.stdout).unwrap(), help);
    for group in ["plugin", "ext"] {
        let out = Command::new(OPENAGENTS)
            .args([group, "--help"])
            .output()
            .unwrap();
        let out = String::from_utf8(out.stdout).unwrap();
        assert!(out.contains("test run TARGET"), "{group}: {out}");
        assert!(out.contains("usage: openagents plugin"), "{group}: {out}");
    }
}

#[test]
fn init_bare_writes_the_template_and_a_run_refuses_its_todo() {
    let work = tempfile::tempdir().unwrap();
    let root = extension(work.path());
    let door = door();
    let status = openagents(&work.path().join("home"), &work.path().join("verse"), &door)
        .args(["plugin", "test", "init", "smoke", "--bare"])
        .current_dir(&root)
        .status()
        .unwrap();
    assert!(status.success());
    let prompt = std::fs::read_to_string(root.join("evals/smoke/prompt.md")).unwrap();
    assert!(prompt.contains("TODO: describe a task someone would give Coder"));
    assert!(root.join("evals/smoke/graders/criteria.md").is_file());
    let out = openagents(&work.path().join("home"), &work.path().join("verse"), &door)
        .args(["ext", "eval", "run", ".", "--trust", "--coder"])
        .arg(fake_agent())
        .current_dir(&root)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("TODO"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn an_untrusted_directory_without_a_terminal_refuses() {
    let work = tempfile::tempdir().unwrap();
    let root = extension(work.path());
    let door = door();
    let out = openagents(&work.path().join("home"), &work.path().join("verse"), &door)
        .args(["ext", "eval", "run"])
        .arg(&root)
        .arg("--coder")
        .arg(fake_agent())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("not trusted"));
}

#[test]
fn sigint_stops_live_children_and_exits_130() {
    let work = tempfile::tempdir().unwrap();
    let root = extension(work.path());
    std::fs::write(
        root.join("evals/overview/prompt.md"),
        "+++\nv = \"openagents.eval-case.v1\"\nruns = 1\n[run]\nenv = { OA_EVAL_FAKE = \"sleep\" }\n+++\n\nGive me an overview of the repository.\n",
    )
    .unwrap();
    let door = door();
    let mut child = openagents(&work.path().join("home"), &work.path().join("verse"), &door)
        .args(["ext", "eval", "run"])
        .arg(&root)
        .args([
            "--trust",
            "--case",
            "overview",
            "--baseline",
            "off",
            "--keep-temp",
        ])
        .arg("--coder")
        .arg(fake_agent())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stderr = child.stderr.take().unwrap();
    let (lines, seen) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let _ = lines.send(line);
        }
    });
    let started = Instant::now();
    let mut transcript = Vec::new();
    while started.elapsed() < Duration::from_secs(60) {
        let Ok(line) = seen.recv_timeout(Duration::from_secs(1)) else {
            continue;
        };
        let begun = line.contains("overview subject #1 …");
        transcript.push(line);
        if begun {
            break;
        }
    }
    std::thread::sleep(Duration::from_millis(1500));
    // SAFETY: a signal to a process this test spawned.
    unsafe { libc::kill(i32::try_from(child.id()).unwrap(), libc::SIGINT) };
    let status = child.wait().unwrap();
    while let Ok(line) = seen.recv_timeout(Duration::from_millis(200)) {
        transcript.push(line);
    }
    assert_eq!(status.code(), Some(130), "{transcript:#?}");
    let kept = transcript
        .iter()
        .find_map(|line| line.trim().strip_prefix("kept "))
        .map(PathBuf::from)
        .expect("the stopped run's directory was kept");
    let stdout = std::fs::read_to_string(kept.join("out/stdout.jsonl")).unwrap();
    let pid = serde_json::from_str::<Value>(stdout.lines().next().unwrap()).unwrap()["pid"]
        .as_i64()
        .unwrap();
    // SAFETY: signal 0 only asks whether the process exists.
    let alive = unsafe { libc::kill(i32::try_from(pid).unwrap(), 0) } == 0;
    assert!(!alive, "the agent {pid} was stopped with the run");
    let _ = std::fs::remove_dir_all(kept);
}

/// A local relay with Blossom media, on a disposable database.
struct Relay {
    url: String,
    stop: nostr_relay::gateway::ShutdownHandle,
    _runtime: tokio::runtime::Runtime,
    _media: tempfile::TempDir,
}

fn relay(database_url: String) -> Relay {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let media = tempfile::tempdir().unwrap();
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let url = format!("ws://127.0.0.1:{port}");
    let mut config = nostr_relay::gateway::GatewayConfig::new(
        database_url,
        format!("127.0.0.1:{port}").parse().unwrap(),
    );
    config.relay_url = Some(url.clone());
    config.db_connections = 2;
    config.shutdown_grace = Duration::from_secs(1);
    config.limits.events_per_minute_ip = 1_000;
    config.limits.events_per_minute_pubkey = 1_000;
    config.limits.media_per_minute_ip = 1_000;
    config.limits.media_per_minute_pubkey = 1_000;
    config.media = Some(nostr_relay::gateway::MediaConfig {
        root: media.path().to_path_buf(),
        cloud_base_url: None,
        max_blob_bytes: 10 * 1024 * 1024,
        max_bytes_per_pubkey: 1 << 30,
    });
    let gateway = runtime
        .block_on(nostr_relay::gateway::Gateway::start(config))
        .unwrap();
    let stop = gateway.shutdown_handle();
    runtime.spawn(gateway.run());
    Relay {
        url,
        stop,
        _runtime: runtime,
        _media: media,
    }
}

#[test]
fn a_published_result_is_checked_and_confirmed_on_a_local_relay() {
    let Ok(database_url) = std::env::var("NOSTR_RELAY_TEST_DATABASE_URL") else {
        eprintln!("skipped: set NOSTR_RELAY_TEST_DATABASE_URL or run scripts/test-postgres.sh");
        return;
    };
    if std::env::var("NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE").as_deref() != Ok("1") {
        eprintln!("skipped: set NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE=1");
        return;
    }
    let relay = relay(database_url);
    let work = tempfile::tempdir().unwrap();
    let root = extension(work.path());
    let door = door();
    let home = work.path().join("home");
    let (alice, bob) = (work.path().join("alice"), work.path().join("bob"));

    // Alice runs the suite: Better, since only the subject arm has the
    // skill the door answers the overview with.
    let out = openagents(&home, &alice, &door)
        .args(["--json", "ext", "eval", "run"])
        .arg(&root)
        .args(["--trust", "--runs", "2", "--concurrency", "4", "--coder"])
        .arg(fake_agent())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert_eq!(out.status.code(), Some(0), "{stderr}");
    let report: Value = serde_json::from_slice(&out.stdout).expect("--json prints the report");
    assert_eq!(report["verdict"], "pass", "{report}");
    let results = std::fs::read_dir(root.join("evals/results"))
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .next()
        .unwrap();

    // Alice adds it to the Gym.
    let out = openagents(&home, &alice, &door)
        .args(["--json", "ext", "eval", "publish"])
        .arg(results.join("report.json"))
        .args(["--relay", &relay.url])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let published: Value = serde_json::from_slice(&out.stdout).unwrap();
    let result_id = published["result"].as_str().unwrap().to_string();

    // Publishing again reuses both events.
    let again = openagents(&home, &alice, &door)
        .args(["--json", "ext", "eval", "publish"])
        .arg(results.join("report.json"))
        .args(["--relay", &relay.url])
        .output()
        .unwrap();
    let again: Value = serde_json::from_slice(&again.stdout).unwrap();
    assert_eq!(again["result"], published["result"]);
    assert_eq!(again["suite_release"], published["suite_release"]);

    // Bob checks it from his own copy of the extension.
    let bobs = work.path().join("bob-work");
    std::fs::create_dir_all(&bobs).unwrap();
    let bob_root = extension(&bobs);
    let out = openagents(&home, &bob, &door)
        .args(["--json", "ext", "eval", "check", &result_id])
        .arg(&bob_root)
        .args([
            "--trust",
            "--runs",
            "2",
            "--concurrency",
            "4",
            "--relay",
            &relay.url,
        ])
        .arg("--coder")
        .arg(fake_agent())
        .current_dir(&bobs)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert_eq!(out.status.code(), Some(0), "{stderr}");
    let checked: Value = serde_json::from_slice(
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .last()
            .unwrap()
            .as_bytes(),
    )
    .unwrap();
    assert_eq!(checked["linkage"], "confirm", "{checked}");
    assert_eq!(checked["original"], result_id.as_str());
    relay.stop.shutdown();
}

/// A signed-in engine stand-in. HOME and all login paths belong to this test.
#[cfg(unix)]
fn signed_in(work: &Path, codex: bool, running: bool) -> Command {
    use std::os::unix::fs::PermissionsExt;
    let home = work.join("login");
    std::fs::create_dir_all(home.join(if codex { ".codex" } else { ".claude" })).unwrap();
    std::fs::write(
        home.join(if codex {
            ".codex/auth.json"
        } else {
            ".claude/.credentials.json"
        }),
        "{}",
    )
    .unwrap();
    let binary = work.join("engine");
    let output = if codex {
        "[ \"$1\" = exec ] || exit 3\nwhile [ \"$1\" != --output-schema ]; do shift; done\n[ -f \"$2\" ] || exit 4\nwhile [ \"$1\" != --output-last-message ]; do shift; done\nexec > \"$2\""
    } else {
        "case \"$*\" in *'--output-format json'*) ;; *) exit 3 ;; esac"
    };
    let proposal = if running {
        r#"{"answer":"Welcome aboard, glad you are here!"}"#
    } else {
        r#"{"say":"We help you check the plugin.","asking":false,"name":null,"summary":null,"skill":null,"uses":[]}"#
    };
    let result = if codex {
        proposal.to_string()
    } else {
        json!({"type":"result","is_error":false,"result":proposal}).to_string()
    };
    std::fs::write(
        &binary,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\n{output}\nprintf '%s\\n' '{}'\n",
            work.join("calls").display(),
            result
        ),
    )
    .unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut command = Command::new(OPENAGENTS);
    command
        .env_clear()
        .env("HOME", &home)
        .env("PATH", "/usr/bin:/bin")
        .env("OPENAGENTS_HOME", work.join("oa"))
        .env("VERSE_HOME", work.join("verse"))
        .env("SUPERVISE_MEMORY_MAX", "none")
        .env(
            if codex {
                "CODER_ONE_CODEX_BIN"
            } else {
                "CODER_ONE_CLAUDE_BIN"
            },
            binary,
        );
    command
}

#[test]
#[cfg(unix)]
fn keyless_interview_uses_each_signed_in_engine() {
    for codex in [false, true] {
        let work = tempfile::tempdir().unwrap();
        let root = extension(work.path());
        let out = signed_in(work.path(), codex, false)
            .args(["plugin", "test", "init"])
            .arg(&root)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let screen = String::from_utf8_lossy(&out.stdout);
        let error = String::from_utf8_lossy(&out.stderr);
        assert!(
            screen.contains("We help you check the plugin."),
            "{screen}\n{error}"
        );
        assert!(error.contains("answers ended"), "{error}");
        assert!(!error.contains("invalid JSON"), "{error}");
        let calls = std::fs::read_to_string(work.path().join("calls")).unwrap();
        assert!(calls.contains("## JSON schema"));
        assert!(calls.contains(if codex {
            "--output-schema"
        } else {
            "--output-format json"
        }));
    }
}

#[test]
#[cfg(unix)]
fn keyless_run_uses_each_signed_in_engine_in_both_arms() {
    for codex in [false, true] {
        let work = tempfile::tempdir().unwrap();
        let root = extension(work.path());
        let out = signed_in(work.path(), codex, true)
            .args(["--json", "plugin", "test", "run"])
            .arg(&root)
            .args(["--trust", "--case", "greeting", "--runs", "1", "--coder"])
            .arg(fake_agent())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(!stderr.contains("needs a model key"), "{stderr}");
        let report: Value =
            serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{e}: {stderr}"));
        assert!(report.is_object());
        let calls = std::fs::read_to_string(work.path().join("calls")).unwrap();
        assert_eq!(
            calls.matches("Return JSON matching").count(),
            2,
            "both arms use the same engine: {stderr}"
        );
        assert!(stderr.contains("greeting subject #1 completed"), "{stderr}");
        assert!(
            stderr.contains("greeting baseline #1 completed"),
            "{stderr}"
        );
    }
}

#[test]
#[cfg(unix)]
fn a_fileless_claude_login_is_discovered_through_auth_status() {
    let work = tempfile::tempdir().unwrap();
    let root = extension(work.path());
    let mut command = signed_in(work.path(), false, false);
    std::fs::remove_file(work.path().join("login/.claude/.credentials.json")).unwrap();
    let binary = work.path().join("engine");
    let script = std::fs::read_to_string(&binary).unwrap();
    std::fs::write(&binary, script.replacen("#!/bin/sh\n", "#!/bin/sh\nif [ \"$1\" = auth ]; then printf '%s\\n' '{\"loggedIn\":true}'; exit 0; fi\n", 1)).unwrap();
    let out = command
        .args(["plugin", "test", "init"])
        .arg(root)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("We help you check the plugin."),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
