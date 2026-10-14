//! A shell command that greps the whole checkout recursively (#11580).
//!
//! `grep -r PATTERN .` walks everything under the checkout, build output
//! included: a Rust checkout's `target/` turned one search into 92 seconds
//! and a capped 68 KB of matches. The Grep tool and `rg` skip what
//! `.gitignore` ignores. This is a typed rule over the command's words —
//! the program, its recursive flag and its path operands — never a guess
//! at what the person meant.

/// The refusal for a recursive `grep` over the whole working directory,
/// or `None` when the command does no such thing.
#[must_use]
pub fn refusal(command: &str) -> Option<String> {
    segments(command).any(|words| whole_tree_grep(&words)).then(|| {
        "Refused: `grep -r` over the whole checkout also walks build output and everything else .gitignore ignores (a Rust target/ folder alone takes minutes). Search with the Grep tool, which skips them, or with `rg PATTERN`, which respects .gitignore. To grep one folder, name it: grep -rn PATTERN src/".to_owned()
    })
}

/// The command split at `;`, `&&`, `||`, `|`, and newlines, each part as
/// shell words (quotes removed).
fn segments(command: &str) -> impl Iterator<Item = Vec<String>> + '_ {
    command
        .split(|c| matches!(c, ';' | '\n' | '|' | '&' | '(' | ')' | '{' | '}' | '`'))
        .map(|part| {
            part.split_whitespace()
                .map(|word| word.trim_matches(|c| c == '\'' || c == '"').to_owned())
                .collect::<Vec<_>>()
        })
        .filter(|words| !words.is_empty())
}

/// Options of `grep` whose value is the next word.
const VALUED: [&str; 16] = [
    "-e",
    "-f",
    "-m",
    "-A",
    "-B",
    "-C",
    "-d",
    "-D",
    "--regexp",
    "--file",
    "--max-count",
    "--include",
    "--exclude",
    "--exclude-from",
    "--context",
    "--label",
];

/// Whether `words` run `grep` recursively over the working directory
/// itself (no path, `.`, `./`, `..`, or `$PWD`) without excluding any
/// folder.
fn whole_tree_grep(words: &[String]) -> bool {
    let mut words = words
        .iter()
        .skip_while(|word| {
            word.contains('=') && !word.starts_with('-')
                || matches!(word.as_str(), "env" | "command" | "time" | "nice")
        })
        .map(String::as_str);
    let Some(program) = words.next() else {
        return false;
    };
    if !matches!(program.rsplit('/').next(), Some("grep" | "egrep" | "fgrep")) {
        return false;
    }
    let mut recursive = false;
    let mut pattern_given = false;
    let mut operands = Vec::new();
    let mut options_done = false;
    let mut words = words.peekable();
    while let Some(word) = words.next() {
        if options_done || !word.starts_with('-') || word == "-" {
            operands.push(word);
            continue;
        }
        if word == "--" {
            options_done = true;
            continue;
        }
        if word.starts_with("--exclude-dir") {
            return false;
        }
        if matches!(word, "--recursive" | "--dereference-recursive") {
            recursive = true;
            continue;
        }
        if matches!(word, "-e" | "-f" | "--regexp" | "--file")
            || word.starts_with("--regexp=")
            || word.starts_with("--file=")
        {
            pattern_given = true;
        }
        if VALUED.contains(&word) {
            words.next();
            continue;
        }
        if !word.starts_with("--") {
            // A cluster such as `-rn`; a valued letter ends it (`-rne PAT`).
            for (at, letter) in word.char_indices().skip(1) {
                match letter {
                    'r' | 'R' => recursive = true,
                    'e' | 'f' | 'm' | 'A' | 'B' | 'C' | 'd' | 'D' => {
                        if matches!(letter, 'e' | 'f') {
                            pattern_given = true;
                        }
                        if at + 1 == word.len() {
                            words.next();
                        }
                        break;
                    }
                    _ => {}
                }
            }
        }
    }
    if !recursive {
        return false;
    }
    let paths = if pattern_given {
        &operands[..]
    } else {
        operands.get(1..).unwrap_or(&[])
    };
    paths.is_empty()
        || paths.iter().any(|path| {
            matches!(
                path.trim_end_matches('/'),
                "" | "." | ".." | "$PWD" | "${PWD}" | "$(pwd)"
            )
        })
}

#[cfg(test)]
mod tests {
    use super::refusal;

    #[test]
    fn a_recursive_grep_over_the_checkout_is_refused() {
        for command in [
            // The command from #11580.
            r#"grep -rn "gemini-3.8-flash" . || grep -rn "3.8" ."#,
            "grep -r needle",
            "grep -R needle ./",
            "grep --recursive -n needle .",
            "cd /repo && grep -rIl needle . | head",
            "grep -rn -e needle -e other",
            "LC_ALL=C grep -rni needle .",
            "/usr/bin/grep -rn needle $PWD",
        ] {
            assert!(refusal(command).is_some(), "{command}");
        }
    }

    #[test]
    fn a_scoped_or_plain_grep_runs() {
        for command in [
            "grep -rn needle src/",
            "grep -rn needle crates/coder-new/src docs",
            "grep -n needle README.md",
            "grep -rn --exclude-dir=target needle .",
            "git grep -n needle",
            "rg needle",
            "grep -c . file.txt",
            "ps aux | grep -i coder",
            "echo grep -r . is slow",
            "grep -e -r file.txt",
        ] {
            assert!(refusal(command).is_none(), "{command}");
        }
    }
}
