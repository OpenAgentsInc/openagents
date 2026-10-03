//! Eval regressions through the CLI and a local HTTP door.
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::Command;

fn cli(home: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_openagents"));
    cmd.env("HOME", home)
        .env("TMPDIR", home)
        .env_remove("TYPESAFE_API_KEY")
        .env_remove("TYPESAFE_BASE_URL");
    cmd
}

fn exercise(suffix: &str, recordable: bool, json: bool) {
    let home = tempfile::tempdir().unwrap();
    let store = home.path().join("store.jsonl");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let mut posts = 0;
        for _ in 0..79 {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                .unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());
            let mut first = String::new();
            reader.read_line(&mut first).unwrap();
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some((key, value)) = line.split_once(':') {
                    if key.eq_ignore_ascii_case("content-length") {
                        length = value.trim().parse::<usize>().unwrap();
                    }
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let (status, response) = if first.starts_with("GET ") {
                assert_eq!(first, "GET /v1/models HTTP/1.1\r\n");
                ("200 OK", r#"{"models":[]}"#)
            } else {
                assert_eq!(first, "POST /v1/systemone HTTP/1.1\r\n");
                posts += 1;
                if recordable {
                    (
                        "409 Conflict",
                        r#"{"error":{"code":"busy","message":"test refusal"}}"#,
                    )
                } else {
                    ("404 Not Found", "not found")
                }
            };
            write!(
                socket,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                response.len()
            )
            .unwrap();
        }
        assert_eq!(posts, 78);
    });
    let mut cmd = cli(home.path());
    if json {
        cmd.arg("--json");
    }
    let output = cmd
        .args([
            "eval",
            "run",
            "--door",
            &format!("hosted={origin}{suffix}"),
            "--timeout",
            "1",
            "--partition",
            "development",
            "--record",
        ])
        .arg(&store)
        .output()
        .unwrap();
    server.join().unwrap();
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        output.status.code(),
        Some(if recordable { 0 } else { 1 }),
        "{text}"
    );
    if json {
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["recorded_rows"], if recordable { 78 } else { 0 });
        if !recordable {
            assert!(value["recorded_to"].is_null());
            let losses = value["doors"][0]["lost"].as_array().unwrap();
            assert_eq!(losses.len(), 78);
            assert!(
                losses[0]["detail"]
                    .as_str()
                    .unwrap()
                    .contains(&format!("HTTP 404: POST {origin}/v1/systemone"))
            );
        }
    } else if recordable {
        assert!(
            text.contains(&format!("recorded to {}", store.display())),
            "{text}"
        );
        assert!(!text.contains("nothing recorded"));
    } else {
        assert!(text.contains("nothing recorded"), "{text}");
        assert!(text.contains("incomplete:"));
        assert_eq!(text.matches("78 lost: HTTP 404:").count(), 1, "{text}");
        assert!(text.contains(&format!("POST {origin}/v1/systemone")));
        assert!(!text.lines().any(|line| line.starts_with("recorded to ")));
    }
    if recordable {
        assert_eq!(std::fs::read_to_string(store).unwrap().lines().count(), 78);
    } else {
        assert!(!store.exists());
    }
}

#[test]
fn endpoint_forms_and_recording_match_real_writes() {
    for suffix in ["", "/", "/v1/systemone", "/v1/systemone/"] {
        exercise(suffix, true, false);
    }
}

#[test]
fn all_losses_are_grouped_and_nothing_is_recorded() {
    exercise("/v1/systemone", false, false);
    exercise("/v1/systemone/", false, true);
}

#[test]
fn help_names_hosted_jev_and_accepts_endpoint_urls() {
    let home = tempfile::tempdir().unwrap();
    let output = cli(home.path()).args(["eval", "--help"]).output().unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("origin or full POST /v1/systemone URL"));
    assert!(text.contains("Hosted Jev: --door jev-latest=https://api.typesafe.ai/v1/systemone"));
    assert!(text.contains("TYPESAFE_API_KEY"));
}
