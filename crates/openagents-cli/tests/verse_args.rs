use std::io::Write;
use std::process::{Command, Stdio};

fn run(words: &[&str], input: Option<&str>) -> std::process::Output {
    let home = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_openagents"))
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .args(words)
        .env("HOME", home.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(input) = input {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    child.wait_with_output().unwrap()
}

#[test]
fn unknown_flags_and_extra_arguments_are_usage_errors() {
    for (words, message, usage) in [
        (
            vec!["verse", "who", "--bogus"],
            "--bogus isn't an option of verse who",
            "usage: openagents verse who",
        ),
        (
            vec!["verse", "who", "--radius", "5"],
            "--radius isn't an option of verse who",
            "usage: openagents verse who",
        ),
        (
            vec!["verse", "quests", "--xp-referee", "KEY"],
            "--xp-referee isn't an option of verse quests",
            "usage: openagents verse quests",
        ),
        (
            vec!["verse", "chat", "extra", "args"],
            "unexpected argument `extra` for verse chat",
            "usage: openagents verse chat",
        ),
        (
            vec!["zone", "info", "extra"],
            "unexpected argument `extra` for zone info",
            "usage: openagents zone info",
        ),
        (
            vec!["zone", "info", "--bogus"],
            "--bogus isn't an option of zone info",
            "usage: openagents zone info",
        ),
        (
            vec!["xp", "--bogus"],
            "--bogus isn't an option of xp",
            "usage: openagents xp",
        ),
        (
            vec!["xp", "verify-card", "-", "extra"],
            "unexpected argument `extra` for xp",
            "usage: openagents xp",
        ),
        (
            vec!["verse", "look", "--radius"],
            "--radius needs a value",
            "usage: openagents verse look",
        ),
    ] {
        let out = run(&words, None);
        assert_eq!(out.status.code(), Some(64), "{words:?}");
        let error = String::from_utf8(out.stderr).unwrap();
        assert!(error.contains(message), "{error}");
        assert!(error.contains(usage), "{error}");
        assert!(out.stdout.is_empty());
    }
}

#[test]
fn card_errors_are_readable_and_fail_for_stdin_and_files() {
    let out = run(&["xp", "verify-card", "naddr1qqqqq"], None);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(out.stderr).unwrap(),
        "openagents xp: naddr1qqqqq isn't an naddr: bech32 string has an invalid length\n"
    );
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), "{}").unwrap();
    for (source, input, label) in [
        ("-", Some("{}"), "standard input"),
        (
            file.path().to_str().unwrap(),
            None,
            file.path().to_str().unwrap(),
        ),
    ] {
        for json in [false, true] {
            let mut words = vec!["xp", "verify-card", source];
            if json {
                words.insert(0, "--json");
            }
            let out = run(&words, input);
            assert_eq!(out.status.code(), Some(1));
            let message = format!(
                "{label} isn't a signed Nostr event: expected a JSON event with id, pubkey, created_at, kind, tags, content, and sig"
            );
            assert_eq!(
                String::from_utf8(out.stderr).unwrap(),
                format!("openagents xp: {message}\n")
            );
            if json {
                assert_eq!(
                    serde_json::from_slice::<serde_json::Value>(&out.stdout).unwrap()["error"],
                    message
                );
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn look_filters_stored_entities_by_radius_in_meters() {
    use futures_util::{SinkExt, StreamExt};
    use glam::{Quat, Vec3};
    use serde_json::{Value, json};
    use tokio_tungstenite::tungstenite::Message;
    use verse::mv::{self, State};
    let signer = nostr::domain::RelaySigner::from_secret_hex(&"01".repeat(32)).unwrap();
    let events: Vec<_> = [("inside", 4.0), ("boundary", 5.0), ("outside", 18.0)]
        .into_iter()
        .map(|(id, x)| {
            let state = State {
                v: 1,
                id: id.into(),
                role: "avatar".into(),
                p: Vec3::new(x, 0.0, 0.0).to_array(),
                q: Quat::IDENTITY.to_array(),
                t: 1000,
                online: true,
                follows: None,
                name: Some(id.into()),
                set: None,
                b: None,
            };
            mv::state_event(&signer, "fixture", &state, 1)
        })
        .collect();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let events = events.clone();
            tokio::spawn(async move {
                let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
                while let Some(Ok(Message::Text(text))) = socket.next().await {
                    let frame: Value = serde_json::from_str(&text).unwrap();
                    if frame[0] == "REQ" {
                        for event in &events {
                            socket
                                .send(Message::Text(
                                    json!(["EVENT", frame[1], event]).to_string().into(),
                                ))
                                .await
                                .unwrap();
                        }
                        socket
                            .send(Message::Text(json!(["EOSE", frame[1]]).to_string().into()))
                            .await
                            .unwrap();
                    }
                }
            });
        }
    });
    let out = tokio::task::spawn_blocking(move || {
        run(
            &[
                "--json", "verse", "look", "--world", "fixture", "--relay", &url, "--at", "0,0,0",
                "--radius", "5", "--wait", "2",
            ],
            None,
        )
    })
    .await
    .unwrap();
    server.abort();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["radius_m"], 5.0);
    let entities = value["entities"].as_array().unwrap();
    assert_eq!(entities.len(), 2);
    assert_eq!(entities[0]["entity"], "inside");
    assert_eq!(entities[0]["distance"], 4.0);
    assert_eq!(entities[1]["entity"], "boundary");
    assert_eq!(entities[1]["distance"], 5.0);
}
