//! Which shell commands are noisy, and the rewrite that wraps them.
//!
//! The rule is a declared table of build, test, and lint programs, never a
//! reading of the agent's words. A command is split on `&&`, `||`, and `;`
//! outside quotes, and only its noisy segments are wrapped, so a leading
//! `cd crates/x &&` still runs in the agent's own shell. Anything the
//! splitter does not follow (pipes, redirections, subshells, command
//! substitution, background jobs, newlines) is left alone: the agent
//! already shaped that output itself.

/// One segment and the separator after it (`""` for the last).
type Segments = Vec<(String, String)>;

/// `command` split on `&&`, `||`, and `;` outside quotes, or `None` when
/// it holds shell syntax the splitter does not follow.
fn segments(command: &str) -> Option<Segments> {
    let chars: Vec<char> = command.chars().collect();
    let mut out = Vec::new();
    let mut current = String::new();
    let (mut single, mut double) = (false, false);
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if single {
            single = c != '\'';
            current.push(c);
            i += 1;
            continue;
        }
        match c {
            '\\' => {
                current.push(c);
                if let Some(n) = next {
                    current.push(n);
                }
                i += 2;
                continue;
            }
            '`' => return None,
            '$' if next == Some('(') => return None,
            '"' => double = !double,
            _ if double => {}
            '\'' => single = true,
            '\n' | '>' | '<' | '(' | ')' => return None,
            '&' if next == Some('&') => {
                out.push((std::mem::take(&mut current), "&&".to_owned()));
                i += 2;
                continue;
            }
            '|' if next == Some('|') => {
                out.push((std::mem::take(&mut current), "||".to_owned()));
                i += 2;
                continue;
            }
            '&' | '|' => return None,
            ';' => {
                out.push((std::mem::take(&mut current), ";".to_owned()));
                i += 1;
                continue;
            }
            _ => {}
        }
        current.push(c);
        i += 1;
    }
    if single || double {
        return None;
    }
    out.push((current, String::new()));
    Some(out)
}

/// The words of a segment after leading assignments and wrappers
/// (`env`, `time`, `nice`, `command`, `timeout DURATION`).
fn program_words(segment: &str) -> Vec<&str> {
    let mut words: Vec<&str> = segment.split_whitespace().collect();
    loop {
        match words.first() {
            Some(word) if word.contains('=') && !word.starts_with('-') => {
                words.remove(0);
            }
            Some(&("env" | "time" | "nice" | "command")) => {
                words.remove(0);
            }
            Some(&"timeout") => {
                words.drain(..words.len().min(2));
            }
            _ => return words,
        }
    }
}

/// The first word after `words[0]` that is not a flag or a toolchain.
fn subcommand<'a>(words: &[&'a str]) -> Option<&'a str> {
    words
        .iter()
        .skip(1)
        .find(|word| !word.starts_with('-') && !word.starts_with('+'))
        .copied()
}

/// Whether one segment runs a build, test, or lint program.
#[must_use]
pub fn noisy(segment: &str) -> bool {
    let words = program_words(segment);
    let Some(first) = words.first() else {
        return false;
    };
    let program = first.rsplit('/').next().unwrap_or(first);
    let sub = subcommand(&words);
    match program {
        "cargo" => matches!(
            sub,
            Some("test" | "t" | "build" | "b" | "check" | "c" | "clippy" | "nextest" | "bench")
        ),
        "npm" | "pnpm" | "yarn" | "bun" => match sub {
            Some("test" | "t") => true,
            Some("run") => words
                .iter()
                .skip_while(|word| **word != "run")
                .nth(1)
                .is_some_and(|script| {
                    matches!(*script, "test" | "build" | "lint" | "check" | "typecheck")
                }),
            Some("build" | "lint") => program != "npm",
            _ => false,
        },
        "npx" | "bunx" | "uv" | "poetry" | "pipenv" => {
            let rest = words
                .iter()
                .skip(1)
                .skip_while(|word| word.starts_with('-') || **word == "run")
                .copied()
                .collect::<Vec<_>>()
                .join(" ");
            !rest.is_empty() && noisy(&rest)
        }
        "pytest" | "py.test" | "jest" | "vitest" | "mocha" | "rspec" | "tsc" | "eslint"
        | "ctest" | "ninja" | "make" | "gradle" | "gradlew" | "mvn" | "xcodebuild" => true,
        "python" | "python3" => {
            words.get(1) == Some(&"-m") && matches!(words.get(2), Some(&("pytest" | "unittest")))
        }
        "go" => matches!(sub, Some("test" | "build" | "vet")),
        "swift" | "dotnet" => matches!(sub, Some("build" | "test")),
        "mix" => matches!(sub, Some("test" | "compile")),
        "bazel" | "bazelisk" => matches!(sub, Some("build" | "test")),
        "cmake" => words.contains(&"--build"),
        _ => false,
    }
}

/// `text` as one single-quoted shell word.
#[must_use]
pub fn quote(text: &str) -> String {
    if !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '-' | '_' | ':' | '='))
    {
        text.to_owned()
    } else {
        format!("'{}'", text.replace('\'', "'\\''"))
    }
}

/// `command` with each noisy segment run through `invoker` (the squeezer's
/// command line, already quoted), or `None` when nothing is wrapped.
#[must_use]
pub fn rewrite(command: &str, invoker: &str) -> Option<String> {
    if command.contains(invoker) {
        return None;
    }
    let parts = segments(command)?;
    let mut wrapped = false;
    let mut out = String::new();
    for (segment, separator) in parts {
        let body = segment.trim();
        if noisy(body) {
            let lead = &segment[..segment.len() - segment.trim_start().len()];
            let trail = &segment[segment.trim_end().len()..];
            out.push_str(lead);
            out.push_str(invoker);
            out.push(' ');
            out.push_str(&quote(body));
            out.push_str(trail);
            wrapped = true;
        } else {
            out.push_str(&segment);
        }
        out.push_str(&separator);
    }
    wrapped.then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_names_builds_and_tests() {
        for command in [
            "cargo test -p parser",
            "cargo +nightly clippy --all-targets",
            "RUST_LOG=debug cargo t",
            "timeout 600 cargo nextest run",
            "npm test",
            "pnpm run build",
            "yarn lint",
            "npx jest src",
            "uv run pytest -q",
            "python3 -m pytest tests/",
            "go test ./...",
            "make",
            "/usr/bin/make -j8",
            "cmake --build build",
        ] {
            assert!(noisy(command), "{command}");
        }
        for command in [
            "ls -la",
            "git status",
            "cargo fmt",
            "cargo run --bin x",
            "npm install",
            "npm run dev",
            "python3 script.py",
            "",
        ] {
            assert!(!noisy(command), "{command}");
        }
    }

    #[test]
    fn only_noisy_segments_are_wrapped() {
        let out = rewrite("cd crates/x && cargo test -p x; git status", "sq").unwrap();
        assert_eq!(out, "cd crates/x && sq 'cargo test -p x'; git status");
        assert_eq!(rewrite("git status && ls", "sq"), None);
    }

    #[test]
    fn shaped_or_unfollowed_commands_are_left_alone() {
        for command in [
            "cargo test 2>&1 | tail -20",
            "cargo test > out.txt",
            "cargo build &",
            "(cd x && cargo test)",
            "cargo test $(cat args)",
            "cargo test `cat args`",
            "cargo test\ncargo build",
            "cargo test 'unterminated",
        ] {
            assert_eq!(rewrite(command, "sq"), None, "{command}");
        }
    }

    #[test]
    fn quoted_separators_do_not_split() {
        let out = rewrite("cargo test -- 'a && b' \"c; d\"", "sq").unwrap();
        assert_eq!(out, "sq 'cargo test -- '\\''a && b'\\'' \"c; d\"'");
    }

    #[test]
    fn a_wrapped_command_is_not_wrapped_twice() {
        let once = rewrite("cargo test", "/bin/coder coder-squeeze").unwrap();
        assert_eq!(rewrite(&once, "/bin/coder coder-squeeze"), None);
    }

    #[test]
    fn quoting_round_trips_through_a_shell() {
        let text = "it's \"quoted\" && $HOME";
        let out = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("printf %s {}", quote(text)))
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), text);
    }
}
