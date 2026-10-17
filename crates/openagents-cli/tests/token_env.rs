//! An `OPENAGENTS_TOKEN` that isn't a login key is refused, never replaced
//! by the login saved on this computer (#11935).

use std::process::Command;

#[test]
fn a_token_that_is_not_a_login_is_refused() {
    let home = tempfile::tempdir().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_openagents"))
        .args(["agents", "list"])
        .env("HOME", home.path())
        .env("OPENAGENTS_SCRATCH", home.path().join("scratch"))
        .env("OPENAGENTS_TOKEN", "not-a-login-key")
        .env_remove("OPENAGENTS_APP_TOKEN")
        .output()
        .unwrap();
    assert!(!out.status.success());
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        said.contains("OPENAGENTS_TOKEN isn't a login key"),
        "{said}"
    );
}
