//! Commands that would move or throw away uncommitted work in a shared
//! checkout (AGENTS.md: never stash, reset, checkout, or restore another
//! agent's work). Run refuses them before they start and points at a fresh
//! worktree instead. This parses the command's words; it routes nothing.

/// Why `command` is refused, or `None` when it may run.
#[must_use]
pub fn refusal(command: &str) -> Option<String> {
    segments(command).find_map(|words| {
        let (subcommand, rest) = git_subcommand(&words)?;
        let refused = match subcommand {
            "stash" => !matches!(rest.first().map(String::as_str), Some("list" | "show")),
            "reset" => rest.iter().any(|word| word == "--hard" || word == "--merge" || word == "--keep"),
            "checkout" => rest.iter().any(|word| word == "--" || word == "." || word == "-f" || word == "--force"),
            "restore" => !rest.iter().any(|word| word == "--staged" || word == "-S") || rest.iter().any(|word| word == "--worktree" || word == "-W"),
            "clean" => rest.iter().any(|word| word.starts_with("-") && !word.starts_with("--") && word.contains('f') || word == "--force"),
            _ => false,
        };
        refused.then(|| {
            format!(
                "Refused: `git {subcommand}` would move or discard uncommitted work in this checkout, which may belong to another agent or the user. Make your change in a fresh worktree instead (git worktree add --detach <dir> origin/main), test and push from there, and leave this checkout's changes alone."
            )
        })
    })
}

/// The command split at `;`, `&&`, `||`, `|`, and newlines, each part as
/// shell words (quotes removed; good enough to find a git subcommand).
fn segments(command: &str) -> impl Iterator<Item = Vec<String>> + '_ {
    command
        .split(|c| matches!(c, ';' | '\n' | '|' | '&' | '(' | ')' | '{' | '}'))
        .map(|part| {
            part.split_whitespace()
                .map(|word| word.trim_matches(|c| c == '\'' || c == '"').to_owned())
                .collect::<Vec<_>>()
        })
        .filter(|words| !words.is_empty())
}

/// `git`'s subcommand and the words after it, skipping `git`'s own options
/// (`-C dir`, `-c key=value`, `--git-dir=…`) and any leading `env`/`VAR=…`.
fn git_subcommand(words: &[String]) -> Option<(&str, &[String])> {
    let start = words
        .iter()
        .position(|word| word == "git" || word.ends_with("/git"))?;
    let mut index = start + 1;
    while let Some(word) = words.get(index) {
        match word.as_str() {
            "-C" | "-c" | "--git-dir" | "--work-tree" | "--namespace" => index += 2,
            option if option.starts_with('-') => index += 1,
            subcommand => return Some((subcommand, &words[index + 1..])),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::refusal;

    #[test]
    fn commands_that_move_uncommitted_work_are_refused() {
        for command in [
            "git stash",
            "git stash push crates/coder-new",
            "cd /repo && git stash push crates/coder-new >/dev/null 2>&1 && cargo test",
            "git -C /repo stash -u",
            "git reset --hard origin/main",
            "git checkout -- src/lib.rs",
            "git checkout .",
            "git restore src/lib.rs",
            "git clean -fd",
        ] {
            assert!(refusal(command).is_some(), "{command}");
        }
    }

    #[test]
    fn ordinary_git_and_other_commands_run() {
        for command in [
            "git status --short",
            "git stash list",
            "git stash show -p stash@{0}",
            "git diff && git log -3 --oneline",
            "git checkout -b scratch",
            "git restore --staged src/lib.rs",
            "git worktree add --detach /tmp/wt origin/main",
            "git reset --soft HEAD~1",
            "git clean -n",
            "echo stash reset checkout",
            "cargo test -p coder-new",
        ] {
            assert!(refusal(command).is_none(), "{command}");
        }
    }
}
