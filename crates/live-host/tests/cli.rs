use serde_json::{Value, json};
use std::process::Command;

fn run(args: &[&str]) -> (bool, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_live-host"))
        .args(args)
        .output()
        .unwrap();
    let value = if output.stdout.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&output.stdout).unwrap()
    };
    (output.status.success(), value)
}

#[test]
fn cli_initializes_and_refuses_unknown_sessions() {
    let temp = tempfile::tempdir().unwrap();
    let key = temp.path().join("key");
    let secret = secp256k1::SecretKey::from_byte_array([41; 32]).unwrap();
    std::fs::write(&key, secret.display_secret().to_string()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let policy = temp.path().join("policy.json");
    std::fs::write(
        &policy,
        serde_json::to_vec(&json!({"owners":[],"tasks":[]})).unwrap(),
    )
    .unwrap();
    let state = temp.path().join("state");
    let (s, k) = (state.to_str().unwrap(), key.to_str().unwrap());
    assert!(
        run(&[
            "init",
            "--state",
            s,
            "--key-file",
            k,
            "--policy",
            policy.to_str().unwrap()
        ])
        .0
    );
    assert!(
        !run(&[
            "init",
            "--state",
            s,
            "--key-file",
            k,
            "--policy",
            policy.to_str().unwrap()
        ])
        .0
    );
    assert!(
        !run(&[
            "status",
            "--state",
            s,
            "--key-file",
            k,
            "--session",
            &"ab".repeat(32)
        ])
        .0
    );
    let (ok, operation) = run(&["device-operation", "--state", s, "--key-file", k]);
    assert!(ok);
    assert!(
        operation["id"]
            .as_str()
            .unwrap()
            .ends_with(":live/device-input")
    );
    let grants = temp.path().join("grants.json");
    std::fs::write(&grants, b"[]").unwrap();
    let (ok, rules) = run(&[
        "input-policy",
        "--state",
        s,
        "--key-file",
        k,
        "--input",
        grants.to_str().unwrap(),
    ]);
    assert!(ok);
    assert_eq!(rules["rules"], json!([]));
    assert_eq!(
        run(&["--help"]).1["help"]
            .as_str()
            .map(|h| h.starts_with("usage")),
        Some(true)
    );
}

#[cfg(unix)]
#[test]
fn caller_cli_reaches_the_resident_without_loading_the_host_key() {
    use live_host::resident::{Operation, Outcome, Request, Response, read_reply};
    use std::{
        os::unix::fs::PermissionsExt,
        time::{Duration, Instant},
    };
    struct Running(std::process::Child);
    impl Drop for Running {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let host = secp256k1::SecretKey::from_byte_array([51; 32]).unwrap();
    let caller = secp256k1::SecretKey::from_byte_array([52; 32]).unwrap();
    let pk = |key: &secp256k1::SecretKey| {
        key.x_only_public_key(&secp256k1::Secp256k1::new())
            .0
            .to_string()
    };
    let state = temp.path().join("state");
    live_host::controller::Controller::initialize(
        state.clone(),
        host,
        live_host::controller::Policy {
            owners: std::collections::BTreeSet::from([pk(&caller)]),
            tasks: Default::default(),
        },
    )
    .unwrap();
    let key = temp.path().join("host.key");
    std::fs::write(&key, host.display_secret().to_string()).unwrap();
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    let socket = state.join("live.sock");
    let mut child = Running(
        Command::new(env!("CARGO_BIN_EXE_live-host"))
            .args([
                "serve",
                "--state",
                state.to_str().unwrap(),
                "--key-file",
                key.to_str().unwrap(),
                "--socket",
                socket.to_str().unwrap(),
                "--synthetic",
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let started = Instant::now();
    while !socket.exists() {
        assert!(child.0.try_wait().unwrap().is_none());
        assert!(started.elapsed() < Duration::from_secs(3));
        std::thread::sleep(Duration::from_millis(5));
    }
    let query = json!({"v":"openagents.livehost-operation.v1","requires":[]});
    let event = workspace_host::host::request(&caller, &pk(&host), query.clone(), live_host::now())
        .unwrap();
    let request = Request::new(Operation::Inspect { event });
    let input = temp.path().join("request.json");
    std::fs::write(&input, serde_json::to_vec(&request).unwrap()).unwrap();
    let (ok, value) = run(&[
        "request",
        "--socket",
        socket.to_str().unwrap(),
        "--input",
        input.to_str().unwrap(),
    ]);
    assert!(ok);
    let response: Response = serde_json::from_value(value).unwrap();
    assert_eq!(response.id, request.id);
    let Outcome::Ok { reply } = response.outcome else {
        panic!("resident operation");
    };
    let body = read_reply(&reply, &pk(&host), &caller, &query).unwrap();
    assert!(
        body["operation"]["id"]
            .as_str()
            .unwrap()
            .ends_with(":live/device-input")
    );
}
