use super::*;
use microcoder_loop::models::{Ask, NextAction};

pub(super) fn target(agent: Cli, installed: bool, credential: Credential) -> Target {
    Target {
        agent: Agent::Cli(agent),
        binary: installed.then(|| PathBuf::from(format!("/bin/{}", agent.program()))),
        credential,
        problem: None,
        refusal: None,
        providers: Vec::new(),
    }
}

/// A Codex usage-limit refusal observed at 1000 that resets at 5000.
pub(super) fn codex_refusal() -> Refusal {
    Refusal::new(
        Provider::Codex,
        capacity::Kind::UsageLimit,
        1_000,
        Some(5_000),
    )
}

/// Microcoder's providers: Codex, `refused` when its book entry holds,
/// and Claude, connected or not.
pub(super) fn providers(codex: bool, refused: bool, claude: bool) -> Vec<ProviderState> {
    let connection = |on: bool| {
        if on {
            Connection::Connected
        } else {
            Connection::Missing("no login".to_string())
        }
    };
    vec![
        ProviderState {
            provider: Provider::Codex,
            model: microcoder::CODEX_MODEL.to_string(),
            connection: connection(codex),
            refusal: refused.then(codex_refusal),
        },
        ProviderState {
            provider: Provider::Claude,
            model: microcoder::CLAUDE_MODEL.to_string(),
            connection: connection(claude),
            refusal: None,
        },
    ]
}

pub(super) fn finish(reply: &str) -> NextAction {
    NextAction {
        rationale: "answer".to_string(),
        commands: Vec::new(),
        view: Vec::new(),
        freeze_tests: false,
        expand: Vec::new(),
        finished: true,
        reply: reply.to_string(),
        ask: Ask::None,
    }
}

pub(super) fn running(command: &str) -> NextAction {
    NextAction {
        finished: false,
        commands: vec![command.to_string()],
        ..finish("")
    }
}

/// The fixed clock the Microcoder tests run at.
pub(super) fn at_two_thousand() -> u64 {
    2_000
}

/// A Microcoder door over `providers` in a fresh workspace, with the
/// capacity book in `dir`, or `None` on a host that cannot enforce a
/// boundary.
pub(super) fn microcoder_door(
    dir: &Path,
    providers: Vec<ProviderState>,
    script: Vec<(Provider, Vec<microcoder::Scripted>)>,
) -> Option<(std::sync::Arc<DelegateDoor>, PathBuf)> {
    let workdir = dir.join("work");
    std::fs::create_dir_all(&workdir).unwrap();
    if let Err(why) = boundary_available(&workdir) {
        eprintln!("skipped: {why}");
        return None;
    }
    let door = DelegateDoor::new(
        Target::microcoder(providers),
        None,
        workdir.clone(),
        None,
        String::new(),
    )
    .writing_under(dir.join("artifacts"))
    .reading_capacity_in(dir.join("tasks"))
    .clocked(at_two_thousand)
    .scripting(script);
    Some((std::sync::Arc::new(door), workdir))
}
