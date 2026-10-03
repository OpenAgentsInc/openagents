//! A Codex login kept under `$CODEX_HOME`, not `~/.codex`: the readiness
//! probe says Codex is signed in from it, and the engine process the host
//! launches for the run, whose environment is otherwise cleared, is given
//! the same `CODEX_HOME`, so the run finds the login the probe found
//! (issue #10083). Temporary folders only: `HOME` and `CODEX_HOME` name
//! scratch directories, and no real home is read.
//!
//! One test in this binary, so setting the process environment races no
//! other test.
#![cfg(unix)]

use coder::task::adapter;
use coder::task::autostart::{Engine, Launch, Process};
use microcoder_loop::capacity::{Connection, Provider, probe};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

/// A ChatGPT login whose access token names no expiry, which a check lets
/// through.
fn login() -> String {
    r#"{"auth_mode":"chatgpt","tokens":{"access_token":"a.b.c","account_id":"acct-scratch"}}"#
        .to_string()
}

#[test]
fn a_run_admitted_from_codex_home_finds_the_same_login() {
    let temp = tempfile::tempdir().unwrap();
    let person = temp.path().join("person");
    let codex = temp.path().join("elsewhere/codex");
    std::fs::create_dir_all(&person).unwrap();
    std::fs::create_dir_all(&codex).unwrap();
    std::fs::write(codex.join("auth.json"), login()).unwrap();
    // SAFETY: the only test in this binary; no other thread reads the
    // environment while it changes.
    unsafe {
        std::env::set_var("HOME", &person);
        std::env::set_var("CODEX_HOME", &codex);
    }
    assert!(!person.join(".codex/auth.json").exists());

    // The readiness probe: signed in, from CODEX_HOME.
    assert_eq!(probe(Provider::Codex), Connection::Connected);
    let found = codex_transport::codex::Login::default_path().unwrap();
    assert_eq!(found, codex.join("auth.json"));

    // The engine the host starts for the run records its environment.
    let seen = temp.path().join("environment");
    let controller = temp.path().join("controller");
    std::fs::write(
        &controller,
        format!(
            "#!/bin/sh\n/usr/bin/env > '{}'\nprintf '%s\\n' '{{\"owner_process\":1,\"grant_digest\":\"d\"}}'\n",
            seen.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&controller, std::fs::Permissions::from_mode(0o700)).unwrap();
    let engine = Engine {
        adapter: adapter::NAME.into(),
        controller: controller.clone(),
        model: "gpt-6-luna".into(),
        effort: None,
        max_steps: None,
        wall_seconds: None,
        memory_bytes: 1 << 30,
        write_workspace: false,
        decision_endpoint: "https://api.typesafe.ai".into(),
        decision_model: "jev-latest".into(),
        routes: Vec::new(),
        usage_probe: None,
        access: adapter::Access::Boundary,
        claude: coder::task::autostart::ClaudeRuns::default(),
    };
    let grant = temp.path().join("grant.json");
    let store = temp.path().join("store");
    let launched = Process.launch(&engine, &grant, &store).unwrap();
    assert_eq!(launched.grant_digest, "d");

    let environment = std::fs::read_to_string(&seen).unwrap();
    let variable = |name: &str| {
        environment
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{name}=")))
            .map(PathBuf::from)
    };
    assert_eq!(variable("CODEX_HOME").as_deref(), Some(codex.as_path()));
    assert_eq!(variable("HOME").as_deref(), Some(person.as_path()));
    // In that environment the engine resolves CODEX_HOME before ~/.codex,
    // so its login is the one the probe found.
    assert_eq!(
        variable("CODEX_HOME").unwrap().join("auth.json"),
        found,
        "the run looks where the probe looked"
    );
}
