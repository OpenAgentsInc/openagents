//! A chat session's private worktree and the shared checkout it came from
//! (#11372). Several sessions in one repository used to edit the launcher's
//! checkout at once; now each works in `.coder/worktrees/chat-<session>` on
//! `chat/<session>`, and the checkout it was started from is read-only for
//! it: Edit and Write refuse paths under it, and Run refuses commands that
//! write there. Reads stay free.
//!
//! The fence lives in the Coder SDK (#11532) so every surface that starts a
//! session registers the same one ([`hook`]): Coder chat, issue runs, the
//! briefed agent and Coder One's delegations.
//!
//! Scope (owner, #11720): a chat's fence protects only the shared checkout.
//! Writes elsewhere on the disk (the temp folder, `~/.cargo`, tool caches,
//! global Git config) stay free, on every platform and harness. A task
//! session (Solver and issue-run hands) fences the whole disk instead, by
//! naming `/` as its shared side ([`Boundary::protects`]).

use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use crate::events::{Call, Event};
use crate::outcome::{HookOutcome, Permission};
use crate::registry::Hook;

/// Guidance for a session that works in its own worktree.
pub const INSTRUCTIONS: &str = "You work in your own git worktree on your own branch; the folder you were started from is another checkout that other sessions and the user may be editing, and it is read-only for you: Edit, Write and commands that write under it are refused. To land, commit on this branch and `git push origin HEAD:main`; if the push is rejected, `git fetch origin && git rebase origin/main` and push again. Never split hunks: this checkout has no one else's changes.\n";

/// The worktree a session works in and the checkout it must not write to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Boundary {
    /// The session's own worktree.
    pub worktree: PathBuf,
    /// The checkout the session was started from, read-only for it.
    pub shared: PathBuf,
    /// The worktree's branch (`chat/<session>`), so a resumed session can
    /// reattach and `sessions delete` can drop it.
    pub branch: String,
}

impl Boundary {
    /// The boundary a Coder worktree implies when `cwd` is in one
    /// (`<checkout>/.coder/worktrees/<name>`): the worktree, and the
    /// checkout it was made from as the read-only side. `None` elsewhere.
    /// A harness session started in a session worktree gets the same fence
    /// as Coder's own tools without being handed it (#11485).
    #[must_use]
    pub fn around(cwd: &Path) -> Option<Self> {
        let parts = cwd.components().collect::<Vec<_>>();
        let at = parts.windows(3).position(|window| {
            window[0].as_os_str() == ".coder"
                && window[1].as_os_str() == "worktrees"
                && matches!(window[2], Component::Normal(_))
        })?;
        let shared = parts[..at].iter().collect::<PathBuf>();
        let worktree = parts[..at + 3].iter().collect::<PathBuf>();
        let name = parts[at + 2].as_os_str().to_string_lossy();
        let branch = name
            .strip_prefix("chat-")
            .map(|short| format!("chat/{short}"))
            .unwrap_or_default();
        (!shared.as_os_str().is_empty()).then_some(Self {
            worktree,
            shared,
            branch,
        })
    }

    /// Whether a path, as the tool or command names it from `cwd`, lands
    /// under the shared checkout and outside the session's own worktree.
    #[must_use]
    pub fn protects(&self, cwd: &Path, path: &str) -> bool {
        let path = path.trim_matches(|c| c == '\'' || c == '"');
        if path.is_empty() {
            return false;
        }
        let home = path
            .strip_prefix("~/")
            .zip(user_home::home_os())
            .map(|(rest, home)| PathBuf::from(home).join(rest));
        let joined = cwd.join(home.as_deref().unwrap_or_else(|| Path::new(path)));
        let normal = normalize(&joined);
        let shared = normalize(&self.shared);
        let own = normalize(&self.worktree);
        // `/` as the shared side is the whole disk (a task's fence): on
        // Windows that is every drive, which no `C:\` path starts with
        // (#11695).
        let everywhere = self.shared.as_os_str() == "/";
        (everywhere || normal.starts_with(&shared)) && !normal.starts_with(&own)
    }

    /// Why a file tool call is refused, or `None` when it may run.
    #[must_use]
    pub fn file_refusal(&self, tool: &str, path: &str, cwd: &Path) -> Option<String> {
        (matches!(tool, "Edit" | "Write" | "MultiEdit" | "NotebookEdit")
            && self.protects(cwd, path))
        .then(|| {
            format!(
                "Refused: {path} is in the shared checkout, which other sessions and the user may be editing. Work in your own worktree ({}); read the shared file if you need its contents.",
                self.worktree.display()
            )
        })
    }

    /// Why a shell command is refused, or `None` when it may run. Finds
    /// redirects, copies, moves, in-place edits, formatters, and mutating
    /// git subcommands aimed under the shared checkout; a plain `cd` into
    /// it followed by such a command counts too.
    #[must_use]
    pub fn run_refusal(&self, command: &str, cwd: &Path) -> Option<String> {
        // A PowerShell script Coder can't read (an encoded command, a
        // script file) could write anywhere, so it is refused (#11708).
        if crate::floors::unreadable(command) {
            return Some(crate::floors::UNREADABLE_REFUSAL.to_owned());
        }
        // Paths held in shell variables (`F=/repo/x; echo >> "$F"`) are read
        // through; a write whose target still can't be worked out before
        // the command runs (`$(...)`, a loop variable) is refused rather
        // than guessed at (#11522).
        let expanded = expand_variables(command);
        let command = expanded.as_str();
        let mut here = cwd.to_path_buf();
        for words in segments(command) {
            if let Some(target) = words
                .iter()
                .position(|word| {
                    matches!(
                        word.to_ascii_lowercase().as_str(),
                        "cd" | "chdir" | "pushd" | "set-location" | "sl"
                    )
                })
                .and_then(|index| {
                    // `cd /d D:\x` (cmd) and `Set-Location -Path x` name
                    // the directory after a switch.
                    words[index + 1..].iter().find(|word| {
                        !(cfg!(windows) && word.eq_ignore_ascii_case("/d"))
                            && !word.starts_with('-')
                    })
                })
            {
                // A directory that can't be worked out counts as the
                // shared checkout.
                here = if unresolved(target) {
                    self.shared.clone()
                } else {
                    here.join(target)
                };
            }
            if let Some(path) = self.written_path(&words, &here) {
                if unresolved(&path) {
                    return Some(format!(
                        "Refused: this command writes to {path}, and Coder can't tell where that is before it runs. The shared checkout ({}) is read-only for this session, so write to a plain path in your own worktree ({}) instead.",
                        self.shared.display(),
                        self.worktree.display()
                    ));
                }
                return Some(format!(
                    "Refused: this command writes to {path}, which is in the shared checkout that other sessions and the user may be editing. Run it in your own worktree ({}) instead.",
                    self.worktree.display()
                ));
            }
            // In the shared checkout only reading runs: a command that is
            // not a known reader is refused there rather than trusted not to
            // write (#11485). Git and cargo were judged above.
            if self.protects(&here, ".")
                && let Some(program) = program(&words)
                && !reads_only(&program, &words)
            {
                return Some(format!(
                    "Refused: `{program}` would run in the shared checkout ({}), which other sessions and the user may be editing, and it may write there. Run it in your own worktree ({}) instead; reading there with ls, cat, rg, or git status and log is fine.",
                    here.display(),
                    self.worktree.display()
                ));
            }
        }
        // A script (`python3 - <<EOF`, `node -e`, `bash -c`) can write
        // anywhere it names, so one that names a path in the shared checkout
        // is refused, whatever it does with it (#11485).
        // A shell wrapper (`/bin/zsh -lc '...'`) is not such a script: what
        // it runs is judged on its own ([`crate::floors::scripts`]).
        if segments(command)
            .filter(|words| crate::floors::wrapped_script(words).is_none())
            .filter_map(|words| program(&words))
            .any(|program| interpreter(&program))
            && let Some(path) = named_paths(command).find(|path| self.protects(cwd, path))
        {
            return Some(format!(
                "Refused: this runs a script that names {path}, which is in the shared checkout that other sessions and the user may be editing. Write in your own worktree ({}) instead; read the shared file with Read or cat if you need its contents.",
                self.worktree.display()
            ));
        }
        None
    }

    fn written_path(&self, words: &[String], here: &Path) -> Option<String> {
        // Redirects: `> path`, `>> path`, `>path`, `2>path`, `tee path`.
        for (index, word) in words.iter().enumerate() {
            // `*>` and `*>>` are PowerShell's every-stream redirects.
            let trimmed =
                word.trim_start_matches(|c: char| c.is_ascii_digit() || c == '&' || c == '*');
            let target = if trimmed == ">" || trimmed == ">>" {
                words.get(index + 1).map(String::as_str)
            } else if let Some(rest) = trimmed
                .strip_prefix(">>")
                .or_else(|| trimmed.strip_prefix('>'))
            {
                // `2>&1` duplicates a descriptor; it names no file.
                (!rest.is_empty() && !rest.starts_with('&')).then_some(rest)
            } else {
                None
            };
            if let Some(target) = target.filter(|target| !discarded(target))
                && (unresolved(target) || self.protects(here, target))
            {
                return Some(target.to_owned());
            }
        }
        let Some(program) = words.first() else {
            return None;
        };
        let name = crate::floors::program_name(program);
        let name = name.as_str();
        // `cmd`'s switches (`copy /Y`, `rd /s /q`) are not operands on
        // Windows; elsewhere `/x` is a path.
        let operands = words[1..]
            .iter()
            .filter(|word| !word.starts_with('-') && !(cfg!(windows) && cmd_switch(word)))
            .map(String::as_str)
            .collect::<Vec<_>>();
        // PowerShell's file cmdlets (#11708), by their names and aliases:
        // anything given as the written path, or any operand when none is
        // named, is judged.
        if let Some((params, last)) = powershell_writer(name) {
            let named = words[1..]
                .windows(2)
                .filter(|pair| {
                    params
                        .iter()
                        .any(|param| pair[0].eq_ignore_ascii_case(param))
                })
                .map(|pair| pair[1].as_str())
                .collect::<Vec<_>>();
            // Unnamed, a copy's destination follows its first operand,
            // the source; every later operand is judged.
            let judged = match (named.is_empty(), last) {
                (false, _) => named,
                (true, true) => operands.get(1..).unwrap_or_default().to_vec(),
                (true, false) => operands,
            };
            return judged
                .into_iter()
                .find(|path| unresolved(path) || self.protects(here, path))
                .map(str::to_owned);
        }
        let candidate = match name {
            "cp" | "mv" | "ln" | "install" | "rsync" | "copy" | "xcopy" | "move" => {
                operands.last().map(|path| (*path).to_owned())
            }
            // `robocopy SOURCE DEST [FILES]`.
            "robocopy" => operands.get(1).map(|path| (*path).to_owned()),
            "tee" | "truncate" | "touch" | "rm" | "rmdir" | "mkdir" | "patch" | "chmod"
            | "chown" | "chgrp" | "unlink" | "mkfifo" | "del" | "erase" | "rd" | "md" | "ren"
            | "rename" | "attrib" | "icacls" | "mklink" => {
                // cmd's `del a b` and `rd x y` take several.
                return operands
                    .iter()
                    .find(|path| unresolved(path) || self.protects(here, path))
                    .map(|path| (*path).to_owned());
            }
            "sed" | "perl" if words.iter().any(|word| word.starts_with("-i")) => {
                operands.last().map(|path| (*path).to_owned())
            }
            "dd" => {
                return operands
                    .iter()
                    .find_map(|word| word.strip_prefix("of="))
                    .map(|path| path.to_owned())
                    .filter(|path| unresolved(path) || self.protects(here, path));
            }
            "tar"
                if words.iter().any(|word| {
                    word.starts_with('x') || word.starts_with("-x") || word == "--extract"
                }) =>
            {
                // An unpacker writes wherever it runs.
                return self
                    .protects(here, ".")
                    .then(|| format!("{} ({name})", here.display()));
            }
            "unzip"
                if !words
                    .iter()
                    .any(|word| matches!(word.as_str(), "-l" | "-t" | "-v")) =>
            {
                // `unzip` extracts by default.
                return self
                    .protects(here, ".")
                    .then(|| format!("{} ({name})", here.display()));
            }
            "unrar"
                if words
                    .iter()
                    .any(|word| matches!(word.as_str(), "x" | "e" | "extract")) =>
            {
                return self
                    .protects(here, ".")
                    .then(|| format!("{} ({name})", here.display()));
            }
            "rustfmt" => operands.first().map(|path| (*path).to_owned()),
            "cargo" => {
                let under = cargo_dir(words)
                    .map(|dir| {
                        if unresolved(&dir.to_string_lossy()) {
                            self.shared.clone()
                        } else {
                            here.join(dir)
                        }
                    })
                    .unwrap_or_else(|| here.to_path_buf());
                let writes = matches!(
                    operands.first(),
                    Some(&"fmt" | &"fix" | &"clean" | &"add" | &"remove")
                );
                return (writes && self.protects(&under, "."))
                    .then(|| format!("{} (cargo {})", under.display(), operands[0]));
            }
            _ => {
                let (subcommand, rest, dir) = git_subcommand(words)?;
                let under = dir.map_or_else(
                    || here.to_path_buf(),
                    |dir| {
                        if unresolved(dir) {
                            self.shared.clone()
                        } else {
                            here.join(dir)
                        }
                    },
                );
                let writes = match subcommand {
                    "add" | "am" | "apply" | "checkout" | "cherry-pick" | "clean" | "commit"
                    | "merge" | "mv" | "pull" | "push" | "rebase" | "reset" | "restore"
                    | "revert" | "rm" | "switch" | "init" => true,
                    "stash" => !matches!(rest.first().map(String::as_str), Some("list" | "show")),
                    "worktree" => !matches!(rest.first().map(String::as_str), Some("list")),
                    // `tag`/`branch` write when a name or a flag creates,
                    // moves, or deletes one; listings carry only flags.
                    "tag" | "branch" => {
                        rest.iter().any(|word| !word.starts_with('-'))
                            || rest.iter().any(|word| {
                                matches!(
                                    word.as_str(),
                                    "-d" | "-D"
                                        | "-m"
                                        | "-M"
                                        | "-c"
                                        | "-C"
                                        | "-f"
                                        | "--force"
                                        | "-u"
                                        | "--set-upstream-to"
                                        | "--unset-upstream"
                                        | "--edit-description"
                                        | "--create-reflog"
                                ) || (subcommand == "tag"
                                    && matches!(
                                        word.as_str(),
                                        "-e" | "--edit" | "-a" | "--annotate" | "-s" | "--sign"
                                    ))
                            })
                    }
                    // `config` writes with a mutating flag or a `key value`
                    // pair; `config key` and `--get`/`-l` forms read.
                    "config" => {
                        rest.iter().filter(|word| !word.starts_with('-')).count() > 1
                            || rest.iter().any(|word| {
                                matches!(
                                    word.as_str(),
                                    "--add"
                                        | "--unset"
                                        | "--unset-all"
                                        | "--replace"
                                        | "--replace-all"
                                        | "-e"
                                        | "--edit"
                                        | "--remove-section"
                                        | "--rename-section"
                                        | "--set"
                                )
                            })
                    }
                    _ => false,
                };
                return (writes && self.protects(&under, "."))
                    .then(|| format!("{} (git {subcommand})", under.display()));
            }
        };
        candidate.filter(|path| unresolved(path) || self.protects(here, path))
    }
}

/// Whether a word still holds something the shell fills in when it runs
/// (`$NAME`, `${...}`, `$(...)`, backticks), so where it points can't be
/// told beforehand.
fn unresolved(word: &str) -> bool {
    // `%NAME%` is cmd's, filled in when the line is read (#11708).
    word.contains('$') || word.contains('`') || word.contains('%')
}

/// Whether a redirect target throws its output away rather than naming a
/// file: `/dev/null`, cmd's `nul`, PowerShell's `$null`.
fn discarded(target: &str) -> bool {
    let target = target.trim_matches(|c| c == '\'' || c == '"');
    target == "/dev/null"
        || target.eq_ignore_ascii_case("nul")
        || target.eq_ignore_ascii_case("$null")
}

/// A cmd switch (`/Y`, `/s`, `/MIR`, `/XD:x`), not a path.
fn cmd_switch(word: &str) -> bool {
    word.strip_prefix('/').is_some_and(|rest| {
        let name = rest.split(':').next().unwrap_or(rest);
        !name.is_empty()
            && name.len() <= 4
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '?')
            && !rest.contains(['/', '\\'])
    })
}

/// How a PowerShell file cmdlet names the path it writes, by the cmdlet's
/// lowercase name or alias: the parameters that name it, and whether,
/// unnamed, it follows a first operand that is only read (a copy's
/// source) rather than being any operand.
/// `None` for anything else. Aliases that are also Unix programs (`cp`,
/// `rm`, `tee`) are judged as those.
fn powershell_writer(name: &str) -> Option<(&'static [&'static str], bool)> {
    const PATH: &[&str] = &["-path", "-literalpath", "-filepath", "-lp"];
    const DESTINATION: &[&str] = &["-destination", "-destinationpath"];
    match name {
        "set-content" | "add-content" | "clear-content" | "out-file" | "new-item"
        | "remove-item" | "rename-item" | "set-item" | "tee-object" | "export-csv"
        | "export-clixml" | "sc" | "ac" | "clc" | "ni" | "ri" | "rni" | "si" => Some((PATH, false)),
        "copy-item" | "move-item" | "expand-archive" | "compress-archive" | "cpi" | "mi" => {
            Some((DESTINATION, true))
        }
        _ => None,
    }
}

/// `command` with `$NAME` and `${NAME}` filled in where the command itself
/// sets `NAME` once to a plain value (`F=/repo/x`, `export F="/repo/x"`),
/// or, for the home and temporary folders, from this process; cmd's
/// `%NAME%` and PowerShell's `$env:NAME` are filled in for those folders
/// too (#11708). Anything else is left as written, so [`unresolved`] still
/// sees it.
fn expand_variables(command: &str) -> String {
    let mut values: std::collections::HashMap<String, Option<String>> =
        std::collections::HashMap::new();
    let bytes = command.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let starts_word = index == 0
            || matches!(
                bytes[index - 1],
                b' ' | b'\t' | b'\n' | b';' | b'&' | b'|' | b'(' | b'{'
            );
        let name_len = command[index..]
            .bytes()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == b'_')
            .count();
        if starts_word
            && name_len > 0
            && !bytes[index].is_ascii_digit()
            && bytes.get(index + name_len) == Some(&b'=')
        {
            let name = command[index..index + name_len].to_owned();
            let (value, end) = assigned_value(&command[index + name_len + 1..]);
            let value = value.filter(|value| !unresolved(value));
            let entry = values.entry(name).or_insert_with(|| value.clone());
            if *entry != value {
                // Set twice to different things: which one a later use sees
                // depends on order, so it stays unknown.
                *entry = None;
            }
            index += name_len + 1 + end;
            continue;
        }
        index += command[index..].chars().next().map_or(1, char::len_utf8);
    }
    let lookup = |name: &str| -> Option<String> {
        match values.get(name) {
            Some(value) => value.clone(),
            None => process_folder(name),
        }
    };
    let mut out = String::with_capacity(command.len());
    let mut rest = command;
    while let Some(at) = rest.find(['$', '%']) {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        if rest[at..].starts_with('%') {
            // cmd fills `%NAME%` in before any `set` on the line runs, so
            // only this process's folders are known.
            let len = after
                .bytes()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == b'_')
                .count();
            match (after.as_bytes().get(len) == Some(&b'%'))
                .then(|| process_folder(&after[..len]))
                .flatten()
            {
                Some(value) => {
                    out.push_str(&value);
                    rest = &after[len + 1..];
                }
                None => {
                    out.push('%');
                    rest = after;
                }
            }
            continue;
        }
        if let Some(env) = after
            .get(..4)
            .filter(|head| head.eq_ignore_ascii_case("env:"))
            .map(|_| &after[4..])
        {
            let len = env
                .bytes()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == b'_')
                .count();
            match process_folder(&env[..len]) {
                Some(value) => {
                    out.push_str(&value);
                    rest = &env[len..];
                }
                None => {
                    out.push('$');
                    rest = after;
                }
            }
            continue;
        }
        let (name, used) = if let Some(braced) = after.strip_prefix('{') {
            match braced.find('}') {
                Some(close)
                    if braced[..close]
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_') =>
                {
                    (&braced[..close], close + 2)
                }
                _ => ("", 0),
            }
        } else {
            let len = after
                .bytes()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == b'_')
                .count();
            (&after[..len], len)
        };
        match (!name.is_empty()).then(|| lookup(name)).flatten() {
            Some(value) => out.push_str(&value),
            None => {
                out.push('$');
                out.push_str(&after[..used]);
            }
        }
        rest = &after[used..];
    }
    out.push_str(rest);
    out
}

/// The home or temporary folder a well-known variable names, as this
/// process sees it: `HOME`, `TMPDIR`, and Windows' `USERPROFILE`, `TEMP`
/// and `TMP`. On Windows, where `HOME` and `TMPDIR` are usually unset, a
/// POSIX shell (Git Bash) is given the user's folder and the temporary
/// folder for them (#11708).
fn process_folder(name: &str) -> Option<String> {
    let known = ["HOME", "TMPDIR", "USERPROFILE", "TEMP", "TMP"]
        .into_iter()
        .find(|known| known.eq_ignore_ascii_case(name))?;
    // Unix variable names are case-sensitive.
    if !cfg!(windows) && known != name {
        return None;
    }
    std::env::var(known).ok().or_else(|| {
        cfg!(windows).then_some(())?;
        match known {
            "HOME" => user_home::home_os().map(|home| PathBuf::from(home).display().to_string()),
            "TMPDIR" => Some(std::env::temp_dir().display().to_string()),
            _ => None,
        }
    })
}

/// The value an assignment's text starts with, unquoted, and how many
/// bytes it took; `None` when it is built from more than plain text.
fn assigned_value(text: &str) -> (Option<String>, usize) {
    let mut value = String::new();
    let mut chars = text.char_indices().peekable();
    let mut quote = None;
    while let Some(&(at, c)) = chars.peek() {
        match (quote, c) {
            (None, '\'' | '"') => quote = Some(c),
            (Some(open), c) if c == open => quote = None,
            (None, ' ' | '\t' | '\n' | ';' | '&' | '|' | ')') => return (Some(value), at),
            (None, '(') => return (None, at),
            _ => value.push(c),
        }
        chars.next();
    }
    (Some(value), text.len())
}

/// Why a harness's tool call is refused under the session's `boundary`,
/// or `None` when it may run: Edit, Write, MultiEdit and NotebookEdit under
/// the shared checkout, a patch that touches it, and a shell command that
/// [`crate::git_guard`] refuses or that writes there. `cwd` is where the
/// call runs. The Claude Code and Codex hooks call this (#11485), so the
/// fence Coder's own tools keep holds on those harnesses too.
#[must_use]
pub fn tool_refusal(
    boundary: Option<&Boundary>,
    tool: &str,
    input: &Value,
    cwd: &Path,
) -> Option<String> {
    match tool {
        "Bash" | "Run" | "Monitor" | "shell" | "exec_command" | "local_shell" => {
            let text = crate::floors::command_text(input)?;
            let here = input
                .get("workdir")
                .and_then(Value::as_str)
                .map_or_else(|| cwd.to_path_buf(), |dir| cwd.join(dir));
            // The command and each script a shell wrapper in it runs
            // (`bash -lc '...'`), so wrapping never hides a write.
            crate::floors::scripts(&text).iter().find_map(|script| {
                crate::git_guard::refusal(script).or_else(|| boundary?.run_refusal(script, &here))
            })
        }
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => {
            let path = input
                .get("file_path")
                .or_else(|| input.get("notebook_path"))
                .or_else(|| input.get("path"))
                .and_then(Value::as_str)?;
            boundary?.file_refusal(tool, path, cwd)
        }
        "apply_patch" => {
            let boundary = boundary?;
            let text = input
                .get("input")
                .or_else(|| input.get("patch"))
                .or_else(|| input.get("command"))
                .and_then(Value::as_str)?;
            text.lines()
                .filter_map(|line| {
                    [
                        "*** Update File: ",
                        "*** Add File: ",
                        "*** Delete File: ",
                        "*** Move to: ",
                    ]
                    .iter()
                    .find_map(|head| line.strip_prefix(head))
                })
                .find_map(|path| boundary.file_refusal("Edit", path.trim(), cwd))
        }
        _ => None,
    }
}

/// The fence as a Coder SDK hook (#11485) for a session in `cwd`: on
/// `PreToolUse`, a call [`tool_refusal`] refuses is denied with its reason.
/// Under bypass nothing else stops Claude Code's own Edit, Write and Bash
/// from writing the shared checkout. The boundary is the Coder worktree
/// `cwd` is in ([`Boundary::around`]); with none, only [`crate::git_guard`]
/// judges.
#[must_use]
pub fn hook(cwd: PathBuf) -> Hook {
    let boundary = Boundary::around(&cwd);
    hook_on(cwd, boundary)
}

/// [`hook`] with the session's `boundary` given, for a session whose fence
/// isn't the Coder worktree it runs in (an issue run's, whose shared side
/// is the whole disk); `None` leaves only [`crate::git_guard`].
#[must_use]
pub fn hook_on(cwd: PathBuf, boundary: Option<Boundary>) -> Hook {
    Hook::floor(
        "worktree-fence",
        Event::PreToolUse,
        Permission::deny(
            "Refused: Coder could not check whether this writes to the shared checkout. Work in your own worktree.",
        ),
        move |call: &Call| {
            let tool = call.tool.as_deref().unwrap_or_default();
            Ok(
                match tool_refusal(boundary.as_ref(), tool, &call.input, &cwd) {
                    Some(reason) => HookOutcome::deny(&reason),
                    None => HookOutcome::none(),
                },
            )
        },
    )
}

/// The program a simple command runs, past `VAR=value` assignments and
/// the wrappers that run another program.
fn program(words: &[String]) -> Option<String> {
    let word = words.iter().find(|word| {
        let assignment = word.split_once('=').is_some_and(|(name, _)| {
            !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        });
        !assignment
            && !matches!(
                word.as_str(),
                "sudo" | "env" | "time" | "nohup" | "command" | "exec" | "&"
            )
    })?;
    Some(crate::floors::program_name(word))
}

/// Whether `program` (with its `words`) only reads where it runs. Git and
/// cargo are judged by their subcommands in [`Boundary::written_path`].
fn reads_only(program: &str, words: &[String]) -> bool {
    match program {
        "find" => !words.iter().any(|word| {
            matches!(
                word.as_str(),
                "-delete"
                    | "-exec"
                    | "-execdir"
                    | "-ok"
                    | "-okdir"
                    | "-fprint"
                    | "-fprint0"
                    | "-fprintf"
                    | "-fls"
            )
        }),
        "git" | "cargo" | "cd" | "pushd" | "popd" | "ls" | "cat" | "head" | "tail" | "wc"
        | "rg" | "grep" | "egrep" | "fgrep" | "fd" | "file" | "stat" | "du" | "df" | "tree"
        | "less" | "more" | "bat" | "diff" | "cmp" | "echo" | "printf" | "pwd" | "true"
        | "false" | "test" | "[" | "which" | "type" | "sort" | "uniq" | "cut" | "tr" | "jq"
        | "xxd" | "od" | "nl" | "column" | "basename" | "dirname" | "realpath" | "readlink"
        | "date" | "sleep" | "exit" | "sed" | "awk" | "gh" | "shasum" | "sha256sum" | "md5"
        | "md5sum"
        // Windows readers (#11708): cmd's, and PowerShell's by name and
        // alias.
        | "dir" | "findstr" | "where" | "get-content" | "gc" | "get-childitem"
        | "gci" | "select-string" | "sls" | "get-item" | "gi" | "test-path" | "resolve-path"
        | "get-location" | "gl" | "write-output" | "write-host" | "get-filehash"
        | "set-location" | "chdir" | "measure-object" | "select-object" | "sort-object"
        | "where-object" | "format-list" | "format-table" | "out-string" | "out-host" => true,
        _ => false,
    }
}

/// Whether `program` runs a script it is given.
fn interpreter(program: &str) -> bool {
    program.starts_with("python")
        || matches!(
            program,
            "node"
                | "nodejs"
                | "perl"
                | "ruby"
                | "php"
                | "bash"
                | "sh"
                | "zsh"
                | "dash"
                | "fish"
                | "deno"
                | "bun"
                | "osascript"
                | "lua"
                | "eval"
                | "cmd"
                | "powershell"
                | "pwsh"
                | "cscript"
                | "wscript"
        )
}

/// Every path-looking run of characters in `command`, script bodies
/// included: one that starts with `/`, `~/`, `./` or `../`, or on Windows
/// a drive (`C:\\x`, `C:/x`) or `.\\`, `..\\`, `~\\` (#11708).
fn named_paths(command: &str) -> impl Iterator<Item = &str> {
    command
        .split(|c: char| {
            !(c.is_alphanumeric()
                || matches!(
                    c,
                    '/' | '.' | '_' | '-' | '~' | '+' | '@' | '%' | '\\' | ':'
                ))
        })
        .flat_map(|word| {
            // A drive starts a path even after other text (`f=C:\\x`).
            let drive = word.char_indices().find_map(|(at, c)| {
                let rest = &word[at..];
                (c.is_ascii_alphabetic()
                    && (at == 0 || !word.as_bytes()[at - 1].is_ascii_alphanumeric())
                    && (rest[1..].starts_with(":\\") || rest[1..].starts_with(":/")))
                .then_some(rest)
            });
            // Elsewhere `:` parts paths from what's before them
            // (`file:/x`, `host:/x`).
            word.split(':').chain(drive)
        })
        .filter(|word| {
            word.starts_with('/')
                || word.starts_with("~/")
                || word.starts_with("./")
                || word.starts_with("../")
                || word.starts_with(".\\")
                || word.starts_with("..\\")
                || word.starts_with("~\\")
                || (word.len() > 2
                    && word.as_bytes()[0].is_ascii_alphabetic()
                    && (word[1..].starts_with(":\\") || word[1..].starts_with(":/")))
        })
}

fn cargo_dir(words: &[String]) -> Option<PathBuf> {
    words
        .iter()
        .position(|word| word == "--manifest-path")
        .and_then(|index| words.get(index + 1))
        .map(|manifest| {
            Path::new(manifest)
                .parent()
                .unwrap_or(Path::new("."))
                .to_path_buf()
        })
}

/// `git`'s subcommand, the words after it, and its `-C dir` when given.
fn git_subcommand(words: &[String]) -> Option<(&str, &[String], Option<&str>)> {
    let start = words
        .iter()
        .position(|word| crate::floors::program_name(word) == "git")?;
    let mut index = start + 1;
    let mut dir = None;
    while let Some(word) = words.get(index) {
        match word.as_str() {
            "-C" | "--work-tree" => {
                dir = words.get(index + 1).map(String::as_str);
                index += 2;
            }
            "-c" | "--git-dir" | "--namespace" => index += 2,
            option if option.starts_with('-') => index += 1,
            subcommand => return Some((subcommand, &words[index + 1..], dir)),
        }
    }
    None
}

/// The command split at `;`, `&&`, `||`, `|`, newlines, and a lone `&`
/// (cmd's separator, a POSIX shell's background), as shell words.
fn segments(command: &str) -> impl Iterator<Item = Vec<String>> + '_ {
    command
        .split(|c| matches!(c, ';' | '\n' | '|' | '(' | ')' | '{' | '}'))
        .flat_map(|part| part.split("&&"))
        .flat_map(|part| part.split(" & "))
        .map(|part| {
            part.split_whitespace()
                // A comment (an unquoted `#` starting a word) runs to the end
                // of its line.
                .take_while(|word| !word.starts_with('#'))
                .map(|word| word.trim_matches(|c| c == '\'' || c == '"').to_owned())
                // A shell keyword (`do sed -i ...`) leads into the command.
                .skip_while(|word| {
                    matches!(
                        word.as_str(),
                        "do" | "then" | "else" | "elif" | "if" | "while" | "until" | "!"
                    )
                })
                .collect::<Vec<_>>()
        })
        .filter(|words| !words.is_empty())
}

/// `path` with `.` and `..` folded away, without touching the file system,
/// so a path that does not exist yet still compares.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            // `\\?\C:\x` and `C:\x` are the same place (#11695).
            Component::Prefix(prefix) => match prefix.kind() {
                std::path::Prefix::VerbatimDisk(drive) | std::path::Prefix::Disk(drive) => {
                    out.push(format!("{}:", char::from(drive).to_ascii_uppercase()));
                }
                _ => out.push(component),
            },
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::Boundary;
    use std::path::{Path, PathBuf};

    fn boundary() -> Boundary {
        Boundary {
            worktree: PathBuf::from("/repo/.coder/worktrees/chat-abc"),
            shared: PathBuf::from("/repo"),
            branch: "chat/abc".into(),
        }
    }

    #[test]
    fn edits_under_the_shared_checkout_are_refused_and_the_worktree_is_free() {
        let b = boundary();
        let wt = Path::new("/repo/.coder/worktrees/chat-abc");
        assert!(
            b.file_refusal("Edit", "/repo/crates/x/src/lib.rs", wt)
                .is_some()
        );
        assert!(b.file_refusal("Write", "../../../README.md", wt).is_some());
        assert!(
            b.file_refusal("Read", "/repo/crates/x/src/lib.rs", wt)
                .is_none()
        );
        assert!(b.file_refusal("Edit", "crates/x/src/lib.rs", wt).is_none());
        assert!(
            b.file_refusal("Edit", "/repo/.coder/worktrees/chat-abc/x.rs", wt)
                .is_none()
        );
        assert!(b.file_refusal("Write", "/elsewhere/notes.md", wt).is_none());
    }

    #[test]
    fn commands_that_write_under_the_shared_checkout_are_refused() {
        let b = boundary();
        let wt = Path::new("/repo/.coder/worktrees/chat-abc");
        for command in [
            "git show origin/main:ui.rs > /repo/crates/coder-new/src/ui.rs",
            "cat x >> /repo/notes.md",
            "cp src/lib.rs /repo/src/lib.rs",
            "mv src/lib.rs /repo/src/lib.rs",
            "sed -i 's/a/b/' /repo/src/lib.rs",
            "cd /repo && cargo fmt",
            "cargo fmt --manifest-path /repo/Cargo.toml",
            "git -C /repo merge --ff-only origin/main",
            "git -C /repo push origin HEAD:main",
            "git -C /repo tag v1",
            "git -C /repo branch scratch",
            "git -C /repo config user.email a@b.c",
            "git --work-tree /repo --git-dir /repo/.git checkout -- x.rs",
            "cd /repo && git checkout main",
            "cd /repo; git stash",
            "tee /repo/out.txt",
            "dd if=/dev/zero of=/repo/out",
            "cd /repo && tar xf bundle.tar",
            "cd /repo && unzip bundle.zip",
            "cd /repo/crates && rm src/lib.rs",
            // #11485: scripts that name the shared checkout, and anything
            // but reading run from inside it.
            "python3 - <<'EOF'\np = open('/repo/crates/x/src/lib.rs', 'w')\np.write('x')\nEOF",
            "node -e \"require('fs').writeFileSync('/repo/notes.md', 'x')\"",
            "bash -c 'echo x > ../../../notes.md'",
            "cd /repo && python3 fix.py",
            "cd /repo && npm install",
            "cd /repo && make",
            "cd /repo && find . -name '*.orig' -delete",
        ] {
            assert!(b.run_refusal(command, wt).is_some(), "{command}");
        }
    }

    #[test]
    fn reads_of_the_shared_checkout_and_work_in_the_worktree_run() {
        let b = boundary();
        let wt = Path::new("/repo/.coder/worktrees/chat-abc");
        for command in [
            "cat /repo/README.md",
            "git -C /repo diff",
            "git -C /repo stash list",
            "git -C /repo tag -l",
            "git -C /repo branch --show-current",
            "git -C /repo branch -a",
            "cd /repo && git status; git branch -a --list",
            "# look around\ncd /repo && git log -3",
            "git -C /repo fetch origin",
            "git -C /repo config --get user.name",
            "cd /repo && git status",
            "cargo fmt",
            "cargo test -p coder-new > out.txt",
            "git commit -am 'x' && git push origin HEAD:main",
            "git fetch origin && git rebase origin/main",
            "git tag v1 && git push origin v1",
            "rg pattern /repo/crates",
            "cp /repo/README.md notes.md",
            "git show origin/main:ui.rs > src/ui.rs",
            "cd /repo && ls crates && rg foo && git log -3 --oneline",
            "cd /repo && cargo test -p coder-new",
            "python3 - <<'EOF'\nopen('src/lib.rs', 'w').write('x')\nEOF",
            "python3 -c 'print(1)'",
            "python3 /repo/.coder/worktrees/chat-abc/tool.py",
        ] {
            assert!(b.run_refusal(command, wt).is_none(), "{command}");
        }
    }

    #[test]
    fn writes_through_shell_variables_are_read_through_or_refused() {
        let b = boundary();
        let wt = Path::new("/repo/.coder/worktrees/chat-abc");
        // #11522: the path in a variable, or one that can't be worked out
        // before the command runs.
        for command in [
            "F=/repo/calc/__init__.py; echo '# touched' >> \"$F\"; git -C /repo status",
            "F=/repo/x; echo >> \"$F\"",
            "F=/repo/x && echo >> ${F}",
            "echo >> ${F}",
            "echo > \"$(dirname /repo/calc/x.py)/x\"",
            "echo > `pwd`/x",
            "export D=\"/repo\"; cp a.rs $D/a.rs",
            "D=/repo; git -C \"$D\" commit -am x",
            "cd \"$SOMEWHERE\" && make",
            "for f in *.rs; do sed -i 's/a/b/' \"$f\"; done",
            "F=/repo/x; python3 -c \"open('$F','w').write('x')\"",
            "tee \"$OUT\" < x",
        ] {
            assert!(b.run_refusal(command, wt).is_some(), "{command}");
        }
        for command in [
            "F=/repo/.coder/worktrees/chat-abc/x; echo >> \"$F\"",
            "F=src/x.rs; echo >> \"$F\" && cat \"$F\"",
            "cargo test 2>&1 | tail -40",
            "cd /repo && git status 2>&1",
            "echo \"$HOME\" && cat \"$F\"",
            "cargo test > \"$TMPDIR/out.txt\"",
        ] {
            assert!(b.run_refusal(command, wt).is_none(), "{command}");
        }
        // Through a shell wrapper and a harness's Bash too.
        assert!(
            super::tool_refusal(
                Some(&b),
                "Bash",
                &serde_json::json!({"command": "bash -lc 'F=/repo/x; echo >> \"$F\"'"}),
                wt
            )
            .is_some()
        );
    }

    /// Windows shells name the same writes their own way (#11708): cmd's
    /// `%VAR%`, `&` and `copy`/`del`, PowerShell's `$env:VAR`, `*>` and
    /// file cmdlets, and the `cmd /c` and `powershell -Command` wrappers
    /// Codex on Windows runs every command through.
    #[test]
    fn windows_shells_are_held_to_the_same_fence() {
        let b = boundary();
        let wt = Path::new("/repo/.coder/worktrees/chat-abc");
        for command in [
            "echo hi > %F%",
            "set F=/repo/x & echo hi > %F%",
            "$env:F = '/repo/x'; Set-Content -Path $env:F -Value hi",
            "Set-Content -Path /repo/x -Value hi",
            "Add-Content /repo/notes.md 'x'",
            "'x' | Out-File -FilePath /repo/out.txt",
            "New-Item -ItemType File -Path /repo/x.rs",
            "Remove-Item -Recurse -Force /repo/target",
            "Copy-Item a.rs /repo/a.rs",
            "Copy-Item -Path a.rs -Destination /repo/a.rs",
            "Move-Item a.rs /repo/a.rs",
            "cargo test *> /repo/out.txt",
            "copy a.rs /repo/a.rs",
            "cd /repo & del x.rs",
            "cd /repo & cargo fmt",
            "git.exe -C /repo push origin HEAD:main",
            "C:\\Git\\bin\\git.exe -C /repo commit -am x",
            "Set-Location /repo; New-Item x.rs",
        ] {
            assert!(b.run_refusal(command, wt).is_some(), "{command}");
        }
        for command in [
            "cargo test > $null",
            "cargo test 2>nul",
            "cargo test > out.txt 2>&1",
            "Copy-Item /repo/a.rs a.rs",
            "Get-Content /repo/README.md",
            "cd /repo; Get-ChildItem; git status",
            "Set-Content -Path src/x.rs -Value hi",
            "printf '%s' x > out.txt",
        ] {
            assert!(b.run_refusal(command, wt).is_none(), "{command}");
        }
        for command in [
            serde_json::json!(["cmd.exe", "/c", "copy a.rs /repo/a.rs"]),
            serde_json::json!([
                "powershell.exe",
                "-NoProfile",
                "-Command",
                "Remove-Item /repo/x"
            ]),
            serde_json::json!([
                "powershell",
                "-ExecutionPolicy",
                "Bypass",
                "Set-Content /repo/x hi"
            ]),
            serde_json::json!("pwsh -c 'Set-Content -Path /repo/x -Value hi'"),
            serde_json::json!([
                "powershell.exe",
                "-Command",
                "python -c \"open('/repo/x','w')\""
            ]),
        ] {
            assert!(
                super::tool_refusal(
                    Some(&b),
                    "shell",
                    &serde_json::json!({"command": command}),
                    wt
                )
                .is_some(),
                "{command}"
            );
        }
        assert!(
            super::tool_refusal(
                Some(&b),
                "shell",
                &serde_json::json!({"command": ["powershell.exe", "-Command", "cargo test -p x"]}),
                wt
            )
            .is_none()
        );
    }

    /// PowerShell Coder can't read (an encoded command, a script file) is
    /// refused with or without a fence, in string and argv forms; readable
    /// PowerShell is read through as before (#11708).
    #[test]
    fn powershell_it_cant_read_is_refused() {
        let b = boundary();
        let wt = Path::new("/repo/.coder/worktrees/chat-abc");
        for command in crate::floors::tests::unreadable_powershell() {
            let input = serde_json::json!({"command": command});
            for fence in [Some(&b), None] {
                assert_eq!(
                    super::tool_refusal(fence, "shell", &input, wt).as_deref(),
                    Some(crate::floors::UNREADABLE_REFUSAL),
                    "{command}"
                );
            }
            let text = crate::floors::command_text(&input).unwrap();
            assert!(b.run_refusal(&text, wt).is_some(), "{command}");
        }
        for command in crate::floors::tests::readable_powershell() {
            let input = serde_json::json!({"command": command});
            assert!(
                super::tool_refusal(Some(&b), "shell", &input, wt).is_none(),
                "{command}"
            );
        }
    }

    /// Drive paths on Windows: in redirects, cmdlets and scripts (#11708).
    #[cfg(windows)]
    #[test]
    fn windows_drive_paths_are_fenced() {
        let b = Boundary {
            worktree: PathBuf::from("C:\\repo\\.coder\\worktrees\\chat-abc"),
            shared: PathBuf::from("C:\\repo"),
            branch: "chat/abc".into(),
        };
        let wt = Path::new("C:\\repo\\.coder\\worktrees\\chat-abc");
        for command in [
            "echo hi > C:\\repo\\x",
            "echo hi > C:/repo/x",
            "Set-Content C:\\repo\\x hi",
            "python -c \"open(r'C:\\repo\\x','w')\"",
            "cd /d C:\\repo & del /q x.rs",
            "rd /s /q C:\\repo\\target",
        ] {
            assert!(b.run_refusal(command, wt).is_some(), "{command}");
        }
        for command in [
            "echo hi > C:\\repo\\.coder\\worktrees\\chat-abc\\x",
            "cargo test > \"$TMPDIR/out.txt\"",
            "cargo test > %TEMP%\\out.txt",
            "cargo test > $env:TEMP\\out.txt",
            "copy /Y a.rs b.rs",
        ] {
            assert!(b.run_refusal(command, wt).is_none(), "{command}");
        }
    }

    /// #11720 (owner): a chat's fence covers the shared checkout only. The
    /// rc.20 check's writes: one outside the repository stays free, on
    /// purpose; one into the shared checkout is refused.
    #[cfg(windows)]
    #[test]
    fn a_chat_fence_is_the_shared_checkout_only() {
        let b = Boundary {
            worktree: PathBuf::from("C:\\repo\\.coder\\worktrees\\chat-abc"),
            shared: PathBuf::from("C:\\repo"),
            branch: "chat/abc".into(),
        };
        let wt = Path::new("C:\\repo\\.coder\\worktrees\\chat-abc");
        assert!(
            b.run_refusal("cmd /c \"echo x > %TEMP%\\..\\..\\outside.txt\"", wt)
                .is_none()
        );
        assert!(
            b.file_refusal("Write", "D:\\elsewhere\\notes.txt", wt)
                .is_none()
        );
        let refused = b
            .run_refusal("cmd /c \"echo x > %CD%\\..\\..\\..\\shared.txt\"", wt)
            .expect("a write into the shared checkout is refused");
        assert!(refused.starts_with("Refused:"), "{refused}");
        assert!(
            b.run_refusal("cmd /c \"echo x > C:\\repo\\shared.txt\"", wt)
                .is_some()
        );
    }

    #[test]
    fn a_coder_worktree_implies_its_boundary() {
        let b = Boundary::around(Path::new("/repo/.coder/worktrees/chat-abc")).unwrap();
        assert_eq!(b, boundary());
        let inner =
            Boundary::around(Path::new("/repo/.coder/worktrees/chat-abc/crates/x")).unwrap();
        assert_eq!(inner, boundary());
        assert!(Boundary::around(Path::new("/repo")).is_none());
        assert!(Boundary::around(Path::new("/repo/.coder")).is_none());
    }

    #[test]
    fn harness_tool_calls_are_judged_by_the_same_fence() {
        use super::tool_refusal;
        use serde_json::json;
        let b = boundary();
        let wt = Path::new("/repo/.coder/worktrees/chat-abc");
        for (tool, input) in [
            (
                "Edit",
                json!({"file_path": "/repo/src/lib.rs", "old_string": "a", "new_string": "b"}),
            ),
            (
                "Write",
                json!({"file_path": "/repo/README.md", "content": "x"}),
            ),
            (
                "MultiEdit",
                json!({"file_path": "/repo/src/lib.rs", "edits": []}),
            ),
            (
                "NotebookEdit",
                json!({"notebook_path": "/repo/n.ipynb", "new_source": ""}),
            ),
            (
                "Bash",
                json!({"command": "python3 - <<'EOF'\nopen('/repo/src/lib.rs','w').write('')\nEOF"}),
            ),
            ("Bash", json!({"command": "cat x > /repo/notes.md"})),
            (
                "shell",
                json!({"command": ["bash", "-lc", "cd /repo && cargo fmt"]}),
            ),
            (
                "apply_patch",
                json!({"input": "*** Begin Patch\n*** Update File: /repo/src/lib.rs\n@@\n-a\n+b\n*** End Patch"}),
            ),
        ] {
            assert!(
                tool_refusal(Some(&b), tool, &input, wt).is_some(),
                "{tool} {input}"
            );
        }
        // A shell wrapper never hides a write or a discard (Codex sends
        // `["bash", "-lc", "..."]`).
        for (tool, input) in [
            (
                "shell",
                json!({"command": ["bash", "-lc", "rm src/x.rs"], "workdir": "/repo"}),
            ),
            (
                "shell",
                json!({"command": ["bash", "-lc", "touch /repo/notes.md"]}),
            ),
            ("Bash", json!({"command": "sh -c 'cp a /repo/b'"})),
            (
                "Bash",
                json!({"command": "env A=1 zsh -c \"git -C /repo commit -am x\""}),
            ),
            ("shell", json!({"command": ["bash", "-lc", "git stash"]})),
            (
                "Bash",
                json!({"command": "bash -lc \"sh -c 'git reset --hard'\""}),
            ),
        ] {
            assert!(
                tool_refusal(Some(&b), tool, &input, wt).is_some(),
                "{tool} {input}"
            );
        }
        assert!(
            tool_refusal(
                Some(&b),
                "shell",
                &json!({"command": ["bash", "-lc", "cargo test -p x"]}),
                wt
            )
            .is_none()
        );
        // Reading the shared checkout through Codex's wrapper is fine; a
        // script inside it that names the checkout is not.
        assert!(
            tool_refusal(
                Some(&b),
                "Bash",
                &json!({"command": "/bin/zsh -lc 'cat /repo/README.md'"}),
                wt
            )
            .is_none()
        );
        assert!(
            tool_refusal(
                Some(&b),
                "Bash",
                &json!({"command": "/bin/zsh -lc \"python3 -c 'open(\\\"/repo/x\\\", \\\"w\\\")'\""}),
                wt
            )
            .is_some()
        );
        // The shell the agent left in the shared checkout counts as there.
        assert!(
            tool_refusal(
                Some(&b),
                "Bash",
                &json!({"command": "sed 's/a/b/' x > y"}),
                Path::new("/repo")
            )
            .is_some()
        );
        // Stashing or discarding work is refused with or without a worktree.
        assert!(
            tool_refusal(
                None,
                "Bash",
                &json!({"command": "git stash"}),
                Path::new("/repo")
            )
            .is_some()
        );
        for (tool, input) in [
            ("Edit", json!({"file_path": "src/lib.rs"})),
            (
                "Write",
                json!({"file_path": "/repo/.coder/worktrees/chat-abc/x.rs"}),
            ),
            ("Read", json!({"file_path": "/repo/src/lib.rs"})),
            (
                "Bash",
                json!({"command": "cargo test -p x && git commit -am x && git push origin HEAD:main"}),
            ),
            ("Bash", json!({"command": "cat /repo/README.md"})),
        ] {
            assert!(
                tool_refusal(Some(&b), tool, &input, wt).is_none(),
                "{tool} {input}"
            );
        }
        assert!(
            tool_refusal(
                None,
                "Edit",
                &json!({"file_path": "/repo/src/lib.rs"}),
                Path::new("/repo")
            )
            .is_none()
        );
    }

    /// #11695: a task's fence (`/` as the shared side) refuses writes on
    /// any Windows drive outside the session's folder, and reads a plain
    /// `C:\` path and its `\\?\C:\` form as the same place.
    #[cfg(windows)]
    #[test]
    fn a_task_fence_covers_every_windows_drive() {
        let b = Boundary {
            worktree: PathBuf::from(r"\\?\C:\work\.coder\worktrees\task"),
            shared: PathBuf::from("/"),
            branch: String::new(),
        };
        let cwd = Path::new(r"\\?\C:\work\.coder\worktrees\task");
        for outside in [
            r"C:\Windows\hosts",
            r"D:\elsewhere\x.txt",
            "/etc/hosts",
            r"..\..\x.rs",
        ] {
            assert!(b.file_refusal("Write", outside, cwd).is_some(), "{outside}");
        }
        for inside in [
            "src/lib.rs",
            r"C:\work\.coder\worktrees\task\src\lib.rs",
            r"\\?\C:\work\.coder\worktrees\task\a.rs",
        ] {
            assert!(b.file_refusal("Write", inside, cwd).is_none(), "{inside}");
        }
    }
}
