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

#[derive(Debug, Clone)]
pub struct PolicyConfig {
    pub tier: Tier,
    pub roots: Vec<PathBuf>,
    pub pre_approved: Vec<String>,
    pub curated_execute: Vec<String>,
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
    agents: Option<serde_json::Value>,
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
    Ok(PolicyConfig {
        tier,
        roots: resolve_roots(&stored.roots.unwrap_or_default()),
        pre_approved,
        curated_execute,
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
        agents: Some(serde_json::json!({})),
    };
    let encoded = serde_json::to_string_pretty(&stored)
        .map_err(|error| format!("the Computer configuration could not be encoded: {error}"))?;
    write_private_file(&config.paths.config, &format!("{encoded}\n"))
}

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
    std::fs::write(path, contents)
        .map_err(|error| format!("could not write {}: {error}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("could not secure {}: {error}", path.display()))?;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeReport {
    pub schema: String,
    pub host: HostReport,
    #[serde(rename = "codingAgents")]
    pub coding_agents: Vec<ToolReport>,
    pub toolchains: Vec<ToolReport>,
    pub roots: Vec<String>,
    pub worktrees: Vec<WorktreeReport>,
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
                text.truncate(512);
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
    }
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
fn serve_connection(
    origin: &str,
    token: &crate::auth::Secret,
    machine_id: &str,
    hello: &serde_json::Value,
    config: &PolicyConfig,
    journal: &Journal,
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

    let (sender, receiver): (Sender<Outgoing>, Receiver<Outgoing>) = std::sync::mpsc::channel();
    let active = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let cancellations: Arc<Mutex<BTreeMap<String, Cancellation>>> =
        Arc::new(Mutex::new(BTreeMap::new()));

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
            if response_ref != join_ref {
                continue;
            }
            if payload.get("status").and_then(|v| v.as_str()) == Some("ok") {
                joined = true;
                on_event("joined");
                reference += 1;
                let frame = phoenix_frame(
                    Some(join_ref),
                    &reference.to_string(),
                    &topic,
                    "hello",
                    hello,
                );
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
                let report = probe(&config.roots);
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
            // ACP delegation is a separate subsystem this build does not carry,
            // and `devin` is a second delegation kind it does not carry either.
            // Saying so is the honest answer; pretending to accept either would
            // leave the server waiting for output that never comes.
            //
            // `OpenAgentsWeb.ComputerChannel` pushes a request by the name of
            // its kind — `handle_info({:computer_request, kind, …})` for `kind
            // in [:run, :devin, :agent]` does `push(socket,
            // Atom.to_string(kind), …)` — so every one of those names arrives
            // here as an event carrying a `request_id` the server is tracking.
            // `devin` used to fall through to the catch-all below and be
            // dropped without a frame or a journal line, which is the exact
            // failure this arm was written to prevent, one kind over.
            "agent" | "devin" => {
                let request = CommandRequest {
                    argv: vec![format!("<{event}>")],
                    cwd: String::new(),
                };
                let _ = journal.append(
                    &request_id,
                    &request,
                    "unsupported",
                    "refused",
                    "ACP delegation is unavailable",
                );
                reference += 1;
                let frame = phoenix_frame(
                    Some(join_ref),
                    &reference.to_string(),
                    &topic,
                    "refused",
                    &serde_json::json!({
                        "request_id": request_id,
                        "reason": "unsupported",
                        "detail": "ACP delegation is unavailable",
                    }),
                );
                let _ = socket.send(Message::Text(frame.into()));
            }
            "cancel" => {
                if let Some(cancellation) = cancellations.lock().unwrap().get(&request_id) {
                    cancellation.cancel();
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
    let mut attempts: u32 = 0;
    loop {
        let end = serve_connection(
            origin,
            token,
            machine_id,
            hello,
            config,
            journal,
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
            let report = probe(&roots);
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
            if json {
                let value = serde_json::json!({
                    "schema": "openagents.computer_policy.v1",
                    "tier": config.tier.label(),
                    "roots": roots.iter().map(|r| r.display().to_string()).collect::<Vec<_>>(),
                    "pre_approved": config.pre_approved,
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
            let initial = probe(&config.roots);
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
