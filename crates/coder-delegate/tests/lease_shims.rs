//! A delegation's environment puts the shim directory first on `PATH`
//! once the process turned the shims on; the `screencapture` shim is
//! there, and the `cargo` shim is not, unless the machine opted back in.
//! Its own test binary, because turning the shims on is process-wide.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use coder_delegate::delegate::{Agent, Cli, Control, Credential};

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
        hooks: None,
        system: None,
        episode: coder_delegate::deadline::Deadline::unbounded(),
        gate: None,
        granted: None,
        runs: 1,
        control: Control::default(),
    }
}

fn configured(cli: &Cli, binary: &Path) -> std::process::Command {
    let mut command = std::process::Command::new(binary);
    cli.configure_command(&mut command);
    command
}

fn first_on_path(command: &std::process::Command) -> Option<PathBuf> {
    let path = command
        .get_envs()
        .find(|(name, _)| *name == "PATH")
        .and_then(|(_, value)| value)?;
    std::env::split_paths(path).next()
}

#[test]
fn a_delegation_gets_the_shim_directory_first_on_path() {
    let dir = tempfile::tempdir().unwrap();
    let shims = dir.path().join("lease-shims");
    let claude = cli(Agent::ClaudeCode, dir.path());
    let binary = dir.path().join("agent");
    assert_ne!(
        first_on_path(&configured(&claude, &binary)),
        Some(shims.clone())
    );

    coder_lease::shim::enable(&shims, None).unwrap();
    assert!(shims.join("screencapture").is_file());
    for agent in [Agent::ClaudeCode, Agent::Codex, Agent::OpenCode] {
        let cli = cli(agent, dir.path());
        let live = configured(&cli, &binary);
        assert_eq!(first_on_path(&live), Some(shims.clone()), "{agent:?}");
    }
    coder_lease::shim::disable();
}

#[test]
fn a_delegation_gets_durable_scratch_and_its_briefing_names_it() {
    use coder_delegate::delegate::{BRIEFING_CAP, Briefing, BriefingInputs, DURABLE_SCRATCH};
    let dir = tempfile::tempdir().unwrap();
    let inputs = BriefingInputs {
        instruction: "Fix the parser.".to_string(),
        requirements: Vec::new(),
        files: Vec::new(),
        spans: Vec::new(),
        commands: Vec::new(),
        last_output: None,
        conclusion: "Nothing explored.".to_string(),
        directions: "Work here.".to_string(),
    };
    let binary = dir.path().join("agent");
    let scratch_of = |command: &std::process::Command| {
        command
            .get_envs()
            .find(|(name, _)| *name == coder_lease::scratch::SCRATCH_VAR)
            .and_then(|(_, value)| value.map(PathBuf::from))
    };
    coder_lease::scratch::disable();
    let claude = cli(Agent::ClaudeCode, dir.path());
    assert_eq!(scratch_of(&configured(&claude, &binary)), None);
    assert!(
        !Briefing::build(&inputs, BRIEFING_CAP)
            .text
            .contains("OPENAGENTS_SCRATCH")
    );

    let made = coder_lease::scratch::enable(&dir.path().join("scratch")).unwrap();
    assert!(made.is_dir());
    assert!(made.starts_with(dir.path().join("scratch")));
    for agent in [Agent::ClaudeCode, Agent::Codex, Agent::OpenCode] {
        let cli = cli(agent, dir.path());
        assert_eq!(
            scratch_of(&configured(&cli, &binary)),
            Some(made.clone()),
            "{agent:?}"
        );
    }
    let text = Briefing::build(&inputs, BRIEFING_CAP).text;
    assert!(text.contains(DURABLE_SCRATCH.trim()), "{text}");
    coder_lease::scratch::disable();
}
