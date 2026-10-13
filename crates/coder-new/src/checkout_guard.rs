//! A chat session's private worktree and the shared checkout it came from
//! (#11372). Several sessions in one repository used to edit the launcher's
//! checkout at once; now each works in `.coder/worktrees/chat-<session>` on
//! `chat/<session>`, and the checkout it was started from is read-only for
//! it: Edit and Write refuse paths under it, and Run refuses commands that
//! write there. Reads stay free.

use std::path::{Component, Path, PathBuf};

use serde_json::Value;

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
            .zip(std::env::var_os("HOME"))
            .map(|(rest, home)| PathBuf::from(home).join(rest));
        let joined = cwd.join(home.as_deref().unwrap_or_else(|| Path::new(path)));
        let normal = normalize(&joined);
        let shared = normalize(&self.shared);
        let own = normalize(&self.worktree);
        normal.starts_with(&shared) && !normal.starts_with(&own)
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
                .position(|word| word == "cd")
                .and_then(|index| words.get(index + 1))
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
            let trimmed = word.trim_start_matches(|c: char| c.is_ascii_digit() || c == '&');
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
            if let Some(target) = target
                && (unresolved(target) || self.protects(here, target))
            {
                return Some(target.to_owned());
            }
        }
        let Some(program) = words.first() else {
            return None;
        };
        let name = program.rsplit('/').next().unwrap_or(program);
        let operands = words[1..]
            .iter()
            .filter(|word| !word.starts_with('-'))
            .map(String::as_str)
            .collect::<Vec<_>>();
        let candidate = match name {
            "cp" | "mv" | "ln" | "install" | "rsync" => {
                operands.last().map(|path| (*path).to_owned())
            }
            "tee" | "truncate" | "touch" | "rm" | "rmdir" | "mkdir" | "patch" | "chmod"
            | "chown" | "chgrp" | "unlink" | "mkfifo" => {
                operands.first().map(|path| (*path).to_owned())
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
                                        | "-e"
                                        | "--edit"
                                        | "-a"
                                        | "--annotate"
                                        | "-s"
                                        | "--sign"
                                        | "--create-reflog"
                                )
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
    word.contains('$') || word.contains('`')
}

/// `command` with `$NAME` and `${NAME}` filled in where the command itself
/// sets `NAME` once to a plain value (`F=/repo/x`, `export F="/repo/x"`),
/// or, for `HOME` and `TMPDIR`, from this process. Anything else is left as
/// written, so [`unresolved`] still sees it.
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
            None if matches!(name, "HOME" | "TMPDIR") => std::env::var(name).ok(),
            None => None,
        }
    };
    let mut out = String::with_capacity(command.len());
    let mut rest = command;
    while let Some(at) = rest.find('$') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
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
pub fn sdk_hook(cwd: PathBuf) -> coder_sdk::Hook {
    let boundary = Boundary::around(&cwd);
    coder_sdk::Hook::floor(
        "worktree-fence",
        coder_sdk::Event::PreToolUse,
        coder_sdk::Permission::deny(
            "Refused: Coder could not check whether this writes to the shared checkout. Work in your own worktree.",
        ),
        move |call: &coder_sdk::Call| {
            let tool = call.tool.as_deref().unwrap_or_default();
            Ok(
                match tool_refusal(boundary.as_ref(), tool, &call.input, &cwd) {
                    Some(reason) => coder_sdk::HookOutcome::deny(&reason),
                    None => coder_sdk::HookOutcome::none(),
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
                "sudo" | "env" | "time" | "nohup" | "command" | "exec"
            )
    })?;
    Some(word.rsplit('/').next().unwrap_or(word).to_owned())
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
        | "md5sum" => true,
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
        )
}

/// Every path-looking run of characters in `command`, script bodies
/// included: one that starts with `/`, `~/`, `./` or `../`.
fn named_paths(command: &str) -> impl Iterator<Item = &str> {
    command
        .split(|c: char| {
            !(c.is_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | '~' | '+' | '@' | '%'))
        })
        .filter(|word| {
            word.starts_with('/')
                || word.starts_with("~/")
                || word.starts_with("./")
                || word.starts_with("../")
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
        .position(|word| word == "git" || word.ends_with("/git"))?;
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

/// The command split at `;`, `&&`, `||`, `|`, and newlines, as shell words.
fn segments(command: &str) -> impl Iterator<Item = Vec<String>> + '_ {
    command
        .split(|c| matches!(c, ';' | '\n' | '|' | '(' | ')' | '{' | '}'))
        .flat_map(|part| part.split("&&"))
        .map(|part| {
            part.split_whitespace()
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
}
