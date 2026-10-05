//! Regression checks through the CLI, with isolated identities and a local relay.
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::io::Write;
use std::process::{Command, Stdio};

fn command(home: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_openagents"));
    cmd.env("HOME", home)
        .env_remove("VERSE_RELAY")
        .env_remove("VERSE_XP_RELAY");
    cmd
}

#[test]
fn unknown_options_and_extra_arguments_have_command_usage() {
    let home = tempfile::tempdir().unwrap();
    for (args, label, usage) in [
        (
            vec!["verse", "who", "--bogus"],
            "--bogus isn't an option of verse who",
            "verse who",
        ),
        (
            vec!["verse", "who", "--bogus=5"],
            "--bogus isn't an option of verse who",
            "verse who",
        ),
        (
            vec!["verse", "who", "--radius", "5"],
            "--radius isn't an option of verse who",
            "verse who",
        ),
        (
            vec!["verse", "quests", "--xp-referee", "KEY"],
            "--xp-referee isn't an option of verse quests",
            "verse quests",
        ),
        (
            vec!["verse", "chat", "extra", "args"],
            "unexpected argument `extra`",
            "verse chat",
        ),
        (
            vec!["zone", "info", "extra"],
            "unexpected argument `extra`",
            "zone info",
        ),
        (
            vec!["zone", "info", "--bogus"],
            "--bogus isn't an option of zone info",
            "zone info",
        ),
        (
            vec!["zone", "build", "extra"],
            "unexpected argument `extra`",
            "zone build",
        ),
        (vec!["xp", "--bogus"], "--bogus isn't an option of xp", "xp"),
        (
            vec!["xp", "verify-card", "-", "extra"],
            "unexpected argument `extra`",
            "xp verify-card",
        ),
        (
            vec!["verse", "xp", "verify-card", "-", "extra"],
            "unexpected argument `extra`",
            "verse xp verify-card",
        ),
        (
            vec!["verse", "look", "--radius"],
            "--radius needs a value",
            "verse look",
        ),
        (
            vec!["verse", "look", "--radius", "--wait", "2"],
            "--radius needs a value",
            "verse look",
        ),
    ] {
        let out = command(home.path()).args(&args).output().unwrap();
        let text = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(64), "{args:?}: {text}");
        assert!(text.contains(label), "{args:?}: {text}");
        assert!(
            text.contains(&format!("usage: openagents {usage}")),
            "{text}"
        );
        if args[0] == "xp" {
            assert!(text.starts_with("openagents xp:"), "{text}");
        }
    }
    assert!(
        !home.path().join(".openagents").exists(),
        "usage validation precedes identity creation"
    );
}

#[test]
fn invalid_cards_are_readable_failures_for_stdin_and_files() {
    let home = tempfile::tempdir().unwrap();
    let out = command(home.path())
        .args(["xp", "verify-card", "naddr1qqqqq"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(text.starts_with("openagents xp:"), "{text}");
    assert!(
        text.contains("naddr1qqqqq isn't a valid Nostr address"),
        "{text}"
    );
    assert!(!text.contains("InvalidLength"), "{text}");
    for json_mode in [false, true] {
        let mut cmd = command(home.path());
        if json_mode {
            cmd.arg("--json");
        }
        let mut child = cmd
            .args(["xp", "verify-card", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(b"{}").unwrap();
        let out = child.wait_with_output().unwrap();
        let text = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(1), "{text}");
        assert!(
            text.contains("standard input isn't a signed Nostr event"),
            "{text}"
        );
        assert!(!text.contains("missing field `id`"), "{text}");
        if json_mode {
            let value: Value = serde_json::from_slice(&out.stdout).unwrap();
            assert!(value["error"].as_str().unwrap().contains("standard input"));
        }
    }
    let file = home.path().join("card.json");
    std::fs::write(&file, "{}").unwrap();
    let out = command(home.path())
        .args(["xp", "verify-card"])
        .arg(&file)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains(file.to_str().unwrap()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn look_radius_is_in_meters_and_filters_stored_and_live_poses() {
    use glam::{Quat, Vec3};
    use tokio_tungstenite::tungstenite::Message;
    use verse::mv::{self, EntityPose, Frame, State};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let signer = verse::xp::fixture::signer(801);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let states: Vec<_> = [("near", 3.0), ("edge", 5.0), ("far", 18.0)]
        .into_iter()
        .map(|(id, x)| {
            mv::state_event(
                &signer,
                "verse-plaza",
                &State {
                    v: 1,
                    id: id.into(),
                    role: "avatar".into(),
                    p: [x, 0.0, 0.0],
                    q: Quat::IDENTITY.to_array(),
                    t: now * 1000,
                    online: true,
                    follows: None,
                    name: Some(id.into()),
                    set: None,
                    b: None,
                },
                now,
            )
        })
        .collect();
    let frame = mv::frame_event(
        &signer,
        "verse-plaza",
        &Frame {
            v: 1,
            s: "fixture".into(),
            n: 1,
            t: now * 1000,
            e: vec![
                EntityPose::new(
                    "live-near",
                    "avatar",
                    Vec3::new(4.0, 0.0, 0.0),
                    Quat::IDENTITY,
                ),
                EntityPose::new(
                    "live-far",
                    "avatar",
                    Vec3::new(20.0, 0.0, 0.0),
                    Quat::IDENTITY,
                ),
            ],
        },
        now,
    );
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
        while let Some(Ok(Message::Text(text))) = socket.next().await {
            let request: Value = serde_json::from_str(&text).unwrap();
            if request[0] == "REQ" {
                let sub = &request[1];
                let live = request[2]["#c"].is_array();
                let events = if live {
                    vec![frame.clone()]
                } else {
                    states.clone()
                };
                for event in events {
                    socket
                        .send(Message::Text(
                            json!(["EVENT", sub, event]).to_string().into(),
                        ))
                        .await
                        .unwrap();
                }
                socket
                    .send(Message::Text(json!(["EOSE", sub]).to_string().into()))
                    .await
                    .unwrap();
            }
        }
    });
    let home = tempfile::tempdir().unwrap();
    let out = command(home.path())
        .args([
            "--json", "verse", "look", "--at", "0,0,0", "--radius", "5", "--wait", "2", "--relay",
            &url,
        ])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["radius_m"], 5.0);
    let ids: Vec<_> = value["entities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["entity"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["near", "live-near", "edge"], "{value}");
    server.abort();
}

#[test]
fn a_blocked_player_stays_in_the_computers_list_until_unblocked() {
    let home = tempfile::tempdir().unwrap();
    let walker = "ab".repeat(32);
    let run = |args: &[&str]| -> Value {
        let output = command(home.path())
            .arg("--json")
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    };
    let blocked = run(&["verse", "block", &walker.to_uppercase()]);
    assert_eq!(blocked["changed"], true, "{blocked}");
    assert_eq!(blocked["blocked"], json!([walker]));
    assert!(home.path().join(".openagents/verse/blocked.json").is_file());
    // Another command, as after a relaunch, reads the same list.
    let listed = run(&["verse", "blocked"]);
    assert_eq!(listed["blocked"], json!([walker]));
    assert_eq!(listed["muted"], json!([]));
    // A unique prefix of a listed key is enough to unblock it.
    let unblocked = run(&["verse", "unblock", &walker[..8]]);
    assert_eq!(unblocked["changed"], true, "{unblocked}");
    assert_eq!(unblocked["blocked"], json!([]));
}
