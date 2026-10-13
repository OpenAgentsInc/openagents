//! One Coder permission and its floors on every harness (#11475, phase 5
//! of #11451).
//!
//! **One permission name, one table.** Coder has one permission today,
//! [`PERMISSION`] (`bypass`, owner decision 2): the agent's tools run
//! without asking. [`TABLE`] maps it to each harness's own modes, and a test
//! fails when a harness's launch drifts from its row. A harness that cannot
//! honor a request refuses it at start (the harness contract, #11452).
//!
//! **Floors on every harness.** Under bypass, Claude Code never calls
//! `can_use_tool` and Codex never asks, so the floors of
//! [`crate::risk_policy`] (#11170) reached only Coder's own tools. Now the
//! same rules decide on every harness, host-owned and deny-or-ask only:
//!
//! - the Coder loop, in its tool gate: the Run tool's commands and file
//!   reads ([`hold`]);
//! - Claude Code, in a `PreToolUse` hook Coder installs in the session
//!   through the Coder SDK ([`sdk_hook`], #11496);
//! - Codex, in a `PreToolUse` hook Coder passes on its command line
//!   ([`codex_args`]), which runs this program's [`HOOK_COMMAND`].
//!
//! [`classify`] reads a tool call: a shell command is split into its simple
//! commands and each program's argument list is matched against a declared
//! table of floored commands (an explicit command-line parser, never a
//! reading of the person's words); a file read is matched on the path's
//! shape. A production deploy, merging a pull request, reading or changing a
//! secret, a payment, and a DNS change are floored. The checkout's
//! approval policy gives each its rule ([`crate::risk_policy::Policy`]), and
//! a file never lowers a floored rule below `ask`, so a file a model can
//! write never opens a floor. `ask` shows the person one plain choice
//! screen where a person is there to answer (the terminal's approval card);
//! with nobody to answer, and always on Codex, the call is held: refused,
//! with a note telling the agent to hand that step back to the person.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use serde_json::{Value, json};

use crate::harness_plugin::Harness;
use crate::risk_policy::{self, Rule};

/// Coder's one permission.
pub const PERMISSION: &str = "bypass";

/// How one harness honors [`PERMISSION`], and where its floors are
/// enforced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub harness: Harness,
    pub permission: &'static str,
    /// The harness's own mode or flags for it.
    pub native: &'static [&'static str],
    /// Where the floors are enforced.
    pub floors: &'static str,
}

/// [`PERMISSION`] on each harness. `docs/coder/permissions.md` shows the
/// same table; [`tests`] fail when a launch drifts from it.
pub const TABLE: [Row; 3] = [
    Row {
        harness: Harness::CoderLoop,
        permission: PERMISSION,
        native: &["no approval gate"],
        floors: "Coder's tool gate: the Run tool's commands and file reads",
    },
    Row {
        harness: Harness::ClaudeCode,
        permission: PERMISSION,
        native: &[
            "permissionMode=bypassPermissions",
            "allowDangerouslySkipPermissions",
        ],
        floors: "a PreToolUse hook Coder installs in the session",
    },
    Row {
        harness: Harness::Codex,
        permission: PERMISSION,
        native: &[
            "approval_policy=\"untrusted\"",
            "sandbox=\"danger-full-access\"",
        ],
        floors: "Codex's approval requests on app-server, answered by the hooks Coder runs for the session",
    },
];

/// Codex's `exec`, the fallback where app-server won't start (#11510): its
/// own modes, and where the floors hold there.
pub const CODEX_EXEC_FALLBACK: Row = Row {
    harness: Harness::Codex,
    permission: PERMISSION,
    native: &[
        "--dangerously-bypass-approvals-and-sandbox",
        "approval_policy=\"never\"",
    ],
    floors: "a PreToolUse hook Coder passes with -c hooks.PreToolUse, run without hook trust",
};

/// The row for `harness`.
#[must_use]
pub fn row(harness: Harness) -> &'static Row {
    TABLE
        .iter()
        .find(|row| row.harness == harness)
        .expect("every harness has a row")
}

/// The program word that runs the Codex hook: `<this program> coder-floor-hook`.
pub const HOOK_COMMAND: &str = "coder-floor-hook";

/// The floors table itself lives in the Coder SDK, so every surface that
/// starts an agent session registers the same floors (#11509).
pub use coder_sdk::floors::{
    FLOORS_POLICY, Floor, classify, command, command_text, declined, held, refused, scripts,
    secret_path, wrapped_script,
};

/// The rule the checkout at `cwd` gives `ability`; a policy that cannot be
/// read asks.
#[must_use]
pub fn rule(cwd: &Path, ability: &str) -> Rule {
    let top = std::process::Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--show-toplevel"])
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()));
    risk_policy::Policy::load(top.as_deref()).map_or(Rule::Ask, |policy| policy.rule(ability))
}

fn slot() -> &'static Mutex<Option<Arc<crate::approval::Desk>>> {
    static DESK: OnceLock<Mutex<Option<Arc<crate::approval::Desk>>>> = OnceLock::new();
    DESK.get_or_init(|| Mutex::new(None))
}

#[cfg(test)]
thread_local! {
    /// A test's desk is its own thread's, so a test that answers floors
    /// never catches another test's call.
    static TEST_DESK: std::cell::RefCell<Option<Arc<crate::approval::Desk>>> =
        const { std::cell::RefCell::new(None) };
}

/// Where a floor asks the person in this process: the terminal's approval
/// card for the chat's running turn. `None` when nobody can answer.
pub fn install_desk(desk: Option<Arc<crate::approval::Desk>>) {
    #[cfg(test)]
    {
        TEST_DESK.with(|cell| *cell.borrow_mut() = desk);
    }
    #[cfg(not(test))]
    {
        *slot()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = desk;
    }
}

fn desk() -> Option<Arc<crate::approval::Desk>> {
    #[cfg(test)]
    {
        let _ = slot;
        TEST_DESK.with(|cell| cell.borrow().clone())
    }
    #[cfg(not(test))]
    {
        crate::approval::desk().or_else(|| {
            slot()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()
        })
    }
}

/// The floor's decision on one call by `agent` (its name as the rail shows
/// it): `Ok` lets it run; `Err` is what the agent reads instead. Asks the
/// person on the approval card when someone can answer, and waits.
///
/// # Errors
/// The call is floored and was not approved.
pub async fn hold(
    agent: &str,
    tool: &str,
    input: &Value,
    cwd: &Path,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<(), String> {
    let Some(floor) = classify(tool, input) else {
        return Ok(());
    };
    #[cfg(test)]
    if OPEN.with(std::cell::Cell::get) {
        return Ok(());
    }
    match rule(cwd, floor.ability) {
        Rule::Allow => Ok(()),
        Rule::Deny => Err(refused(&floor)),
        Rule::Ask => {
            let Some(desk) = desk() else {
                return Err(held(&floor));
            };
            let title = format!("Let {agent} {}?", floor.what);
            let (approved, via) = desk
                .confirm_action(
                    floor.ability,
                    &title,
                    &floor.subject,
                    "It waits until you choose. Deny and it carries on without this step.",
                    cancel,
                )
                .await;
            // The answer is recorded with exactly what it approves, as the
            // card says (#11170). Tests never write the person's file.
            if let (Some(via), Some(path)) = (via, approvals())
                && let Err(why) = risk_policy::record(
                    &path,
                    floor.ability,
                    &floor.subject,
                    if approved {
                        risk_policy::Decision::Approved
                    } else {
                        risk_policy::Decision::Denied
                    },
                    &risk_policy::local_user(),
                    &via,
                )
            {
                return Err(format!(
                    "The answer could not be recorded, so nothing ran: {why}"
                ));
            }
            if approved {
                Ok(())
            } else {
                Err(declined(&floor))
            }
        }
    }
}

#[cfg(test)]
thread_local! {
    static OPEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Lets this test thread's floored calls through, for tests about
/// something else that run one.
#[cfg(test)]
pub(crate) fn open_for_test(open: bool) {
    OPEN.with(|cell| cell.set(open));
}

/// Where answers are recorded: the person's approvals file, or in a test
/// only a file the test names.
fn approvals() -> Option<PathBuf> {
    if cfg!(test) && std::env::var_os(risk_policy::APPROVALS_VAR).is_none() {
        return None;
    }
    risk_policy::approvals_path()
}

/// The floors as a Coder SDK hook (#11496, #11497): one floor on
/// `PreToolUse`, the same table on Claude Code and the Coder loop. A
/// floored call whose rule is not `allow` asks, with the words a held or
/// refused call reads; the backend's asker ([`asker`]) then applies the
/// whole rule ([`hold`]): `deny` refuses, `ask` shows the approval card
/// where someone can answer, and holds the call where nobody can. A floor
/// that fails asks too.
#[must_use]
pub fn sdk_hook(cwd: PathBuf) -> coder_sdk::Hook {
    coder_sdk::floors::hook(rules(cwd), coder_sdk::floors::Asking::Ask)
}

/// The checkout's approval policy at `cwd` as the floors' rules.
#[must_use]
pub fn rules(cwd: PathBuf) -> coder_sdk::floors::Rules {
    Arc::new(move |ability| match rule(&cwd, ability) {
        Rule::Allow => coder_sdk::floors::Rule::Allow,
        Rule::Ask => coder_sdk::floors::Rule::Ask,
        Rule::Deny => coder_sdk::floors::Rule::Deny,
    })
}

/// Answers the floors' `ask` with the whole rule ([`hold`]) for `agent`
/// (its name as the rail shows it): the call runs (`NoOpinion`) or is
/// refused with what the agent reads; `cancel` ends a wait on the card. An
/// ask from another hook, on a call
/// the floors don't cover, is held with its own step.
#[must_use]
pub fn asker(
    cwd: PathBuf,
    agent: impl Fn() -> String + Send + Sync + 'static,
    cancel: Option<Arc<std::sync::atomic::AtomicBool>>,
) -> coder_sdk::Asker {
    let agent = Arc::new(agent);
    Arc::new(move |applied: coder_sdk::Applied| {
        let cwd = cwd.clone();
        let agent = agent.clone();
        let cancel = cancel.clone().unwrap_or_default();
        Box::pin(async move {
            let tool = applied.tool.clone().unwrap_or_default();
            if classify(&tool, &applied.input).is_none() {
                return match applied.permission {
                    coder_sdk::Permission::Ask { step } => {
                        coder_sdk::Permission::Deny { reason: step }
                    }
                    other => other,
                };
            }
            match hold(&(*agent)(), &tool, &applied.input, &cwd, &cancel).await {
                Ok(()) => coder_sdk::Permission::NoOpinion,
                Err(reason) => coder_sdk::Permission::Deny { reason },
            }
        })
    })
}

fn deny_output(reason: &str) -> Value {
    json!({
        "hookEventName": "PreToolUse",
        "permissionDecision": "deny",
        "permissionDecisionReason": reason,
    })
}

/// The arguments that install the floors' hook in a Codex run: the hook,
/// and running it without a trust prompt (Coder installed it itself).
#[must_use]
pub fn codex_args(program: &Path) -> Vec<String> {
    let command = format!(
        "{} {HOOK_COMMAND}",
        shell_quote(&program.display().to_string())
    );
    vec![
        "-c".into(),
        format!(
            "hooks.PreToolUse=[{{hooks=[{{type=\"command\",command={}}}]}}]",
            toml_string(&command)
        ),
        "--dangerously-bypass-hook-trust".into(),
    ]
}

/// The command an agent's own hooks run for Coder's bridge (#11508):
/// `program coder-hook --format acp`.
#[must_use]
pub fn acp_bridge_command(program: &Path) -> String {
    format!(
        "{} {BRIDGE_COMMAND} --format acp",
        shell_quote(&program.display().to_string())
    )
}

fn shell_quote(text: &str) -> String {
    if text
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '-' | '_'))
    {
        text.to_owned()
    } else {
        format!("'{}'", text.replace('\'', "'\\''"))
    }
}

fn toml_string(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_owned())
}

/// The Codex hook: reads one `PreToolUse` call on standard input and
/// writes the decision: the session worktree's fence
/// ([`crate::checkout_guard::tool_refusal`]), then the floors. Nobody can answer from inside Codex, so a
/// floored call is held (denied with a note to hand it back), and anything
/// else is let through.
#[must_use]
pub fn hook_decision(input: &Value) -> Option<Value> {
    let tool = input["tool_name"].as_str().unwrap_or_default();
    let cwd = input["cwd"]
        .as_str()
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    // The session's worktree fence first (#11485), as on Claude Code.
    if let Some(reason) = crate::checkout_guard::tool_refusal(
        crate::checkout_guard::Boundary::around(&cwd).as_ref(),
        tool,
        &input["tool_input"],
        &cwd,
    ) {
        return Some(json!({"hookSpecificOutput": deny_output(&reason)}));
    }
    let floor = classify(tool, &input["tool_input"])?;
    let reason = match rule(&cwd, floor.ability) {
        Rule::Allow => return None,
        Rule::Deny => refused(&floor),
        Rule::Ask => held(&floor),
    };
    Some(json!({"hookSpecificOutput": deny_output(&reason)}))
}

/// `coder-floor-hook`: the hook bridge (#11500) in Codex's format. A run
/// Coder started names its session's hook host ([`coder_sdk::bridge`]),
/// which answers from the whole registry (the floors and the command
/// hooks); with no host, [`hook_decision`] answers alone. Always exits
/// cleanly; an unreadable call is let through for the harness's own rules.
pub fn hook_main() {
    let _ = bridge_main(&["--format".into(), "codex".into()]);
}

/// The general hook bridge, `coder-hook --format claude|codex`, for any
/// harness with Claude- or Codex-style command hooks; the floors answer
/// when no host does.
pub const BRIDGE_COMMAND: &str = coder_sdk::bridge::COMMAND;

/// [`BRIDGE_COMMAND`]'s program. Returns the exit code.
pub fn bridge_main(args: &[String]) -> i32 {
    coder_sdk::bridge::main(args, hook_decision)
}

#[cfg(test)]
#[path = "floors_tests.rs"]
mod tests;
