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
//!   ([`claude_hook`]);
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
            "--dangerously-bypass-approvals-and-sandbox",
            "approval_policy=\"never\"",
        ],
        floors: "a PreToolUse hook Coder passes with -c hooks.PreToolUse, run without hook trust",
    },
];

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

/// A floored call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Floor {
    /// The risk policy's ability.
    pub ability: &'static str,
    /// What it does, in plain words.
    pub what: &'static str,
    /// Exactly what it acts on: the command or the path.
    pub subject: String,
}

/// A shell command's simple commands, split at unquoted `;`, `|`, `&` and
/// newlines, each as its words with quotes removed.
fn simple_commands(command: &str) -> Vec<Vec<String>> {
    let mut commands = Vec::new();
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let mut chars = command.chars().peekable();
    let end_word = |word: &mut String, quoted: &mut bool, words: &mut Vec<String>| {
        if !word.is_empty() || *quoted {
            words.push(std::mem::take(word));
        }
        *quoted = false;
    };
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                quoted = true;
                for c in chars.by_ref() {
                    if c == '\'' {
                        break;
                    }
                    word.push(c);
                }
            }
            '"' => {
                quoted = true;
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => {
                            if let Some(next) = chars.next() {
                                word.push(next);
                            }
                        }
                        other => word.push(other),
                    }
                }
            }
            '\\' => {
                if let Some(next) = chars.next() {
                    word.push(next);
                }
            }
            ' ' | '\t' => end_word(&mut word, &mut quoted, &mut words),
            ';' | '|' | '&' | '\n' | '(' | ')' => {
                end_word(&mut word, &mut quoted, &mut words);
                if !words.is_empty() {
                    commands.push(std::mem::take(&mut words));
                }
            }
            other => word.push(other),
        }
    }
    end_word(&mut word, &mut quoted, &mut words);
    if !words.is_empty() {
        commands.push(words);
    }
    commands
}

/// The program and its arguments, past environment assignments and the
/// wrappers that run another program (`sudo`, `env`, `time`, `nohup`,
/// `command`, `exec`, and the package runners `npx`, `bunx`, `pnpx`,
/// `pnpm exec`/`dlx`, `yarn dlx`).
fn argv(words: &[String]) -> Option<(String, &[String])> {
    let mut at = 0;
    while let Some(word) = words.get(at) {
        let wrapper = matches!(
            word.as_str(),
            "sudo" | "env" | "time" | "nohup" | "command" | "exec" | "npx" | "bunx" | "pnpx"
        );
        if matches!(word.as_str(), "pnpm" | "yarn")
            && words
                .get(at + 1)
                .is_some_and(|next| matches!(next.as_str(), "exec" | "dlx"))
        {
            at += 2;
            continue;
        }
        let assignment = word.contains('=')
            && word.split('=').next().is_some_and(|name| {
                !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            });
        if wrapper || assignment || (at > 0 && word.starts_with('-') && words[at - 1] == "sudo") {
            at += 1;
            continue;
        }
        break;
    }
    let program = words.get(at)?;
    let name = program.rsplit('/').next().unwrap_or(program).to_owned();
    Some((name, &words[at + 1..]))
}

/// The arguments that are not flags.
fn positional(args: &[String]) -> Vec<&str> {
    args.iter()
        .map(String::as_str)
        .filter(|arg| !arg.starts_with('-'))
        .collect()
}

fn has(args: &[String], flag: &str) -> bool {
    args.iter()
        .any(|arg| arg == flag || arg.starts_with(&format!("{flag}=")))
}

/// Whether `path` has the shape of a secret: a `.secrets` folder, an `.env`
/// file (not an example), a private key, or a credentials file.
#[must_use]
pub fn secret_path(path: &str) -> bool {
    let path = Path::new(path);
    if path.components().any(|part| part.as_os_str() == ".secrets") {
        return true;
    }
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let env = name == ".env"
        || (name.starts_with(".env.")
            && !matches!(
                name,
                ".env.example" | ".env.sample" | ".env.template" | ".env.dist"
            ));
    let key = matches!(name, "id_rsa" | "id_ed25519" | "id_ecdsa" | "id_dsa")
        || name.ends_with(".pem")
        || name.ends_with(".p12")
        || name.ends_with(".key");
    let credentials = matches!(
        name,
        ".netrc" | "credentials.json" | ".credentials.json" | "credentials"
    ) || (name == "auth.json"
        && path
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|dir| dir == ".codex"));
    env || key || credentials
}

/// The floor one simple command reaches, if any.
fn command_floor(words: &[String], shown: &str) -> Option<Floor> {
    let (program, args) = argv(words)?;
    let rest = positional(args);
    let floor = |ability, what| {
        Some(Floor {
            ability,
            what,
            subject: shown.to_owned(),
        })
    };
    match (program.as_str(), rest.as_slice()) {
        ("gh", ["pr", "merge", ..]) => floor(risk_policy::PR_MERGE, "merge a pull request"),
        ("gh", ["api", ..])
            if args.iter().any(|arg| arg.ends_with("/merge"))
                && args.windows(2).any(|pair| {
                    matches!(pair[0].as_str(), "-X" | "--method")
                        && pair[1].eq_ignore_ascii_case("put")
                }) =>
        {
            floor(risk_policy::PR_MERGE, "merge a pull request")
        }
        ("gh", ["secret", ..]) | ("wrangler", ["secret", ..]) => {
            floor(risk_policy::SECRETS, "change a secret")
        }
        ("gcloud", ["secrets", "versions", "access", ..]) => {
            floor(risk_policy::SECRETS, "read a secret")
        }
        ("gcloud", ["secrets", "create" | "delete" | "update", ..])
        | ("gcloud", ["secrets", "versions", "add" | "destroy" | "disable", ..]) => {
            floor(risk_policy::SECRETS, "change a secret")
        }
        ("security", ["find-generic-password" | "find-internet-password", ..])
            if has(args, "-w") || has(args, "-g") =>
        {
            floor(risk_policy::SECRETS, "read a secret")
        }
        ("op", ["read", ..] | ["item", "get", ..]) => floor(risk_policy::SECRETS, "read a secret"),
        (
            "cat" | "less" | "more" | "head" | "tail" | "bat" | "xxd" | "od" | "hexdump" | "base64"
            | "strings" | "sed" | "awk" | "grep" | "rg" | "jq" | "cut" | "cp" | "scp",
            paths,
        ) if paths.iter().any(|path| secret_path(path)) => {
            floor(risk_policy::SECRETS, "read a secret")
        }
        ("openagents", ["deploy", "production", ..]) if !has(args, "--approval") => {
            floor(risk_policy::DEPLOY_PRODUCTION, "deploy to production")
        }
        ("wrangler", ["deploy" | "publish", ..])
            if !has(args, "--dry-run")
                && !args.windows(2).any(|pair| {
                    matches!(pair[0].as_str(), "--env" | "-e")
                        && matches!(
                            pair[1].as_str(),
                            "staging" | "dev" | "development" | "preview"
                        )
                }) =>
        {
            floor(risk_policy::DEPLOY_PRODUCTION, "deploy to production")
        }
        ("gcloud", ["run" | "app" | "functions", "deploy", ..])
        | ("fly" | "flyctl", ["deploy", ..]) => {
            floor(risk_policy::DEPLOY_PRODUCTION, "deploy to production")
        }
        ("vercel", _) if has(args, "--prod") => {
            floor(risk_policy::DEPLOY_PRODUCTION, "deploy to production")
        }
        (
            "gcloud",
            [
                "dns",
                "record-sets",
                "create" | "update" | "delete" | "transaction",
                ..,
            ],
        ) => floor(risk_policy::DNS, "change DNS"),
        ("openagents", ["wallet", "send" | "pay", ..])
        | ("lncli", ["payinvoice" | "sendpayment" | "sendcoins" | "sendmany", ..])
        | ("bitcoin-cli", ["sendtoaddress" | "sendmany" | "sendrawtransaction", ..])
        | (
            "stripe",
            [
                "payouts" | "transfers" | "charges" | "payment_intents" | "refunds",
                "create",
                ..,
            ],
        ) => floor(risk_policy::PAYMENTS, "send a payment"),
        _ => None,
    }
}

/// The floor a shell `command` reaches, if any.
#[must_use]
pub fn command(command: &str) -> Option<Floor> {
    simple_commands(command)
        .iter()
        .find_map(|words| command_floor(words, command.trim()))
}

/// The floor a tool call reaches, if any: a shell command (`Bash`, `Run`,
/// Codex's `shell`), a file read, or the OpenAgents CLI tool.
#[must_use]
pub fn classify(tool: &str, input: &Value) -> Option<Floor> {
    match tool {
        "Bash" | "Run" | "shell" | "exec_command" | "local_shell" => {
            let text = match input.get("command").or_else(|| input.get("cmd"))? {
                Value::String(text) => text.clone(),
                Value::Array(words) => words
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(" "),
                _ => return None,
            };
            command(&text)
        }
        "Read" | "read_file" | "View" => {
            let path = input
                .get("file_path")
                .or_else(|| input.get("path"))
                .and_then(Value::as_str)?;
            secret_path(path).then(|| Floor {
                ability: risk_policy::SECRETS,
                what: "read a secret",
                subject: path.to_owned(),
            })
        }
        "openagents_cli" => {
            let words: Vec<String> = std::iter::once("openagents".to_owned())
                .chain(
                    input
                        .get("arguments")
                        .and_then(Value::as_array)?
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned),
                )
                .collect();
            command_floor(&words, &words.join(" "))
        }
        _ => None,
    }
}

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

/// What a held call tells the agent.
#[must_use]
pub fn held(floor: &Floor) -> String {
    format!(
        "Held for the person: this would {} ({}). Do not do it another way. Finish what you can, and in your reply ask the person to do this step themselves or to approve it.",
        floor.what, floor.subject
    )
}

/// What a refused call tells the agent.
#[must_use]
pub fn refused(floor: &Floor) -> String {
    format!(
        "Not allowed here: this would {} ({}), which this project's approval policy never allows from a chat. Do not do it another way.",
        floor.what, floor.subject
    )
}

/// What the person said no to.
#[must_use]
pub fn declined(floor: &Floor) -> String {
    format!(
        "The person said no: this would {} ({}). Do not do it another way; continue without it.",
        floor.what, floor.subject
    )
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

/// The `PreToolUse` hook Coder installs in a Claude Code session: the same
/// floors, asking the person on the approval card while one can answer.
#[must_use]
pub fn claude_hook(cwd: PathBuf) -> claude_agent_sdk::HookMatcher {
    let hook = claude_agent_sdk::hook_fn(move |input: Value, _tool_use_id| {
        let cwd = cwd.clone();
        async move {
            let tool = input["tool_name"].as_str().unwrap_or_default().to_owned();
            let tool_input = input["tool_input"].clone();
            let cancel = std::sync::atomic::AtomicBool::new(false);
            let agent = crate::ui::claude_rail_label();
            Ok(
                match hold(&agent, &tool, &tool_input, &cwd, &cancel).await {
                    Ok(()) => claude_agent_sdk::SyncHookJSONOutput::continue_execution().into(),
                    Err(reason) => deny(&reason),
                },
            )
        }
    });
    claude_agent_sdk::HookMatcher::new(None, hook)
}

/// A `PreToolUse` denial with its reason.
fn deny(reason: &str) -> claude_agent_sdk::HookJSONOutput {
    claude_agent_sdk::SyncHookJSONOutput {
        hook_specific_output: Some(deny_output(reason)),
        ..claude_agent_sdk::SyncHookJSONOutput::default()
    }
    .into()
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
/// writes the floor's decision. Nobody can answer from inside Codex, so a
/// floored call is held (denied with a note to hand it back), and anything
/// else is let through.
#[must_use]
pub fn hook_decision(input: &Value) -> Option<Value> {
    let tool = input["tool_name"].as_str().unwrap_or_default();
    let floor = classify(tool, &input["tool_input"])?;
    let cwd = input["cwd"]
        .as_str()
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let reason = match rule(&cwd, floor.ability) {
        Rule::Allow => return None,
        Rule::Deny => refused(&floor),
        Rule::Ask => held(&floor),
    };
    Some(json!({"hookSpecificOutput": deny_output(&reason)}))
}

/// `coder-floor-hook`: [`hook_decision`] over standard input and output.
/// Always exits cleanly; an unreadable call is let through for the harness's
/// own rules.
pub fn hook_main() {
    use std::io::{Read, Write};
    let mut text = String::new();
    let _ = std::io::stdin().read_to_string(&mut text);
    let Ok(input) = serde_json::from_str::<Value>(&text) else {
        return;
    };
    if let Some(output) = hook_decision(&input) {
        let _ = writeln!(std::io::stdout(), "{output}");
    }
}

#[cfg(test)]
#[path = "floors_tests.rs"]
mod tests;
