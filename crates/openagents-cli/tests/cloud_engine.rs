use std::process::Command;

#[test]
fn cloud_work_refuses_short_login_before_any_network_or_launch() {
    let home = tempfile::tempdir().unwrap();
    let auth = home.path().join("auth.json");
    // Expired synthetic JWT. No real login or token is read.
    std::fs::write(
        &auth,
        r#"{"tokens":{"access_token":"fixture.eyJleHAiOjF9.fixture"}}"#,
    )
    .unwrap();
    for placement in ["gce", "boat"] {
        let output = Command::new(env!("CARGO_BIN_EXE_openagents"))
            .args(["chat", "work", "--on", placement, "--issues", "10419"])
            .env("HOME", home.path())
            .env("OA_CODER_CODEX_AUTH", &auth)
            .env_remove("OA_BOAT_ENGINE_LOGINS")
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success());
        assert!(text.contains("at least 2 h"), "{text}");
        assert!(
            text.contains("open Codex on the Mac once to refresh it, then rerun"),
            "{text}"
        );
        assert!(text.contains("No engine was started"), "{text}");
        assert!(text.contains("--engine-fallback"), "{text}");
    }
}

#[test]
fn cloud_help_explains_fallback_and_refresh() {
    let output = Command::new(env!("CARGO_BIN_EXE_openagents"))
        .args(["chat", "--help"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("--engine-fallback"), "{text}");
    assert!(text.contains("open Codex on the Mac once"), "{text}");
}

#[test]
fn explicit_fallback_passes_token_preflight_for_both_cloud_targets() {
    let home = tempfile::tempdir().unwrap();
    let checkout = tempfile::tempdir().unwrap();
    for placement in ["gce", "boat"] {
        let output = Command::new(env!("CARGO_BIN_EXE_openagents"))
            .args([
                "chat",
                "work",
                "--on",
                placement,
                "--issues",
                "10419",
                "--engine-fallback",
            ])
            .current_dir(checkout.path())
            .env("HOME", home.path())
            .env("OA_CODER_CODEX_AUTH", home.path().join("missing.json"))
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success());
        assert!(!text.contains("No engine was started"), "{text}");
        assert!(!text.contains("unknown option"), "{text}");
        assert!(
            text.contains("checkout") || text.contains("Git") || text.contains("git"),
            "{text}"
        );
    }
}
