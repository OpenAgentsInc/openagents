use crate::bash::try_parse_bash;
use crate::bash::try_parse_word_only_commands_sequence;

pub fn is_known_safe_command(command: &[String]) -> bool {
    if is_safe_to_call_with_exec(command) {
        return true;
    }

    // Support `bash -lc "..."` where the script consists solely of one or
    // more "plain" commands (only bare words / quoted strings) combined with
    // a conservative allow‑list of shell operators that themselves do not
    // introduce side effects ( "&&", "||", ";", and "|" ). If every
    // individual command in the script is itself a known‑safe command, then
    // the composite expression is considered safe.
    if let [bash, flag, script] = command
        && bash == "bash"
        && flag == "-lc"
        && let Some(tree) = try_parse_bash(script)
        && let Some(all_commands) = try_parse_word_only_commands_sequence(&tree, script)
        && !all_commands.is_empty()
        && all_commands
            .iter()
            .all(|cmd| is_safe_to_call_with_exec(cmd))
    {
        return true;
    }

    false
}

fn is_safe_to_call_with_exec(command: &[String]) -> bool {
    let cmd0 = command.first().map(String::as_str);

    match cmd0 {
        #[rustfmt::skip]
        Some(
            "cat" |
            "cd" |
            "echo" |
            "false" |
            "grep" |
            "head" |
            "ls" |
            "nl" |
            "pwd" |
            "tail" |
            "true" |
            "wc" |
            "which") => {
            true
        },

        Some("find") => {
            // Certain options to `find` can delete files, write to files, or
            // execute arbitrary commands, so we cannot auto-approve the
            // invocation of `find` in such cases.
            #[rustfmt::skip]
            const UNSAFE_FIND_OPTIONS: &[&str] = &[
                // Options that can execute arbitrary commands.
                "-exec", "-execdir", "-ok", "-okdir",
                // Option that deletes matching files.
                "-delete",
                // Options that write pathnames to a file.
                "-fls", "-fprint", "-fprint0", "-fprintf",
            ];

            !command
                .iter()
                .any(|arg| UNSAFE_FIND_OPTIONS.contains(&arg.as_str()))
        }

        // Ripgrep
        Some("rg") => {
            const UNSAFE_RIPGREP_OPTIONS_WITH_ARGS: &[&str] = &[
                // Takes an arbitrary command that is executed for each match.
                "--pre",
                // Takes a command that can be used to obtain the local hostname.
                "--hostname-bin",
            ];
            const UNSAFE_RIPGREP_OPTIONS_WITHOUT_ARGS: &[&str] = &[
                // Calls out to other decompression tools, so do not auto-approve
                // out of an abundance of caution.
                "--search-zip",
                "-z",
            ];

            !command.iter().any(|arg| {
                UNSAFE_RIPGREP_OPTIONS_WITHOUT_ARGS.contains(&arg.as_str())
                    || UNSAFE_RIPGREP_OPTIONS_WITH_ARGS
                        .iter()
                        .any(|&opt| arg == opt || arg.starts_with(&format!("{opt}=")))
            })
        }

        // Git
        Some("git") => matches!(
            command.get(1).map(String::as_str),
            Some("branch" | "status" | "log" | "diff" | "show")
        ),

        // Rust
        Some("cargo") if command.get(1).map(String::as_str) == Some("check") => true,

        // Special-case `sed -n {N|M,N}p FILE`
        Some("sed")
            if {
                command.len() == 4
                    && command.get(1).map(String::as_str) == Some("-n")
                    && is_valid_sed_n_arg(command.get(2).map(String::as_str))
                    && command.get(3).map(String::is_empty) == Some(false)
            } =>
        {
            true
        }

        // ── anything else ─────────────────────────────────────────────────
        _ => false,
    }
}

// (bash parsing helpers implemented in crate::bash)

/* ----------------------------------------------------------
Example
---------------------------------------------------------- */

/// Returns true if `arg` matches /^(\d+,)?\d+p$/
fn is_valid_sed_n_arg(arg: Option<&str>) -> bool {
    // unwrap or bail
    let s = match arg {
        Some(s) => s,
        None => return false,
    };

    // must end with 'p', strip it
    let core = match s.strip_suffix('p') {
        Some(rest) => rest,
        None => return false,
    };

    // split on ',' and ensure 1 or 2 numeric parts
    let parts: Vec<&str> = core.split(',').collect();
    match parts.as_slice() {
        // single number, e.g. "10"
        [num] => !num.is_empty() && num.chars().all(|c| c.is_ascii_digit()),

        // two numbers, e.g. "1,5"
        [a, b] => {
            !a.is_empty()
                && !b.is_empty()
                && a.chars().all(|c| c.is_ascii_digit())
                && b.chars().all(|c| c.is_ascii_digit())
        }

        // anything else (more than one comma) is invalid
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::string::ToString;

    fn vec_str(args: &[&str]) -> Vec<String> {
        args.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn known_safe_examples() {
        assert!(is_safe_to_call_with_exec(&vec_str(&["ls"])));
        assert!(is_safe_to_call_with_exec(&vec_str(&["git", "status"])));
        assert!(is_safe_to_call_with_exec(&vec_str(&[
            "sed", "-n", "1,5p", "file.txt"
        ])));
        assert!(is_safe_to_call_with_exec(&vec_str(&[
            "nl",
            "-nrz",
            "Cargo.toml"
        ])));

        // Safe `find` command (no unsafe options).
        assert!(is_safe_to_call_with_exec(&vec_str(&[
            "find", ".", "-name", "file.txt"
        ])));
    }

    #[test]
    fn unknown_or_partial() {
        assert!(!is_safe_to_call_with_exec(&vec_str(&["foo"])));
        assert!(!is_safe_to_call_with_exec(&vec_str(&["git", "fetch"])));
        assert!(!is_safe_to_call_with_exec(&vec_str(&[
            "sed", "-n", "xp", "file.txt"
        ])));

        // Unsafe `find` commands.
        for args in [
            vec_str(&["find", ".", "-name", "file.txt", "-exec", "rm", "{}", ";"]),
            vec_str(&[
                "find", ".", "-name", "*.py", "-execdir", "python3", "{}", ";",
            ]),
            vec_str(&["find", ".", "-name", "file.txt", "-ok", "rm", "{}", ";"]),
            vec_str(&["find", ".", "-name", "*.py", "-okdir", "python3", "{}", ";"]),
            vec_str(&["find", ".", "-delete", "-name", "file.txt"]),
            vec_str(&["find", ".", "-fls", "/etc/passwd"]),
            vec_str(&["find", ".", "-fprint", "/etc/passwd"]),
            vec_str(&["find", ".", "-fprint0", "/etc/passwd"]),
            vec_str(&["find", ".", "-fprintf", "/root/suid.txt", "%#m %u %p\n"]),
        ] {
            assert!(
                !is_safe_to_call_with_exec(&args),
                "expected {args:?} to be unsafe"
            );
        }
    }

    #[test]
    fn ripgrep_rules() {
        // Safe ripgrep invocations – none of the unsafe flags are present.
        assert!(is_safe_to_call_with_exec(&vec_str(&[
            "rg",
            "Cargo.toml",
            "-n"
        ])));

        // Unsafe flags that do not take an argument (present verbatim).
        for args in [
            vec_str(&["rg", "--search-zip", "files"]),
            vec_str(&["rg", "-z", "files"]),
        ] {
            assert!(
                !is_safe_to_call_with_exec(&args),
                "expected {args:?} to be considered unsafe due to zip-search flag",
            );
        }

        // Unsafe flags that expect a value, provided in both split and = forms.
        for args in [
            vec_str(&["rg", "--pre", "pwned", "files"]),
            vec_str(&["rg", "--pre=pwned", "files"]),
            vec_str(&["rg", "--hostname-bin", "pwned", "files"]),
            vec_str(&["rg", "--hostname-bin=pwned", "files"]),
        ] {
            assert!(
                !is_safe_to_call_with_exec(&args),
                "expected {args:?} to be considered unsafe due to external-command flag",
            );
        }
    }

    #[test]
    fn bash_lc_safe_examples() {
        assert!(is_known_safe_command(&vec_str(&["bash", "-lc", "ls"])));
        assert!(is_known_safe_command(&vec_str(&["bash", "-lc", "ls -1"])));
        assert!(is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "git status"
        ])));
        assert!(is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "grep -R \"Cargo.toml\" -n"
        ])));
        assert!(is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "sed -n 1,5p file.txt"
        ])));
        assert!(is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "sed -n '1,5p' file.txt"
        ])));

        assert!(is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "find . -name file.txt"
        ])));
    }

    #[test]
    fn bash_lc_safe_examples_with_operators() {
        assert!(is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "grep -R \"Cargo.toml\" -n || true"
        ])));
        assert!(is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "ls && pwd"
        ])));
        assert!(is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "echo 'hi' ; ls"
        ])));
        assert!(is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "ls | wc -l"
        ])));
    }

    #[test]
    fn bash_lc_unsafe_examples() {
        assert!(
            !is_known_safe_command(&vec_str(&["bash", "-lc", "git", "status"])),
            "Four arg version is not known to be safe."
        );
        assert!(
            !is_known_safe_command(&vec_str(&["bash", "-lc", "'git status'"])),
            "The extra quoting around 'git status' makes it a program named 'git status' and is therefore unsafe."
        );

        assert!(
            !is_known_safe_command(&vec_str(&["bash", "-lc", "find . -name file.txt -delete"])),
            "Unsafe find option should not be auto-approved."
        );

        // Disallowed because of unsafe command in sequence.
        assert!(
            !is_known_safe_command(&vec_str(&["bash", "-lc", "ls && rm -rf /"])),
            "Sequence containing unsafe command must be rejected"
        );

        // Disallowed because of parentheses / subshell.
        assert!(
            !is_known_safe_command(&vec_str(&["bash", "-lc", "(ls)"])),
            "Parentheses (subshell) are not provably safe with the current parser"
        );
        assert!(
            !is_known_safe_command(&vec_str(&["bash", "-lc", "ls || (pwd && echo hi)"])),
            "Nested parentheses are not provably safe with the current parser"
        );

        // Disallowed redirection.
        assert!(
            !is_known_safe_command(&vec_str(&["bash", "-lc", "ls > out.txt"])),
            "> redirection should be rejected"
        );
    }
}
