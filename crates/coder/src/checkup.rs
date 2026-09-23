//! `coder doctor`: which door a turn would use, and why.
//!
//! The report answers what an operator asks before trusting a turn: which
//! Coder this is, which door answers, the delegation targets and whether
//! each has a credential, the Jev key the briefing depends on, whether
//! this host can enforce the executor's boundary, and what answers when
//! no target is available. It reads the environment and the filesystem
//! and spawns nothing, so it costs no tokens. A credential is reported as
//! found, not as verified: the first turn is the call that proves it.

use coder::delegate_door::{self, Chosen, Mode, Settings};
use coder::generate::{DEFAULT_DOOR_URL, Door};
use coder::identity;

/// The code `coder doctor` exits with when a turn would not start.
const EXIT_BROKEN: u8 = 1;

/// Prints the report and returns the process's exit code: zero when a turn
/// would start, one when the environment would refuse it.
pub fn run() -> u8 {
    let (lines, starts) = report();
    for line in lines {
        println!("{line}");
    }
    if starts { 0 } else { EXIT_BROKEN }
}

/// The report's lines, and whether a turn would start.
fn report() -> (Vec<String>, bool) {
    let mut lines = vec![identity::line()];
    if let Ok(path) = std::env::current_exe() {
        lines.push(format!("{:<10} {}", "binary", path.display()));
    }
    let settings = match Settings::read() {
        Ok(settings) => settings,
        Err(why) => {
            lines.push(format!("{:<10} refused: {why}", "door"));
            return (lines, false);
        }
    };
    let choice = settings.choose();
    let starts = choice.is_ok();
    let chosen = choice.as_ref().ok().map(|choice| &choice.chosen);
    match &choice {
        Ok(choice) => {
            let door = match &choice.chosen {
                Chosen::Delegate(target) => {
                    format!("{} ({})", delegate_door::NAME, target.agent.word())
                }
                Chosen::Fallback => "fallback".to_string(),
            };
            lines.push(format!("{:<10} {door} because {}", "door", choice.reason));
        }
        Err(why) => lines.push(format!("{:<10} refused: {why}", "door")),
    }

    lines.push(format!(
        "{:<10} {}={} · {}={} · {}={}",
        "settings",
        delegate_door::MODE_VAR,
        settings.mode.word(),
        delegate_door::AGENT_VAR,
        settings
            .preferred
            .map_or("unset", coder_one::delegate::Agent::word),
        delegate_door::MODEL_VAR,
        settings.model.as_deref().unwrap_or("unset"),
    ));
    lines.push("targets".to_string());
    for target in &settings.targets {
        let mark = match chosen {
            Some(Chosen::Delegate(picked)) if picked.agent == target.agent => "  (chosen)",
            _ => "",
        };
        let state = match (&target.binary, target.credential) {
            (None, _) => "not installed".to_string(),
            (Some(path), coder_one::delegate::Credential::Missing) => {
                format!("{}, no credential", path.display())
            }
            (Some(path), credential) => {
                format!(
                    "{}, credential found ({})",
                    path.display(),
                    credential.word()
                )
            }
        };
        lines.push(format!("  {:<12} {state}{mark}", target.agent.word()));
    }

    let policy = coder_one::terminal::policy();
    let executor = &policy.policy.executor;
    let model = settings
        .model
        .clone()
        .unwrap_or_else(|| executor.model.clone());
    lines.push(format!(
        "{:<10} {} on {model} · effort {} · tools {} · prompt cache {} · deadline {}s · policy {}",
        "executor",
        executor.agent.agent().word(),
        executor.effort.as_deref().unwrap_or("default"),
        executor.tools.as_deref().unwrap_or("default"),
        executor.prompt_cache_ttl.as_deref().unwrap_or("default"),
        delegate_door::deadline().as_secs(),
        policy.name.as_deref().unwrap_or("unnamed"),
    ));

    let (jev, source) = delegate_door::jev_from(&delegate_door::env_value);
    lines.push(match jev {
        Some(_) => format!("{:<10} {source}", "jev"),
        None => format!(
            "{:<10} missing: {source}. A delegated turn's briefing carries the request alone.",
            "jev"
        ),
    });

    let workdir = std::env::current_dir().unwrap_or_default();
    lines.push(match delegate_door::boundary_available(&workdir) {
        Ok(()) => format!(
            "{:<10} available ({}): read-only turns cannot write the workspace",
            "boundary",
            coder_boundary::backend_path()
        ),
        Err(why) => format!(
            "{:<10} unavailable: {why}. A delegated turn fails rather than run unbounded.",
            "boundary"
        ),
    });

    lines.push(format!("{:<10} {}", "fallback", fallback()));
    lines.push(format!(
        "{:<10} {}",
        "trace",
        coder::trace::directory().map_or("off (CODER_TRACE)".to_string(), |dir| {
            dir.display().to_string()
        })
    ));
    if settings.mode == Mode::Off {
        lines.push("note       delegation is off, so every turn uses the fallback".to_string());
    }
    (lines, starts)
}

/// What answers when no target is available: the door the environment
/// builds without delegation.
fn fallback() -> String {
    match Door::from_env() {
        Ok(Door::Live(door)) => {
            let url = std::env::var("CODER_DOOR_URL")
                .ok()
                .filter(|url| !url.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_DOOR_URL.to_string());
            let model = Door::Live(door).model().to_string();
            format!("live Open Responses door: {model} at {url}")
        }
        Ok(Door::Stub(_)) => "stub: no Open Responses key is set (CODER_DOOR_KEY or \
             CODER_AI_GATEWAY_KEY), so a fallback turn answers with a canned line"
            .to_string(),
        Ok(door) => format!("{} ({})", door.name(), door.label()),
        Err(why) => format!("refused: {why}"),
    }
}
