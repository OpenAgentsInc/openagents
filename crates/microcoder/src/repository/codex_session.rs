//! A repository turn on a lean Codex session (#10250).
//!
//! The Codex counterpart of [`super::claude_session`] (#10246): one
//! `codex exec --json` session, briefed by Jev, in place of Microcoder's
//! step loop, whose per-step requests read only 6-9% of their input from
//! Codex's prompt cache on the #10209 panel. A Codex route whose endpoint
//! is [`CODEX_SESSION_ENDPOINT`] runs it:
//!
//! - **Briefing**: the delegate recipe's briefing, knowledge, and frozen
//!   checks ([`super::recipe`]) are the session's input, as on a whole
//!   agent; with the recipe off, the request alone.
//! - **Settings**: [`executor`]: the route's model (the owner's
//!   `gpt-6.1-sol` by default) at medium reasoning effort
//!   (`model_reasoning_effort`, low for a question), the headless core
//!   system prompt as `model_instructions_file`, and Codex's own tools and
//!   per-session prompt cache, which have no setting; standard processing
//!   (`service_tier="default"`) whatever the person's Codex config says;
//!   the recipe's `codex-session` row.
//! - **Process**: `codex exec --json --dangerously-bypass-approvals-and-sandbox`,
//!   supervised in its own process group by `coder_delegate`'s CLI adapter,
//!   in the workspace, with the owner's login environment and Codex's own
//!   login (`$CODEX_HOME`, else `~/.codex`). Full access only: the grant
//!   names this endpoint only then (`autostart::Engine::codex`).
//! - **Follow-ups**: a later turn of the same task resumes the session
//!   (`codex exec resume`); one that can't resume starts afresh.
//! - **Cost**: Codex reports no cost, so the turn's cost is the list-price
//!   estimate from the usage it reports (`coder_delegate::delegate::codex_cost`),
//!   with the cache reads in the turn's stats.
//!
//! The transcript, capacity, and cancellation behave as on the Claude
//! session ([`super::lean_session`]).

use std::path::{Path, PathBuf};

use coder::task::adapter::{Host, Route as GrantRoute};
use coder::task::capacity::Provider;
use coder_delegate::delegate::{Agent, Credential};
use coder_delegate::policy::{AgentName, ExecutorPolicy, SessionPolicy};

use super::devin::Turn;
use super::lean_session::Lean;

pub use coder::task::capacity::CODEX_SESSION_ENDPOINT;

/// The step extension that names the Codex session a turn used, which the
/// next turn of the task resumes.
pub const SESSION_NOTE: &str = "codex_session";
/// The engine a lean-session turn records.
pub const ENGINE: &str = "codex-session";
/// The effort when neither the recipe nor the route names one: the owner's
/// default for `gpt-6.1-sol`.
pub const EFFORT: &str = "medium";
/// Codex settings the session always passes: standard processing, the
/// tier the loop's requests use and the list-price estimate assumes. A
/// person's `service_tier = "fast"` would bill priority processing at a
/// multiple of it.
pub const CODEX_CONFIG: &[&str] = &["service_tier=\"default\""];

/// Codex for this host, or why there is none: `CODER_ONE_CODEX_BIN`, else
/// the first `codex` on `PATH`, else `~/.local/bin/codex`.
pub(crate) fn binary() -> Result<PathBuf, String> {
    let path = coder_delegate::delegate::binary(Agent::Codex, |name| std::env::var(name).ok())
        .ok_or_else(|| {
            format!(
                "no codex binary: set {}, or install Codex and run `codex login`",
                Agent::Codex.binary_variable()
            )
        })?;
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!("{} isn't a file", path.display()))
    }
}

/// The lean Codex session's executor policy for `model` at `effort`, with
/// no deadline but the terminal turn's quiet guard.
#[must_use]
pub fn executor(model: &str, effort: &str) -> ExecutorPolicy {
    ExecutorPolicy {
        agent: AgentName::Codex,
        version: None,
        model: model.to_owned(),
        effort: Some(effort.to_owned()),
        // Codex has no tool-list or cache-TTL setting.
        tools: None,
        prompt_cache_ttl: None,
        deadline_sec: coder_delegate::terminal::TURN_WALL.as_secs(),
        system: coder_delegate::system::Policy::preset("core"),
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

/// The lean Codex session's settings for the shared turn.
pub(crate) const LEAN: Lean = Lean {
    engine: ENGINE,
    agent: "Codex",
    kind: SESSION_NOTE,
    recipe_row: route_contract::recipe::CODEX_SESSION,
    effort: EFFORT,
    provider: Provider::Codex,
    executor,
    credential,
    codex_config: CODEX_CONFIG,
};

/// The credential Codex will use: its `auth.json`, else `OPENAI_API_KEY`.
/// The loop's capacity probe already found a signed-in Codex, so a login
/// this can't see is still Codex's own.
fn credential(home: Option<&Path>) -> Credential {
    let env = |name: &str| match name {
        "HOME" => home
            .map(|home| home.to_string_lossy().into_owned())
            .or_else(|| std::env::var(name).ok()),
        _ => std::env::var(name).ok(),
    };
    let auth = coder_delegate::delegate::codex_auth_file(env).is_some_and(|path| path.is_file());
    match Credential::detect_codex(|name| std::env::var(name).ok(), auth) {
        Credential::Missing => Credential::CodexAuthFile,
        found => found,
    }
}

/// Runs the turn as one lean Codex session.
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
    fn the_executor_is_one_codex_exec_session_on_the_core_prompt() {
        let policy = executor("gpt-6.1-sol", "medium");
        assert_eq!(policy.effort.as_deref(), Some("medium"));
        assert!(policy.tools.is_none() && policy.prompt_cache_ttl.is_none());
        let system = policy.system.clone().expect("the headless core prompt");
        assert_eq!(system.mode, coder_delegate::system::Mode::Replace);
        assert!(system.validate(Agent::Codex).is_empty());
        let cli = coder_delegate::policy::executor(
            &policy,
            ExecutorHost {
                binary: None,
                credential: Credential::CodexAuthFile,
                workdir: PathBuf::from("/w"),
                artifacts: PathBuf::from("/a"),
                artifacts_label: "a".into(),
                env: Vec::new(),
            },
        );
        let mut cli = cli;
        cli.codex_config = CODEX_CONFIG.iter().map(|&s| s.to_owned()).collect();
        assert_eq!(cli.agent, Agent::Codex);
        let args = cli.live_args(&coder_delegate::delegate::Launch {
            session: coder_delegate::delegate::SessionArg::Resume("t-1".into()),
            steerable: false,
        });
        let args = args.join(" ");
        assert!(args.starts_with("exec resume t-1 --json"), "{args}");
        assert!(args.contains("-m gpt-6.1-sol"), "{args}");
        assert!(args.contains("model_reasoning_effort=medium"), "{args}");
        assert!(args.contains("model_instructions_file="), "{args}");
        assert!(args.contains("-c service_tier=\"default\""), "{args}");
        assert!(
            args.contains("--dangerously-bypass-approvals-and-sandbox"),
            "{args}"
        );
    }

    #[test]
    fn the_session_books_limits_against_codex_at_the_recipe_effort() {
        assert_eq!(LEAN.provider, Provider::Codex);
        assert_eq!(
            route_contract::recipe::effort(
                LEAN.recipe_row,
                Some(route_contract::recipe::TaskClass::Hard),
                Some("medium"),
            )
            .as_deref(),
            Some("medium")
        );
    }
}
