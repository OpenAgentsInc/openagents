//! A chat session's private worktree and the shared checkout it came from
//! (#11372). Several sessions in one repository used to edit the launcher's
//! checkout at once; now each works in `.coder/worktrees/chat-<session>` on
//! `chat/<session>`, and the checkout it was started from is read-only for
//! it: Edit and Write refuse paths under it, and Run refuses commands that
//! write there. Reads stay free.

use std::path::{Component, Path, PathBuf};

/// Guidance for a session that works in its own worktree.
pub const INSTRUCTIONS: &str = "You work in your own git worktree on your own branch; the folder you were started from is another checkout that other sessions and the user may be editing, and it is read-only for you: Edit, Write and commands that write under it are refused. To land, commit on this branch and `git push origin HEAD:main`; if the push is rejected, `git fetch origin && git rebase origin/main` and push again. Never split hunks: this checkout has no one else's changes.\n";

/// The worktree a session works in and the checkout it must not write to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Boundary {
    /// The session's own worktree.
    pub worktree: PathBuf,
    /// The checkout the session was started from, read-only for it.
    pub shared: PathBuf,
}

impl Boundary {
    /// Whether a path, as the tool or command names it from `cwd`, lands
    /// under the shared checkout and outside the session's own worktree.
    #[must_use]
    pub fn protects(&self, cwd: &Path, path: &str) -> bool {
        let path = path.trim_matches(|c| c == '\'' || c == '"');
        if path.is_empty() {
            return false;
        }
        let joined = if path.starts_with('~') {
            return false;
        } else {
            cwd.join(path)
        };
        let normal = normalize(&joined);
        let shared = normalize(&self.shared);
        let own = normalize(&self.worktree);
        normal.starts_with(&shared) && !normal.starts_with(&own)
    }

    /// Why a file tool call is refused, or `None` when it may run.
    #[must_use]
    pub fn file_refusal(&self, tool: &str, path: &str, cwd: &Path) -> Option<String> {
        (matches!(tool, "Edit" | "Write") && self.protects(cwd, path)).then(|| {
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
        let mut here = cwd.to_path_buf();
        for words in segments(command) {
            if let Some(target) = words
                .iter()
                .position(|word| word == "cd")
                .and_then(|index| words.get(index + 1))
            {
                here = here.join(target);
            }
            if let Some(path) = self.written_path(&words, &here) {
                return Some(format!(
                    "Refused: this command writes to {path}, which is in the shared checkout that other sessions and the user may be editing. Run it in your own worktree ({}) instead.",
                    self.worktree.display()
                ));
            }
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
                (!rest.is_empty()).then_some(rest)
            } else {
                None
            };
            if let Some(target) = target
                && self.protects(here, target)
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
            "cp" | "mv" | "ln" | "install" => operands.last().map(|path| (*path).to_owned()),
            "tee" | "truncate" | "touch" | "rm" | "rmdir" | "mkdir" | "patch" | "chmod" => {
                operands.first().map(|path| (*path).to_owned())
            }
            "sed" | "perl" if words.iter().any(|word| word.starts_with("-i")) => {
                operands.last().map(|path| (*path).to_owned())
            }
            "rustfmt" => operands.first().map(|path| (*path).to_owned()),
            "cargo" => {
                let under = cargo_dir(words).unwrap_or_else(|| here.to_path_buf());
                let writes = matches!(
                    operands.first(),
                    Some(&"fmt" | &"fix" | &"clean" | &"add" | &"remove")
                );
                return (writes && self.protects(&under, "."))
                    .then(|| format!("{} (cargo {})", under.display(), operands[0]));
            }
            _ => {
                let (subcommand, rest, dir) = git_subcommand(words)?;
                let under = dir.map_or_else(|| here.to_path_buf(), |dir| here.join(dir));
                let writes = matches!(
                    subcommand,
                    "add"
                        | "am"
                        | "apply"
                        | "checkout"
                        | "cherry-pick"
                        | "clean"
                        | "commit"
                        | "merge"
                        | "mv"
                        | "pull"
                        | "rebase"
                        | "reset"
                        | "restore"
                        | "revert"
                        | "rm"
                        | "stash"
                        | "switch"
                        | "worktree"
                ) && !(subcommand == "stash"
                    && matches!(rest.first().map(String::as_str), Some("list" | "show")))
                    && !(subcommand == "worktree"
                        && matches!(rest.first().map(String::as_str), Some("list")));
                return (writes && self.protects(&under, "."))
                    .then(|| format!("{} (git {subcommand})", under.display()));
            }
        };
        candidate.filter(|path| self.protects(here, path))
    }
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
            "-C" => {
                dir = words.get(index + 1).map(String::as_str);
                index += 2;
            }
            "-c" | "--git-dir" | "--work-tree" | "--namespace" => index += 2,
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
            "sed -i 's/a/b/' /repo/src/lib.rs",
            "cd /repo && cargo fmt",
            "cargo fmt --manifest-path /repo/Cargo.toml",
            "git -C /repo merge --ff-only origin/main",
            "cd /repo && git checkout main",
            "cd /repo; git stash",
            "tee /repo/out.txt",
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
            "cd /repo && git status",
            "cargo fmt",
            "cargo test -p coder-new > out.txt",
            "git commit -am 'x' && git push origin HEAD:main",
            "git fetch origin && git rebase origin/main",
            "rg pattern /repo/crates",
            "cp /repo/README.md notes.md",
            "git show origin/main:ui.rs > src/ui.rs",
        ] {
            assert!(b.run_refusal(command, wt).is_none(), "{command}");
        }
    }
}
