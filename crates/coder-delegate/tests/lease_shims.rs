//! A delegation's environment puts the `cargo` lease shim first on `PATH`
//! once the process turned the shims on, and its briefing says heavy
//! builds are leased. Its own test binary, because turning the shims on is
//! process-wide.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use coder_delegate::delegate::{Agent, Cli, Control, Credential, Launch, SessionArg};

fn cli(agent: Agent, dir: &Path) -> Cli {
    Cli {
        agent,
        binary: Some(dir.join("agent")),
        model: "stand-in".to_string(),
        deadline: Duration::from_secs(30),
        workdir: dir.to_path_buf(),
        artifacts: dir.join("artifacts"),
        artifacts_label: "artifacts".to_string(),
        env: Vec::new(),
        credential: Credential::CliLogin,
        effort: None,
        tools: None,
        prompt_cache_ttl: None,
        codex_config: Vec::new(),
        system: None,
        episode: coder_delegate::deadline::Deadline::unbounded(),
        gate: None,
        granted: None,
        runs: 1,
        control: Control::default(),
    }
}

fn first_on_path(command: &std::process::Command) -> Option<PathBuf> {
    let path = command
        .get_envs()
        .find(|(name, _)| *name == "PATH")
        .and_then(|(_, value)| value)?;
    std::env::split_paths(path).next()
}

#[test]
fn a_delegation_gets_the_cargo_shim_first_on_path() {
    let dir = tempfile::tempdir().unwrap();
    let shims = dir.path().join("lease-shims");
    let launch = Launch {
        session: SessionArg::New(None),
        steerable: false,
    };
    let claude = cli(Agent::ClaudeCode, dir.path());
    let binary = dir.path().join("agent");
    assert_ne!(
        first_on_path(&claude.live_command(&binary, &launch)),
        Some(shims.clone())
    );

    coder_lease::shim::enable(&shims, None).unwrap();
    assert!(shims.join("cargo").is_file());
    for agent in [Agent::ClaudeCode, Agent::Codex, Agent::OpenCode] {
        let cli = cli(agent, dir.path());
        let live = cli.live_command(&binary, &launch);
        assert_eq!(first_on_path(&live), Some(shims.clone()), "{agent:?}");
        let batch = cli.command(&binary, &dir.path().join("b"), &dir.path().join("s"));
        assert_eq!(first_on_path(&batch), Some(shims.clone()), "{agent:?}");
    }
    coder_lease::shim::disable();
}
