#![cfg(unix)]

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn command(home: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_openagents"));
    cmd.env("HOME", home)
        .env("XDG_RUNTIME_DIR", home)
        .env_remove("OPENAGENTS_RELAY")
        .env_remove("OPENAGENTS_HOST_LISTEN")
        .env_remove("OPENAGENTS_HOST_GENERATION")
        .env_remove("OPENAGENTS_HOST_READY_FILE")
        .env_remove("OPENAGENTS_CHAT_HOME")
        .stdin(Stdio::null());
    cmd
}

#[test]
fn fresh_home_names_the_missing_store_and_one_startup_command() {
    let home = tempfile::tempdir().unwrap();
    let output = command(home.path())
        .args(["host", "serve"])
        .output()
        .unwrap();
    let error = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{error}");
    assert!(
        error.contains("host access store is not initialized"),
        "{error}"
    );
    assert!(
        error.contains("openagents host serve --keys \"$HOME/.openagents/connect\" --iroh"),
        "{error}"
    );
    assert!(
        !error.contains("artifact or transport check failed"),
        "{error}"
    );
}

#[test]
fn long_socket_is_reported_before_missing_access_store() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("x".repeat(110));
    let output = command(home.path())
        .args(["host", "serve", "--control-socket"])
        .arg(path)
        .output()
        .unwrap();
    let error = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{error}");
    assert!(error.contains("control socket path is too long"), "{error}");
    assert!(error.contains("--control-socket PATH"), "{error}");
    assert!(!error.contains("host access:"), "{error}");
}

struct Stop(Child);
impl Drop for Stop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn keys_and_control_start_without_relay_and_status_explains_missing_iroh() {
    let home = tempfile::tempdir().unwrap();
    let keys = home.path().join("keys");
    let log_path = home.path().join("host.log");
    let log = std::fs::File::create(&log_path).unwrap();
    let mut host = Stop(
        command(home.path())
            .args(["host", "serve", "--keys"])
            .arg(keys)
            .arg("--control")
            .stdout(Stdio::null())
            .stderr(Stdio::from(log))
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        assert!(
            host.0.try_wait().unwrap().is_none(),
            "host exited before status answered: {}",
            std::fs::read_to_string(&log_path).unwrap()
        );
        let output = command(home.path())
            .args(["connect", "status"])
            .output()
            .unwrap();
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            assert!(text.contains("Offline:"), "{text}");
            assert!(text.contains("without --iroh"), "{text}");
            assert!(text.contains("openagents host serve --iroh"), "{text}");
            let output = command(home.path())
                .args(["--json", "connect", "status"])
                .output()
                .unwrap();
            assert!(output.status.success());
            let status: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert!(
                status["offline_reason"]
                    .as_str()
                    .unwrap()
                    .contains("--iroh")
            );
            break;
        }
        assert!(Instant::now() < deadline, "status never answered");
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn relay_environment_is_used_but_explicit_relay_takes_precedence() {
    let home = tempfile::tempdir().unwrap();
    let output = command(home.path())
        .env("OPENAGENTS_RELAY", "not-a-relay")
        .args(["host", "serve"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&output.stderr).contains("relay is not allowed"));
    let output = command(home.path())
        .env("OPENAGENTS_RELAY", "not-a-relay")
        .args(["host", "serve", "--relay", "wss://relay.openagents.com"])
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("host access store is not initialized")
    );
}
