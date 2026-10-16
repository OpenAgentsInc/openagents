use serde_json::{Value, json};
use std::{path::Path, process::Command};

fn run(args: &[&str]) -> (bool, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_session-host"))
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

fn write_key(path: &Path, byte: u8) {
    let secret = secp256k1::SecretKey::from_byte_array([byte; 32]).unwrap();
    std::fs::write(path, secret.display_secret().to_string()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
}

#[test]
fn cli_admits_an_adapter_and_refuses_opening_without_an_environment() {
    let temp = tempfile::tempdir().unwrap();
    let key = temp.path().join("key");
    write_key(&key, 31);
    let state = temp.path().join("state");
    let policy = temp.path().join("policy.json");
    std::fs::write(
        &policy,
        serde_json::to_vec(&json!({"owners":[],"controllers":[]})).unwrap(),
    )
    .unwrap();
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
    let (ok, adapter) = run(&[
        "admit",
        "--state",
        s,
        "--key-file",
        k,
        "--engine",
        "loop-boundary",
    ]);
    assert!(ok, "{adapter}");
    assert_eq!(adapter["schema"], "openagents.session-adapter.v1");
    let (ok, _) = run(&[
        "admit",
        "--state",
        s,
        "--key-file",
        k,
        "--engine",
        "unknown",
    ]);
    assert!(!ok);
    let (ok, _) = run(&[
        "status",
        "--state",
        s,
        "--key-file",
        k,
        "--session",
        &"ab".repeat(32),
    ]);
    assert!(!ok);
}

#[test]
#[cfg(unix)]
fn cli_resident_owns_engines_and_accepts_clients_without_the_host_key() {
    use session_host::{
        host::{Bundle, Policy, SessionHost},
        resident::{Operation, Request},
    };
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("state");
    let keyfile = temp.path().join("key");
    write_key(&keyfile, 32);
    let secret = secp256k1::SecretKey::from_byte_array([32; 32]).unwrap();
    SessionHost::initialize(root.clone(), secret, Policy::default()).unwrap();
    let socket = root.join("host.sock");
    let mut child = Child(
        Command::new(env!("CARGO_BIN_EXE_session-host"))
            .args([
                "serve",
                "--state",
                root.to_str().unwrap(),
                "--key-file",
                keyfile.to_str().unwrap(),
                "--socket",
                socket.to_str().unwrap(),
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap(),
    );
    let started = std::time::Instant::now();
    while !socket.exists() {
        assert!(child.0.try_wait().unwrap().is_none());
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        !run(&[
            "admit",
            "--state",
            root.to_str().unwrap(),
            "--key-file",
            keyfile.to_str().unwrap(),
            "--engine",
            "loop-mid-turn"
        ])
        .0
    );
    let host = secret
        .x_only_public_key(&secp256k1::Secp256k1::new())
        .0
        .to_string();
    let event = workspace_host::host::request(
        &secret,
        &host,
        json!({"v":"unsupported"}),
        session_host::now(),
    )
    .unwrap();
    let mut request = Request::new(Operation::Submit {
        bundle: Bundle {
            event,
            artifacts: Default::default(),
        },
    });
    request.id = "invalid".into();
    let input = temp.path().join("request.json");
    std::fs::write(&input, serde_json::to_vec(&request).unwrap()).unwrap();
    let (ok, response) = run(&[
        "request",
        "--socket",
        socket.to_str().unwrap(),
        "--input",
        input.to_str().unwrap(),
    ]);
    assert!(!ok);
    assert_eq!(response["outcome"]["code"], "malformed");
    assert!(child.0.try_wait().unwrap().is_none());
}
