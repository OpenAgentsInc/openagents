//! A repository turn on a lean Claude Code session (#10246).
//!
//! The cost audit's 61% and 63% savings
//! (`docs/cost/2026-10-02-system-one-cost-efficiency-audit.md`) came from
//! Jev briefing one Claude Code session with lean settings, not from
//! Microcoder's step loop, whose fresh `claude -p` per step writes most of
//! its prompt to the cache and reads little back (#10209). A Claude route
//! whose endpoint is [`CLAUDE_SESSION_ENDPOINT`] runs that configuration:
//!
//! - **Briefing**: the delegate recipe's briefing, knowledge, and frozen
//!   checks ([`super::recipe`]) are the session's input, as on a whole
//!   agent; with the recipe off, the request alone.
//! - **Lean settings**: [`executor`]: Opus 5.5 (the route's model), six
//!   tools (`Bash, Read, Edit, Write, Glob, Grep`), the lean-session
//!   system prompt ([`SYSTEM`]) in place of Claude Code's own: the headless
//!   core, but stopping once the named checks pass and taking few,
//!   parallel steps (#10254), the five-minute prompt
//!   cache, no claude.ai connectors, and medium effort (low for a
//!   question), the recipe's `claude-session` row.
//! - **Process**: `claude -p --output-format stream-json`, supervised in its
//!   own process group by `coder_delegate`'s CLI adapter, in the workspace,
//!   with the owner's login environment. Full access only: the grant names
//!   this endpoint only then (`autostart::Engine::claude`).
//! - **Transcript**: commands, file changes, and the reply are appended to
//!   the ATIF transcript as they arrive, bounded.
//! - **Follow-ups**: a later turn of the same task resumes the session
//!   (`--resume`); one that can't resume starts afresh.
//! - **Cost**: Claude Code's own `total_cost_usd` (list price on a
//!   subscription), with the cache reads and writes in the turn's stats.
//! - **Capacity**: a session that ends on a usage or rate limit before it
//!   answered is a refusal for the capacity book, as on the loop.
//! - **Cancellation**: a cancelled task stops the session's process.

use std::path::{Path, PathBuf};

use coder::task::adapter::{Host, Route as GrantRoute};
use coder::task::capacity::Provider;
use coder_delegate::delegate::Credential;
use coder_delegate::policy::{AgentName, ExecutorPolicy, SessionPolicy};

use super::devin::Turn;
use super::lean_session::Lean;

pub use coder::task::capacity::CLAUDE_SESSION_ENDPOINT;

/// The step extension that names the Claude Code session a turn used,
/// which the next turn of the task resumes.
pub const SESSION_NOTE: &str = "claude_session";
/// The engine a lean-session turn records.
pub const ENGINE: &str = "claude-code-session";
/// The six tools the audit's lean session runs with.
pub const TOOLS: &str = "Bash,Read,Edit,Write,Glob,Grep";
/// The prompt-cache TTL: cache writes at 1.25x input instead of 2x.
pub const PROMPT_CACHE_TTL: &str = "5m";
/// The effort when neither the recipe nor the route names one.
pub const EFFORT: &str = "medium";
/// The system prompt preset. The headless `core` asks for tests nobody
/// named, and with it the session took more turns than raw Claude Code on
/// small fixes (#10254); `lean-session` stops once the named checks pass
/// and asks for few, parallel steps.
pub const SYSTEM: &str = "lean-session";

/// Claude Code for this host, or why there is none.
pub(crate) fn binary() -> Result<PathBuf, String> {
    if let Some(named) =
        std::env::var_os(microcoder_loop::claude::BIN_VAR).filter(|value| !value.is_empty())
    {
        let path = PathBuf::from(named);
        return if path.is_file() {
            Ok(path)
        } else {
            Err(format!(
                "{} names {}, which isn't a file",
                microcoder_loop::claude::BIN_VAR,
                path.display()
            ))
        };
    }
    microcoder_loop::claude::locate().ok_or_else(|| {
        "no claude binary: set CLAUDE_BIN, or install Claude Code and run `claude` to log in"
            .to_owned()
    })
}

/// The lean session's executor policy for `model` at `effort`: the
/// audit's `matched-opus-medium-v8` executor, with no deadline but the
/// terminal turn's quiet guard.
#[must_use]
pub fn executor(model: &str, effort: &str) -> ExecutorPolicy {
    ExecutorPolicy {
        agent: AgentName::ClaudeCode,
        version: None,
        model: model.to_owned(),
        effort: Some(effort.to_owned()),
        tools: Some(TOOLS.to_owned()),
        prompt_cache_ttl: Some(PROMPT_CACHE_TTL.to_owned()),
        deadline_sec: coder_delegate::terminal::TURN_WALL.as_secs(),
        system: coder_delegate::system::Policy::preset(SYSTEM),
        session: Some(SessionPolicy {
            steer: None,
            stop_when: Some(coder_delegate::session::Trigger::Quiet {
                ms: u64::try_from(coder_delegate::terminal::TURN_QUIET.as_millis())
                    .unwrap_or(u64::MAX),
            }),
            resume: None,
        }),
        microluna: None,
    }
}

/// The lean Claude Code session's settings for the shared turn.
pub(crate) const LEAN: Lean = Lean {
    engine: ENGINE,
    agent: "Claude Code",
    kind: SESSION_NOTE,
    recipe_row: route_contract::recipe::CLAUDE_SESSION,
    effort: EFFORT,
    provider: Provider::Claude,
    executor,
    credential,
    codex_config: &[],
};

/// The credential Claude Code will use. The loop's capacity probe already
/// found a signed-in Claude Code; a login kept in the system keychain has
/// no credentials file to find.
fn credential(home: Option<&Path>) -> Credential {
    match Credential::detect(
        |name| std::env::var(name).ok(),
        coder_delegate::delegate::stored_login(home),
    ) {
        Credential::Missing => Credential::CliLogin,
        found => found,
    }
}

/// Runs the turn as one lean Claude Code session.
pub(crate) async fn turn(
    host: &Host,
    route: &GrantRoute,
    program: PathBuf,
    recipe: Option<&mut super::recipe::Recipe>,
) -> Turn {
    super::lean_session::turn(&LEAN, host, route, program, recipe).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_delegate::policy::ExecutorHost;

    #[test]
    fn the_executor_is_the_audits_lean_session() {
        let policy = executor("claude-opus-5-5", "medium");
        assert_eq!(policy.tools.as_deref(), Some(TOOLS));
        assert_eq!(policy.prompt_cache_ttl.as_deref(), Some("5m"));
        assert_eq!(policy.effort.as_deref(), Some("medium"));
        let system = policy.system.clone().expect("a trimmed system prompt");
        assert_eq!(system.mode, coder_delegate::system::Mode::Replace);
        assert!(system.sections.iter().any(|id| id == "pace"));
        assert!(!system.sections.iter().any(|id| id == "verify"));
        assert!(
            system
                .validate(coder_delegate::delegate::Agent::ClaudeCode)
                .is_empty()
        );
        let cli = coder_delegate::policy::executor(
            &policy,
            ExecutorHost {
                binary: None,
                credential: Credential::CliLogin,
                workdir: PathBuf::from("/w"),
                artifacts: PathBuf::from("/a"),
                artifacts_label: "a".into(),
                env: Vec::new(),
            },
        );
        assert_eq!(cli.agent, coder_delegate::delegate::Agent::ClaudeCode);
        assert_eq!(cli.model, "claude-opus-5-5");
        assert!(cli.system.is_some());
    }
}
