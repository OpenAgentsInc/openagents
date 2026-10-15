//! Whether the `openagents` a chat runs has the commands Coder's own
//! instructions name (#11595).
//!
//! Coder's instructions are written against the CLI built from the same
//! commit (the bundled `coder::cli_route::tree`). An older `openagents`
//! on this computer can lack one, such as `openagents projects list`;
//! the chat then said "unknown command" and looked elsewhere. Now the CLI
//! is asked what it runs (`openagents capabilities`, or its `--help`
//! groups when it is older than that command), once per binary. A command
//! it lacks is answered with the one update command, or, when Coder
//! updates itself, by installing the channel's release after checking
//! that release's `openagents` has the command, then running a read-only
//! request again once.

use std::{
    collections::{BTreeSet, HashMap},
    io::Read,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime},
};

use coder::cli_route::tree::{self, CommandTree, Effect};
use serde_json::{Value, json};

use crate::bundled_runtime::RuntimeEvent;
use crate::update;

/// The longest a capability probe waits for the CLI.
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

/// What one `openagents` says it runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capabilities {
    /// Its `--version` line, as it says it.
    pub version: Option<String>,
    known: Known,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Known {
    /// Every command's words, from `openagents capabilities`.
    Commands(BTreeSet<Vec<String>>),
    /// Only the top-level groups, from the `--help` of a CLI older than
    /// `capabilities`.
    Groups(BTreeSet<String>),
}

impl Capabilities {
    /// Whether `path` (group first) is a command or a group of commands
    /// this CLI runs.
    #[must_use]
    pub fn supports(&self, path: &[String]) -> bool {
        match &self.known {
            Known::Commands(commands) => commands.iter().any(|command| command.starts_with(path)),
            Known::Groups(groups) => path.first().is_some_and(|group| groups.contains(group)),
        }
    }
}

/// Asks `program` what it runs: `capabilities`, else its `--help`.
/// `None` when it answers neither.
#[must_use]
pub fn probe(program: &Path) -> Option<Capabilities> {
    if let Some(found) = stdout(program, &["capabilities"]).and_then(|text| parse(&text)) {
        return Some(found);
    }
    let help = stdout(program, &["--help"])?;
    let groups: BTreeSet<String> = coder::cli_route::usage::groups(&help)
        .into_iter()
        .map(|row| row.name)
        .collect();
    if groups.is_empty() {
        return None;
    }
    let version = stdout(program, &["--version"])
        .and_then(|text| text.lines().next().map(|line| line.trim().to_owned()))
        .filter(|line| !line.is_empty());
    Some(Capabilities {
        version,
        known: Known::Groups(groups),
    })
}

fn parse(text: &str) -> Option<Capabilities> {
    let value: Value = serde_json::from_str(text.trim()).ok()?;
    if value["schema"] != tree::CAPABILITIES_SCHEMA {
        return None;
    }
    let commands = value["commands"]
        .as_array()?
        .iter()
        .map(|command| {
            command
                .as_array()?
                .iter()
                .map(|word| word.as_str().map(str::to_owned))
                .collect::<Option<Vec<String>>>()
        })
        .collect::<Option<BTreeSet<Vec<String>>>>()?;
    Some(Capabilities {
        version: value["version"].as_str().map(str::to_owned),
        known: Known::Commands(commands),
    })
}

/// `program arguments`' standard output when it exits 0 in time.
fn stdout(program: &Path, arguments: &[&str]) -> Option<String> {
    let mut child = acp_client::process::command(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut pipe = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = pipe.read_to_string(&mut text);
        text
    });
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < PROBE_TIMEOUT => {
                std::thread::sleep(Duration::from_millis(10));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    let text = reader.join().ok()?;
    status.success().then_some(text)
}

type Stamp = (u64, Option<SystemTime>);

fn cache() -> &'static Mutex<HashMap<PathBuf, (Stamp, Option<Arc<Capabilities>>)>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, (Stamp, Option<Arc<Capabilities>>)>>> =
        OnceLock::new();
    CACHE.get_or_init(Mutex::default)
}

/// [`probe`], once for each binary: a replaced file is asked again.
#[must_use]
pub fn cached(program: &Path) -> Option<Arc<Capabilities>> {
    let meta = std::fs::metadata(program).ok()?;
    let stamp = (meta.len(), meta.modified().ok());
    if let Some((seen, found)) = cache().lock().ok()?.get(program)
        && *seen == stamp
    {
        return found.clone();
    }
    let found = probe(program).map(Arc::new);
    if let Ok(mut cache) = cache().lock() {
        cache.insert(program.to_owned(), (stamp, found.clone()));
    }
    found
}

// ---- What Coder's instructions name --------------------------------------

/// Every text Coder hands a chat that names `openagents` commands, and
/// whether its `[…]` arrays are `openagents_cli` arguments.
#[must_use]
pub fn instructions() -> Vec<(String, bool)> {
    let description = |tool: Value| {
        tool["function"]["description"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    };
    vec![
        (crate::identity::PROJECTS.to_owned(), false),
        (crate::identity::FEEDBACK.to_owned(), false),
        (
            crate::plugin_tools::OPENAGENTS_CLI_GUIDANCE.to_owned(),
            true,
        ),
        (crate::computer_tool::instructions().to_owned(), false),
        (crate::ops_tool::instructions().to_owned(), false),
        (crate::web_asks::NO_HOST.to_owned(), false),
        (
            description(crate::bundled_runtime::cli_tool_definition()),
            true,
        ),
        (description(crate::github_tool::definition()), false),
    ]
}

/// The commands `text` names, as `tree` words: `openagents GROUP WORD…`
/// in prose, and with `arrays` `["GROUP","WORD",…]` argument arrays. A word is part of
/// the command while the tree has it there; `Err` names a group, or a word
/// under a group that is no command, that `tree` does not have.
///
/// # Errors
///
/// The first named command `tree` does not have.
pub fn named(text: &str, arrays: bool, tree: &CommandTree) -> Result<Vec<Vec<String>>, String> {
    let mut found = Vec::new();
    let listed = if arrays {
        argument_arrays(text)
    } else {
        Vec::new()
    };
    for words in prose(text).into_iter().chain(listed) {
        for path in descend(&words, tree)? {
            if !found.contains(&path) {
                found.push(path);
            }
        }
    }
    Ok(found)
}

/// The command-like words after each `openagents ` in prose: each word a
/// set of `|` alternatives, up to a placeholder, option, or punctuation.
fn prose(text: &str) -> Vec<Vec<Vec<String>>> {
    let mut out = Vec::new();
    for (at, _) in text.match_indices("openagents ") {
        let before = text[..at].chars().next_back();
        if before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '/') {
            continue;
        }
        let mut words = Vec::new();
        for raw in text[at + "openagents ".len()..].split_whitespace() {
            let word = raw.trim_start_matches(['`', '(']);
            let bare = word.trim_end_matches(['`', ',', '.', ';', ':', ')', ']', '"']);
            match alternatives(bare) {
                Some(alternatives) => words.push(alternatives),
                None => break,
            }
            if bare.len() != word.len() || word.len() != raw.len() {
                break;
            }
        }
        if !words.is_empty() {
            out.push(words);
        }
    }
    out
}

/// The leading quoted words of each `[…]` array whose first item is a
/// lowercase word.
fn argument_arrays(text: &str) -> Vec<Vec<Vec<String>>> {
    let mut out = Vec::new();
    for (at, _) in text.match_indices('[') {
        let mut rest = &text[at + 1..];
        let mut words = Vec::new();
        'items: loop {
            let mut item = Vec::new();
            loop {
                let Some(open) = rest.strip_prefix('"') else {
                    break 'items;
                };
                let Some(close) = open.find('"') else {
                    break 'items;
                };
                let word = &open[..close];
                if alternatives(word).is_none() {
                    break 'items;
                }
                item.push(word.to_owned());
                rest = &open[close + 1..];
                match rest.strip_prefix('|') {
                    Some(more) => rest = more,
                    None => break,
                }
            }
            words.push(item);
            match rest.strip_prefix(',') {
                Some(more) => rest = more.trim_start(),
                None => break,
            }
        }
        if !words.is_empty() {
            out.push(words);
        }
    }
    out
}

/// `word` as command alternatives (`add|remove`), each lowercase letters,
/// digits, and hyphens starting with a letter; `None` for anything else.
fn alternatives(word: &str) -> Option<Vec<String>> {
    let parts: Vec<String> = word.split('|').map(str::to_owned).collect();
    parts
        .iter()
        .all(|part| {
            part.starts_with(|c: char| c.is_ascii_lowercase())
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
        .then_some(parts)
}

/// The tree paths `words` name, alternatives expanded.
fn descend(words: &[Vec<String>], tree: &CommandTree) -> Result<Vec<Vec<String>>, String> {
    let Some((first, rest)) = words.split_first() else {
        return Ok(Vec::new());
    };
    let mut paths = Vec::new();
    for group in first {
        if tree.group(group).is_none() {
            return Err(format!("`openagents {group}` is no command"));
        }
        paths.push(vec![group.clone()]);
    }
    for alternatives in rest {
        let mut next = Vec::new();
        let mut stop = false;
        for path in &paths {
            let node = tree.node(path).expect("each path is in the tree");
            if alternatives.iter().all(|word| node.child(word).is_some()) {
                for word in alternatives {
                    let mut longer = path.clone();
                    longer.push(word.clone());
                    next.push(longer);
                }
            } else if node.leaf.is_some() {
                // The command is complete; the word is its argument or prose.
                stop = true;
            } else {
                return Err(format!(
                    "`openagents {} {}` is no command",
                    path.join(" "),
                    alternatives.join("|")
                ));
            }
        }
        if stop {
            break;
        }
        paths = next;
    }
    Ok(paths)
}

/// The commands Coder's instructions name, as this build's tree has them.
#[must_use]
pub fn required() -> &'static [Vec<String>] {
    static REQUIRED: OnceLock<Vec<Vec<String>>> = OnceLock::new();
    REQUIRED.get_or_init(|| {
        let mut all: Vec<Vec<String>> = Vec::new();
        for (text, arrays) in instructions() {
            for path in named(&text, arrays, tree::bundled()).unwrap_or_default() {
                if !all.contains(&path) {
                    all.push(path);
                }
            }
        }
        all
    })
}

/// The command `arguments` (group first) run, as far as this build's tree
/// has its words: `None` for a group or word it does not know.
#[must_use]
pub fn command_of(arguments: &[String], tree: &CommandTree) -> Option<Vec<String>> {
    let words: Vec<String> = tree::tree_argv(arguments)
        .into_iter()
        .take_while(|word| !word.starts_with('-'))
        .collect();
    let (first, rest) = words.split_first()?;
    let mut node = tree.group(first)?;
    let mut path = vec![first.clone()];
    for word in rest {
        match node.child(word) {
            Some(child) => {
                node = child;
                path.push(word.clone());
            }
            None => break,
        }
    }
    Some(path)
}

// ---- What a chat is told, and the update ---------------------------------

/// The commands an `openagents` lacks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Missing {
    pub program: PathBuf,
    pub version: Option<String>,
    pub commands: Vec<Vec<String>>,
}

impl Missing {
    fn commands_named(&self) -> String {
        let mut names: Vec<String> = self
            .commands
            .iter()
            .take(3)
            .map(|path| format!("`openagents {}`", path.join(" ")))
            .collect();
        if self.commands.len() > 3 {
            names.push(format!("{} more", self.commands.len() - 3));
        }
        names.join(", ")
    }

    fn which(&self) -> String {
        match &self.version {
            Some(version) => format!("{} ({version})", self.program.display()),
            None => self.program.display().to_string(),
        }
    }

    /// What the chat says: an update is needed, and the one command that
    /// installs it.
    #[must_use]
    pub fn message(&self) -> String {
        format!(
            "The openagents command on this computer, {}, has no {}, which Coder uses for this: \
             it needs an update. Tell the person to update it with: {} \
             Do not look for the same thing another way.",
            self.which(),
            self.commands_named(),
            update::installer_command()
        )
    }

    /// The TUI's one line.
    #[must_use]
    pub fn notice(&self) -> String {
        format!(
            "The openagents command at {} has no {}. Update it with: {}",
            self.which(),
            self.commands_named(),
            update::installer_command()
        )
    }
}

/// Which of `needed` the CLI at `program` lacks; `None` when it has them
/// all or does not answer (it then runs as before).
#[must_use]
pub fn missing(program: &Path, needed: &[Vec<String>]) -> Option<Missing> {
    let found = cached(program)?;
    let commands: Vec<Vec<String>> = needed
        .iter()
        .filter(|path| !found.supports(path))
        .cloned()
        .collect();
    (!commands.is_empty()).then(|| Missing {
        program: program.to_owned(),
        version: found.version.clone(),
        commands,
    })
}

/// The updater's context when Coder installs updates on its own here:
/// automatic, `auto`, and a standalone install it can replace.
#[must_use]
pub fn auto_update() -> Option<update::Context> {
    let env = |name: &str| std::env::var(name).ok();
    let context = update::Context::from_env(&crate::home::data_dir()?).ok()?;
    (context.config.automatic(&env, cfg!(debug_assertions))
        && context.config.mode == update::Mode::Auto
        && matches!(context.kind, update::InstallKind::Standalone(_)))
    .then_some(context)
}

/// One update attempt per Coder process: later callers get its result.
async fn update_once(context: &update::Context, need: &[Vec<String>]) -> Result<String, String> {
    static TRIED: AtomicBool = AtomicBool::new(false);
    static RESULT: Mutex<Option<Result<String, String>>> = Mutex::new(None);
    if TRIED.swap(true, Ordering::SeqCst) {
        return RESULT
            .lock()
            .ok()
            .and_then(|result| result.clone())
            .unwrap_or_else(|| Err("Coder is already updating.".into()));
    }
    let result = update::install_for_cli(context, need)
        .await
        .map(|version| version.to_string());
    if let Ok(mut slot) = RESULT.lock() {
        *slot = Some(result.clone());
    }
    result
}

/// Runs `arguments` on the CLI: first, when this build's tree has their
/// command and the CLI does not ([`missing`]), the chat is told the update
/// command, or, with automatic updates, the verified release is installed
/// and a read-only request runs again once.
pub async fn run(
    arguments: &[String],
    cwd: &Path,
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    let program = crate::bundled_runtime::cli_binary().ok_or_else(|| {
        format!(
            "The bundled OpenAgents CLI is missing. Reinstall Coder with `{}`.",
            crate::account::INSTALL_COMMAND
        )
    })?;
    let bundled = tree::bundled();
    let Some(gap) = command_of(arguments, bundled)
        .and_then(|command| missing(&program, std::slice::from_ref(&command)))
    else {
        return crate::bundled_runtime::cli_at(&program, arguments, cwd, cancel, emit).await;
    };
    let Some(context) = auto_update() else {
        return Err(gap.message());
    };
    let version = update_once(&context, &gap.commands).await.map_err(|why| {
        format!(
            "{} Coder could not update it on its own: {why}",
            gap.message()
        )
    })?;
    let program = crate::bundled_runtime::cli_binary().unwrap_or(program);
    if missing(&program, &gap.commands).is_some() {
        return Err(gap.message());
    }
    let updated = format!(
        "Coder installed release {version}, whose openagents has {}.",
        gap.commands_named()
    );
    let read_only = bundled
        .leaf(&gap.commands[0])
        .is_some_and(|leaf| leaf.effect == Effect::ReadOnly);
    if !read_only {
        return Ok(json!({
            "updated": updated,
            "ran": false,
            "next": "Call openagents_cli again with the same arguments to run it.",
        }));
    }
    let mut value = crate::bundled_runtime::cli_at(&program, arguments, cwd, cancel, emit).await?;
    value["updated"] = Value::String(updated);
    Ok(value)
}

static NOTE: Mutex<Option<String>> = Mutex::new(None);

/// What a chat's instructions add while the CLI lacks a command they
/// name: the update command, so the chat says it rather than looking
/// elsewhere.
#[must_use]
pub fn prompt_note() -> Option<String> {
    NOTE.lock().ok()?.as_ref().map(|line| format!("{line}\n"))
}

/// At the start of a session: checks the CLI chats run against the
/// commands the instructions name, updates it when Coder updates itself,
/// and returns the TUI's one line.
#[must_use]
pub fn spawn_preflight() -> std::sync::mpsc::Receiver<String> {
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let Some(program) = crate::bundled_runtime::cli_binary() else {
            return;
        };
        let Some(gap) = missing(&program, required()) else {
            return;
        };
        let line = match auto_update() {
            Some(context) => {
                let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                else {
                    return;
                };
                match runtime.block_on(update_once(&context, &gap.commands)) {
                    Ok(version) => {
                        let _ = send.send(format!(
                            "Updated the openagents command to release {version}: it now has {}.",
                            gap.commands_named()
                        ));
                        return;
                    }
                    Err(why) => format!("{} (Coder could not update it: {why})", gap.notice()),
                }
            }
            None => gap.notice(),
        };
        if let Ok(mut note) = NOTE.lock() {
            *note = Some(gap.message());
        }
        let _ = send.send(line);
    });
    receive
}

#[cfg(test)]
#[path = "cli_capabilities_tests.rs"]
pub(crate) mod tests;
