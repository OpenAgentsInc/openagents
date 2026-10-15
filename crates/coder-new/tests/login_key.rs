//! `coder login --key` (#11305): the sign-in key comes from standard
//! input, never the command line, and is saved as a device sign-in is.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Command, Stdio};

const KEY: &str = "oa_sk_0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// A site that answers `GET /v1/keys` for [`KEY`] and `404` to anything
/// else, on its own thread. Answers its origin.
fn site() -> String {
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
                head.push(line);
            }
            let length = head
                .iter()
                .find_map(|l| {
                    l.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                })
                .unwrap_or(0);
            let mut body = vec![0; length];
            let _ = reader.read_exact(&mut body);
            let keys = head[0].starts_with("GET /v1/keys ")
                && head.iter().any(|l| {
                    l.trim()
                        .eq_ignore_ascii_case(&format!("authorization: Bearer {KEY}"))
                });
            let (status, body) = if keys {
                (
                    "200 OK",
                    r#"{"kind":"signin","account":{"id":"acct_1","label":"Octo"},"keys":[{"id":"a","name":"agent","created_at":1,"expires_at":null,"current":true}]}"#,
                )
            } else if head[0].starts_with("GET /v1/keys ") {
                ("401 Unauthorized", r#"{"error":{"code":"signed_out"}}"#)
            } else {
                ("404 Not Found", r#"{"error":{"code":"not_found"}}"#)
            };
            let _ = write!(
                stream,
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    origin
}

fn coder(
    origin: &str,
    state: &std::path::Path,
    args: &[&str],
    input: &str,
) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_coder-new"))
        .args(args)
        .arg("--state")
        .arg(state)
        .env("OPENAGENTS_ORIGIN", origin)
        .env("HOME", state)
        .env_remove("OPENAGENTS_TOKEN")
        .env_remove("OPENAGENTS_APP_TOKEN")
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

#[test]
fn login_key_reads_the_key_from_stdin_and_saves_it() {
    let origin = site();
    let state = tempfile::tempdir().unwrap();
    let out = coder(
        &origin,
        state.path(),
        &["login", "--key"],
        &format!("{KEY}\n"),
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("Logged in to OpenAgents as Octo with a login key."),
        "{stdout}"
    );
    assert!(!stdout.contains(KEY), "the key is never printed");
    let saved = std::fs::read_to_string(state.path().join("account.json")).unwrap();
    assert!(saved.contains(KEY) && saved.contains("acct_1"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(state.path().join("account.json"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

#[test]
fn login_key_refuses_a_bad_key_and_a_key_on_the_command_line() {
    let origin = site();
    let state = tempfile::tempdir().unwrap();
    let wrong = format!("oa_sk_{}", "f".repeat(64));
    let out = coder(
        &origin,
        state.path(),
        &["login", "--key"],
        &format!("{wrong}\n"),
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("doesn't work"));
    assert!(!state.path().join("account.json").exists());

    let argv = format!("--key={KEY}");
    let out = coder(&origin, state.path(), &["login", &argv], "");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("Don't put the key on the command line"));
    assert!(!state.path().join("account.json").exists());
}
