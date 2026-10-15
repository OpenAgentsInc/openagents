//! `openagents keys` and `openagents login --key` (#11305) against a small
//! stand-in for the website's `/v1/keys` routes.
#![cfg(unix)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

/// The stand-in's keys: (token, record).
type Book = Arc<Mutex<Vec<(String, Value)>>>;

fn id(n: usize) -> String {
    format!("{n:03x}{}", "c".repeat(61))
}

fn token(n: usize) -> String {
    format!("oa_sk_{n:064x}")
}

/// Serve `/v1/keys` for the keys in `book`, as the account "Octo".
fn site(book: Book) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut head = Vec::new();
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                head.push(line.trim_end().to_owned());
            }
            let header = |name: &str| {
                head.iter().find_map(|l| {
                    l.split_once(':')
                        .filter(|(n, _)| n.eq_ignore_ascii_case(name))
                        .map(|(_, v)| v.trim().to_owned())
                })
            };
            let length: usize = header("content-length")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let mut body = vec![0; length];
            let _ = reader.read_exact(&mut body);
            let mut words = head[0].split(' ');
            let (method, target) = (words.next().unwrap(), words.next().unwrap());
            let (path, query) = target.split_once('?').unwrap_or((target, ""));
            let bearer = header("authorization")
                .and_then(|v| v.strip_prefix("Bearer ").map(str::to_owned))
                .unwrap_or_default();
            let (status, answer) = route(&book, method, path, query, &bearer, &body);
            let text = answer.to_string();
            let _ = write!(
                stream,
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{text}",
                text.len()
            );
        }
    });
    origin
}

fn route(
    book: &Book,
    method: &str,
    path: &str,
    query: &str,
    bearer: &str,
    body: &[u8],
) -> (&'static str, Value) {
    let mut keys = book.lock().unwrap();
    let kind = if query.contains("kind=api") {
        "api"
    } else {
        "signin"
    };
    if !keys
        .iter()
        .any(|(t, k)| t == bearer && k["kind"] == "signin")
    {
        return ("401 Unauthorized", json!({"error": {"code": "signed_out"}}));
    }
    let view = |(t, k): &(String, Value)| {
        let mut k = k.clone();
        k["current"] = json!(t == bearer);
        k.as_object_mut().unwrap().remove("kind");
        k
    };
    match (method, path) {
        ("GET", "/v1/keys") => (
            "200 OK",
            json!({"kind": kind, "account": {"id": "acct_1", "label": "Octo"},
                "keys": keys.iter().filter(|(_, k)| k["kind"] == kind).map(view).collect::<Vec<_>>()}),
        ),
        ("POST", "/v1/keys") => {
            let asked: Value = serde_json::from_slice(body).unwrap();
            let kind = asked["kind"].as_str().unwrap_or("signin").to_owned();
            let n = keys.len() + 100;
            let secret = if kind == "api" {
                format!("oak_{n}.secret")
            } else {
                token(n)
            };
            let record = json!({"id": id(n), "name": asked["name"], "created_at": 1_791_504_000u64,
                "last_used_at": null,
                "expires_at": asked["expires_in_days"].as_u64().map(|d| 1_791_504_000 + d * 86_400),
                "kind": kind});
            keys.push((secret.clone(), record.clone()));
            let mut shown = view(&(String::new(), record));
            shown["current"] = json!(false);
            (
                "200 OK",
                json!({"kind": kind, "key": shown, "token": secret}),
            )
        }
        ("DELETE", _) => {
            let id = path.trim_start_matches("/v1/keys/");
            let before = keys.len();
            keys.retain(|(_, k)| !(k["id"] == id && k["kind"] == kind));
            if keys.len() == before {
                (
                    "404 Not Found",
                    json!({"error": {"code": "unknown_key", "message": "There's no working key with that id."}}),
                )
            } else {
                (
                    "200 OK",
                    json!({"kind": kind, "key": {"id": id, "state": "revoked"}}),
                )
            }
        }
        _ => ("404 Not Found", json!({"error": {"code": "not_found"}})),
    }
}

fn first_key(name: &str) -> (String, Value) {
    (
        token(1),
        json!({"id": id(1), "name": name, "created_at": 1_791_504_000u64,
            "last_used_at": 1_791_590_400u64, "expires_at": 1_791_504_000u64 + 30 * 86_400, "kind": "signin"}),
    )
}

fn openagents(home: &Path, env: &[(&str, &str)], args: &[&str], input: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_openagents"))
        .args(args)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", home)
        .envs(env.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn json_out(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn keys_list_create_show_revoke_and_rotate() {
    let book: Book = Arc::new(Mutex::new(vec![first_key("ci agent")]));
    let origin = site(book.clone());
    let home = tempfile::tempdir().unwrap();
    let first = token(1);
    let env = [
        ("OPENAGENTS_ORIGIN", origin.as_str()),
        ("OPENAGENTS_TOKEN", first.as_str()),
    ];
    let run = |args: &[&str]| openagents(home.path(), &env, args, "");

    // list: name, id, dates; never a secret.
    let listed = run(&["keys", "list"]);
    let text = String::from_utf8_lossy(&listed.stdout);
    assert!(listed.status.success(), "{text}");
    assert!(
        text.contains("\"ci agent\"") && text.contains("last used 2026-10-10"),
        "{text}"
    );
    assert!(
        text.contains("expires 2026-11-08") && text.contains("(this key)"),
        "{text}"
    );
    assert!(!text.contains("oa_sk_"));
    let listed = json_out(&run(&["--json", "keys", "list"]));
    assert_eq!(listed["keys"][0]["name"], "ci agent");
    assert!(!listed.to_string().contains("oa_sk_"));

    // create: the secret once, with the line that uses it.
    let made = run(&["keys", "create", "--name", "build", "--expires", "90d"]);
    let text = String::from_utf8_lossy(&made.stdout).into_owned();
    assert!(made.status.success(), "{text}");
    let secret = token(101);
    assert!(
        text.contains(&secret) && text.contains("won't be shown again"),
        "{text}"
    );
    assert!(text.contains(&format!(
        "OPENAGENTS_ORIGIN={origin} OPENAGENTS_TOKEN={secret} coder"
    )));
    let made = json_out(&run(&["--json", "keys", "create", "--name", "forever"]));
    assert!(made["token"].as_str().unwrap().starts_with("oa_sk_"));
    assert_eq!(made["key"]["expires_at"], Value::Null);
    assert!(made["use"].as_str().unwrap().contains("OPENAGENTS_TOKEN="));

    // show, by name and by the start of the id.
    let shown = json_out(&run(&["--json", "keys", "show", "build"]));
    assert_eq!(shown["name"], "build");
    assert!(!shown.to_string().contains("oa_sk_"));
    let by_id = json_out(&run(&["--json", "keys", "show", &id(101)[..12]]));
    assert_eq!(by_id["name"], "build");

    // rotate: a new key, same name and expiry, then the old one is gone.
    let rotated = json_out(&run(&["--json", "keys", "rotate", "build"]));
    assert_eq!(rotated["key"]["name"], "build");
    assert_eq!(
        rotated["key"]["expires_at"].as_u64(),
        Some(1_791_504_000 + 90 * 86_400)
    );
    assert_eq!(rotated["revoked"], id(101));
    let names: Vec<String> = book
        .lock()
        .unwrap()
        .iter()
        .map(|(_, k)| k["name"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(names.iter().filter(|n| *n == "build").count(), 1);

    // revoke, by name; an unknown one says so.
    let revoked = run(&["keys", "revoke", "forever"]);
    assert!(revoked.status.success());
    assert!(String::from_utf8_lossy(&revoked.stdout).contains("Revoked \"forever\""));
    let missing = run(&["keys", "revoke", "forever"]);
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("No key is named"));

    // API keys go through the same group.
    let api = json_out(&run(&[
        "--json", "keys", "create", "--name", "laptop", "--kind", "api",
    ]));
    assert!(api["token"].as_str().unwrap().starts_with("oak_"));
    assert!(api["use"].as_str().unwrap().contains("OPENAGENTS_API_KEY="));
    let apis = json_out(&run(&["--json", "keys", "list", "--kind", "api"]));
    assert_eq!(apis["keys"].as_array().unwrap().len(), 1);
    let gone = run(&["keys", "revoke", "laptop", "--kind", "api"]);
    assert!(
        gone.status.success(),
        "{}",
        String::from_utf8_lossy(&gone.stderr)
    );

    // A revoked credential is refused.
    book.lock().unwrap().retain(|(t, _)| *t != first);
    let refused = run(&["keys", "list"]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("Not logged in"));
}

#[test]
fn login_key_reads_stdin_and_the_saved_key_signs_keys_in() {
    let book: Book = Arc::new(Mutex::new(vec![first_key("agent")]));
    let origin = site(book);
    let home = tempfile::tempdir().unwrap();
    let env = [("OPENAGENTS_ORIGIN", origin.as_str())];

    let on_argv = openagents(
        home.path(),
        &env,
        &["login", &format!("--key={}", token(1))],
        "",
    );
    assert!(!on_argv.status.success());
    assert!(
        String::from_utf8_lossy(&on_argv.stderr).contains("Don't put the key on the command line")
    );

    let signed = openagents(
        home.path(),
        &env,
        &["login", "--key"],
        &format!("{}\n", token(1)),
    );
    let text = String::from_utf8_lossy(&signed.stdout);
    assert!(
        signed.status.success(),
        "{text}{}",
        String::from_utf8_lossy(&signed.stderr)
    );
    assert!(text.contains("Logged in to OpenAgents as Octo with a login key."));
    assert!(!text.contains("oa_sk_"));
    let saved = home.path().join(".openagents/coder/account.json");
    assert!(std::fs::read_to_string(&saved).unwrap().contains(&token(1)));

    // A sign-in kept under the folder Coder used before #11361 still reads.
    let older = home.path().join(".openagents/coder-new");
    std::fs::create_dir(&older).unwrap();
    std::fs::rename(&saved, older.join("account.json")).unwrap();
    std::fs::remove_dir(saved.parent().unwrap()).unwrap();

    // With nothing in the environment, the saved key is used.
    let listed = json_out(&openagents(
        home.path(),
        &env,
        &["--json", "keys", "list"],
        "",
    ));
    assert_eq!(listed["keys"][0]["current"], true);
}
