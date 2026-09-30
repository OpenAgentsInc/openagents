//! Separate CLI processes over the shared synthetic NIP-42 relay fixture.
use coder_access::{Access, Client, RelayPolicy, client, protocol::pubkey};
use secp256k1::SecretKey;
use std::path::Path;
use std::process::{Command, Output, Stdio};

#[path = "../../coder-control/src/tests/relay.rs"]
mod relay;

const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;

fn cli(state: &Path, args: &[&str]) -> Output {
    let output = Command::new(env!("CARGO_BIN_EXE_coder-access"))
        .arg("--state")
        .arg(state)
        .arg("--loopback-test")
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}
fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

#[tokio::test]
async fn cli_invites_serves_lists_and_revokes() {
    let (relay, _relay_task, _) = relay::start().await;
    let temp = tempfile::tempdir().unwrap();
    let state = temp.path().join("access");
    let owner = SecretKey::new(&mut secp256k1::rand::rng());
    let init = stdout(&cli(&state, &["init", "--owner", &pubkey(&owner)]));
    let host = init
        .lines()
        .next()
        .unwrap()
        .strip_prefix("host ")
        .unwrap()
        .to_owned();

    let invite = cli(
        &state,
        &[
            "invite", "--relay", &relay, "--rights", "observe", "--no-qr",
        ],
    );
    let code = stdout(&invite)
        .lines()
        .find(|l| l.starts_with("coder-host:"))
        .unwrap()
        .to_owned();
    // The capability goes to stdout for display, never to the diagnostic stream.
    assert!(!String::from_utf8_lossy(&invite.stderr).contains(&code));

    let server = Command::new(env!("CARGO_BIN_EXE_coder-access"))
        .arg("--state")
        .arg(&state)
        .args([
            "--loopback-test",
            "serve-once",
            "--relay",
            &relay,
            "--wait-secs",
            "20",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let phone = SecretKey::new(&mut secp256k1::rand::rng());
    let access = client::redeem(&code, &phone, POLICY).await.unwrap();
    let served = tokio::task::spawn_blocking(move || server.wait_with_output().unwrap())
        .await
        .unwrap();
    assert!(
        served.status.success(),
        "{}",
        String::from_utf8_lossy(&served.stderr)
    );
    assert_eq!(access.grant.host, host);

    let listed = stdout(&cli(&state, &["list"]));
    assert!(
        listed.contains(&pubkey(&phone)) && listed.contains("Active"),
        "{listed}"
    );
    cli(&state, &["revoke", "--device", &pubkey(&phone)]);
    let listed = stdout(&cli(&state, &["list", "--json"]));
    assert!(listed.contains("\"revoked\""), "{listed}");

    // The revoked device's saved access still verifies offline: only the
    // host's current record can establish revocation.
    let saved = serde_json::to_vec(&access).unwrap();
    assert!(Client::device(Access::parse(&saved).unwrap(), phone, POLICY).is_ok());

    // Another owner cannot replace the locally established one.
    let other = Command::new(env!("CARGO_BIN_EXE_coder-access"))
        .arg("--state")
        .arg(&state)
        .args(["init", "--owner", &pubkey(&phone)])
        .output()
        .unwrap();
    assert!(!other.status.success());
}

#[tokio::test]
async fn cli_reverse_enrollment_is_approved_by_the_owner() {
    use std::io::{BufRead, BufReader};
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;
    let (relay, _relay_task, _) = relay::start().await;
    let temp = tempfile::tempdir().unwrap();
    let state = temp.path().join("access");
    let owner = SecretKey::new(&mut secp256k1::rand::rng());
    let init = stdout(&cli(&state, &["init", "--owner", &pubkey(&owner)]));
    let host = init
        .lines()
        .next()
        .unwrap()
        .strip_prefix("host ")
        .unwrap()
        .to_owned();
    // A throwaway fixture key in a private file, as the CLI requires.
    let key_file = temp.path().join("owner.key");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&key_file).unwrap();
    #[cfg(windows)]
    private_fs::restrict(&key_file).unwrap();
    std::io::Write::write_all(&mut file, owner.display_secret().to_string().as_bytes()).unwrap();

    let mut requester = Command::new(env!("CARGO_BIN_EXE_coder-access"))
        .arg("--state")
        .arg(&state)
        .args([
            "--loopback-test",
            "request",
            "--relay",
            &relay,
            "--rights",
            "observe,operate",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    // Read on a blocking thread so the in-process relay keeps serving the publish.
    let stdout = requester.stdout.take().unwrap();
    let (enrollment, code, lines) = tokio::task::spawn_blocking(move || {
        let mut lines = BufReader::new(stdout).lines();
        let enrollment = lines.next().unwrap().unwrap();
        let code = lines.next().unwrap().unwrap();
        (enrollment, code, lines)
    })
    .await
    .unwrap();
    let code = code.strip_prefix("code ").unwrap().to_owned();
    assert!(enrollment.starts_with("enrollment "));

    let laptop = SecretKey::new(&mut secp256k1::rand::rng());
    let (relay_arg, state_arg, key_arg, host_arg) =
        (relay.clone(), state.clone(), key_file.clone(), host.clone());
    let device = pubkey(&laptop);
    let approved = tokio::task::spawn_blocking(move || {
        Command::new(env!("CARGO_BIN_EXE_coder-access"))
            .arg("--state")
            .arg(&state_arg)
            .env("CODER_ACCESS_KEY_FILE", &key_arg)
            .args([
                "--loopback-test",
                "approve",
                "--relay",
                &relay_arg,
                "--host",
                &host_arg,
                "--device",
                &device,
                "--code",
                &code,
            ])
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    assert!(
        approved.status.success(),
        "{}",
        String::from_utf8_lossy(&approved.stderr)
    );
    let authorization = serde_json::from_slice(&approved.stdout).unwrap();
    let now = coder_access::unix_time().unwrap();
    let access = Access::from_authorization(authorization, &laptop, &host, now, POLICY).unwrap();
    assert_eq!(access.grant.rights.to_list(), "observe,operate");
    let (status, rest) = tokio::task::spawn_blocking(move || {
        let rest: Vec<_> = lines.map_while(|l| l.ok()).collect();
        (requester.wait().unwrap(), rest)
    })
    .await
    .unwrap();
    assert!(status.success());
    assert!(
        rest.iter().any(|l| l.starts_with("approved device")),
        "{rest:?}"
    );
}
