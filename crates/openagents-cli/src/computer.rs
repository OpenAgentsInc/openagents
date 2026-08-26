//! Computer agent daemon, environment probing, security policy engine, and
//! execution journal.
//!
//! Ported from `packages/openagents-cli/src/computer-policy.ts`,
//! `computer-config.ts`, `computer-probe.ts`, `computer-journal.ts`,
//! `computer-executor.ts`, `computer-client.ts`, `computer-channel.ts`, and
//! `computer-up.ts`.
//!
//! The authority runs one way. This machine decides what may run on it; the
//! server can ask, and every answer is a decision this file made against a
//! local configuration file the owner controls. The policy therefore starts
//! closed — the default tier reaches nothing and the default root set is empty,
//! so no working directory is reachable — and widens only where the owner
//! declares it. The version this replaces held three unconditional `true`s.

use crate::acp::{AcpEvent, AcpFailure, AcpHarness, PermissionQuery};
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// tiers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Probe,
    Curated,
    Shell,
}

impl Tier {
    pub fn rank(self) -> u8 {
        match self {
            Tier::Probe => 0,
            Tier::Curated => 1,
            Tier::Shell => 2,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tier::Probe => "probe",
            Tier::Curated => "curated",
            Tier::Shell => "shell",
        }
    }

    pub fn parse(value: &str) -> Option<Tier> {
        match value {
            "probe" => Some(Tier::Probe),
            "curated" => Some(Tier::Curated),
            "shell" => Some(Tier::Shell),
            _ => None,
        }
    }
}

/// A request at `requested` is inside a ceiling of `ceiling`.
pub fn tier_allows(ceiling: Tier, requested: Tier) -> bool {
    requested.rank() <= ceiling.rank()
}

// ---------------------------------------------------------------------------
// paths and configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputerPaths {
    pub config: PathBuf,
    pub journal: PathBuf,
}

impl ComputerPaths {
    /// The same two files the TypeScript CLI reads, so both binaries see one
    /// machine's policy and one machine's audit trail.
    pub fn in_directory(directory: &Path) -> Self {
        Self {
            config: directory.join("computer.json"),
            journal: directory.join("journal.ndjson"),
        }
    }

    pub fn default_paths() -> Self {
        Self::in_directory(&crate::auth::config_directory())
    }
}

/// An ACP agent the owner declared in `computer.json`.
///
/// A declared agent widens what may be delegated here, so it is the owner's
/// statement rather than the server's: the controller runs `argv` and passes
/// through only the environment variables named in `env`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentEntry {
    pub argv: Vec<String>,
    #[serde(default)]
    pub env: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PolicyConfig {
    pub tier: Tier,
    pub roots: Vec<PathBuf>,
    pub pre_approved: Vec<String>,
    pub curated_execute: Vec<String>,
    /// ACP agents the owner declared, by id.
    pub agents: BTreeMap<String, AgentEntry>,
    /// Whether a forge credential the server delivers with a delegation may be
    /// used for a delegated push from this machine.
    ///
    /// The Computers page has an `Allow scoped forge credentials on this
    /// computer` checkbox, and the server withholds the credential entirely
    /// unless it is ticked. This is the same decision made again on this side,
    /// because the machine is what decides what runs here — and like every
    /// other part of this policy it starts closed.
    pub scoped_forge_credentials: bool,
    pub paths: ComputerPaths,
}

impl PolicyConfig {
    /// Closed. No tier above `probe`, and no root, so nothing is reachable
    /// until the owner declares something.
    pub fn closed(paths: ComputerPaths) -> Self {
        Self {
            tier: Tier::Probe,
            roots: Vec::new(),
            pre_approved: Vec::new(),
            curated_execute: default_curated_execute(),
            agents: BTreeMap::new(),
            scoped_forge_credentials: false,
            paths,
        }
    }
}

pub fn default_curated_execute() -> Vec<String> {
    [
        "git", "gh", "ls", "cat", "head", "tail", "wc", "pwd", "which", "find", "rg", "grep",
        "sed", "node", "npm", "npx", "pnpm", "python3", "cargo", "go", "make", "mix",
    ]
    .iter()
    .map(|value| value.to_string())
    .collect()
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct StoredConfiguration {
    #[serde(skip_serializing_if = "Option::is_none")]
    tier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    roots: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pre_approved: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    curated_execute: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    registry_agents: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    agents: Option<BTreeMap<String, AgentEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scoped_forge_credentials: Option<bool>,
}

const MAXIMUM_CONFIGURATION_BYTES: u64 = 16_384;

/// Read the local policy.
///
/// A missing file is the closed default, which is a real answer. A file that
/// cannot be read, is too large, or does not decode is an error: continuing
/// with the default would silently widen or narrow the owner's policy without
/// saying so.
pub fn load_config(paths: &ComputerPaths) -> Result<PolicyConfig, String> {
    let text = match std::fs::read_to_string(&paths.config) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(PolicyConfig::closed(paths.clone()))
        }
        Err(error) => {
            return Err(format!(
                "the local Computer configuration at {} could not be read: {error}",
                paths.config.display()
            ))
        }
    };
    if text.len() as u64 > MAXIMUM_CONFIGURATION_BYTES {
        return Err(format!(
            "the local Computer configuration at {} is larger than {MAXIMUM_CONFIGURATION_BYTES} bytes",
            paths.config.display()
        ));
    }
    let stored: StoredConfiguration = serde_json::from_str(&text).map_err(|error| {
        format!(
            "the local Computer configuration at {} is not valid JSON: {error}",
            paths.config.display()
        )
    })?;
    let tier = match stored.tier.as_deref() {
        None => Tier::Probe,
        Some(value) => Tier::parse(value).ok_or_else(|| {
            format!(
                "the local Computer configuration names an unknown tier {value}. \
                 Use probe, curated, or shell"
            )
        })?,
    };
    let mut pre_approved: Vec<String> = Vec::new();
    for value in stored.pre_approved.unwrap_or_default() {
        if !pre_approved.contains(&value) {
            pre_approved.push(value);
        }
        if pre_approved.len() == 64 {
            break;
        }
    }
    let mut curated_execute: Vec<String> = Vec::new();
    for value in stored
        .curated_execute
        .unwrap_or_else(default_curated_execute)
    {
        if !value.is_empty() && !curated_execute.contains(&value) {
            curated_execute.push(value);
        }
        if curated_execute.len() == 64 {
            break;
        }
    }
    let mut agents: BTreeMap<String, AgentEntry> = BTreeMap::new();
    for (id, entry) in stored.agents.unwrap_or_default() {
        // A declared agent with no command names nothing. Dropping it is not a
        // silent narrowing: it never widened anything to begin with.
        if id.is_empty() || entry.argv.is_empty() || agents.len() >= 32 {
            continue;
        }
        agents.insert(id, entry);
    }
    Ok(PolicyConfig {
        tier,
        roots: resolve_roots(&stored.roots.unwrap_or_default()),
        pre_approved,
        curated_execute,
        agents,
        scoped_forge_credentials: stored.scoped_forge_credentials.unwrap_or(false),
        paths: paths.clone(),
    })
}

pub fn write_config(config: &PolicyConfig) -> Result<(), String> {
    let stored = StoredConfiguration {
        tier: Some(config.tier.label().to_string()),
        roots: Some(
            config
                .roots
                .iter()
                .map(|root| root.display().to_string())
                .collect(),
        ),
        pre_approved: Some(config.pre_approved.clone()),
        curated_execute: Some(config.curated_execute.clone()),
        registry_agents: Some(false),
        agents: Some(config.agents.clone()),
        scoped_forge_credentials: Some(config.scoped_forge_credentials),
    };
    let encoded = serde_json::to_string_pretty(&stored)
        .map_err(|error| format!("the Computer configuration could not be encoded: {error}"))?;
    write_private_file(&config.paths.config, &format!("{encoded}\n"))
}

/// Write `computer.json` or the agent-key store `0600`, staged and renamed.
///
/// This wrote in place — `fs::write` onto the target, then `chmod` — which two
/// `oa` processes under one config directory could interleave into one file,
/// leaving JSON that neither of them wrote and that the next run refuses to
/// decode. It also left the file world-readable for the moment between the
/// create and the `chmod`, which for the agent-key store is a window on
/// credentials. Staging under a name unique to this call and renaming fixes
/// both: `rename` inside a directory is atomic, so the last writer wins whole
/// and no reader sees a partial file. See [`crate::auth::unique_temp_path`].
fn write_private_file(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(directory) = path.parent() {
        std::fs::create_dir_all(directory)
            .map_err(|error| format!("could not create {}: {error}", directory.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700));
        }
    }
    let temporary = crate::auth::unique_temp_path(path);
    if let Err(error) = stage_private_file(&temporary, contents) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    std::fs::rename(&temporary, path).map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        format!("could not write {}: {error}", path.display())
    })
}

/// Create the staging file `0600` and put `contents` in it.
fn stage_private_file(temporary: &Path, contents: &str) -> Result<(), String> {
    let mut options = std::fs::OpenOptions::new();
    // The name is unique to this call, so anything already there means the
    // assumption broke; refuse rather than trample it.
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(temporary)
        .map_err(|error| format!("could not write {}: {error}", temporary.display()))?;
    file.write_all(contents.as_bytes())
        .map_err(|error| format!("could not write {}: {error}", temporary.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(temporary, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("could not secure {}: {error}", temporary.display()))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// path semantics
// ---------------------------------------------------------------------------

/// Resolve `.` and `..` without touching the filesystem, the way Node's
/// `path.normalize` does. A root that does not exist yet still has a meaning,
/// and a path argument must be judged before anything runs.
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut parts: Vec<Component> = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match parts.last() {
                Some(Component::Normal(_)) => {
                    parts.pop();
                }
                Some(Component::RootDir) | Some(Component::Prefix(_)) => {}
                _ => parts.push(component),
            },
            other => parts.push(other),
        }
    }
    parts.iter().collect()
}

fn absolutize(value: &Path, base: &Path) -> PathBuf {
    if value.is_absolute() {
        normalize_path(value)
    } else {
        normalize_path(&base.join(value))
    }
}

/// Expand a leading `~` and make the root absolute against the process
/// directory, then normalize it.
pub fn resolve_root(root: &str) -> PathBuf {
    let expanded = if root == "~" {
        crate::auth::home_directory()
    } else if let Some(rest) = root.strip_prefix("~/") {
        crate::auth::home_directory().join(rest)
    } else {
        PathBuf::from(root)
    };
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    absolutize(&expanded, &cwd)
}

pub fn resolve_roots(roots: &[String]) -> Vec<PathBuf> {
    let mut seen: Vec<PathBuf> = Vec::new();
    for root in roots {
        let resolved = resolve_root(root);
        if !seen.contains(&resolved) {
            seen.push(resolved);
        }
    }
    seen
}

/// True when `candidate` is the root itself or lives under it.
pub fn within_root(candidate: &Path, root: &Path) -> bool {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let target = absolutize(candidate, &cwd);
    let base = absolutize(root, &cwd);
    target == base || target.starts_with(&base)
}

// ---------------------------------------------------------------------------
// the curated allowlist
// ---------------------------------------------------------------------------

/// The curated allowlist, in the order the policy prints it. Each entry is the
/// binary and the options it may carry; an empty option list means "no options,
/// and path arguments only inside a declared root".
pub fn curated_allowlist() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        (
            "git",
            vec![
                "status",
                "log",
                "diff",
                "branch",
                "remote",
                "show",
                "rev-parse",
                "ls-files",
                "--version",
            ],
        ),
        ("uname", vec![]),
        ("date", vec![]),
        ("echo", vec![]),
        ("whoami", vec![]),
        ("df", vec![]),
        ("du", vec![]),
        ("ps", vec![]),
        ("uptime", vec![]),
        ("file", vec![]),
        ("stat", vec![]),
        ("ls", vec![]),
        ("cat", vec![]),
        ("head", vec![]),
        ("tail", vec![]),
        ("wc", vec![]),
        ("pwd", vec![]),
        ("which", vec![]),
        ("rg", vec![]),
        ("grep", vec![]),
        ("node", vec!["--version"]),
        ("npm", vec!["--version", "ls"]),
        ("pnpm", vec!["--version", "ls"]),
        ("python3", vec!["--version"]),
        ("cargo", vec!["--version"]),
        ("go", vec!["version"]),
        ("docker", vec!["ps", "images", "version"]),
    ]
}

/// The allowlist as the policy command prints it, one binary to a line, with
/// `gh` last. The TypeScript CLI prints exactly these strings.
pub fn format_allowlist() -> Vec<String> {
    let mut lines: Vec<String> = curated_allowlist()
        .into_iter()
        .map(|(name, options)| {
            if options.is_empty() {
                format!("{name}: no options; path arguments inside declared roots")
            } else {
                format!("{name}: {}", options.join(", "))
            }
        })
        .collect();
    lines.push("gh: read-only queries only".to_string());
    lines
}

const DENIED_COMMANDS: [&str; 22] = [
    "sudo",
    "doas",
    "su",
    "chmod",
    "chown",
    "mkfs",
    "dd",
    "shutdown",
    "reboot",
    "halt",
    "passwd",
    "ssh-keygen",
    "ssh-add",
    "keychain",
    "security",
    "gpg",
    "crontab",
    "systemctl",
    "launchctl",
    "nc",
    "ncat",
    "telnet",
];

const DENIED_PATH_FRAGMENTS: [&str; 12] = [
    ".ssh",
    ".aws",
    ".gnupg",
    ".kube",
    ".netrc",
    ".npmrc",
    ".pypirc",
    ".git-credentials",
    "id_rsa",
    "id_ed25519",
    ".env",
    "credentials.json",
];

const DENIED_PATH_FRAGMENT_KEYCHAINS: &str = "Keychains";

fn has_shell_metacharacter(value: &str) -> bool {
    value.chars().any(|c| {
        matches!(
            c,
            ';' | '&' | '|' | '`' | '$' | '>' | '<' | '\n' | '\r' | '\\'
        )
    })
}

fn command_name(value: &str) -> String {
    let last = value.rsplit(['/', '\\']).next().unwrap_or(value);
    let lowered = last.to_ascii_lowercase();
    lowered
        .strip_suffix(".exe")
        .map(str::to_string)
        .unwrap_or(lowered)
}

fn looks_like_path(value: &str) -> bool {
    Path::new(value).is_absolute()
        || value.starts_with('~')
        || value.contains("../")
        || value.contains("..\\")
        || value.starts_with("./")
        || value.starts_with(".\\")
}

// ---------------------------------------------------------------------------
// the decision
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRequest {
    pub argv: Vec<String>,
    pub cwd: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalReason {
    EmptyCommand,
    TierInsufficient,
    NotAllowlisted,
    RootNotDeclared,
    DeniedCommand,
    DeniedArgument,
    ShellMetacharacter,
    ConfirmationRequired,
}

impl RefusalReason {
    pub fn label(self) -> &'static str {
        match self {
            RefusalReason::EmptyCommand => "empty_command",
            RefusalReason::TierInsufficient => "tier_insufficient",
            RefusalReason::NotAllowlisted => "not_allowlisted",
            RefusalReason::RootNotDeclared => "root_not_declared",
            RefusalReason::DeniedCommand => "denied_command",
            RefusalReason::DeniedArgument => "denied_argument",
            RefusalReason::ShellMetacharacter => "shell_metacharacter",
            RefusalReason::ConfirmationRequired => "confirmation_required",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Allowed {
        needs_confirmation: bool,
    },
    Refused {
        reason: RefusalReason,
        detail: String,
    },
}

impl Decision {
    pub fn allowed(&self) -> bool {
        matches!(self, Decision::Allowed { .. })
    }
}

fn refuse(reason: RefusalReason, detail: &str) -> Decision {
    Decision::Refused {
        reason,
        detail: detail.to_string(),
    }
}

const GH_READ_TOP_LEVEL: [&str; 4] = ["search", "status", "--version", "version"];
const GH_API_WRITE_FLAGS: [&str; 5] = ["-f", "-F", "--field", "--raw-field", "--input"];

fn gh_read_actions(resource: &str) -> Option<&'static [&'static str]> {
    Some(match resource {
        "issue" => &["list", "view", "status"],
        "pr" => &["list", "view", "status", "diff", "checks"],
        "release" => &["list", "view"],
        "run" => &["list", "view"],
        "workflow" => &["list", "view"],
        "repo" => &["list", "view"],
        "gist" => &["list", "view"],
        "cache" => &["list"],
        "label" => &["list"],
        "ruleset" => &["list", "view"],
        "auth" => &["status"],
        _ => return None,
    })
}

pub fn gh_read_only_allowed(args: &[String]) -> bool {
    let Some(resource) = args.first() else {
        return false;
    };
    if GH_READ_TOP_LEVEL.contains(&resource.as_str()) {
        return args.len() == 1;
    }
    if resource == "api" {
        return false;
    }
    let Some(action) = args.get(1) else {
        return false;
    };
    args.len() == 2
        && !args
            .iter()
            .any(|arg| GH_API_WRITE_FLAGS.contains(&arg.as_str()))
        && gh_read_actions(resource)
            .map(|actions| actions.contains(&action.as_str()))
            .unwrap_or(false)
}

fn no_arguments(args: &[String]) -> bool {
    args.is_empty()
}

fn non_option_arguments(args: &[String]) -> bool {
    !args.is_empty() && args.iter().all(|arg| arg == "--" || !arg.starts_with('-'))
}

fn bounded_words(args: &[String]) -> bool {
    args.len() <= 16
        && args
            .iter()
            .all(|arg| !arg.starts_with('-') && arg.len() <= 256)
}

fn git_arguments(args: &[String]) -> bool {
    let Some(subcommand) = args.first() else {
        return false;
    };
    let rest = &args[1..];
    if subcommand == "--version" {
        return rest.is_empty();
    }
    const READ_SUBCOMMANDS: [&str; 8] = [
        "status",
        "log",
        "diff",
        "branch",
        "remote",
        "show",
        "rev-parse",
        "ls-files",
    ];
    if !READ_SUBCOMMANDS.contains(&subcommand.as_str()) {
        return false;
    }
    const DENIED: [&str; 14] = [
        "--exec-path",
        "--ext-diff",
        "--no-ext-diff",
        "--textconv",
        "--no-textconv",
        "--delete",
        "-D",
        "-d",
        "--set-upstream",
        "--unset-upstream",
        "set-url",
        "set-head",
        "set-branches",
        "update-ref",
    ];
    if subcommand == "branch" {
        const READ_OPTIONS: [&str; 10] = [
            "-a",
            "-r",
            "--all",
            "--remotes",
            "--contains",
            "--merged",
            "--no-merged",
            "--list",
            "--verbose",
            "-v",
        ];
        return rest.iter().all(|arg| {
            READ_OPTIONS.contains(&arg.as_str()) || (arg.starts_with("--sort=") && arg.len() <= 128)
        });
    }
    if subcommand == "remote" {
        return rest.iter().all(|arg| arg == "-v" || arg == "--verbose");
    }
    rest.iter().all(|arg| {
        !DENIED.contains(&arg.as_str())
            && !arg.starts_with("--upload-pack=")
            && !arg.starts_with("--output")
    })
}

fn curated_argument_rule(name: &str, args: &[String]) -> Option<bool> {
    Some(match name {
        "git" => git_arguments(args),
        "uname" | "date" | "whoami" | "df" | "ps" | "uptime" | "pwd" => no_arguments(args),
        "echo" | "which" => bounded_words(args),
        "du" | "file" | "stat" | "ls" | "cat" | "head" | "tail" | "wc" | "rg" | "grep" => {
            non_option_arguments(args)
        }
        "node" => args.len() == 1 && args[0] == "--version",
        "npm" | "pnpm" => args.len() == 1 && (args[0] == "--version" || args[0] == "ls"),
        "python3" | "cargo" => args.len() == 1 && args[0] == "--version",
        "go" => args.len() == 1 && args[0] == "version",
        "docker" => {
            args.len() == 1 && (args[0] == "ps" || args[0] == "images" || args[0] == "version")
        }
        _ => return None,
    })
}

/// Decide one command against this machine's policy.
///
/// Every refusal names why. The order matters: a denied binary or a protected
/// path is refused before the tier is consulted, so raising the tier never
/// unlocks `sudo` or a read of `~/.ssh`.
pub fn decide(request: &CommandRequest, config: &PolicyConfig) -> Decision {
    let Some(argv0) = request.argv.first() else {
        return refuse(RefusalReason::EmptyCommand, "no command was supplied");
    };
    if argv0.trim().is_empty() {
        return refuse(RefusalReason::EmptyCommand, "no command was supplied");
    }
    if request
        .argv
        .iter()
        .any(|part| has_shell_metacharacter(part))
    {
        return refuse(
            RefusalReason::ShellMetacharacter,
            "command arguments cannot contain shell metacharacters",
        );
    }
    let name = command_name(argv0);
    if DENIED_COMMANDS.contains(&name.as_str()) {
        return refuse(
            RefusalReason::DeniedCommand,
            &format!("{name} is denied on this machine"),
        );
    }
    for argument in &request.argv {
        let fragment = DENIED_PATH_FRAGMENTS
            .iter()
            .chain(std::iter::once(&DENIED_PATH_FRAGMENT_KEYCHAINS))
            .find(|candidate| argument.contains(**candidate));
        if let Some(fragment) = fragment {
            return refuse(
                RefusalReason::DeniedArgument,
                &format!("argument references a protected path: {fragment}"),
            );
        }
    }
    let cwd = PathBuf::from(&request.cwd);
    if config.roots.is_empty() || !config.roots.iter().any(|root| within_root(&cwd, root)) {
        return refuse(
            RefusalReason::RootNotDeclared,
            "the working directory is outside every declared root",
        );
    }
    let rest = &request.argv[1..];
    let escapes = rest.iter().any(|argument| {
        looks_like_path(argument)
            && !config.roots.iter().any(|root| {
                let candidate = if argument.starts_with('~') {
                    resolve_root(argument)
                } else {
                    absolutize(Path::new(argument), &cwd)
                };
                within_root(&candidate, root)
            })
    });
    if escapes {
        return refuse(
            RefusalReason::DeniedArgument,
            "a path argument is outside every declared root",
        );
    }
    if !tier_allows(config.tier, Tier::Curated) {
        return refuse(
            RefusalReason::TierInsufficient,
            "probe tier permits fixed discovery only",
        );
    }
    if !tier_allows(config.tier, Tier::Shell) {
        if name == "gh" {
            return if gh_read_only_allowed(rest) {
                Decision::Allowed {
                    needs_confirmation: false,
                }
            } else {
                refuse(
                    RefusalReason::NotAllowlisted,
                    "this gh command is not a permitted read-only operation",
                )
            };
        }
        let permitted = curated_allowlist()
            .into_iter()
            .find(|(candidate, _)| *candidate == name);
        let Some(rule) = curated_argument_rule(&name, rest) else {
            return refuse(
                RefusalReason::NotAllowlisted,
                &format!("{name} is not in the curated allowlist"),
            );
        };
        if permitted.is_none() {
            return refuse(
                RefusalReason::NotAllowlisted,
                &format!("{name} is not in the curated allowlist"),
            );
        }
        if !rule {
            let first = rest.first().map(String::as_str).unwrap_or("");
            return refuse(
                RefusalReason::NotAllowlisted,
                format!("{name} {first} is not in the curated allowlist").trim(),
            );
        }
        return Decision::Allowed {
            needs_confirmation: false,
        };
    }
    Decision::Allowed {
        needs_confirmation: !config.pre_approved.contains(&name),
    }
}

// ---------------------------------------------------------------------------
// redaction
// ---------------------------------------------------------------------------

fn redaction_patterns() -> &'static [regex::Regex] {
    use std::sync::OnceLock;
    static PATTERNS: OnceLock<Vec<regex::Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            regex::Regex::new(r"oa_(?:pat|agent|assignment)_[A-Za-z0-9._-]+").unwrap(),
            regex::Regex::new(r"smct_[A-Za-z0-9._-]+").unwrap(),
        ]
    })
}

fn bearer_pattern() -> &'static regex::Regex {
    use std::sync::OnceLock;
    static PATTERN: OnceLock<regex::Regex> = OnceLock::new();
    PATTERN.get_or_init(|| regex::Regex::new(r"(?i)Bearer\s+\S+").unwrap())
}

/// Remove anything that looks like a credential. The journal and the streamed
/// output of an allowed command both pass through here, so a token that lands
/// in a command's output never reaches the file or the wire.
pub fn redact(value: &str) -> String {
    let mut text = value.to_string();
    for pattern in redaction_patterns() {
        text = pattern.replace_all(&text, "[REDACTED]").into_owned();
    }
    bearer_pattern()
        .replace_all(&text, "Bearer [REDACTED]")
        .into_owned()
}

fn bounded(value: &str, limit: usize) -> String {
    let redacted = redact(value);
    if redacted.len() <= limit {
        return redacted;
    }
    let mut end = limit;
    while end > 0 && !redacted.is_char_boundary(end) {
        end -= 1;
    }
    redacted[..end].to_string()
}

// ---------------------------------------------------------------------------
// journal
// ---------------------------------------------------------------------------

pub const JOURNAL_MAX_BYTES: usize = 1_048_576;
pub const JOURNAL_READ_TAIL_BYTES: usize = 262_144;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JournalEntry {
    pub at: String,
    #[serde(rename = "requestId")]
    pub request_id: String,
    pub argv: Vec<String>,
    pub cwd: String,
    pub decision: String,
    pub outcome: String,
    pub detail: String,
}

/// The local record of what was asked and what this machine decided.
///
/// It never leaves the machine. `computer up` writes it, `computer journal`
/// reads it, and nothing sends it to the server.
#[derive(Debug, Clone)]
pub struct Journal {
    path: PathBuf,
}

fn now_iso8601() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let seconds = now.as_secs() as i64;
    let millis = now.subsec_millis();
    let days = seconds.div_euclid(86_400);
    let time = seconds.rem_euclid(86_400);
    // Civil-from-days, Howard Hinnant's algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    format!(
        "{year:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        time / 3_600,
        (time % 3_600) / 60,
        time % 60
    )
}

impl Journal {
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one bounded, redacted entry, trimming the file from the front
    /// once it passes the retention limit.
    pub fn append(
        &self,
        request_id: &str,
        request: &CommandRequest,
        decision: &str,
        outcome: &str,
        detail: &str,
    ) -> Result<(), String> {
        let entry = JournalEntry {
            at: now_iso8601(),
            request_id: bounded(request_id, 64),
            argv: request
                .argv
                .iter()
                .take(8)
                .map(|value| bounded(value, 128))
                .collect(),
            cwd: bounded(&request.cwd, 1_024),
            decision: bounded(decision, 64),
            outcome: bounded(outcome, 64),
            detail: bounded(detail, 512),
        };
        let line = serde_json::to_string(&entry)
            .map_err(|error| format!("the Computer journal entry could not be encoded: {error}"))?;
        let line = format!("{line}\n");

        if let Some(directory) = self.path.parent() {
            std::fs::create_dir_all(directory)
                .map_err(|error| format!("could not create {}: {error}", directory.display()))?;
        }
        let existing = match std::fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => {
                return Err(format!(
                    "the local Computer journal at {} could not be read: {error}",
                    self.path.display()
                ))
            }
        };
        let retained_limit = JOURNAL_MAX_BYTES.saturating_sub(line.len());
        let retained: Vec<u8> = if existing.len() <= retained_limit {
            existing
        } else {
            let tail = &existing[existing.len() - retained_limit..];
            match tail.iter().position(|byte| *byte == b'\n') {
                Some(index) => tail[index + 1..].to_vec(),
                None => Vec::new(),
            }
        };
        let mut contents = retained;
        contents.extend_from_slice(line.as_bytes());
        if let Some(directory) = self.path.parent() {
            let _ = std::fs::create_dir_all(directory);
        }
        std::fs::write(&self.path, &contents).map_err(|error| {
            format!(
                "the local Computer journal at {} could not be written: {error}",
                self.path.display()
            )
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    /// Read the last `limit` entries.
    ///
    /// A missing file is an empty journal, which is a real answer: nothing has
    /// been asked of this machine. A file that exists and cannot be read is an
    /// error rather than an empty list.
    pub fn read(&self, limit: usize) -> Result<Vec<JournalEntry>, String> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let bytes = match std::fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(format!(
                    "the local Computer journal at {} could not be read: {error}",
                    self.path.display()
                ))
            }
        };
        let tail = if bytes.len() > JOURNAL_READ_TAIL_BYTES {
            &bytes[bytes.len() - JOURNAL_READ_TAIL_BYTES..]
        } else {
            &bytes[..]
        };
        let text = String::from_utf8_lossy(tail);
        let lines: Vec<&str> = text
            .split('\n')
            .filter(|line| !line.trim().is_empty())
            .collect();
        let start = lines.len().saturating_sub(limit);
        Ok(lines[start..]
            .iter()
            .filter_map(|line| serde_json::from_str::<JournalEntry>(line).ok())
            .collect())
    }
}

// ---------------------------------------------------------------------------
// executor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct ExecutionLimits {
    pub timeout: Duration,
    pub maximum_output_bytes: usize,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            maximum_output_bytes: 64 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionOutcome {
    pub exit_code: Option<i32>,
    pub truncated: bool,
    pub timed_out: bool,
    pub cancelled: bool,
    pub duration_ms: u64,
}

const ENVIRONMENT_NAMES: [&str; 8] = [
    "PATH", "HOME", "LANG", "LC_ALL", "TMPDIR", "SHELL", "USER", "TERM",
];

/// The only environment an allowed command sees. A delegated command that
/// inherited the whole environment would inherit this process's credentials.
pub fn scrubbed_environment() -> Vec<(String, String)> {
    ENVIRONMENT_NAMES
        .iter()
        .filter_map(|name| {
            std::env::var(name)
                .ok()
                .map(|value| (name.to_string(), value))
        })
        .collect()
}

/// A handle a caller can use to stop a running command.
#[derive(Clone, Default)]
pub struct Cancellation {
    inner: Arc<Mutex<Option<u32>>>,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
}

impl Cancellation {
    pub fn cancel(&self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(pid) = *self.inner.lock().unwrap() {
            terminate_group(pid);
        }
    }

    pub fn cancelled(&self) -> bool {
        self.cancelled.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[cfg(unix)]
fn terminate_group(pid: u32) {
    unsafe {
        // The child was started in its own process group, so this reaches the
        // whole tree rather than only the shell-less leader.
        if libc::kill(-(pid as i32), libc::SIGTERM) != 0 {
            libc::kill(pid as i32, libc::SIGTERM);
        }
    }
}

#[cfg(not(unix))]
fn terminate_group(_pid: u32) {}

#[cfg(unix)]
fn kill_group(pid: u32) {
    unsafe {
        if libc::kill(-(pid as i32), libc::SIGKILL) != 0 {
            libc::kill(pid as i32, libc::SIGKILL);
        }
    }
}

#[cfg(not(unix))]
fn kill_group(_pid: u32) {}

/// Run one already-allowed command with bounded output and a bounded lifetime.
///
/// Output is redacted as it streams, so a token printed by the command never
/// leaves this machine, and it stops at the byte limit rather than growing
/// without bound.
pub fn execute_command(
    argv: &[String],
    cwd: &str,
    limits: ExecutionLimits,
    cancellation: &Cancellation,
    mut on_chunk: impl FnMut(&str),
) -> ExecutionOutcome {
    let started = Instant::now();
    let Some(program) = argv.first() else {
        return ExecutionOutcome {
            exit_code: Some(127),
            truncated: false,
            timed_out: false,
            cancelled: false,
            duration_ms: 0,
        };
    };
    let mut command = Command::new(program);
    command
        .args(&argv[1..])
        .current_dir(cwd)
        .env_clear()
        .envs(scrubbed_environment())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => {
            return ExecutionOutcome {
                exit_code: Some(127),
                truncated: false,
                timed_out: false,
                cancelled: false,
                duration_ms: started.elapsed().as_millis() as u64,
            }
        }
    };
    let pid = child.id();
    *cancellation.inner.lock().unwrap() = Some(pid);

    let (sender, receiver) = std::sync::mpsc::channel::<Vec<u8>>();
    let mut readers = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        readers.push(spawn_reader(stdout, sender.clone()));
    }
    if let Some(stderr) = child.stderr.take() {
        readers.push(spawn_reader(stderr, sender.clone()));
    }
    drop(sender);

    let mut bytes = 0usize;
    let mut truncated = false;
    let mut timed_out = false;
    let mut exit_code: Option<i32> = None;
    let deadline = started + limits.timeout;

    loop {
        while let Ok(chunk) = receiver.try_recv() {
            let text = redact(&String::from_utf8_lossy(&chunk));
            if bytes >= limits.maximum_output_bytes {
                truncated = true;
                continue;
            }
            let remaining = limits.maximum_output_bytes - bytes;
            let mut end = remaining.min(text.len());
            while end > 0 && !text.is_char_boundary(end) {
                end -= 1;
            }
            if end < text.len() {
                truncated = true;
            }
            bytes += end;
            if end > 0 {
                on_chunk(&text[..end]);
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                exit_code = status.code();
                break;
            }
            Ok(None) => {}
            Err(_) => break,
        }
        if Instant::now() >= deadline && !timed_out {
            timed_out = true;
            terminate_group(pid);
        }
        if timed_out && Instant::now() >= deadline + Duration::from_secs(2) {
            kill_group(pid);
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    for reader in readers {
        let _ = reader.join();
    }
    while let Ok(chunk) = receiver.try_recv() {
        let text = redact(&String::from_utf8_lossy(&chunk));
        if bytes >= limits.maximum_output_bytes {
            truncated = true;
            continue;
        }
        let remaining = limits.maximum_output_bytes - bytes;
        let mut end = remaining.min(text.len());
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        if end < text.len() {
            truncated = true;
        }
        bytes += end;
        if end > 0 {
            on_chunk(&text[..end]);
        }
    }

    ExecutionOutcome {
        exit_code,
        truncated,
        timed_out,
        cancelled: cancellation.cancelled(),
        duration_ms: started.elapsed().as_millis() as u64,
    }
}

fn spawn_reader<R: Read + Send + 'static>(
    mut source: R,
    sender: Sender<Vec<u8>>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut buffer = [0u8; 8192];
        loop {
            match source.read(&mut buffer) {
                Ok(0) | Err(_) => return,
                Ok(count) => {
                    if sender.send(buffer[..count].to_vec()).is_err() {
                        return;
                    }
                }
            }
        }
    })
}

// ---------------------------------------------------------------------------
// ACP delegation
// ---------------------------------------------------------------------------

/// The longest prompt a delegation may carry, matching the TypeScript
/// controller and the tool's own input ceiling.
pub const MAXIMUM_PROMPT_LENGTH: usize = 32_768;
/// What a delegation gets when the server names no timeout.
pub const AGENT_DEFAULT_TIMEOUT_MS: u64 = 300_000;
/// The most a delegation may ask for. `OpenAgents.Computer` waits an hour at
/// most, so a longer local run would only outlive the caller.
pub const AGENT_MAXIMUM_TIMEOUT_MS: u64 = 3_600_000;
/// The most streamed output one delegation may send. `OpenAgents.Computer`
/// collects 64 KiB and drops the rest, so this is where the truncation is
/// decided rather than discovered.
pub const AGENT_MAXIMUM_OUTPUT_BYTES: usize = 64 * 1024;
pub const MAXIMUM_SESSION_ID_LENGTH: usize = 128;

/// How a known coding agent is put into ACP mode.
///
/// Every one of these is a binary the probe already looks for. An agent that is
/// installed but has no ACP mode this build knows is not delegable by name; the
/// owner declares it in `computer.json` instead, which is the honest way to
/// widen what this machine runs.
pub fn acp_invocation(agent_id: &str) -> Option<Vec<String>> {
    let argv: &[&str] = match agent_id {
        "devin" => &["devin", "acp"],
        "opencode" => &["opencode", "acp"],
        "gemini" => &["gemini", "--experimental-acp"],
        _ => return None,
    };
    Some(argv.iter().map(|part| part.to_string()).collect())
}

/// An agent this machine can actually run, and how.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAgent {
    pub id: String,
    pub argv: Vec<String>,
    /// Environment variable names passed through to the child on top of the
    /// scrubbed set. Only what the owner declared.
    pub env: Vec<String>,
    /// `configured` when the owner declared it, `local` when the probe found it.
    pub source: &'static str,
}

/// Which agents this machine will delegate to.
///
/// Two sources, and neither is the server: what the owner declared in
/// `computer.json`, and what the probe actually found installed. An agent that
/// is not on this list is refused by name — the alternative is spawning
/// whatever string the server sent.
pub fn agent_catalog(config: &PolicyConfig, installed: &[ToolReport]) -> Vec<ResolvedAgent> {
    let mut catalog: Vec<ResolvedAgent> = Vec::new();
    for tool in installed {
        if !tool.present {
            continue;
        }
        if let Some(argv) = acp_invocation(&tool.name) {
            catalog.push(ResolvedAgent {
                id: tool.name.clone(),
                argv,
                env: Vec::new(),
                source: "local",
            });
        }
    }
    // A declared agent wins over a discovered one of the same id: the owner
    // said how to run it.
    for (id, entry) in &config.agents {
        catalog.retain(|found| &found.id != id);
        catalog.push(ResolvedAgent {
            id: id.clone(),
            argv: entry.argv.clone(),
            env: entry.env.clone(),
            source: "configured",
        });
    }
    catalog.sort_by(|left, right| left.id.cmp(&right.id));
    catalog
}

/// Resolve one requested agent, or say what this machine does have.
pub fn resolve_agent(catalog: &[ResolvedAgent], requested: &str) -> Result<ResolvedAgent, String> {
    if let Some(found) = catalog.iter().find(|entry| entry.id == requested) {
        // A declared `argv` is still an argv this machine runs. The same
        // metacharacter rule every other command gets applies to it, so a
        // configuration cannot become a shell.
        if found.argv.iter().any(|part| has_shell_metacharacter(part)) {
            return Err(format!(
                "the declared command for agent {requested} contains shell metacharacters"
            ));
        }
        return Ok(found.clone());
    }
    let available: Vec<&str> = catalog.iter().map(|entry| entry.id.as_str()).collect();
    Err(format!(
        "agent {requested} is unavailable; available agents: {}",
        if available.is_empty() {
            "(none)".to_string()
        } else {
            available.join(", ")
        }
    ))
}

/// Every string in a JSON value, so a policy decision reads the whole tool
/// input rather than the keys it happened to expect.
fn strings_within(value: &serde_json::Value, depth: usize, found: &mut Vec<String>) {
    if depth > 6 || found.len() > 64 {
        return;
    }
    match value {
        serde_json::Value::String(text) => found.push(text.clone()),
        serde_json::Value::Array(items) => {
            for item in items {
                strings_within(item, depth + 1, found);
            }
        }
        serde_json::Value::Object(fields) => {
            for item in fields.values() {
                strings_within(item, depth + 1, found);
            }
        }
        _ => {}
    }
}

fn first_string(value: &serde_json::Value, names: &[&str]) -> Option<String> {
    for name in names {
        if let Some(found) = value.get(*name).and_then(|found| found.as_str()) {
            if !found.is_empty() {
                return Some(found.to_string());
            }
        }
    }
    None
}

fn mentions_word(haystack: &str, word: &str) -> bool {
    let bytes = haystack.as_bytes();
    let mut from = 0usize;
    while let Some(offset) = haystack[from..].find(word) {
        let start = from + offset;
        let end = start + word.len();
        let before_ok = start == 0 || !(bytes[start - 1] as char).is_ascii_alphanumeric();
        let after_ok = end == bytes.len() || !(bytes[end] as char).is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
        from = start + 1;
    }
    false
}

/// Metacharacters that defeat a per-segment allowlist rather than separate
/// segments. `ls $(curl http://x)` has `ls` as its first word and runs `curl`;
/// `cat > /etc/hosts` has `cat` as its first word and writes a protected file.
/// Neither is decidable by splitting, so both are refused outright.
fn has_substitution_or_redirection(command: &str) -> bool {
    command.contains('`')
        || command.contains("$(")
        || command.contains("${")
        || command.contains('>')
        || command.contains('<')
        || command.contains('\\')
        || command.contains('\n')
        || command.contains('\r')
}

/// The shell segments a command runs, split on the operators that chain them.
fn command_segments(command: &str) -> Vec<String> {
    let mut segments: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut rest = command;
    while let Some(index) = rest.find(['&', '|', ';']) {
        current.push_str(&rest[..index]);
        segments.push(std::mem::take(&mut current));
        let tail = &rest[index..];
        let skip = if tail.starts_with("&&") || tail.starts_with("||") {
            2
        } else {
            1
        };
        rest = &tail[skip..];
    }
    current.push_str(rest);
    segments.push(current);
    segments
}

/// Decide one thing a delegated agent asked permission to do.
///
/// This is the same policy the `run` path applies, read through the agent's
/// own vocabulary: the tier ceiling decides whether anything at all is
/// permitted, a denied binary or a protected path is refused before the tier
/// is consulted, an edit must land inside a declared root, and an execute must
/// be an allowlisted binary in every segment it chains. A delegated agent is
/// not a way around any of it.
pub fn agent_permission(config: &PolicyConfig, cwd: &Path, query: &PermissionQuery) -> Decision {
    let mut material: Vec<String> = vec![query.title.clone()];
    strings_within(&query.raw_input, 0, &mut material);

    for text in &material {
        let lowered = text.to_ascii_lowercase();
        if let Some(denied) = DENIED_COMMANDS
            .iter()
            .find(|candidate| mentions_word(&lowered, candidate))
        {
            return refuse(
                RefusalReason::DeniedCommand,
                &format!("{denied} is denied on this machine"),
            );
        }
        if let Some(fragment) = DENIED_PATH_FRAGMENTS
            .iter()
            .chain(std::iter::once(&DENIED_PATH_FRAGMENT_KEYCHAINS))
            .find(|candidate| text.contains(**candidate))
        {
            return refuse(
                RefusalReason::DeniedArgument,
                &format!("the request references a protected path: {fragment}"),
            );
        }
    }

    if !tier_allows(config.tier, Tier::Curated) {
        return refuse(
            RefusalReason::TierInsufficient,
            "probe tier permits fixed discovery only",
        );
    }
    if tier_allows(config.tier, Tier::Shell) {
        return Decision::Allowed {
            needs_confirmation: false,
        };
    }

    let within_declared = |candidate: &str| {
        let resolved = if candidate.starts_with('~') {
            resolve_root(candidate)
        } else if Path::new(candidate).is_absolute() {
            normalize_path(Path::new(candidate))
        } else {
            normalize_path(&cwd.join(candidate))
        };
        !config.roots.is_empty() && config.roots.iter().any(|root| within_root(&resolved, root))
    };

    match query.kind.as_str() {
        "read" | "search" | "fetch" | "think" => Decision::Allowed {
            needs_confirmation: false,
        },
        "edit" | "write" | "delete" | "move" => {
            let Some(path) =
                first_string(&query.raw_input, &["path", "file_path", "filePath", "file"])
            else {
                return refuse(
                    RefusalReason::RootNotDeclared,
                    "the agent named no path to write, so it cannot be placed inside a root",
                );
            };
            if within_declared(&path) {
                Decision::Allowed {
                    needs_confirmation: false,
                }
            } else {
                refuse(
                    RefusalReason::RootNotDeclared,
                    "the path is outside every declared root",
                )
            }
        }
        "execute" => {
            let command = first_string(&query.raw_input, &["command", "cmd", "commandLine"])
                .unwrap_or_else(|| query.title.clone());
            if command.trim().is_empty() {
                return refuse(
                    RefusalReason::EmptyCommand,
                    "the agent named no command to run",
                );
            }
            if has_substitution_or_redirection(&command) {
                return refuse(
                    RefusalReason::ShellMetacharacter,
                    "the command uses substitution or redirection, which no allowlist can bound",
                );
            }
            for segment in command_segments(&command) {
                let mut words = segment.split_whitespace();
                let Some(first) = words.next() else {
                    // An empty segment is what a trailing `&&` leaves. It runs
                    // nothing, so it decides nothing.
                    continue;
                };
                let name = command_name(first);
                if name == "cd" {
                    // `cd` is permitted only where the policy already reaches.
                    // Otherwise it is the first half of an escape from every
                    // declared root.
                    let target = words.next().unwrap_or("");
                    if target.is_empty() || !within_declared(target) {
                        return refuse(
                            RefusalReason::RootNotDeclared,
                            "the command changes directory outside every declared root",
                        );
                    }
                    continue;
                }
                if !config.curated_execute.contains(&name) {
                    return refuse(
                        RefusalReason::NotAllowlisted,
                        &format!("{name} is not in the curated allowlist"),
                    );
                }
            }
            Decision::Allowed {
                needs_confirmation: false,
            }
        }
        other => refuse(
            RefusalReason::NotAllowlisted,
            &format!(
                "{} is not a permitted action at the curated tier",
                if other.is_empty() {
                    "an unnamed action"
                } else {
                    other
                }
            ),
        ),
    }
}

// ---------------------------------------------------------------------------
// delegated push
// ---------------------------------------------------------------------------

/// A forge credential the server delivered with one delegation.
///
/// It is scoped to a single repository and a single branch, and it lives only
/// as long as the delegation. It never reaches the child's environment, the
/// journal, or the wire: the only thing that ever reads it is the credential
/// helper this machine writes for one `git push`.
#[derive(Clone)]
pub struct ForgeCredentials {
    pub token: crate::auth::Secret,
    pub repository: String,
    pub branch: String,
}

impl std::fmt::Debug for ForgeCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ForgeCredentials")
            .field("repository", &self.repository)
            .field("branch", &self.branch)
            .finish_non_exhaustive()
    }
}

/// Read the credential out of a delegation payload, if it carries a whole one.
///
/// A token without the repository and branch it is scoped to is not usable:
/// there would be nothing to check the push against. That is reported as
/// incomplete rather than quietly ignored.
pub fn forge_credentials(payload: &serde_json::Value) -> Option<ForgeCredentials> {
    let raw = payload
        .get("assignment_credential")
        .or_else(|| payload.get("forge_credentials"))?;
    let (token, repository, branch) = if let Some(text) = raw.as_str() {
        (
            text.to_string(),
            first_string(payload, &["assignment_repository", "repository"])?,
            first_string(payload, &["assignment_branch", "branch"])?,
        )
    } else {
        let token = first_string(raw, &["token", "value", "password", "access_token"])?;
        let repository = first_string(raw, &["repository"])
            .or_else(|| first_string(payload, &["assignment_repository", "repository"]))?;
        let branch = first_string(raw, &["branch"])
            .or_else(|| first_string(payload, &["assignment_branch", "branch"]))?;
        (token, repository, branch)
    };
    if token.trim().is_empty() || repository.is_empty() || branch.is_empty() {
        return None;
    }
    Some(ForgeCredentials {
        token: crate::auth::Secret::new(token),
        repository,
        branch,
    })
}

fn canonical_branch(branch: &str) -> String {
    if branch.starts_with("refs/heads/") {
        branch.to_string()
    } else {
        format!("refs/heads/{branch}")
    }
}

/// A refspec is the assigned branch, pushed forward, and nothing else.
///
/// A scoped credential that could push any ref would not be scoped. Force,
/// deletion, multi-ref, and any other branch are all refused here rather than
/// at the forge, so this machine is not the thing that tried.
pub fn validate_refspec(refspec: &str, branch: &str) -> Result<(), String> {
    if refspec.is_empty() {
        return Err("the refspec is empty".to_string());
    }
    if refspec.chars().any(|c| c.is_whitespace() || c == ',') {
        return Err("a multi-ref push is not allowed".to_string());
    }
    if refspec.starts_with('+') || refspec.starts_with('-') {
        return Err("a force or option refspec is not allowed".to_string());
    }
    let target = canonical_branch(branch);
    let matches = |value: &str| value == branch || value == target;
    match refspec.split_once(':') {
        Some((source, destination)) => {
            if destination.is_empty() {
                return Err("a refspec with an empty destination is not allowed".to_string());
            }
            if !matches(source) || !matches(destination) {
                return Err(format!("the refspec is not the assigned branch {target}"));
            }
        }
        None => {
            if !matches(refspec) {
                return Err(format!("the refspec is not the assigned branch {target}"));
            }
        }
    }
    Ok(())
}

/// Push the assigned branch with the delivered credential.
///
/// The token never becomes an argument, an environment variable, or part of a
/// URL. It is written to a file only this user can read, inside a directory
/// only this user can enter, and a helper script hands it over only when git
/// asks for exactly the host and path of the assigned repository. Everything
/// is removed when the push ends, whichever way it ends.
pub fn push_delegated(
    directory: &Path,
    remote: &str,
    refspec: &str,
    credentials: &ForgeCredentials,
    origin: &str,
) -> Result<(), String> {
    validate_refspec(refspec, &credentials.branch)?;
    let remote = crate::repo::validate_remote_name(remote).map_err(|error| error.to_string())?;
    let listed = Command::new("git")
        .args(["remote", "get-url", "--", &remote])
        .current_dir(directory)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|error| format!("git remote get-url could not start: {error}"))?;
    if !listed.status.success() {
        return Err(format!("this checkout has no {remote} remote"));
    }
    let url = String::from_utf8_lossy(&listed.stdout).trim().to_string();
    let actual =
        crate::repo::repository_from_remote_url(origin, &url).map_err(|error| error.to_string())?;
    if actual != credentials.repository {
        return Err(format!(
            "the remote repository is {actual}, not the assigned {}",
            credentials.repository
        ));
    }
    let parsed =
        reqwest::Url::parse(&url).map_err(|_| "that remote URL cannot be read".to_string())?;
    let host = parsed.host_str().unwrap_or_default().to_string();
    let host = match parsed.port() {
        Some(port) => format!("{host}:{port}"),
        None => host,
    };
    let path = parsed.path().trim_start_matches('/').to_string();
    let url_origin = format!("{}://{host}", parsed.scheme());

    let workspace = private_temporary_directory()?;
    let outcome = (|| -> Result<(), String> {
        let helper = write_credential_helper(&workspace, credentials.token.expose(), &host, &path)?;
        let mut command = Command::new("git");
        command
            .args([
                "-c",
                "credential.helper=",
                "-c",
                &format!("credential.{url_origin}.helper=!{}", helper.display()),
                "push",
                "--",
                &remote,
                refspec,
            ])
            .current_dir(directory)
            .env_clear()
            .envs(scrubbed_environment())
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let output = command
            .output()
            .map_err(|error| format!("git push could not start: {error}"))?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = redact(&String::from_utf8_lossy(&output.stderr));
        Err(format!("git push failed: {}", bounded(stderr.trim(), 400)))
    })();
    let _ = std::fs::remove_dir_all(&workspace);
    outcome
}

fn private_temporary_directory() -> Result<PathBuf, String> {
    let base = std::env::temp_dir();
    let unique = format!(
        "oa-delegated-push-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or_default()
    );
    let directory = base.join(unique);
    std::fs::create_dir(&directory)
        .map_err(|error| format!("could not create a private working directory: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("could not secure the private working directory: {error}"))?;
    }
    Ok(directory)
}

/// Stage the token and the helper that hands it over.
///
/// Public because the helper is a security boundary rather than an
/// implementation detail: what it answers for, and what it stays silent for,
/// is asserted directly by running it.
pub fn write_credential_helper(
    workspace: &Path,
    token: &str,
    host: &str,
    path: &str,
) -> Result<PathBuf, String> {
    let token_path = workspace.join("token");
    let helper_path = workspace.join("helper");
    std::fs::write(&token_path, format!("{token}\n"))
        .map_err(|error| format!("could not stage the delegated credential: {error}"))?;
    let script = format!(
        r#"#!/bin/sh
if [ "$1" != "get" ]; then
  exit 0
fi
host=""
path=""
while IFS= read -r line; do
  [ -z "$line" ] && break
  case "$line" in
    host=*) host="${{line#host=}}" ;;
    path=*) path="${{line#path=}}" ;;
  esac
done
if [ "$host" != {host} ] || [ "$path" != {path} ]; then
  exit 0
fi
PASSWORD=$(tr -d '\n' < {token_path})
printf 'username=openagents\npassword=%s\n\n' "$PASSWORD"
"#,
        host = shell_quote(host),
        path = shell_quote(path),
        token_path = shell_quote(&token_path.display().to_string()),
    );
    std::fs::write(&helper_path, script)
        .map_err(|error| format!("could not stage the credential helper: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&token_path, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("could not secure the delegated credential: {error}"))?;
        std::fs::set_permissions(&helper_path, std::fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("could not secure the credential helper: {error}"))?;
    }
    Ok(helper_path)
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

// ---------------------------------------------------------------------------
// probe
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolReport {
    pub name: String,
    pub present: bool,
    pub path: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostReport {
    pub platform: String,
    pub release: String,
    pub architecture: String,
    pub hostname: String,
    pub shell: String,
    pub cpu_count: usize,
    pub total_memory_bytes: u64,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeReport {
    pub root: String,
    pub exists: bool,
    pub git: bool,
}

/// One delegable ACP agent, as the server records it.
///
/// `OpenAgents.ComputerAgentJobs.start/4` refuses any `agent_id` that is not in
/// `last_probe["acp_agents"]`, so a report without this list is a machine the
/// server will not delegate to at all — whatever is installed on it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpAgentReport {
    pub id: String,
    pub source: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeReport {
    pub schema: String,
    pub host: HostReport,
    #[serde(rename = "codingAgents")]
    pub coding_agents: Vec<ToolReport>,
    pub toolchains: Vec<ToolReport>,
    pub roots: Vec<String>,
    pub worktrees: Vec<WorktreeReport>,
    pub acp_agents: Vec<AcpAgentReport>,
}

pub const CODING_AGENT_CATALOG: [(&str, &str); 11] = [
    ("claude", "--version"),
    ("codex", "--version"),
    ("devin", "--version"),
    ("gemini", "--version"),
    ("cursor-agent", "--version"),
    ("aider", "--version"),
    ("goose", "--version"),
    ("opencode", "--version"),
    ("amp", "--version"),
    ("copilot", "--version"),
    ("crush", "--version"),
];

pub const TOOLCHAIN_CATALOG: [(&str, &str); 14] = [
    ("git", "--version"),
    ("gh", "--version"),
    ("node", "--version"),
    ("npm", "--version"),
    ("pnpm", "--version"),
    ("bun", "--version"),
    ("deno", "--version"),
    ("python3", "--version"),
    ("uv", "--version"),
    ("cargo", "--version"),
    ("go", "version"),
    ("elixir", "--version"),
    ("docker", "--version"),
    ("tmux", "-V"),
];

fn run_quietly(argv: &[&str], cwd: &Path) -> String {
    let Some(program) = argv.first() else {
        return String::new();
    };
    let output = Command::new(program)
        .args(&argv[1..])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    match output {
        Ok(output) if output.status.success() => {
            let mut text = String::from_utf8_lossy(&output.stdout).to_string();
            if text.len() > 512 {
                // `String::truncate` panics off a character boundary, and this
                // is the stdout of whatever binary the probe found. The version
                // cut twenty lines down already floors the index; this one did
                // not.
                text.truncate(bounded_index(&text, 512));
            }
            text.trim().to_string()
        }
        _ => String::new(),
    }
}

fn probe_one(name: &str, version_argument: &str, cwd: &Path) -> ToolReport {
    let resolver = if cfg!(windows) { "where" } else { "which" };
    let resolved = run_quietly(&[resolver, name], cwd);
    if resolved.is_empty() {
        return ToolReport {
            name: name.to_string(),
            present: false,
            path: String::new(),
            version: String::new(),
        };
    }
    let mut version = run_quietly(&[name, version_argument], cwd);
    version.truncate(bounded_index(&version, 120));
    ToolReport {
        name: name.to_string(),
        present: true,
        path: resolved.lines().next().unwrap_or("").to_string(),
        version,
    }
}

fn bounded_index(value: &str, limit: usize) -> usize {
    if value.len() <= limit {
        return value.len();
    }
    let mut end = limit;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    end
}

fn probe_catalog(catalog: &[(&str, &str)], cwd: &Path) -> Vec<ToolReport> {
    let mut reports: Vec<Option<ToolReport>> = vec![None; catalog.len()];
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for (index, (name, argument)) in catalog.iter().enumerate() {
            handles.push((index, scope.spawn(move || probe_one(name, argument, cwd))));
        }
        for (index, handle) in handles {
            if let Ok(report) = handle.join() {
                reports[index] = Some(report);
            }
        }
    });
    reports
        .into_iter()
        .enumerate()
        .map(|(index, report)| {
            report.unwrap_or_else(|| ToolReport {
                name: catalog[index].0.to_string(),
                present: false,
                path: String::new(),
                version: String::new(),
            })
        })
        .collect()
}

/// The platform and architecture names the controller already knows.
///
/// The server sees `darwin-arm64` from the TypeScript CLI, and a machine that
/// announced `macos-aarch64` would be a different machine to anything matching
/// on that string. The names are the wire contract, not Rust's spelling of it.
pub fn wire_platform() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    }
}

pub fn wire_architecture() -> &'static str {
    match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "x64",
        "x86" => "ia32",
        other => other,
    }
}

fn hostname() -> String {
    let output = Command::new("hostname").stderr(Stdio::null()).output();
    match output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        _ => String::new(),
    }
}

fn worktree_report(root: &Path) -> WorktreeReport {
    let exists = root.is_dir();
    WorktreeReport {
        root: root.display().to_string(),
        exists,
        git: root.join(".git").exists(),
    }
}

/// Inspect this machine with fixed read-only probes. It needs no account and no
/// pairing.
pub fn probe(roots: &[PathBuf]) -> ProbeReport {
    let cwd = roots
        .first()
        .cloned()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let cwd = if cwd.is_dir() {
        cwd
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    };
    let system = sysinfo::System::new_all();
    ProbeReport {
        schema: "openagents.computer_probe.v1".to_string(),
        host: HostReport {
            platform: wire_platform().to_string(),
            release: run_quietly(&["uname", "-r"], &cwd),
            architecture: wire_architecture().to_string(),
            hostname: hostname(),
            shell: std::env::var("SHELL").unwrap_or_default(),
            cpu_count: system.cpus().len(),
            total_memory_bytes: system.total_memory(),
            uptime_seconds: sysinfo::System::uptime(),
        },
        coding_agents: probe_catalog(&CODING_AGENT_CATALOG, &cwd),
        toolchains: probe_catalog(&TOOLCHAIN_CATALOG, &cwd),
        roots: roots
            .iter()
            .map(|root| root.display().to_string())
            .collect(),
        worktrees: roots.iter().map(|root| worktree_report(root)).collect(),
        // Filled in by `probe_for`, which is the only caller that knows what
        // the owner declared. A probe with no policy in hand reports no
        // delegable agents rather than guessing at the catalog.
        acp_agents: Vec::new(),
    }
}

/// The same probe, carrying the delegation catalog this machine will honour.
///
/// The server refuses an `agent_id` that is not in this list, so what it says
/// is what can be delegated — and it is built from the same two sources the
/// controller resolves against, not from a hardcoded roster.
pub fn probe_for(config: &PolicyConfig) -> ProbeReport {
    let mut report = probe(&config.roots);
    let versions: BTreeMap<&str, &str> = report
        .coding_agents
        .iter()
        .map(|tool| (tool.name.as_str(), tool.version.as_str()))
        .collect();
    report.acp_agents = agent_catalog(config, &report.coding_agents)
        .into_iter()
        .map(|entry| AcpAgentReport {
            version: versions
                .get(entry.id.as_str())
                .copied()
                .unwrap_or_default()
                .to_string(),
            source: entry.source.to_string(),
            id: entry.id,
        })
        .collect();
    report
}

/// The bare host facts, kept for callers that only want the machine shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputerProbeResult {
    pub os: String,
    pub arch: String,
    pub num_cpus: usize,
    pub total_memory_mb: u64,
}

pub fn probe_host() -> ComputerProbeResult {
    let system = sysinfo::System::new_all();
    ComputerProbeResult {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        num_cpus: system.cpus().len(),
        total_memory_mb: system.total_memory() / 1024 / 1024,
    }
}

// ---------------------------------------------------------------------------
// the machine credential
// ---------------------------------------------------------------------------

/// The OS credential store files a machine token under its own service name, so
/// a Computer pairing and an account login never overwrite each other.
const COMPUTER_KEYCHAIN_SERVICE: &str = "openagents-cli-computer";

fn machine_token_file_key(origin: &str) -> String {
    format!("computer:{origin}")
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CredentialFile {
    #[serde(default = "one")]
    version: u8,
    #[serde(default)]
    tokens: BTreeMap<String, String>,
}

fn one() -> u8 {
    1
}

/// Where this machine's token lives. Keyed by origin, exactly like the account
/// credential store, so a machine paired with staging is not offered to
/// production.
pub struct MachineCredentials {
    origin: String,
    path: PathBuf,
    use_os_store: bool,
}

impl MachineCredentials {
    pub fn for_origin(origin: &str) -> Self {
        Self {
            origin: origin.to_string(),
            path: crate::auth::credentials_path(),
            use_os_store: true,
        }
    }

    /// A store confined to a directory with the OS keychain switched off, so a
    /// test exercises the real read, write, and delete without touching the
    /// developer's own credentials.
    pub fn isolated(origin: &str, directory: &Path) -> Self {
        Self {
            origin: origin.to_string(),
            path: directory.join("cli-credentials.json"),
            use_os_store: false,
        }
    }

    fn load(&self) -> Result<CredentialFile, String> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => serde_json::from_str(&text)
                .map_err(|error| format!("could not decode {}: {error}", self.path.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(CredentialFile::default())
            }
            Err(error) => Err(format!("could not read {}: {error}", self.path.display())),
        }
    }

    fn save(&self, file: &CredentialFile) -> Result<(), String> {
        if file.tokens.is_empty() {
            if self.path.exists() {
                std::fs::remove_file(&self.path).map_err(|error| {
                    format!("could not remove {}: {error}", self.path.display())
                })?;
            }
            return Ok(());
        }
        let encoded = serde_json::to_string(file)
            .map_err(|error| format!("could not encode credentials: {error}"))?;
        write_private_file(&self.path, &encoded)
    }

    fn os_get(&self) -> Option<String> {
        if !self.use_os_store || !cfg!(target_os = "macos") && !cfg!(target_os = "linux") {
            return None;
        }
        let output = if cfg!(target_os = "macos") {
            Command::new("security")
                .args([
                    "find-generic-password",
                    "-a",
                    &self.origin,
                    "-s",
                    COMPUTER_KEYCHAIN_SERVICE,
                    "-w",
                ])
                .stderr(Stdio::null())
                .output()
        } else {
            Command::new("secret-tool")
                .args([
                    "lookup",
                    "service",
                    COMPUTER_KEYCHAIN_SERVICE,
                    "origin",
                    &self.origin,
                ])
                .stderr(Stdio::null())
                .output()
        };
        let output = output.ok()?;
        if !output.status.success() {
            return None;
        }
        let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
        // A machine token is an `smct_`. A record that is not one is not a
        // credential this command can use, and it is not reported as one.
        if value.starts_with("smct_") {
            Some(value)
        } else {
            None
        }
    }

    fn os_set(&self, token: &str) -> bool {
        if !self.use_os_store {
            return false;
        }
        if cfg!(target_os = "macos") {
            let status = Command::new("security")
                .args([
                    "add-generic-password",
                    "-U",
                    "-a",
                    &self.origin,
                    "-s",
                    COMPUTER_KEYCHAIN_SERVICE,
                    "-w",
                    token,
                ])
                .stderr(Stdio::null())
                .stdout(Stdio::null())
                .status();
            return matches!(status, Ok(status) if status.success());
        }
        if cfg!(target_os = "linux") {
            let child = Command::new("secret-tool")
                .args([
                    "store",
                    "--label",
                    COMPUTER_KEYCHAIN_SERVICE,
                    "service",
                    COMPUTER_KEYCHAIN_SERVICE,
                    "origin",
                    &self.origin,
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
            if let Ok(mut child) = child {
                if let Some(mut pipe) = child.stdin.take() {
                    let _ = pipe.write_all(token.as_bytes());
                }
                return matches!(child.wait(), Ok(status) if status.success());
            }
        }
        false
    }

    fn os_remove(&self) {
        if !self.use_os_store {
            return;
        }
        if cfg!(target_os = "macos") {
            let _ = Command::new("security")
                .args([
                    "delete-generic-password",
                    "-a",
                    &self.origin,
                    "-s",
                    COMPUTER_KEYCHAIN_SERVICE,
                ])
                .stderr(Stdio::null())
                .stdout(Stdio::null())
                .status();
        } else if cfg!(target_os = "linux") {
            let _ = Command::new("secret-tool")
                .args([
                    "clear",
                    "service",
                    COMPUTER_KEYCHAIN_SERVICE,
                    "origin",
                    &self.origin,
                ])
                .stderr(Stdio::null())
                .status();
        }
    }

    pub fn get(&self) -> Result<Option<crate::auth::Secret>, String> {
        if let Some(token) = self.os_get() {
            return Ok(Some(crate::auth::Secret::new(token)));
        }
        let file = self.load()?;
        Ok(file
            .tokens
            .get(&machine_token_file_key(&self.origin))
            .filter(|value| !value.trim().is_empty())
            .map(|value| crate::auth::Secret::new(value.trim())))
    }

    pub fn set(&self, token: &crate::auth::Secret) -> Result<(), String> {
        if self.os_set(token.expose()) {
            return Ok(());
        }
        let mut file = self.load()?;
        file.version = 1;
        file.tokens.insert(
            machine_token_file_key(&self.origin),
            token.expose().to_string(),
        );
        self.save(&file)
    }

    pub fn remove(&self) -> Result<bool, String> {
        let had = self.get()?.is_some();
        self.os_remove();
        let mut file = self.load()?;
        if file
            .tokens
            .remove(&machine_token_file_key(&self.origin))
            .is_some()
        {
            self.save(&file)?;
        }
        Ok(had)
    }
}

// ---------------------------------------------------------------------------
// pairing and status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingStart {
    pub pairing_id: String,
    pub code: String,
    pub poll_secret: String,
    pub verify_url: String,
    pub expires_at: String,
    pub interval_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingClaim {
    pub status: String,
    pub machine_id: String,
    pub name: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineStatus {
    pub machine_id: String,
    pub name: String,
    pub status: String,
    pub token_expires_at: String,
}

pub struct ComputerClient {
    pub origin: String,
    http: reqwest::Client,
}

fn error_code(value: &serde_json::Value) -> Option<String> {
    value
        .get("error")
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

impl ComputerClient {
    pub fn new(origin: &str) -> Self {
        Self {
            origin: origin.trim_end_matches('/').to_string(),
            http: reqwest::Client::new(),
        }
    }

    pub async fn start(
        &self,
        name: &str,
        tier: Tier,
        agent_version: &str,
        roots: &[PathBuf],
    ) -> Result<PairingStart, String> {
        let url = format!("{}/controller/pairings", self.origin);
        let body = serde_json::json!({
            "name": name,
            "tier": tier.label(),
            "platform": format!("{}-{}", wire_platform(), wire_architecture()),
            "agent_version": agent_version,
            "roots": roots.iter().map(|r| r.display().to_string()).collect::<Vec<_>>(),
        });
        let response = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|error| {
                format!("the Computer pairing request could not reach {url}: {error}")
            })?;
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        let parsed: serde_json::Value =
            serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
        if status == 404 && error_code(&parsed).as_deref() == Some("computer_controller_disabled") {
            return Err(format!(
                "the OpenAgents Computer surface is not enabled on {}",
                self.origin
            ));
        }
        if status == 422 {
            return Err(format!("{} refused this Computer pairing", self.origin));
        }
        if status != 200 && status != 201 {
            return Err(format!(
                "{} could not register this Computer pairing (HTTP {status})",
                self.origin
            ));
        }
        serde_json::from_value(parsed).map_err(|error| {
            format!("the Computer pairing response did not match the API contract: {error}")
        })
    }

    /// One poll. `Ok(None)` means the owner has not approved yet.
    pub async fn poll(&self, pairing: &PairingStart) -> Result<Option<PairingClaim>, String> {
        let url = format!(
            "{}/controller/pairings/{}",
            self.origin,
            urlencode(&pairing.pairing_id)
        );
        let response = self
            .http
            .get(&url)
            .header("x-pairing-secret", &pairing.poll_secret)
            .send()
            .await
            .map_err(|error| format!("the Computer pairing poll could not reach {url}: {error}"))?;
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        if status == 410 {
            return Err("the Computer pairing expired before the owner approved it".to_string());
        }
        if status == 404 || status == 401 || status == 403 {
            return Err("the Computer pairing was refused or is no longer available".to_string());
        }
        if status != 200 {
            return Err(format!(
                "{} could not poll this Computer pairing (HTTP {status})",
                self.origin
            ));
        }
        let parsed: serde_json::Value = serde_json::from_str(&text)
            .map_err(|_| "the Computer pairing poll did not answer with JSON".to_string())?;
        if parsed.get("status").and_then(|v| v.as_str()) == Some("pending") {
            return Ok(None);
        }
        serde_json::from_value(parsed).map(Some).map_err(|error| {
            format!("the Computer claim response did not match the API contract: {error}")
        })
    }

    /// Poll until the owner approves or the pairing expires.
    pub async fn wait(&self, pairing: &PairingStart) -> Result<PairingClaim, String> {
        let interval = Duration::from_secs(pairing.interval_seconds.max(1));
        let deadline = Instant::now() + pairing_window(&pairing.expires_at)?;
        loop {
            if let Some(claim) = self.poll(pairing).await? {
                return Ok(claim);
            }
            if Instant::now() + interval > deadline {
                return Err("the Computer pairing expired before the owner approved it".to_string());
            }
            tokio::time::sleep(interval).await;
        }
    }

    /// `Ok(None)` means the server answered 401: this machine token is no longer
    /// accepted. A transport failure or any other status is an error, never a
    /// quiet "unpaired".
    pub async fn status(
        &self,
        token: &crate::auth::Secret,
    ) -> Result<Option<MachineStatus>, String> {
        let url = format!("{}/controller/status", self.origin);
        let response = self
            .http
            .get(&url)
            .header("authorization", format!("Bearer {}", token.expose()))
            .send()
            .await
            .map_err(|error| {
                format!("the Computer status request could not reach {url}: {error}")
            })?;
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        if status == 401 {
            return Ok(None);
        }
        if status != 200 {
            return Err(format!(
                "{} could not read this Computer status (HTTP {status})",
                self.origin
            ));
        }
        serde_json::from_str(&text).map(Some).map_err(|error| {
            format!("the Computer status response did not match the API contract: {error}")
        })
    }
}

fn urlencode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

/// How long the pairing has left, from the RFC 3339 expiry the server sent.
fn pairing_window(expires_at: &str) -> Result<Duration, String> {
    let seconds = parse_rfc3339_seconds(expires_at)
        .ok_or_else(|| "the Computer pairing expiry did not match the API contract".to_string())?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or(0);
    Ok(Duration::from_secs((seconds - now).max(0) as u64))
}

/// Enough of RFC 3339 to read the expiry the controller sends, which is always
/// UTC with a `Z`.
pub fn parse_rfc3339_seconds(value: &str) -> Option<i64> {
    let bytes = value.as_bytes();
    if bytes.len() < 20 || bytes[4] != b'-' || bytes[7] != b'-' || bytes[10] != b'T' {
        return None;
    }
    let year: i64 = value.get(0..4)?.parse().ok()?;
    let month: i64 = value.get(5..7)?.parse().ok()?;
    let day: i64 = value.get(8..10)?.parse().ok()?;
    let hour: i64 = value.get(11..13)?.parse().ok()?;
    let minute: i64 = value.get(14..16)?.parse().ok()?;
    let second: i64 = value.get(17..19)?.parse().ok()?;
    // Days-from-civil, Howard Hinnant's algorithm.
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + hour * 3_600 + minute * 60 + second)
}

// ---------------------------------------------------------------------------
// the outbound channel
// ---------------------------------------------------------------------------

const HEARTBEAT: Duration = Duration::from_secs(30);
const RECONNECT_BACKOFF: Duration = Duration::from_millis(250);
const MAXIMUM_RECONNECT_ATTEMPTS: u32 = 3;
const MAXIMUM_BACKOFF: Duration = Duration::from_secs(10);
const MAXIMUM_CONCURRENCY: usize = 2;
const MAXIMUM_ARGV_LENGTH: usize = 64;
const MAXIMUM_ARGUMENT_LENGTH: usize = 1_024;

fn socket_url(origin: &str, token: &str) -> String {
    let base = origin.trim_end_matches('/');
    let scheme_swapped = if let Some(rest) = base.strip_prefix("https") {
        format!("wss{rest}")
    } else if let Some(rest) = base.strip_prefix("http") {
        format!("ws{rest}")
    } else {
        base.to_string()
    };
    format!(
        "{scheme_swapped}/controller/socket/websocket?vsn=2.0.0&token={}",
        urlencode(token)
    )
}

/// A frame queued by a worker thread for the socket loop to write. The socket is
/// read and written from one thread only; everything else talks to it through
/// this queue.
enum Outgoing {
    Frame {
        event: String,
        payload: serde_json::Value,
    },
}

fn phoenix_frame(
    join_ref: Option<&str>,
    reference: &str,
    topic: &str,
    event: &str,
    payload: &serde_json::Value,
) -> String {
    serde_json::json!([join_ref, reference, topic, event, payload]).to_string()
}

/// Why a connection ended. `retryable` decides whether `serve` reconnects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionEnd {
    pub reason: String,
    pub retryable: bool,
}

pub fn reconnectable_transport_reason(reason: &str) -> bool {
    reason == "closed"
        || reason == "phx_close"
        || reason == "phx_error"
        || reason == "heartbeat_timeout"
        || reason == "socket_not_open"
        || reason.starts_with("error:")
}

/// Serve one connection until it ends, and say why it ended.
///
/// Everything the server asks for goes through [`decide`] first and lands in the
/// journal either way. A frame this build has no handler for is refused as
/// `unsupported` rather than silently ignored.
#[allow(clippy::too_many_arguments)]
fn serve_connection(
    origin: &str,
    token: &crate::auth::Secret,
    machine_id: &str,
    hello: &serde_json::Value,
    config: &PolicyConfig,
    journal: &Journal,
    catalog: &[ResolvedAgent],
    mut on_event: impl FnMut(&str),
) -> ConnectionEnd {
    use tungstenite::{client::IntoClientRequest, Message};

    let _ = rustls::crypto::ring::default_provider().install_default();

    let url = socket_url(origin, token.expose());
    let request = match url.as_str().into_client_request() {
        Ok(request) => request,
        Err(error) => {
            return ConnectionEnd {
                reason: format!("error:{error}"),
                retryable: false,
            }
        }
    };
    let (mut socket, _response) = match connect_bounded(request) {
        Ok(pair) => pair,
        Err(reason) => {
            // A connection that never opened is still a transport event this
            // machine's owner should be able to read back.
            let request = CommandRequest {
                argv: vec!["<connection>".to_string()],
                cwd: String::new(),
            };
            let _ = journal.append("connection", &request, "transport", "closed", &reason);
            let retryable = reconnectable_transport_reason(&reason);
            return ConnectionEnd { reason, retryable };
        }
    };

    let topic = format!("computer:{machine_id}");
    let join_ref = "1";
    let mut reference: u64 = 1;
    let mut heartbeat_at = Instant::now() + HEARTBEAT;
    let mut heartbeat_pending = false;
    let mut heartbeat_ref = String::new();
    let mut joined = false;
    let mut hello_ref = String::new();

    let (sender, receiver): (Sender<Outgoing>, Receiver<Outgoing>) = std::sync::mpsc::channel();
    let active = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let cancellations: Arc<Mutex<BTreeMap<String, Cancellation>>> =
        Arc::new(Mutex::new(BTreeMap::new()));
    let agent_jobs: Arc<Mutex<BTreeMap<String, AgentJob>>> = Arc::new(Mutex::new(BTreeMap::new()));

    let send_join = phoenix_frame(
        Some(join_ref),
        join_ref,
        &topic,
        "phx_join",
        &serde_json::json!({}),
    );
    if socket.send(Message::Text(send_join.into())).is_err() {
        return ConnectionEnd {
            reason: "socket_not_open".to_string(),
            retryable: true,
        };
    }

    let end = loop {
        // Drain anything the workers produced before touching the socket again.
        while let Ok(Outgoing::Frame { event, payload }) = receiver.try_recv() {
            reference += 1;
            let frame = phoenix_frame(
                Some(join_ref),
                &reference.to_string(),
                &topic,
                &event,
                &payload,
            );
            if socket.send(Message::Text(frame.into())).is_err() {
                break;
            }
        }

        if Instant::now() >= heartbeat_at {
            if heartbeat_pending {
                break ConnectionEnd {
                    reason: "heartbeat_timeout".to_string(),
                    retryable: true,
                };
            }
            reference += 1;
            heartbeat_ref = reference.to_string();
            heartbeat_pending = true;
            heartbeat_at = Instant::now() + HEARTBEAT;
            let frame = phoenix_frame(
                None,
                &heartbeat_ref,
                "phoenix",
                "heartbeat",
                &serde_json::json!({}),
            );
            if socket.send(Message::Text(frame.into())).is_err() {
                break ConnectionEnd {
                    reason: "socket_not_open".to_string(),
                    retryable: true,
                };
            }
        }

        let message = match socket.read() {
            Ok(message) => message,
            Err(tungstenite::Error::Io(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                continue;
            }
            Err(tungstenite::Error::ConnectionClosed) | Err(tungstenite::Error::AlreadyClosed) => {
                break ConnectionEnd {
                    reason: "closed".to_string(),
                    retryable: true,
                }
            }
            Err(error) => {
                let reason = format!("error:{error}");
                let retryable = reconnectable_transport_reason(&reason);
                break ConnectionEnd { reason, retryable };
            }
        };

        let text = match message {
            Message::Text(text) => text.to_string(),
            Message::Close(_) => {
                break ConnectionEnd {
                    reason: "closed".to_string(),
                    retryable: true,
                }
            }
            Message::Ping(payload) => {
                let _ = socket.send(Message::Pong(payload));
                continue;
            }
            _ => continue,
        };

        let Ok(frame) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let Some(parts) = frame.as_array() else {
            continue;
        };
        if parts.len() != 5 {
            continue;
        }
        let response_ref = parts[1].as_str().unwrap_or_default().to_string();
        let response_topic = parts[2].as_str().unwrap_or_default().to_string();
        let event = parts[3].as_str().unwrap_or_default().to_string();
        let payload = parts[4].clone();

        if response_topic == "phoenix" && event == "phx_reply" && response_ref == heartbeat_ref {
            heartbeat_pending = false;
            continue;
        }
        if response_topic != topic {
            continue;
        }
        if event == "phx_reply" {
            // The server's answer to `hello` carries whether it accepted this
            // machine's probe report — the inventory it later decides
            // delegation against. A rejected hello used to be invisible here,
            // which made a machine that had announced nothing look identical
            // to one that had announced everything.
            if !hello_ref.is_empty() && response_ref == hello_ref {
                let accepted = payload.get("status").and_then(|v| v.as_str()) == Some("ok");
                let detail = if accepted {
                    "hello accepted".to_string()
                } else {
                    format!(
                        "hello refused: {}",
                        payload
                            .get("response")
                            .and_then(|value| value.get("reason"))
                            .and_then(|value| value.as_str())
                            .unwrap_or("unknown")
                    )
                };
                on_event(if accepted {
                    "hello_ok"
                } else {
                    "hello_refused"
                });
                let announcement = CommandRequest {
                    argv: vec!["<hello>".to_string()],
                    cwd: String::new(),
                };
                let _ = journal.append(
                    "connection",
                    &announcement,
                    "transport",
                    if accepted { "accepted" } else { "refused" },
                    &detail,
                );
                continue;
            }
            if response_ref != join_ref {
                continue;
            }
            if payload.get("status").and_then(|v| v.as_str()) == Some("ok") {
                joined = true;
                on_event("joined");
                reference += 1;
                hello_ref = reference.to_string();
                let frame = phoenix_frame(Some(join_ref), &hello_ref, &topic, "hello", hello);
                let _ = socket.send(Message::Text(frame.into()));
            } else {
                let refusal = payload
                    .get("response")
                    .and_then(|value| value.get("reason"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                break ConnectionEnd {
                    // `machine_reconnecting` is the server telling this machine
                    // that its previous connection has not been reaped yet, so
                    // it is worth waiting for. Every other refusal is a decision.
                    retryable: refusal == "machine_reconnecting",
                    reason: format!("join_refused:{refusal}"),
                };
            }
            continue;
        }
        if event == "phx_close" || event == "phx_error" {
            break ConnectionEnd {
                reason: event,
                retryable: true,
            };
        }
        if !joined {
            continue;
        }

        let Some(request_id) = payload.get("request_id").and_then(|v| v.as_str()) else {
            continue;
        };
        let request_id = request_id.to_string();
        on_event(&format!("{event}:{}", short(&request_id)));

        match event.as_str() {
            "probe" => {
                let request = CommandRequest {
                    argv: vec!["<probe>".to_string()],
                    cwd: config
                        .roots
                        .first()
                        .map(|root| root.display().to_string())
                        .unwrap_or_default(),
                };
                let _ = journal.append(
                    &request_id,
                    &request,
                    "received",
                    "pending",
                    "read-only probe requested",
                );
                let report = probe_for(config);
                let _ = journal.append(
                    &request_id,
                    &request,
                    "allowed",
                    "completed",
                    "probe completed",
                );
                reference += 1;
                let frame = phoenix_frame(
                    Some(join_ref),
                    &reference.to_string(),
                    &topic,
                    "probe_result",
                    &serde_json::json!({
                        "request_id": request_id,
                        "probe": serde_json::to_value(&report).unwrap_or(serde_json::Value::Null),
                    }),
                );
                let _ = socket.send(Message::Text(frame.into()));
            }
            "run" => {
                handle_run(
                    &request_id,
                    &payload,
                    config,
                    journal,
                    &sender,
                    &active,
                    &cancellations,
                );
            }
            // `OpenAgentsWeb.ComputerChannel` pushes a request by the name of
            // its kind — `handle_info({:computer_request, kind, …})` for `kind
            // in [:run, :devin, :agent]` does `push(socket,
            // Atom.to_string(kind), …)` — so every one of those names arrives
            // here as an event carrying a `request_id` the server is tracking.
            // `devin` once fell through to the catch-all below and was dropped
            // without a frame or a journal line, leaving the server blocked on
            // a request this side had already thrown away. Both names are
            // served, and both end in a terminal frame.
            "agent" | "devin" => {
                handle_agent(
                    &event,
                    &request_id,
                    &payload,
                    origin,
                    config,
                    journal,
                    &sender,
                    &active,
                    &agent_jobs,
                    catalog,
                );
            }
            "cancel" => {
                if let Some(cancellation) = cancellations.lock().unwrap().get(&request_id) {
                    cancellation.cancel();
                }
                if let Some(job) = agent_jobs.lock().unwrap().get(&request_id) {
                    let _ = job.cancel.send(true);
                }
                let request = CommandRequest {
                    argv: vec!["<cancel>".to_string()],
                    cwd: String::new(),
                };
                let _ = journal.append(
                    &request_id,
                    &request,
                    "allowed",
                    "cancelling",
                    "process group termination requested",
                );
            }
            _ => {}
        }
    };

    let closing = CommandRequest {
        argv: vec!["<connection>".to_string()],
        cwd: String::new(),
    };
    let _ = journal.append("connection", &closing, "transport", "closed", &end.reason);
    for cancellation in cancellations.lock().unwrap().values() {
        cancellation.cancel();
    }
    // A delegation whose channel is gone has nowhere to report. Stopping the
    // agent is what keeps a lost connection from leaving a coding agent running
    // in the owner's checkout with nothing listening.
    for job in agent_jobs.lock().unwrap().values() {
        let _ = job.cancel.send(true);
    }
    let _ = socket.close(None);
    end
}

fn short(value: &str) -> String {
    value.chars().take(8).collect()
}

type WsStream = tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>;

/// Connect with bounded DNS, connect, and read timeouts. The read timeout is
/// what lets the loop above tick for heartbeats and queued frames instead of
/// blocking forever on a quiet socket.
fn connect_bounded(
    request: tungstenite::http::Request<()>,
) -> Result<(WsStream, tungstenite::handshake::client::Response), String> {
    use std::net::ToSocketAddrs;
    let host = request.uri().host().ok_or("error:no_host")?.to_string();
    let secure = request.uri().scheme_str() == Some("wss");
    let port = request
        .uri()
        .port_u16()
        .unwrap_or(if secure { 443 } else { 80 });
    let addresses = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|_| "error:dns_failed".to_string())?;
    for address in addresses {
        let Ok(stream) = std::net::TcpStream::connect_timeout(&address, Duration::from_secs(10))
        else {
            continue;
        };
        let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
        return match tungstenite::client_tls(request, stream) {
            Ok(pair) => Ok(pair),
            // A handshake the server answered with a status is a decision, not
            // transport loss: `403` means this token is not accepted, and
            // reconnecting would only repeat it.
            Err(tungstenite::HandshakeError::Failure(tungstenite::Error::Http(response))) => {
                Err(format!("join_refused:http_{}", response.status().as_u16()))
            }
            Err(tungstenite::HandshakeError::Failure(error)) => Err(format!("error:{error}")),
            Err(tungstenite::HandshakeError::Interrupted(_)) => {
                Err("error:handshake_interrupted".to_string())
            }
        };
    }
    Err("error:connect_failed".to_string())
}

#[allow(clippy::too_many_arguments)]
fn handle_run(
    request_id: &str,
    payload: &serde_json::Value,
    config: &PolicyConfig,
    journal: &Journal,
    sender: &Sender<Outgoing>,
    active: &Arc<std::sync::atomic::AtomicUsize>,
    cancellations: &Arc<Mutex<BTreeMap<String, Cancellation>>>,
) {
    use std::sync::atomic::Ordering;

    let refuse_frame = |reason: &str, detail: &str| Outgoing::Frame {
        event: "refused".to_string(),
        payload: serde_json::json!({
            "request_id": request_id,
            "reason": reason,
            "detail": detail,
        }),
    };

    let Some(request) = request_fields(payload) else {
        let malformed = CommandRequest {
            argv: vec!["<invalid>".to_string()],
            cwd: String::new(),
        };
        let _ = journal.append(
            request_id,
            &malformed,
            "refused",
            "refused",
            "invalid command request",
        );
        let _ = sender.send(refuse_frame(
            "invalid_request",
            "argv and cwd are required and must be bounded",
        ));
        return;
    };
    let _ = journal.append(
        request_id,
        &request,
        "received",
        "pending",
        "command request received",
    );

    if let Some(requested) = payload.get("tier").and_then(|v| v.as_str()) {
        if let Some(requested) = Tier::parse(requested) {
            if !tier_allows(config.tier, requested) {
                let detail = "the requested tier exceeds the local ceiling";
                let _ =
                    journal.append(request_id, &request, "tier_insufficient", "refused", detail);
                let _ = sender.send(refuse_frame("tier_insufficient", detail));
                return;
            }
        }
    }

    match decide(&request, config) {
        Decision::Refused { reason, detail } => {
            let _ = journal.append(request_id, &request, reason.label(), "refused", &detail);
            let _ = sender.send(refuse_frame(reason.label(), &detail));
            return;
        }
        Decision::Allowed {
            needs_confirmation: true,
        } => {
            let detail = "local confirmation is required for this command";
            let _ = journal.append(
                request_id,
                &request,
                "confirmation_required",
                "refused",
                "local confirmation is required",
            );
            let _ = sender.send(refuse_frame("confirmation_required", detail));
            return;
        }
        Decision::Allowed { .. } => {}
    }

    if active.load(Ordering::SeqCst) >= MAXIMUM_CONCURRENCY {
        let _ = journal.append(
            request_id,
            &request,
            "allowed",
            "refused",
            "local execution concurrency limit reached",
        );
        let _ = sender.send(refuse_frame("busy", "the local execution limit is reached"));
        return;
    }

    let defaults = ExecutionLimits::default();
    let limits = ExecutionLimits {
        timeout: Duration::from_millis(bounded_number(
            payload,
            &["timeout_ms", "timeout"],
            defaults.timeout.as_millis() as u64,
            defaults.timeout.as_millis() as u64,
        )),
        maximum_output_bytes: bounded_number(
            payload,
            &[
                "maximum_output_bytes",
                "output_max_bytes",
                "max_output_bytes",
            ],
            defaults.maximum_output_bytes as u64,
            defaults.maximum_output_bytes as u64,
        ) as usize,
    };
    let _ = journal.append(
        request_id,
        &request,
        "allowed",
        "running",
        &format!("timeout={}", limits.timeout.as_millis()),
    );

    active.fetch_add(1, Ordering::SeqCst);
    let cancellation = Cancellation::default();
    cancellations
        .lock()
        .unwrap()
        .insert(request_id.to_string(), cancellation.clone());

    let sender = sender.clone();
    let journal = journal.clone();
    let active = Arc::clone(active);
    let cancellations = Arc::clone(cancellations);
    let request_id = request_id.to_string();
    std::thread::spawn(move || {
        let chunk_sender = sender.clone();
        let chunk_id = request_id.clone();
        let outcome = execute_command(&request.argv, &request.cwd, limits, &cancellation, |text| {
            let _ = chunk_sender.send(Outgoing::Frame {
                event: "chunk".to_string(),
                payload: serde_json::json!({ "request_id": chunk_id, "text": text }),
            });
        });
        active.fetch_sub(1, Ordering::SeqCst);
        cancellations.lock().unwrap().remove(&request_id);
        let terminal = if outcome.cancelled {
            "cancelled"
        } else if outcome.timed_out {
            "timeout"
        } else if outcome.exit_code == Some(0) {
            "completed"
        } else {
            "failed"
        };
        let detail = if outcome.truncated {
            "output truncated"
        } else {
            ""
        };
        let _ = journal.append(&request_id, &request, "allowed", terminal, detail);
        let _ = sender.send(Outgoing::Frame {
            event: "exit".to_string(),
            payload: serde_json::json!({
                "request_id": request_id,
                "status": terminal,
                "exit_code": outcome.exit_code,
                "timed_out": outcome.timed_out,
                "cancelled": outcome.cancelled,
                "truncated": outcome.truncated,
                "duration_ms": outcome.duration_ms,
            }),
        });
    });
}

// ---------------------------------------------------------------------------
// serving one delegation
// ---------------------------------------------------------------------------

/// One live ACP delegation.
///
/// `route` is the request the channel currently answers on, which a reattach
/// moves: the delegation outlives the request that started it, and a caller
/// that comes back after a reconnect gets the same session's output on its own
/// `request_id` rather than a second agent.
struct AgentJob {
    session: Arc<Mutex<String>>,
    route: Arc<Mutex<String>>,
    cancel: tokio::sync::watch::Sender<bool>,
}

/// What the delegated agent may be told about this machine.
///
/// The scrubbed set every command gets, plus exactly the variable names the
/// owner declared for this agent. This process's own credentials are not in
/// either list, and neither is the machine token.
fn agent_environment(entry: &ResolvedAgent) -> Vec<(String, String)> {
    let mut environment = scrubbed_environment();
    for name in &entry.env {
        if ENVIRONMENT_NAMES.contains(&name.as_str()) {
            continue;
        }
        if let Ok(value) = std::env::var(name) {
            environment.push((name.clone(), value));
        }
    }
    environment
}

/// The coding agents this host actually has, for the delegation catalog.
pub fn installed_coding_agents(roots: &[PathBuf]) -> Vec<ToolReport> {
    let cwd = roots
        .first()
        .filter(|root| root.is_dir())
        .cloned()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    probe_catalog(&CODING_AGENT_CATALOG, &cwd)
}

/// Serve one `agent` (or legacy `devin`) request.
///
/// Every path through this function ends in exactly one terminal frame —
/// `refused` or `exit` — carrying the `request_id` the server is waiting on,
/// and every refusal is journaled with the reason that produced it. A request
/// that reached here and got neither is the defect this shape exists to
/// prevent.
#[allow(clippy::too_many_arguments)]
fn handle_agent(
    event: &str,
    request_id: &str,
    payload: &serde_json::Value,
    origin: &str,
    config: &PolicyConfig,
    journal: &Journal,
    sender: &Sender<Outgoing>,
    active: &Arc<std::sync::atomic::AtomicUsize>,
    agents: &Arc<Mutex<BTreeMap<String, AgentJob>>>,
    catalog: &[ResolvedAgent],
) {
    use std::sync::atomic::Ordering;

    // `OpenAgents.Computer.request_devin/3` now sets `agent_id` itself, but the
    // legacy shape reached this machine as the `devin` event with a
    // `session_id` and no agent named. Reading both is what keeps an older
    // caller from being answered `invalid_request` for asking the old way.
    let agent_id = payload
        .get("agent_id")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| {
            if event == "devin" {
                "devin".to_string()
            } else {
                String::new()
            }
        });
    let prompt = payload
        .get("prompt")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let requested_cwd = payload
        .get("cwd")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let resume = payload
        .get("resume_session_id")
        .or_else(|| payload.get("session_id"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .map(|value| {
            value
                .chars()
                .take(MAXIMUM_SESSION_ID_LENGTH)
                .collect::<String>()
        });

    let request = CommandRequest {
        argv: vec![
            format!("<{event}>"),
            agent_id.chars().take(64).collect::<String>(),
        ],
        cwd: requested_cwd.clone(),
    };
    let _ = journal.append(
        request_id,
        &request,
        "received",
        "pending",
        "ACP delegation received",
    );

    let refuse_now = |reason: &str, detail: &str| {
        let _ = journal.append(request_id, &request, reason, "refused", detail);
        let _ = sender.send(Outgoing::Frame {
            event: "refused".to_string(),
            payload: serde_json::json!({
                "request_id": request_id,
                "reason": reason,
                "detail": detail,
            }),
        });
    };

    // The credential is read before anything can refuse, so its delivery is
    // recorded even on a request that never runs. The token itself is never
    // written anywhere: only whether one arrived whole.
    let delivered = forge_credentials(payload);
    let credentials = match (&delivered, config.scoped_forge_credentials) {
        (Some(found), true) => {
            let _ = journal.append(
                request_id,
                &request,
                "credentials_delivered",
                "configured",
                &format!(
                    "scoped forge credentials for {} on {}",
                    found.repository, found.branch
                ),
            );
            delivered.clone()
        }
        (Some(_), false) => {
            // The server only sends one when the owner ticked the box on the
            // Computers page. This machine has not been told the same thing,
            // and the machine is what decides what runs here.
            let _ = journal.append(
                request_id,
                &request,
                "credentials_refused",
                "refused",
                "scoped forge credentials are not enabled in the local Computer configuration",
            );
            None
        }
        (None, _) => {
            if payload.get("assignment_credential").is_some()
                || payload.get("forge_credentials").is_some()
            {
                let _ = journal.append(
                    request_id,
                    &request,
                    "credentials_delivered",
                    "incomplete",
                    "a forge credential arrived without the repository and branch it is scoped to",
                );
            }
            None
        }
    };

    if agent_id.is_empty() || prompt.trim().is_empty() || prompt.len() > MAXIMUM_PROMPT_LENGTH {
        refuse_now(
            "invalid_request",
            "agent_id, prompt, and cwd are required and must be bounded",
        );
        return;
    }
    if agent_id.len() > 64
        || has_shell_metacharacter(&agent_id)
        || agent_id.contains('/')
        || agent_id.contains('\\')
    {
        refuse_now(
            "invalid_request",
            "the agent id is not a name this machine can resolve",
        );
        return;
    }
    if requested_cwd.is_empty() || requested_cwd.len() > 4_096 {
        refuse_now(
            "invalid_request",
            "agent_id, prompt, and cwd are required and must be bounded",
        );
        return;
    }

    // The tier the server asked for cannot exceed the local ceiling, and the
    // ceiling itself has to reach past `probe` before anything is delegated at
    // all: a probe-tier machine answers fixed discovery and nothing else.
    if let Some(requested) = payload.get("tier").and_then(|value| value.as_str()) {
        if let Some(requested) = Tier::parse(requested) {
            if !tier_allows(config.tier, requested) {
                refuse_now(
                    "tier_insufficient",
                    "the requested tier exceeds the local ceiling",
                );
                return;
            }
        }
    }
    if !tier_allows(config.tier, Tier::Curated) {
        refuse_now(
            "tier_insufficient",
            "probe tier permits fixed discovery only",
        );
        return;
    }

    let cwd = if requested_cwd.starts_with('~') {
        resolve_root(&requested_cwd)
    } else {
        normalize_path(Path::new(&requested_cwd))
    };
    if config.roots.is_empty() || !config.roots.iter().any(|root| within_root(&cwd, root)) {
        refuse_now(
            "root_not_declared",
            "the working directory is outside every declared root",
        );
        return;
    }
    if !cwd.is_dir() {
        refuse_now("root_not_declared", "the working directory does not exist");
        return;
    }

    let entry = match resolve_agent(catalog, &agent_id) {
        Ok(entry) => entry,
        Err(detail) => {
            refuse_now("agent_unavailable", &detail);
            return;
        }
    };

    // A resume that names a session still running here rebinds the live
    // delegation onto this request rather than starting a second agent in the
    // same checkout. The old request id is dropped from the map first: its
    // caller is gone, and a cancel arriving late on a dead request must not
    // stop the delegation that replaced it.
    if let Some(resume) = &resume {
        let mut live = agents.lock().unwrap();
        let existing = live
            .iter()
            .find(|(_, job)| job.session.lock().unwrap().as_str() == resume.as_str())
            .map(|(key, _)| key.clone());
        if let Some(previous) = existing {
            let job = live.remove(&previous).expect("the job was just found");
            *job.route.lock().unwrap() = request_id.to_string();
            let session = job.session.lock().unwrap().clone();
            live.insert(request_id.to_string(), job);
            drop(live);
            let _ = journal.append(
                request_id,
                &request,
                "reattached",
                "running",
                "reattached to the live ACP session",
            );
            let _ = sender.send(Outgoing::Frame {
                event: "session".to_string(),
                payload: serde_json::json!({
                    "request_id": request_id,
                    "session_id": session,
                }),
            });
            return;
        }
    }

    if active.load(Ordering::SeqCst) >= MAXIMUM_CONCURRENCY {
        let _ = journal.append(
            request_id,
            &request,
            "allowed",
            "refused",
            "local delegation concurrency limit reached",
        );
        let _ = sender.send(Outgoing::Frame {
            event: "refused".to_string(),
            payload: serde_json::json!({
                "request_id": request_id,
                "reason": "busy",
                "detail": "the local delegation limit is reached",
            }),
        });
        return;
    }

    let timeout = Duration::from_millis(bounded_number(
        payload,
        &["timeout_ms", "timeout"],
        AGENT_DEFAULT_TIMEOUT_MS,
        AGENT_MAXIMUM_TIMEOUT_MS,
    ));
    let output_ceiling = bounded_number(
        payload,
        &[
            "maximum_output_bytes",
            "max_output_bytes",
            "output_max_bytes",
        ],
        AGENT_MAXIMUM_OUTPUT_BYTES as u64,
        AGENT_MAXIMUM_OUTPUT_BYTES as u64,
    ) as usize;

    let (cancel_sender, cancel_receiver) = tokio::sync::watch::channel(false);
    let route = Arc::new(Mutex::new(request_id.to_string()));
    let session = Arc::new(Mutex::new(resume.clone().unwrap_or_default()));
    agents.lock().unwrap().insert(
        request_id.to_string(),
        AgentJob {
            session: Arc::clone(&session),
            route: Arc::clone(&route),
            cancel: cancel_sender.clone(),
        },
    );
    active.fetch_add(1, Ordering::SeqCst);
    let _ = journal.append(
        request_id,
        &request,
        "allowed",
        "running",
        &format!("agent={} source={}", entry.id, entry.source),
    );

    let gate_journal = journal.clone();
    let gate_config = config.clone();
    let gate_request = request.clone();
    let gate_id = request_id.to_string();
    let gate_cwd = cwd.clone();
    let permission = Arc::new(move |query: &PermissionQuery| {
        let decision = agent_permission(&gate_config, &gate_cwd, query);
        let (label, outcome, detail) = match &decision {
            Decision::Allowed { .. } => (
                "permission_granted",
                "running",
                format!("{}: {}", query.kind, query.title),
            ),
            Decision::Refused { reason, detail } => {
                (reason.label(), "permission_refused", detail.clone())
            }
        };
        // Both answers are journaled. A delegated agent that was stopped from
        // doing something is exactly what the owner needs to be able to read
        // back, and a granted one is how they see what it did.
        let _ = gate_journal.append(&gate_id, &gate_request, label, outcome, &detail);
        decision.allowed()
    });

    let push_journal = journal.clone();
    let push_request = request.clone();
    let push_id = request_id.to_string();
    let push_cwd = cwd.clone();
    let push_origin = origin.to_string();
    let on_request: Option<crate::acp::ReverseHandler> = match credentials {
        None => None,
        Some(credentials) => Some(Arc::new(move |method: &str, params: &serde_json::Value| {
            if method != "git/push" {
                return None;
            }
            let remote = first_string(params, &["remote"]).unwrap_or_else(|| "origin".to_string());
            let Some(refspec) = first_string(params, &["refspec", "branch", "ref"]) else {
                let _ = push_journal.append(
                    &push_id,
                    &push_request,
                    "push_refused",
                    "refused",
                    "a delegated push named no refspec",
                );
                return Some(serde_json::json!({
                    "ok": false,
                    "error": "a delegated push requires a refspec",
                }));
            };
            match push_delegated(&push_cwd, &remote, &refspec, &credentials, &push_origin) {
                Ok(()) => {
                    let _ = push_journal.append(
                        &push_id,
                        &push_request,
                        "push_completed",
                        "completed",
                        &format!("{} to {}", credentials.repository, credentials.branch),
                    );
                    Some(serde_json::json!({"ok": true}))
                }
                Err(detail) => {
                    let detail = redact(&detail);
                    let _ = push_journal.append(
                        &push_id,
                        &push_request,
                        "push_refused",
                        "refused",
                        &detail,
                    );
                    Some(serde_json::json!({"ok": false, "error": detail}))
                }
            }
        })),
    };

    let harness = AcpHarness {
        command: entry.argv[0].clone(),
        args: entry.argv[1..].to_vec(),
        // Ask, so the gate below gets to answer. An agent left in its own
        // default mode may be in a bypass mode that never sends
        // `session/request_permission` at all, and a gate nothing consults
        // decides nothing. Best effort — a build that does not know this mode
        // keeps its own, and what still holds either way is where the agent
        // runs: the cwd was checked against the declared roots before the
        // child was started.
        mode: Some(crate::acp::PermissionMode::Prompt),
        permission: Some(permission),
        on_request,
        resume_session_id: resume.clone(),
        env: Some(agent_environment(&entry)),
    };

    let sender = sender.clone();
    let journal = journal.clone();
    let active = Arc::clone(active);
    let agents = Arc::clone(agents);
    let owning_id = request_id.to_string();
    std::thread::spawn(move || {
        let started = Instant::now();
        let runtime = match tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                // Even this ends in a terminal frame. A server waiting on a
                // request the controller silently dropped is the failure this
                // whole path is shaped to avoid.
                active.fetch_sub(1, Ordering::SeqCst);
                agents.lock().unwrap().remove(&owning_id);
                let detail = format!("the delegation runtime could not start: {error}");
                let _ = journal.append(&owning_id, &request, "failed", "failed", &detail);
                let _ = sender.send(Outgoing::Frame {
                    event: "exit".to_string(),
                    payload: serde_json::json!({
                        "request_id": *route.lock().unwrap(),
                        "status": "failed",
                        "session_id": "",
                        "detail": detail,
                        "truncated": false,
                        "duration_ms": 0,
                    }),
                });
                return;
            }
        };

        let streamed = Arc::new(Mutex::new(0usize));
        let truncated = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let outcome = runtime.block_on(async {
            let mut cancel = cancel_receiver;
            let chunk_sender = sender.clone();
            let chunk_route = Arc::clone(&route);
            let chunk_session = Arc::clone(&session);
            let chunk_bytes = Arc::clone(&streamed);
            let chunk_truncated = Arc::clone(&truncated);
            let run = harness.run_detailed(
                &prompt,
                &cwd,
                move |event| {
                    let text = match event {
                        AcpEvent::Session { id } => {
                            *chunk_session.lock().unwrap() = id.clone();
                            let _ = chunk_sender.send(Outgoing::Frame {
                                event: "session".to_string(),
                                payload: serde_json::json!({
                                    "request_id": *chunk_route.lock().unwrap(),
                                    "session_id": id,
                                }),
                            });
                            return;
                        }
                        AcpEvent::Text { chunk } => chunk,
                        AcpEvent::Tool { kind, title } => format!("[{kind}] {title}\n"),
                        AcpEvent::Tokens { input, output } => {
                            format!("[{input} in / {output} out tokens]\n")
                        }
                    };
                    // Redacted before it leaves this machine, and bounded, so
                    // a talkative agent cannot become an unbounded upload.
                    let text = redact(&text);
                    let mut sent = chunk_bytes.lock().unwrap();
                    if *sent >= output_ceiling {
                        chunk_truncated.store(true, Ordering::SeqCst);
                        return;
                    }
                    let remaining = output_ceiling - *sent;
                    let mut end = remaining.min(text.len());
                    while end > 0 && !text.is_char_boundary(end) {
                        end -= 1;
                    }
                    if end < text.len() {
                        chunk_truncated.store(true, Ordering::SeqCst);
                    }
                    *sent += end;
                    drop(sent);
                    if end == 0 {
                        return;
                    }
                    let _ = chunk_sender.send(Outgoing::Frame {
                        event: "chunk".to_string(),
                        payload: serde_json::json!({
                            "request_id": *chunk_route.lock().unwrap(),
                            "text": &text[..end],
                        }),
                    });
                },
                &mut cancel,
            );
            match tokio::time::timeout(timeout, run).await {
                Ok(result) => result,
                Err(_elapsed) => {
                    // Stop the child, then report the timeout as the reason
                    // rather than whatever the cancellation looked like.
                    let _ = cancel_sender.send(true);
                    Err(AcpFailure::Refused(format!(
                        "the delegation did not finish within {}s",
                        timeout.as_secs()
                    )))
                }
            }
        });

        active.fetch_sub(1, Ordering::SeqCst);
        // By the route, not by the request this started on: a reattach moved
        // the job to the resuming request's id, and removing the id it no
        // longer lives under would leave a finished delegation in the map for
        // as long as the connection lasts. The route is read and released
        // before the map is locked — the reattach path locks the map first, so
        // taking them in the other order here is a deadlock.
        let current = route.lock().unwrap().clone();
        let cancelled = agents
            .lock()
            .unwrap()
            .remove(&current)
            .map(|job| *job.cancel.borrow())
            .unwrap_or(false);
        let session_id = session.lock().unwrap().clone();
        let truncated = truncated.load(Ordering::SeqCst);
        let (status, stop_reason, detail) = match &outcome {
            Ok(finished) => {
                let status = match finished.stop_reason.as_str() {
                    "cancelled" => "cancelled",
                    "refusal" => "refused",
                    _ if truncated => "truncated",
                    _ => "completed",
                };
                (
                    status,
                    finished.stop_reason.clone(),
                    if truncated {
                        "output truncated".to_string()
                    } else {
                        String::new()
                    },
                )
            }
            Err(AcpFailure::Cancelled) => (
                "cancelled",
                String::new(),
                "the delegation was stopped".to_string(),
            ),
            Err(AcpFailure::Unstartable(why)) => ("unavailable", String::new(), redact(why)),
            Err(AcpFailure::Refused(why)) => {
                let status = if cancelled {
                    "cancelled"
                } else if why.starts_with("the delegation did not finish within") {
                    "timeout"
                } else {
                    "failed"
                };
                (status, String::new(), redact(why))
            }
        };
        let _ = journal.append(&owning_id, &request, "allowed", status, &detail);
        let _ = sender.send(Outgoing::Frame {
            event: "exit".to_string(),
            payload: serde_json::json!({
                "request_id": current,
                "status": status,
                "session_id": session_id,
                "stop_reason": stop_reason,
                "truncated": truncated,
                "detail": bounded(&detail, 400),
                "duration_ms": started.elapsed().as_millis() as u64,
            }),
        });
    });
}

fn request_fields(payload: &serde_json::Value) -> Option<CommandRequest> {
    let argv = payload.get("argv")?.as_array()?;
    if argv.is_empty() || argv.len() > MAXIMUM_ARGV_LENGTH {
        return None;
    }
    let mut parts = Vec::with_capacity(argv.len());
    for value in argv {
        let text = value.as_str()?;
        if text.len() > MAXIMUM_ARGUMENT_LENGTH {
            return None;
        }
        parts.push(text.to_string());
    }
    let cwd = payload.get("cwd")?.as_str()?;
    if cwd.len() > 4_096 {
        return None;
    }
    Some(CommandRequest {
        argv: parts,
        cwd: cwd.to_string(),
    })
}

fn bounded_number(payload: &serde_json::Value, names: &[&str], fallback: u64, ceiling: u64) -> u64 {
    for name in names {
        if let Some(value) = payload.get(*name).and_then(|value| value.as_f64()) {
            if value.is_finite() && value > 0.0 {
                return (value.floor() as u64).min(ceiling);
            }
        }
    }
    fallback
}

/// Serve until a refusal stops the command, reconnecting on transport loss and
/// on `machine_reconnecting` with bounded backoff.
pub fn serve(
    origin: &str,
    token: &crate::auth::Secret,
    machine_id: &str,
    hello: &serde_json::Value,
    config: &PolicyConfig,
    journal: &Journal,
    mut on_event: impl FnMut(&str),
) -> String {
    // Which agents this machine will delegate to is decided once, here, from
    // what the owner declared and what is actually installed. It is not read
    // from the request, and a reconnect does not widen it.
    let catalog = agent_catalog(config, &installed_coding_agents(&config.roots));
    let mut attempts: u32 = 0;
    loop {
        let end = serve_connection(
            origin,
            token,
            machine_id,
            hello,
            config,
            journal,
            &catalog,
            &mut on_event,
        );
        if !end.retryable || attempts >= MAXIMUM_RECONNECT_ATTEMPTS {
            if end.retryable && attempts > 0 {
                let kind = if end.reason.starts_with("join_refused:machine_reconnecting") {
                    "machine_reconnecting"
                } else {
                    "transport"
                };
                return format!("{kind}_retry_exhausted:{}", end.reason);
            }
            return end.reason;
        }
        attempts += 1;
        let delay = RECONNECT_BACKOFF
            .saturating_mul(1u32 << (attempts - 1))
            .min(MAXIMUM_BACKOFF);
        on_event(&format!("reconnect:{}:{attempts}", end.reason));
        std::thread::sleep(delay);
    }
}

// ---------------------------------------------------------------------------
// the command
// ---------------------------------------------------------------------------

#[derive(Args, Debug)]
pub struct ComputerArgs {
    #[command(subcommand)]
    pub action: ComputerAction,
}

#[derive(Subcommand, Debug)]
pub enum ComputerAction {
    /// Inspect this machine with fixed read-only probes
    Probe {
        #[arg(long, help = "Inspect a declared directory; repeatable")]
        root: Vec<String>,
    },
    /// Show the local tier, declared roots, and curated allowlist
    Policy {
        #[arg(long, help = "Show the policy against these roots; repeatable")]
        root: Vec<String>,
    },
    /// Show local state, pairing state, and file locations
    Status,
    /// Serve bounded Computer requests over an outbound connection
    Up,
    /// Pair this Computer through browser approval and store its machine token
    Pair {
        #[arg(
            long,
            help = "Set the local execution ceiling: probe, curated, or shell"
        )]
        tier: Option<String>,
        #[arg(long, help = "Declare a reachable directory; repeatable")]
        root: Vec<String>,
    },
    /// Remove this Computer's local machine token and pairing state
    Logout,
    /// Show the local record of requests and decisions, including refusals
    Journal {
        #[arg(long, default_value_t = 20, help = "Most entries to show")]
        limit: usize,
    },
}

fn roots_for(config: &PolicyConfig, overrides: &[String]) -> Vec<PathBuf> {
    if overrides.is_empty() {
        config.roots.clone()
    } else {
        resolve_roots(overrides)
    }
}

fn root_list(roots: &[PathBuf]) -> String {
    if roots.is_empty() {
        "(none declared)".to_string()
    } else {
        roots
            .iter()
            .map(|root| root.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

pub async fn run(args: ComputerArgs, endpoint: &crate::auth::Endpoint, json: bool) {
    let fail = crate::cli::fail;
    let paths = ComputerPaths::default_paths();
    let config = match load_config(&paths) {
        Ok(config) => config,
        Err(reason) => fail(&reason),
    };
    let journal = Journal::at(paths.journal.clone());

    match args.action {
        ComputerAction::Probe { root } => {
            let roots = roots_for(&config, &root);
            let report = probe_for(&PolicyConfig {
                roots: roots.clone(),
                ..config.clone()
            });
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).unwrap_or_default()
                );
                return;
            }
            println!(
                "Host: {} {} {}",
                report.host.platform, report.host.release, report.host.architecture
            );
            println!("Hostname: {}", report.host.hostname);
            println!("Roots: {}", root_list(&roots));
            println!(
                "Coding agents present: {}/{}",
                report.coding_agents.iter().filter(|t| t.present).count(),
                report.coding_agents.len()
            );
            println!(
                "Toolchains present: {}/{}",
                report.toolchains.iter().filter(|t| t.present).count(),
                report.toolchains.len()
            );
            println!("Worktrees inspected: {}", report.worktrees.len());
        }
        ComputerAction::Policy { root } => {
            let roots = roots_for(&config, &root);
            let catalog = agent_catalog(&config, &installed_coding_agents(&roots));
            if json {
                let value = serde_json::json!({
                    "schema": "openagents.computer_policy.v1",
                    "tier": config.tier.label(),
                    "roots": roots.iter().map(|r| r.display().to_string()).collect::<Vec<_>>(),
                    "pre_approved": config.pre_approved,
                    "delegable_agents": catalog
                        .iter()
                        .map(|entry| serde_json::json!({"id": entry.id, "source": entry.source}))
                        .collect::<Vec<_>>(),
                    "scoped_forge_credentials": config.scoped_forge_credentials,
                    "authority": "local_machine",
                    "paths": {
                        "config": paths.config.display().to_string(),
                        "journal": paths.journal.display().to_string(),
                    },
                    "allowlist": format_allowlist(),
                    "scope": "local inspection and policy",
                    "network": false,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&value).unwrap_or_default()
                );
                return;
            }
            println!("Authority: this machine decides what may run.");
            println!("Effective tier: {}", config.tier.label());
            println!("Declared roots: {}", root_list(&roots));
            println!("Empty roots mean that no working directory is reachable.");
            println!("Path rules follow this host's POSIX or Windows semantics.");
            println!("Curated allowlist:");
            for line in format_allowlist() {
                println!("  {line}");
            }
            println!(
                "Delegable ACP agents: {}",
                if catalog.is_empty() {
                    "(none)".to_string()
                } else {
                    catalog
                        .iter()
                        .map(|entry| format!("{} ({})", entry.id, entry.source))
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            );
            println!(
                "A delegated agent runs under this same policy: the tier ceiling, the declared \
                 roots, and the curated allowlist decide every action it asks to take."
            );
            println!(
                "Scoped forge credentials: {}",
                if config.scoped_forge_credentials {
                    "allowed for delegated pushes"
                } else {
                    "not allowed; a delivered credential is refused and journaled"
                }
            );
            println!("Configuration: {}", paths.config.display());
            println!("No account, pairing, or network is needed for this command.");
        }
        ComputerAction::Status => {
            let credentials = MachineCredentials::for_origin(&endpoint.origin);
            let stored = match credentials.get() {
                Ok(stored) => stored,
                Err(reason) => fail(&reason),
            };
            // A held token is checked against the server. A transport failure
            // ends the command: reporting "unpaired" because the network was
            // down would be a claim the server never made.
            let remote = match &stored {
                Some(token) => {
                    let client = ComputerClient::new(&endpoint.origin);
                    match client.status(token).await {
                        Ok(status) => status,
                        Err(reason) => fail(&reason),
                    }
                }
                None => None,
            };
            let paired = remote.is_some();
            let state = if paired {
                "paired"
            } else if stored.is_some() {
                "unpaired"
            } else {
                "local"
            };
            if json {
                let value = serde_json::json!({
                    "schema": "openagents.computer_status.v1",
                    "state": state,
                    "paired": paired,
                    "endpoint": endpoint.origin,
                    "tier": config.tier.label(),
                    "roots": config.roots.iter().map(|r| r.display().to_string()).collect::<Vec<_>>(),
                    "machine": {
                        "platform": wire_platform(),
                        "architecture": wire_architecture(),
                        "hostname": hostname(),
                    },
                    "paths": {
                        "config": paths.config.display().to_string(),
                        "journal": paths.journal.display().to_string(),
                    },
                    "journal_retention_bytes": JOURNAL_MAX_BYTES,
                    "journal_read_tail_bytes": JOURNAL_READ_TAIL_BYTES,
                    "remote_state": if paired { "active" } else { "unpaired" },
                    "machine_id": remote.as_ref().map(|status| status.machine_id.clone()),
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&value).unwrap_or_default()
                );
                return;
            }
            println!("Computer state: {state}");
            println!(
                "Pairing: {}",
                if paired {
                    "paired"
                } else if stored.is_some() {
                    "no longer active; run oa computer logout"
                } else {
                    "not configured"
                }
            );
            println!("Endpoint: {}", endpoint.origin);
            println!("Tier: {}", config.tier.label());
            println!("Roots: {}", root_list(&config.roots));
            println!("Configuration: {}", paths.config.display());
            println!("Journal: {}", paths.journal.display());
            println!(
                "Journal retention: last {JOURNAL_MAX_BYTES} bytes; reads inspect the last \
                 {JOURNAL_READ_TAIL_BYTES} bytes"
            );
            println!("The machine, not the server, decides what runs here.");
            println!("Path rules follow this host's POSIX or Windows semantics.");
            if let Some(status) = &remote {
                println!("Machine id: {}", status.machine_id);
            }
            if stored.is_some() && !paired {
                println!(
                    "The server no longer accepts this machine token; run oa computer logout."
                );
            }
        }
        ComputerAction::Up => {
            let credentials = MachineCredentials::for_origin(&endpoint.origin);
            let Some(token) = (match credentials.get() {
                Ok(stored) => stored,
                Err(reason) => fail(&reason),
            }) else {
                fail(&format!(
                    "this Computer is not paired with {}; run oa computer pair first",
                    endpoint.origin
                ));
            };
            let client = ComputerClient::new(&endpoint.origin);
            let status = match client.status(&token).await {
                Ok(Some(status)) => status,
                Ok(None) => fail(&format!(
                    "this Computer is no longer active on {}; run oa computer logout",
                    endpoint.origin
                )),
                Err(reason) => fail(&reason),
            };
            let initial = probe_for(&config);
            let hello = serde_json::json!({
                "agent_version": crate::VERSION,
                "tier": config.tier.label(),
                "roots": config.roots.iter().map(|r| r.display().to_string()).collect::<Vec<_>>(),
                "platform": format!("{}-{}", wire_platform(), wire_architecture()),
                "probe": serde_json::to_value(&initial).unwrap_or(serde_json::Value::Null),
            });
            let origin = endpoint.origin.clone();
            let machine_id = status.machine_id.clone();
            let reason = tokio::task::spawn_blocking(move || {
                serve(
                    &origin,
                    &token,
                    &machine_id,
                    &hello,
                    &config,
                    &journal,
                    |event| eprintln!("oa computer: {event}"),
                )
            })
            .await
            .unwrap_or_else(|error| format!("error:{error}"));

            // Every ending here is the connection stopping. The command says
            // which, and a refusal or an exhausted retry is a failure.
            if reason.contains("retry_exhausted")
                || reason.starts_with("join_refused:")
                || reason.starts_with("error:")
            {
                fail(&format!("the Computer connection stopped: {reason}"));
            }
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "schema": "openagents.computer_connection.v1",
                        "state": "closed",
                        "reason": reason,
                    }))
                    .unwrap_or_default()
                );
            } else {
                println!("Computer connection ended: {reason}");
            }
        }
        ComputerAction::Pair { tier, root } => {
            let credentials = MachineCredentials::for_origin(&endpoint.origin);
            match credentials.get() {
                Ok(Some(_)) => fail(&format!(
                    "this Computer is already paired with {}; run oa computer logout before \
                     pairing again",
                    endpoint.origin
                )),
                Ok(None) => {}
                Err(reason) => fail(&reason),
            }
            let selected = match tier.as_deref() {
                None => config.tier,
                Some(value) => match Tier::parse(value) {
                    Some(tier) => tier,
                    None => fail(&format!(
                        "unknown tier {value}. Use probe, curated, or shell"
                    )),
                },
            };
            let roots = roots_for(&config, &root);
            let next = PolicyConfig {
                tier: selected,
                roots: roots.clone(),
                ..config
            };
            if let Err(reason) = write_config(&next) {
                fail(&reason);
            }
            let client = ComputerClient::new(&endpoint.origin);
            let name = hostname();
            let started = match client.start(&name, selected, crate::VERSION, &roots).await {
                Ok(started) => started,
                Err(reason) => fail(&reason),
            };
            println!("Approve this Computer at {}", started.verify_url);
            println!("Pairing code: {}", started.code);
            println!("Waiting for approval...");
            if !json {
                crate::auth::open_browser(&started.verify_url);
            }
            let claim = match client.wait(&started).await {
                Ok(claim) => claim,
                Err(reason) => fail(&reason),
            };
            let token = crate::auth::Secret::new(claim.token.clone());
            if let Err(reason) = credentials.set(&token) {
                fail(&reason);
            }
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "endpoint": endpoint.origin,
                        "paired": true,
                        "machine_id": claim.machine_id,
                        "name": claim.name,
                        "token_source": "computer_credential_store",
                    }))
                    .unwrap_or_default()
                );
            } else {
                println!("Computer paired with {}.", endpoint.origin);
                println!("Machine id: {}", claim.machine_id);
                println!("The machine token is in the OS credential store.");
            }
        }
        ComputerAction::Logout => {
            let credentials = MachineCredentials::for_origin(&endpoint.origin);
            let removed = match credentials.remove() {
                Ok(removed) => removed,
                Err(reason) => fail(&reason),
            };
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "endpoint": endpoint.origin,
                        "removed": removed,
                        "remote_state": "unverified",
                    }))
                    .unwrap_or_default()
                );
            } else {
                println!(
                    "Removed the local Computer pairing for {}.",
                    endpoint.origin
                );
                println!("No local machine token remains. Remote pairing state is not queried.");
            }
        }
        ComputerAction::Journal { limit } => {
            let entries = match journal.read(limit) {
                Ok(entries) => entries,
                Err(reason) => fail(&reason),
            };
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "schema": "openagents.computer_journal.v1",
                        "entries": entries,
                    }))
                    .unwrap_or_default()
                );
                return;
            }
            if entries.is_empty() {
                println!("No local Computer requests are recorded.");
                return;
            }
            for entry in entries {
                println!(
                    "{} {}/{} {} {}",
                    entry.at,
                    entry.decision,
                    entry.outcome,
                    entry.request_id,
                    entry.argv.join(" ")
                );
            }
        }
    }
}
