//! The floors (#11475): one table of host-owned, deny-or-ask-only rules
//! every harness's sessions run, moved here from Coder so every surface
//! that starts an agent session registers the same floors (#11509): Coder
//! chat, issue runs and the Solver's hands, the briefed agent, Coder One's
//! delegations.
//!
//! [`classify`] reads a tool call: a shell command is split into its
//! simple commands and each program's argument list is matched against a
//! declared table of floored commands (an explicit command-line parser,
//! never a reading of the person's words); a file read is matched on the
//! path's shape. A production deploy, merging a pull request, reading or
//! changing a secret, a payment, and a DNS change are floored.
//!
//! What a floored call comes to is the host's [`Rule`] for its ability:
//! Coder reads the checkout's approval policy; a host nobody answers from
//! (a bench run, a Coder One episode) holds every floored call
//! ([`held_hook`]).

use std::path::Path;
use std::sync::Arc;

use serde_json::Value;

use crate::events::{Call, Event};
use crate::outcome::{HookOutcome, Permission};
use crate::registry::{Hook, Versions};

/// A production deploy.
pub const DEPLOY_PRODUCTION: &str = "deploy.production";
/// A pull request merged.
pub const PR_MERGE: &str = "pr.merge";
/// A DNS record changed.
pub const DNS: &str = "dns";
/// A secret created, changed, or read out.
pub const SECRETS: &str = "secrets";
/// Money sent.
pub const PAYMENTS: &str = "payments";

/// The server Coder's own tools are offered under, so a call to
/// `mcp__coder__openagents_cli` is judged as `openagents_cli`.
pub const SERVER: &str = "coder";

/// The floors table's version in the hook set digest.
pub const FLOORS_POLICY: &str = "coder-floors-v1";

/// The squeezer's subcommand (`squeeze::CODER_SUBCOMMAND`, which this
/// crate can't name: the squeezer depends on it).
pub const SQUEEZE_SUBCOMMAND: &str = "coder-squeeze";

/// The hook ID every surface registers the floors under.
pub const ID: &str = "floors";

/// What a host's policy says for one floored ability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    /// It runs.
    Allow,
    /// It waits for the person.
    Ask,
    /// Never from a session.
    Deny,
}

/// How a floored call whose rule is `ask` is answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asking {
    /// An `Ask` outcome, for a host whose asker can show the person a
    /// card (Coder chat); a refused call asks too, with the refusal's
    /// words, so the asker applies the whole rule.
    Ask,
    /// Held: denied with [`held`]'s words, for a session nobody can answer
    /// from inside.
    Hold,
}

/// The rule a host gives each floored ability.
pub type Rules = Arc<dyn Fn(&str) -> Rule + Send + Sync>;

/// The floors as one `PreToolUse` floor: a floored call whose rule is not
/// `allow` is asked about or held, as `asking` says; one whose rule is
/// `deny` is refused. A floor that fails falls back to holding the call.
#[must_use]
pub fn hook(rules: Rules, asking: Asking) -> Hook {
    let fallback = "Held for the person: Coder could not check this step. Ask the person to do it or approve it.";
    Hook::floor(
        ID,
        Event::PreToolUse,
        match asking {
            Asking::Ask => Permission::ask(fallback),
            Asking::Hold => Permission::deny(fallback),
        },
        move |call: &Call| {
            let tool = call.tool.as_deref().unwrap_or_default();
            // A script Coder can't read may reach any floor: refused.
            if let Some(reason) = unreadable_refusal(tool, &call.input) {
                return Ok(HookOutcome::deny(reason));
            }
            let Some(floor) = classify(tool, &call.input) else {
                return Ok(HookOutcome::none());
            };
            Ok(match (rules(floor.ability), asking) {
                (Rule::Allow, _) => HookOutcome::none(),
                (Rule::Deny, Asking::Ask) => HookOutcome::ask(&refused(&floor)),
                (Rule::Ask, Asking::Ask) => HookOutcome::ask(&held(&floor)),
                (Rule::Deny, Asking::Hold) => HookOutcome::deny(&refused(&floor)),
                (Rule::Ask, Asking::Hold) => HookOutcome::deny(&held(&floor)),
            })
        },
    )
    .pinned(Versions {
        question_set: None,
        policy: Some(FLOORS_POLICY.to_owned()),
        model: None,
    })
}

/// The floors for a session nobody answers from inside and no approval
/// policy covers (the briefed agent's bench run, a Coder One episode):
/// every floored call is held for the person.
#[must_use]
pub fn held_hook() -> Hook {
    hook(Arc::new(|_| Rule::Ask), Asking::Hold)
}

/// The registry of a session nobody answers from inside (#11527, #11529):
/// the held floors ([`held_hook`]) and, when given, the worktree `guard`
/// with the shared-checkout fence on its root (#11532).
#[must_use]
pub fn held_registry(guard: Option<crate::worktree::Guard>) -> crate::Registry {
    let mut registry = crate::Registry::new();
    // Distinct built-in IDs: no registration can fail.
    let _ = registry.register(held_hook());
    if let Some(guard) = guard {
        let _ = registry.register(crate::checkout_guard::hook(guard.root.clone()));
        let _ = registry.register(guard.hook());
    }
    registry
}

/// What the held floors answer alone, in a harness's hook output shape
/// (Claude Code's and Codex's `PreToolUse` reply): the hook bridge's
/// fallback when no host answers ([`crate::bridge::main`]).
#[must_use]
pub fn held_decision(input: &Value) -> Option<Value> {
    let tool = input["tool_name"].as_str()?;
    let reason = match unreadable_refusal(tool, &input["tool_input"]) {
        Some(reason) => reason.to_owned(),
        None => held(&classify(tool, &input["tool_input"])?),
    };
    Some(serde_json::json!({"hookSpecificOutput": {
        "hookEventName": "PreToolUse",
        "permissionDecision": "deny",
        "permissionDecisionReason": reason,
    }}))
}

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
/// The program a command word runs, as a plain lowercase name: past its
/// folder (`/usr/bin/git`, `C:\\Program Files\\Git\\bin\\git.exe`) and a
/// Windows program suffix (`.exe`, `.cmd`, `.bat`, `.com`), so `git.exe`
/// and `Git` are judged as `git` (#11708).
#[must_use]
pub(crate) fn program_name(word: &str) -> String {
    let base = word.rsplit(['/', '\\']).next().unwrap_or(word);
    let lower = base.to_ascii_lowercase();
    [".exe", ".cmd", ".bat", ".com"]
        .iter()
        .find_map(|suffix| lower.strip_suffix(suffix))
        .filter(|name| !name.is_empty())
        .map_or_else(|| lower.clone(), str::to_owned)
}

fn argv(words: &[String]) -> Option<(String, &[String])> {
    let mut at = 0;
    while let Some(word) = words.get(at) {
        let base = program_name(word);
        let wrapper = matches!(
            base.as_str(),
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
        if wrapper
            || assignment
            || (at > 0
                && word.starts_with('-')
                && matches!(program_name(&words[at - 1]).as_str(), "sudo" | "env"))
        {
            at += 1;
            continue;
        }
        break;
    }
    let program = words.get(at)?;
    Some((program_name(program), &words[at + 1..]))
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
        ("gh", ["pr", "merge", ..]) => floor(PR_MERGE, "merge a pull request"),
        ("gh", ["api", ..])
            if args.iter().any(|arg| arg.ends_with("/merge"))
                && args.windows(2).any(|pair| {
                    matches!(pair[0].as_str(), "-X" | "--method")
                        && pair[1].eq_ignore_ascii_case("put")
                }) =>
        {
            floor(PR_MERGE, "merge a pull request")
        }
        ("gh", ["secret", ..]) | ("wrangler", ["secret", ..]) => floor(SECRETS, "change a secret"),
        ("gcloud", ["secrets", "versions", "access", ..]) => floor(SECRETS, "read a secret"),
        ("gcloud", ["secrets", "create" | "delete" | "update", ..])
        | ("gcloud", ["secrets", "versions", "add" | "destroy" | "disable", ..]) => {
            floor(SECRETS, "change a secret")
        }
        ("security", ["find-generic-password" | "find-internet-password", ..])
            if has(args, "-w") || has(args, "-g") =>
        {
            floor(SECRETS, "read a secret")
        }
        ("op", ["read", ..] | ["item", "get", ..]) => floor(SECRETS, "read a secret"),
        (
            "cat" | "less" | "more" | "head" | "tail" | "bat" | "xxd" | "od" | "hexdump" | "base64"
            | "strings" | "sed" | "awk" | "grep" | "rg" | "jq" | "cut" | "cp" | "scp",
            paths,
        ) if paths.iter().any(|path| secret_path(path)) => floor(SECRETS, "read a secret"),
        ("openagents", ["deploy", "production", ..]) if !has(args, "--approval") => {
            floor(DEPLOY_PRODUCTION, "deploy to production")
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
            floor(DEPLOY_PRODUCTION, "deploy to production")
        }
        ("gcloud", ["run" | "app" | "functions", "deploy", ..])
        | ("fly" | "flyctl", ["deploy", ..]) => floor(DEPLOY_PRODUCTION, "deploy to production"),
        ("vercel", _) if has(args, "--prod") => floor(DEPLOY_PRODUCTION, "deploy to production"),
        (
            "gcloud",
            [
                "dns",
                "record-sets",
                "create" | "update" | "delete" | "transaction",
                ..,
            ],
        ) => floor(DNS, "change DNS"),
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
        ) => floor(PAYMENTS, "send a payment"),
        _ => None,
    }
}

/// The floor a shell `command` reaches, if any, including inside the
/// scripts a shell wrapper runs (`bash -lc '...'`, `sh -c`, `zsh -c`, past
/// `env` and assignments), however deep.
#[must_use]
pub fn command(command: &str) -> Option<Floor> {
    // The card names what runs: the script inside a wrapper, not the
    // wrapper.
    scripts(command).iter().find_map(|script| {
        simple_commands(script)
            .iter()
            .find_map(|words| command_floor(words, script.trim()))
    })
}

/// How many shell wrappers deep a command is read.
const WRAPPER_DEPTH: usize = 8;

/// `command` and every script a shell wrapper in it runs, outermost first:
/// `bash -lc 'gh pr merge 5'` gives itself and `gh pr merge 5`. Every
/// harness's checks read each one, so wrapping a command never hides it
/// (Codex's `shell` sends `["bash", "-lc", "..."]`).
#[must_use]
pub fn scripts(command: &str) -> Vec<String> {
    let mut found = vec![command.to_owned()];
    let mut at = 0;
    while at < found.len() && found.len() <= WRAPPER_DEPTH {
        let inner: Vec<String> = simple_commands(&found[at])
            .iter()
            .filter_map(|words| wrapped_script(words))
            .collect();
        found.extend(inner);
        at += 1;
    }
    found
}

/// The script a shell wrapper runs: `bash`, `sh`, `zsh`, `dash`, `ksh` or
/// `fish` given `-c` (alone or in a cluster such as `-lc`), or Coder's
/// squeezer (`coder coder-squeeze SCRIPT`), past the wrappers [`argv`]
/// skips (`env`, `sudo`, assignments).
#[must_use]
pub fn wrapped_script(words: &[String]) -> Option<String> {
    let (program, args) = argv(words)?;
    // Coder's own squeezer runs its one argument as a script (#11486):
    // `coder coder-squeeze 'gh pr merge 5'`. A harness that shows the
    // model its rewritten commands (OpenCode, #11507) can see the model
    // write one itself, so it is read through like a shell.
    if args.first().map(String::as_str) == Some(SQUEEZE_SUBCOMMAND) {
        return args.get(1).cloned();
    }
    // Windows shells (#11708): `cmd /c SCRIPT` and `powershell -Command
    // SCRIPT` (Codex on Windows runs every command so) run the rest of
    // their words as the script.
    if program == "cmd" {
        let at = args
            .iter()
            .position(|arg| matches!(arg.to_ascii_lowercase().as_str(), "/c" | "/k" | "/r"))?;
        return Some(args[at + 1..].join(" ")).filter(|script| !script.is_empty());
    }
    if matches!(program.as_str(), "powershell" | "pwsh") {
        return match powershell_script(&program, args) {
            Powershell::Script(script) => Some(script),
            Powershell::Unreadable | Powershell::Nothing => None,
        };
    }
    if !matches!(
        program.as_str(),
        "bash" | "sh" | "zsh" | "dash" | "ksh" | "fish"
    ) {
        return None;
    }
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let Some(flags) = arg.strip_prefix('-').filter(|f| !f.starts_with('-')) else {
            // A long option (`--login`) takes no script; anything else is
            // a script file, not `-c`.
            if arg.starts_with("--") {
                continue;
            }
            return None;
        };
        if flags.contains('c') {
            return args.next().cloned();
        }
        // `-o option` takes a value.
        if flags.ends_with('o') || flags.ends_with('O') {
            args.next();
        }
    }
    None
}

/// What a `powershell` or `pwsh` command line runs.
enum Powershell {
    /// A script given in plain text (`-Command SCRIPT`).
    Script(String),
    /// A script Coder can't read before it runs: an encoded command
    /// (`-EncodedCommand`, `-enc`, `-e`, `-ec`), a script file (`-File`,
    /// or `pwsh`'s first plain argument), or one read from standard input
    /// (`-Command -`).
    Unreadable,
    /// No script (an interactive shell).
    Nothing,
}

/// What `powershell`/`pwsh` with `args` runs (#11708). Flags are
/// case-insensitive, start with `-` or `/`, and may be shortened to any
/// prefix (`-c`, `-enc`), as PowerShell reads them.
fn powershell_script(program: &str, args: &[String]) -> Powershell {
    let mut at = 0;
    while let Some(arg) = args.get(at) {
        let lower = arg.to_ascii_lowercase().replacen('/', "-", 1);
        // `-EncodedCommand:BASE64` names its value after a colon.
        let flag = lower.split(':').next().unwrap_or_default();
        if !flag.starts_with('-') {
            // Windows PowerShell runs its first plain argument as a
            // command; PowerShell 7 runs it as a script file.
            if program == "pwsh" {
                return Powershell::Unreadable;
            }
            break;
        }
        let prefix_of = |name: &str| flag.len() > 1 && name.starts_with(flag);
        if prefix_of("-command") {
            at += 1;
            break;
        }
        if flag == "-ec"
            || prefix_of("-encodedcommand")
            || (flag.starts_with("-f") && prefix_of("-file"))
        {
            return Powershell::Unreadable;
        }
        let valued = matches!(
            flag.trim_start_matches('-'),
            "executionpolicy"
                | "ex"
                | "ep"
                | "windowstyle"
                | "w"
                | "version"
                | "v"
                | "inputformat"
                | "if"
                | "outputformat"
                | "of"
                | "configurationname"
                | "workingdirectory"
                | "wd"
                | "psconsolefile"
                | "settingsfile"
        );
        at += if valued { 2 } else { 1 };
    }
    let script = args.get(at..).unwrap_or_default().join(" ");
    match script.trim() {
        "" => Powershell::Nothing,
        "-" => Powershell::Unreadable,
        _ => Powershell::Script(script),
    }
}

/// What a guard tells the agent about a command whose script Coder can't
/// read ([`unreadable`]).
pub const UNREADABLE_REFUSAL: &str = "Refused: Coder can't check what an encoded PowerShell command or a script file writes, so it didn't run. Run the commands directly.";

/// Whether `command`, or a script a shell wrapper in it runs, hands
/// PowerShell a script Coder can't read before it runs: an encoded command
/// (`powershell -enc ...`), a script file (`pwsh -File x.ps1`, `pwsh
/// x.ps1`), or standard input. Every guard that reads wrapped scripts
/// refuses these rather than letting them through unchecked (#11708):
/// the floors, [`crate::checkout_guard`] and [`crate::git_guard`].
#[must_use]
pub fn unreadable(command: &str) -> bool {
    scripts(command).iter().any(|script| {
        simple_commands(script).iter().any(|words| {
            argv(words).is_some_and(|(program, args)| {
                matches!(program.as_str(), "powershell" | "pwsh")
                    && matches!(powershell_script(&program, args), Powershell::Unreadable)
            })
        })
    })
}

/// [`UNREADABLE_REFUSAL`] for a shell tool call whose command is
/// [`unreadable`], else `None`.
#[must_use]
pub fn unreadable_refusal(tool: &str, input: &Value) -> Option<&'static str> {
    let server = format!("mcp__{SERVER}__");
    let tool = tool.strip_prefix(&server).unwrap_or(tool);
    (matches!(
        tool,
        "Bash" | "Run" | "Monitor" | "shell" | "exec_command" | "local_shell"
    ) && unreadable(&command_text(input)?))
    .then_some(UNREADABLE_REFUSAL)
}

/// The shell text of a tool call's `command` (or `cmd`): a string as it
/// is, an argument list quoted word by word so each stays one word.
#[must_use]
pub fn command_text(input: &Value) -> Option<String> {
    match input.get("command").or_else(|| input.get("cmd"))? {
        Value::String(text) => Some(text.clone()),
        Value::Array(words) => Some(
            words
                .iter()
                .filter_map(Value::as_str)
                .map(quote_word)
                .collect::<Vec<_>>()
                .join(" "),
        ),
        _ => None,
    }
}

/// One shell word, single-quoted when it holds anything but plain
/// characters.
fn quote_word(word: &str) -> String {
    if !word.is_empty()
        && word.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '/' | '.' | '-' | '_' | '=' | ':' | ',' | '@' | '+')
        })
    {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

/// The floor a tool call reaches, if any: a shell command (`Bash`, `Run`,
/// Codex's `shell`), a file read, or the OpenAgents CLI tool.
#[must_use]
pub fn classify(tool: &str, input: &Value) -> Option<Floor> {
    // Coder's own tools by their plain names, however a harness names them
    // (`mcp__coder__openagents_cli` on Claude Code and Codex).
    let server = format!("mcp__{SERVER}__");
    let tool = tool.strip_prefix(&server).unwrap_or(tool);
    match tool {
        "Bash" | "Run" | "shell" | "exec_command" | "local_shell" => command(&command_text(input)?),
        "Read" | "read_file" | "View" => {
            let path = input
                .get("file_path")
                .or_else(|| input.get("path"))
                .and_then(Value::as_str)?;
            secret_path(path).then(|| Floor {
                ability: SECRETS,
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

#[cfg(test)]
pub(crate) mod tests {
    use serde_json::json;

    use super::*;
    use crate::compose::{Budget, Runner};
    use crate::registry::Registry;

    /// The held floors refuse a floored call with the held words, through a
    /// shell wrapper and under Coder's own tool server's name, and let a
    /// plain one run; the bridge's fallback says the same.
    #[test]
    fn the_held_floors_hold_a_floored_call_and_let_the_rest_run() {
        let mut registry = Registry::new();
        registry.register(held_hook()).unwrap();
        let mut runner = Runner::new(registry, Budget::default());
        let fire = |runner: &mut Runner, tool: &str, input: Value| {
            runner
                .fire(&Call::tool(Event::PreToolUse, tool, input))
                .permission
        };
        let merge = fire(
            &mut runner,
            "Bash",
            json!({"command": "bash -lc 'gh pr merge 5'"}),
        );
        assert!(
            matches!(&merge, Permission::Deny { reason } if reason.starts_with("Held for the person")),
            "{merge:?}"
        );
        let pay = fire(
            &mut runner,
            "mcp__coder__openagents_cli",
            json!({"arguments": ["wallet", "send", "x"]}),
        );
        assert!(matches!(pay, Permission::Deny { .. }), "{pay:?}");
        assert_eq!(
            fire(&mut runner, "Bash", json!({"command": "cargo test"})),
            Permission::NoOpinion
        );
        let reply =
            held_decision(&json!({"tool_name": "Read", "tool_input": {"file_path": ".env"}}))
                .unwrap();
        assert_eq!(reply["hookSpecificOutput"]["permissionDecision"], "deny");
        assert!(
            held_decision(&json!({"tool_name": "Read", "tool_input": {"file_path": "a.rs"}}))
                .is_none()
        );
    }

    /// A host's rules decide: allow lets the call run, deny refuses, ask
    /// asks with the held words for the host's asker.
    #[test]
    fn the_hosts_rules_decide_what_a_floored_call_comes_to() {
        let call = Call::tool(
            Event::PreToolUse,
            "Bash",
            json!({"command": "gh pr merge 5"}),
        );
        let run = |rule: Rule, asking: Asking| {
            let hook = hook(Arc::new(move |_| rule), asking);
            (hook.run)(&call).unwrap().permission
        };
        assert_eq!(run(Rule::Allow, Asking::Ask), Permission::NoOpinion);
        assert!(matches!(
            run(Rule::Ask, Asking::Ask),
            Permission::Ask { .. }
        ));
        assert!(
            matches!(run(Rule::Deny, Asking::Hold), Permission::Deny { reason } if reason.starts_with("Not allowed here"))
        );
    }

    /// PowerShell command lines whose script Coder can't read (#11708), as
    /// a harness sends them: one string, or an argument list.
    pub(crate) fn unreadable_powershell() -> Vec<Value> {
        vec![
            json!("powershell -EncodedCommand ZQBjAGgAbwA="),
            json!(["powershell.exe", "-EncodedCommand", "ZQBjAGgAbwA="]),
            json!("PowerShell.exe -NoProfile -enc ZQBjAGgAbwA="),
            json!(["powershell", "-ENC", "ZQBjAGgAbwA="]),
            json!("pwsh -e ZQBjAGgAbwA="),
            json!(["pwsh.exe", "-E", "ZQBjAGgAbwA="]),
            json!("powershell /enc ZQBjAGgAbwA="),
            json!(["powershell", "/Enc", "ZQBjAGgAbwA="]),
            json!("pwsh -Ec ZQBjAGgAbwA="),
            json!(["pwsh", "-eC", "ZQBjAGgAbwA="]),
            json!("powershell -EncodedCo ZQBjAGgAbwA="),
            json!(["powershell", "-encodedcommand:ZQBjAGgAbwA="]),
            json!("pwsh -File x.ps1"),
            json!(["pwsh", "-File", "x.ps1"]),
            json!("pwsh -f x.ps1"),
            json!(["pwsh", "-NoProfile", "-f", "x.ps1"]),
            json!("pwsh x.ps1"),
            json!(["pwsh.exe", "x.ps1"]),
            json!("powershell -File x.ps1"),
            json!([
                "powershell.exe",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                "x.ps1"
            ]),
            json!("cat x | powershell -Command -"),
            json!("cmd /c powershell -enc ZQBjAGgAbwA="),
            json!(["bash", "-lc", "pwsh -File x.ps1"]),
        ]
    }

    /// Readable PowerShell runs as before (#11708).
    pub(crate) fn readable_powershell() -> Vec<Value> {
        vec![
            json!("powershell -Command \"cargo test\""),
            json!(["powershell.exe", "-Command", "cargo test -p x"]),
            json!("powershell -NoProfile -Command \"cargo test\""),
            json!([
                "powershell",
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                "cargo test"
            ]),
            json!("pwsh -c 'cargo test'"),
            json!("powershell Get-ChildItem"),
            json!("echo encodedcommand -enc"),
        ]
    }

    /// Every guard that reads wrapped scripts refuses PowerShell it can't
    /// read, so `powershell -enc` bypasses none of them (#11708).
    #[test]
    fn powershell_it_cant_read_is_refused_by_the_floors() {
        let allow = hook(Arc::new(|_| Rule::Allow), Asking::Ask);
        for command in unreadable_powershell() {
            let text = command_text(&json!({"command": command})).unwrap();
            assert!(unreadable(&text), "{command}");
            let call = Call::tool(Event::PreToolUse, "Bash", json!({"command": command}));
            assert!(
                matches!((allow.run)(&call).unwrap().permission, Permission::Deny { reason } if reason == UNREADABLE_REFUSAL),
                "{command}"
            );
            let reply =
                held_decision(&json!({"tool_name": "shell", "tool_input": {"command": command}}))
                    .unwrap();
            assert_eq!(
                reply["hookSpecificOutput"]["permissionDecisionReason"],
                UNREADABLE_REFUSAL
            );
        }
        for command in readable_powershell() {
            let text = command_text(&json!({"command": command})).unwrap();
            assert!(!unreadable(&text), "{command}");
            let call = Call::tool(Event::PreToolUse, "Bash", json!({"command": command}));
            assert_eq!(
                (allow.run)(&call).unwrap().permission,
                Permission::NoOpinion,
                "{command}"
            );
        }
        // Read through as before: the script inside is what runs.
        assert_eq!(
            scripts("powershell -NoProfile -Command \"cargo test\"")
                .last()
                .map(String::as_str),
            Some("cargo test")
        );
        assert!(command("powershell -Command 'gh pr merge 5'").is_some());
    }
}
