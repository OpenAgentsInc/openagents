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
            lines.push(format!("{:<10} cannot start a turn: {why}", "replies"));
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
                    format!("delegated to {}", target.agent.word())
                }
                Chosen::Fallback => "from the fallback".to_string(),
            };
            lines.push(format!(
                "{:<10} {door} because {}",
                "replies", choice.reason
            ));
        }
        Err(why) => lines.push(format!("{:<10} cannot start a turn: {why}", "replies")),
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
    lines.push("delegation targets".to_string());
    for target in &settings.targets {
        let mark = match chosen {
            Some(Chosen::Delegate(picked)) if picked.agent == target.agent => "  (chosen)",
            _ => "",
        };
        let state = match (&target.binary, target.credential) {
            _ if !target.agent.is_cli() => microluna_state(),
            (None, _) => "not installed".to_string(),
            (Some(path), coder_one::delegate::Credential::Missing) => {
                format!("{}, not signed in", path.display())
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
    let microluna = matches!(chosen, Some(Chosen::Delegate(target)) if !target.agent.is_cli());
    let model = settings.model.clone().unwrap_or_else(|| {
        if microluna {
            coder_one::delegate::Agent::Microluna
                .default_model()
                .to_string()
        } else {
            executor.model.clone()
        }
    });
    if microluna {
        let bounds = coder_one::terminal::microluna_policy(false);
        lines.push(format!(
            "{:<10} microluna runs {model} in this process · {} mode · up to {} sessions, {} per group of requirements · up to ${:.2} a turn · runs checks between sessions when a turn changes files · stops after {}s · repository checks from policy {}",
            "executor",
            bounds.mode.word(),
            bounds.max_sessions,
            bounds.max_attempts,
            bounds.spend_usd,
            delegate_door::deadline().as_secs(),
            policy.name.as_deref().unwrap_or("unnamed"),
        ));
    } else {
        lines.push(format!(
        "{:<10} {} runs {model} · effort {} · tools {} · prompt cache {} · stops after {}s · policy {}",
        "executor",
        executor.agent.agent().word(),
        executor.effort.as_deref().unwrap_or("default"),
        executor.tools.as_deref().unwrap_or("default"),
        executor.prompt_cache_ttl.as_deref().unwrap_or("default"),
        delegate_door::deadline().as_secs(),
        policy.name.as_deref().unwrap_or("unnamed"),
    ));
    }

    let (jev, source) = delegate_door::jev_from(&delegate_door::env_value);
    lines.push(match jev {
        Some(_) => format!("{:<10} {source}", "jev"),
        None => format!(
            "{:<10} missing: {source}. Without Jev, a delegated turn's briefing holds only your request.",
            "jev"
        ),
    });

    let workdir = std::env::current_dir().unwrap_or_default();
    lines.push(match delegate_door::boundary_available(&workdir) {
        Ok(()) => format!(
            "{:<10} available ({}): a turn that only reads cannot change files",
            "sandbox",
            coder_boundary::backend_path()
        ),
        Err(why) => format!(
            "{:<10} unavailable: {why}. A delegated turn fails instead of running without write limits.",
            "sandbox"
        ),
    });

    lines.push(format!("{:<10} {}", "fallback", fallback()));
    lines.push(format!(
        "{:<10} {}",
        "trace",
        coder::trace::directory().map_or("off (CODER_TRACE=off)".to_string(), |dir| {
            dir.display().to_string()
        })
    ));
    if settings.mode == Mode::Off {
        lines.push("note       delegation is off, so every turn uses the fallback".to_string());
    }
    (lines, starts)
}

/// Where Microluna's Codex login stands: its path and the hours its
/// access token has left, or why it can't be used. It reads the login and
/// never prints a token.
fn microluna_state() -> String {
    match delegate_door::codex_login(&delegate_door::env_value) {
        Ok(login) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_secs());
            let left = login.hours_left(now).map_or_else(
                || "token expiry unknown".to_string(),
                |hours| format!("access token {hours:.1} hours left"),
            );
            format!(
                "runs in this process, Codex login at {} · {left}",
                login.path.display()
            )
        }
        Err(why) => format!("unavailable: {why}"),
    }
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
            format!("Open Responses endpoint: {model} at {url}")
        }
        Ok(Door::Stub(_)) => "stub: no Open Responses key is set (CODER_DOOR_KEY or \
             CODER_AI_GATEWAY_KEY), so a fallback turn replies with a fixed placeholder message"
            .to_string(),
        Ok(door) => format!("{} ({})", door.name(), door.label()),
        Err(why) => format!("refused: {why}"),
    }
}
