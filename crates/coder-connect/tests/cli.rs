#![cfg(feature = "host")]
use secp256k1::SecretKey;
use std::process::Command;

#[test]
fn local_pair_and_revoke_cli_retain_only_public_connection_output() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("synthetic-history");
    let state = temp.path().join("host");
    std::fs::create_dir(&root).unwrap();
    let secret = SecretKey::new(&mut secp256k1::rand::rng());
    let public = coder_connect::protocol::pubkey(&secret);
    let output = Command::new(env!("CARGO_BIN_EXE_coder-connect"))
        .args([
            "pair",
            "--client",
            &public,
            "--relay",
            "wss://relay.example/",
        ])
        .arg("--codex-root")
        .arg(&root)
        .arg("--state")
        .arg(&state)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let code = coder_connect::ConnectionCode::parse(&output.stdout).unwrap();
    code.verify(
        &secret,
        coder_connect::unix_time().unwrap(),
        coder_connect::RelayPolicy::Production,
    )
    .unwrap();
    assert!(!String::from_utf8_lossy(&output.stdout).contains(root.to_str().unwrap()));
    let output = Command::new(env!("CARGO_BIN_EXE_coder-connect"))
        .args(["revoke", "--grant", &code.grant])
        .arg("--state")
        .arg(&state)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"revoked\n");
    assert!(
        coder_connect::host::Host::new(state, coder_connect::RelayPolicy::Production)
            .relays(coder_connect::unix_time().unwrap())
            .unwrap()
            .is_empty()
    );
}

#[path = "../../coder-control/src/tests/relay.rs"]
mod relay;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connect_command_displays_qr_pairs_and_retains_only_the_selected_roots() {
    use coder_connect::{RelayPolicy, pairing};
    use std::io::{BufRead, BufReader};
    use std::os::unix::fs::PermissionsExt;
    use std::process::Stdio;
    let (url, relay, _) = relay::start().await;
    for explicit in [true, false] {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        std::fs::create_dir_all(home.join(".codex")).unwrap();
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        let state = temp.path().join("state");
        let mut command = Command::new(env!("CARGO_BIN_EXE_coder-connect"));
        command
            .args([
                "connect",
                "--relay",
                &url,
                "--loopback-test",
                "--no-browser",
                "--once",
            ])
            .arg("--state")
            .arg(&state)
            .env("HOME", &home)
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if explicit {
            command.arg("--codex-root").arg(home.join(".codex"));
        }
        let mut child = command.spawn().unwrap();
        let output = child.stdout.take().unwrap();
        let (send, receive) = tokio::sync::oneshot::channel();
        let reader = std::thread::spawn(move || {
            let mut send = Some(send);
            let mut transcript = String::new();
            for line in BufReader::new(output).lines() {
                let line = line.unwrap();
                if line.starts_with(pairing::PREFIX)
                    && let Some(send) = send.take()
                {
                    let _ = send.send(line.clone());
                }
                transcript.push_str(&line);
                transcript.push('\n');
            }
            transcript
        });
        let code = match tokio::time::timeout(std::time::Duration::from_secs(5), receive).await {
            Ok(Ok(code)) => code,
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("computer command did not display invitation");
            }
        };
        let invitation = pairing::Invitation::parse(
            &code,
            coder_connect::unix_time().unwrap(),
            RelayPolicy::LoopbackTest,
        )
        .unwrap();
        let page = state.join(format!("pairing-{}.html", invitation.id));
        let html = std::fs::read_to_string(&page).unwrap();
        assert!(html.contains("<svg") && html.contains(&code));
        assert_eq!(
            std::fs::metadata(&page).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let secret = SecretKey::new(&mut secp256k1::rand::rng());
        let code_result = pairing::redeem(&code, &secret, RelayPolicy::LoopbackTest).await;
        if code_result.is_err() {
            let _ = child.kill();
        }
        let status = child.wait().unwrap();
        let connection = code_result.unwrap();
        assert!(status.success());
        let output = reader.join().unwrap();
        assert!(output.contains("Phone paired.") && output.contains(&connection.grant));
        assert!(!page.exists());
        assert_eq!(connection.sources.len(), if explicit { 1 } else { 2 });
        if explicit {
            assert!(!output.contains("Claude:"));
        }
        // A separate serve process can recover the same consumed invitation.
        let mut restarted = Command::new(env!("CARGO_BIN_EXE_coder-connect"))
            .args(["serve", "--relay", &url, "--loopback-test"])
            .arg("--state")
            .arg(&state)
            .env("HOME", &home)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let retry = pairing::redeem(&code, &secret, RelayPolicy::LoopbackTest).await;
        // SAFETY: signal only the child process created by this test.
        unsafe {
            libc::kill(restarted.id() as i32, libc::SIGINT);
        }
        let _ = restarted.wait();
        assert_eq!(retry.unwrap().authorization, connection.authorization);
    }
    relay.abort();
    let _ = relay.await;
}
