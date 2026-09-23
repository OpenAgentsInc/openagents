//! The ask executor's command allowlist.
//!
//! An ask reads; it never writes. The filesystem boundary enforces that for
//! the whole process tree, and this allowlist narrows what the executor may
//! run at all: the Gym's read-only commands, plus `rg`, `cat`, `ls`, and
//! `sed -n`. A command is one program and its arguments. It runs without a
//! shell, so a pipe, a redirect, a substitution, or a second command is
//! refused rather than interpreted.
//!
//! `gym runs rank` spends Jev requests and writes the learning store, so it
//! runs only when the operator passed `--rank`.

use std::path::{Path, PathBuf};

/// The `gym terminal-bench` subcommands that only read.
pub const TERMINAL_BENCH_READS: [&str; 6] = [
    "overview", "compare", "attempt", "evidence", "history", "runbooks",
];

/// The `gym coder` subcommands, all of which only read.
pub const CODER_READS: [&str; 18] = [
    "asks",
    "policy",
    "components",
    "requirements",
    "minitasks",
    "prompt",
    "live",
    "capabilities",
    "briefing",
    "matrix",
    "composition",
    "families",
    "handoff",
    "router",
    "monitor",
    "coverage",
    "recall",
    "study",
];

/// The programs beside `gym` an executor may run.
pub const TOOLS: [&str; 4] = ["rg", "cat", "ls", "sed"];

/// What an executor may run, and where each program is.
#[derive(Clone, Debug)]
pub struct Allowlist {
    /// The `gym` binary.
    pub gym: PathBuf,
    /// Whether `gym runs rank` may run.
    pub rank: bool,
}

/// A command that passed the allowlist: the program by absolute path and
/// its arguments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Allowed {
    pub program: PathBuf,
    pub args: Vec<String>,
}

impl Allowlist {
    /// Checks `command` and resolves its program, or says why it's refused.
    ///
    /// # Errors
    ///
    /// Returns the refusal in words the executor can act on.
    pub fn check(&self, command: &str) -> Result<Allowed, String> {
        let words = split(command)?;
        let Some((first, rest)) = words.split_first() else {
            return Err("the command is empty".to_string());
        };
        let name = Path::new(first)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        if name == "gym" {
            self.gym_args(rest)?;
            return Ok(Allowed {
                program: self.gym.clone(),
                args: rest.to_vec(),
            });
        }
        if first.contains('/') || !TOOLS.contains(&name.as_str()) {
            return Err(format!(
                "{first} isn't allowed: an ask runs `gym runs …`, `gym terminal-bench \
                 {}`, `gym coder …`, `rg`, `cat`, `ls`, or `sed -n`",
                TERMINAL_BENCH_READS.join("|")
            ));
        }
        match name.as_str() {
            "rg" => {
                if let Some(flag) = rest.iter().find(|arg| {
                    arg.starts_with("--pre")
                        || *arg == "-z"
                        || arg.starts_with("--search-zip")
                        || (arg.starts_with('-') && !arg.starts_with("--") && arg.contains('z'))
                }) {
                    return Err(format!("rg {flag} runs other programs; it isn't allowed"));
                }
            }
            "sed" => sed_args(rest)?,
            _ => {}
        }
        let program = which(&name).ok_or_else(|| format!("{name} is not on PATH"))?;
        Ok(Allowed {
            program,
            args: rest.to_vec(),
        })
    }

    fn gym_args(&self, args: &[String]) -> Result<(), String> {
        let sub = args.first().map(String::as_str).unwrap_or_default();
        let next = args.get(1).map(String::as_str).unwrap_or_default();
        match sub {
            "runs" => {
                let ranks = args.iter().any(|arg| arg == "rank");
                if ranks && !self.rank {
                    return Err(
                        "`gym runs rank` spends Jev requests; it runs only when the operator \
                         passes --rank"
                            .to_string(),
                    );
                }
                if !ranks && args.iter().any(|arg| arg == "--record") {
                    return Err("--record writes a file; an ask only reads".to_string());
                }
                Ok(())
            }
            "terminal-bench" if TERMINAL_BENCH_READS.contains(&next) => Ok(()),
            "terminal-bench" => Err(format!(
                "`gym terminal-bench {next}` isn't a read; an ask runs {}",
                TERMINAL_BENCH_READS.join(", ")
            )),
            "coder" if CODER_READS.contains(&next) => {
                if args.iter().any(|arg| arg == "--follow") {
                    Err("--follow never ends; read `gym coder live` once instead".to_string())
                } else {
                    Ok(())
                }
            }
            "coder" => Err(format!(
                "`gym coder {next}` isn't one of the reads: {}",
                CODER_READS.join(", ")
            )),
            other => Err(format!(
                "`gym {other}` isn't allowed; an ask runs `gym runs`, `gym terminal-bench`, \
                 or `gym coder`"
            )),
        }
    }
}

/// `sed -n SCRIPT FILE…` where the script only prints a line or a range.
fn sed_args(args: &[String]) -> Result<(), String> {
    let refuse =
        || Err("sed runs only as `sed -n 'N,Mp' FILE`: print lines, nothing else".to_string());
    let [flag, script, files @ ..] = args else {
        return refuse();
    };
    if flag != "-n" || files.is_empty() || files.iter().any(|f| f.starts_with('-')) {
        return refuse();
    }
    let Some(range) = script.strip_suffix('p') else {
        return refuse();
    };
    let address =
        |part: &str| part == "$" || (!part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()));
    let ok = match range.split_once(',') {
        Some((from, to)) => address(from) && address(to),
        None => address(range),
    };
    if ok { Ok(()) } else { refuse() }
}

/// Splits a command into words the way a shell would for plain words and
/// quotes, and refuses everything a shell would interpret.
///
/// # Errors
///
/// Returns why the command isn't one plain command.
pub fn split(command: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut chars = command.chars();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(c) => word.push(c),
                        None => return Err("a single quote isn't closed".to_string()),
                    }
                }
            }
            '"' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(c) => word.push(c),
                            None => return Err("a double quote isn't closed".to_string()),
                        },
                        Some('$' | '`') => {
                            return Err(
                                "substitution isn't allowed; write the value itself".to_string()
                            );
                        }
                        Some(c) => word.push(c),
                        None => return Err("a double quote isn't closed".to_string()),
                    }
                }
            }
            '\\' => {
                in_word = true;
                if let Some(c) = chars.next() {
                    word.push(c);
                }
            }
            '|' | '&' | ';' | '<' | '>' | '(' | ')' | '$' | '`' | '\n' => {
                return Err(format!(
                    "`{c}` isn't allowed: an ask runs one command with no shell, so no pipes, \
                     redirects, substitutions, or second commands"
                ));
            }
            c if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            c => {
                in_word = true;
                word.push(c);
            }
        }
    }
    if in_word {
        words.push(word);
    }
    Ok(words)
}

/// `name` on `PATH`.
#[must_use]
pub fn which(name: &str) -> Option<PathBuf> {
    crate::minitask::process::which(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(rank: bool) -> Allowlist {
        Allowlist {
            gym: PathBuf::from("/opt/gym"),
            rank,
        }
    }

    #[test]
    fn words_split_like_a_shell_and_shell_syntax_is_refused() {
        assert_eq!(
            split(r#"gym runs --search "log summary" --reason 'near_miss=0.8'"#).unwrap(),
            vec![
                "gym",
                "runs",
                "--search",
                "log summary",
                "--reason",
                "near_miss=0.8"
            ]
        );
        assert_eq!(
            split("rg -n a\\ b docs").unwrap(),
            vec!["rg", "-n", "a b", "docs"]
        );
        for bad in [
            "gym runs | head",
            "cat a > b",
            "ls; rm -rf /",
            "cat $(echo x)",
            "cat \"$HOME\"",
            "ls `pwd`",
            "ls && ls",
            "cat 'open",
        ] {
            assert!(split(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_gym_reads_pass_and_writes_and_other_programs_do_not() {
        let allow = list(false);
        for good in [
            "gym runs --reason unearned_success --json",
            "gym runs show tb4--x/y__1 --json",
            "gym runs group --by reason --json",
            "gym terminal-bench attempt JOB TRIAL --timeline --json",
            "gym coder matrix --json",
            "/elsewhere/gym coder study --json",
        ] {
            let allowed = allow.check(good).unwrap_or_else(|e| panic!("{good}: {e}"));
            assert_eq!(allowed.program, PathBuf::from("/opt/gym"), "{good}");
        }
        for bad in [
            "gym runs rank",
            "gym runs rank --record x.json",
            "gym runs --record x.json",
            "gym terminal-bench run --profile tb4",
            "gym terminal-bench materialize",
            "gym terminal-bench resume",
            "gym coder live --follow",
            "gym coder nonsense",
            "gym eval",
            "python3 -c 'print(1)'",
            "/bin/cat x",
            "rm -rf /",
            "rg --pre cat x",
            "rg -z x",
            "rg -nz x",
            "sed -i s/a/b/ f",
            "sed -n 1,5w out f",
            "sed -n 'e rm' f",
            "sed -n 1p",
        ] {
            assert!(allow.check(bad).is_err(), "{bad}");
        }
        assert!(list(true).check("gym runs rank --limit 5").is_ok());
        for tool in ["ls docs", "sed -n 10,40p docs/gym.md", "sed -n '$p' f"] {
            if which(split(tool).unwrap()[0].as_str()).is_some() {
                assert!(allow.check(tool).is_ok(), "{tool}");
            }
        }
    }
}
