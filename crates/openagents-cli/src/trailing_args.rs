//! A trailing command must not swallow one of `oa`'s own flags.
//!
//! `oa box exec`, `oa box run`, and `oa memory add` take the rest of the line
//! as one opaque argument for something else: a command for the box, or the
//! text of a memory. Clap calls that `trailing_var_arg`, and it is the right
//! shape — `oa box exec bx_1 grep --color foo` has to send `--color` to
//! `grep`, not read it as a flag of `oa`.
//!
//! The cost is that a flag `oa` does define is captured the same way. This ran
//! against production:
//!
//! ```text
//! oa box exec bx_8bhkse3n "echo hi" --conversation 3dd6d813-...
//! ```
//!
//! `echo hi --conversation 3dd6d813-...` ran on the box, `--conversation` was
//! never read, and nothing in the output said so — the conversation id went to
//! a remote shell and the flag it was written for was ignored in silence.
//! Forge issue #109.
//!
//! So the trailing arguments are scanned before anything is dispatched, and a
//! token that names a flag of that same subcommand is refused. The flag set is
//! read out of clap's own `Command`, so a flag added to `BoxAction::Exec`
//! tomorrow is covered the day it is added rather than the day someone
//! remembers to edit a list here. `--` still passes anything through
//! deliberately, and `every_trailing_var_arg_in_the_tree_is_registered` holds
//! the registry below to every `trailing_var_arg` in the tree.

use clap::CommandFactory;

use crate::cli::{BoxAction, Cli, Commands, MemoryAction};

/// Every subcommand whose last positional swallows the rest of the line.
///
/// The tests walk the whole command tree for `trailing_var_arg` positionals
/// and fail if one of them is missing here, so a fourth such subcommand cannot
/// be added without either the guard or a deliberate decision to skip it.
pub const TRAILING_COMMANDS: &[&[&str]] = &[&["box", "exec"], &["box", "run"], &["memory", "add"]];

/// Refuse the parsed command when its trailing arguments carry one of its own
/// flags, reading the separator out of this process's argv.
pub fn check_command(command: &Commands) -> Result<(), String> {
    let argv: Vec<String> = std::env::args().collect();
    check_argv(command, &argv)
}

/// The same, against an argv the caller supplies.
pub fn check_argv(command: &Commands, argv: &[String]) -> Result<(), String> {
    let Some((path, trailing)) = trailing_of(command) else {
        return Ok(());
    };
    check(path, trailing, has_separator(argv))
}

/// The subcommand path and the trailing arguments, for the subcommands that
/// have any.
fn trailing_of(command: &Commands) -> Option<(&'static [&'static str], &Vec<String>)> {
    match command {
        Commands::Box(args) => match &args.action {
            BoxAction::Exec { command, .. } => Some((&["box", "exec"], command)),
            BoxAction::Run { command, .. } => Some((&["box", "run"], command)),
            _ => None,
        },
        Commands::Memory(args) => match &args.action {
            MemoryAction::Add { body, .. } => Some((&["memory", "add"], body)),
            _ => None,
        },
        _ => None,
    }
}

/// Whether the invocation wrote an explicit `--`.
///
/// Clap consumes the first one, so it is not in the parsed trailing arguments
/// and has to be read off argv. A caller who wrote it has said where the
/// boundary is, and gets what they asked for without a word from here.
pub fn has_separator<S: AsRef<str>>(argv: &[S]) -> bool {
    argv.iter().any(|argument| argument.as_ref() == "--")
}

/// The refusal for one subcommand's trailing arguments.
pub fn check(path: &[&str], trailing: &[String], separated: bool) -> Result<(), String> {
    if separated {
        return Ok(());
    }
    let own = own_long_flags(path);
    for token in trailing {
        let Some(name) = long_flag_name(token) else {
            continue;
        };
        if own.iter().any(|flag| flag == name) {
            return Err(refusal(path, name));
        }
    }
    Ok(())
}

/// The long name a token names, if it names one.
///
/// Only long tokens are matched. `--conversation` and `--conversation=abc` are
/// the shape of the mistake, and no flag on any of these subcommands has a
/// short name of its own. Matching short tokens would refuse `curl -v` and
/// `rm -rf` over `-v` on the root command, which is a flag of a different
/// program in every case that matters.
fn long_flag_name(token: &str) -> Option<&str> {
    let rest = token.strip_prefix("--").filter(|rest| !rest.is_empty())?;
    rest.split('=').next()
}

/// Every long flag the named subcommand accepts, as clap holds it.
///
/// Globals count. `oa box exec --json bx_1 uptime` reads `--json`, so
/// `oa box exec bx_1 uptime --json` is the same mistake as the one #109
/// reports, and refusing it names a flag the reader can move rather than
/// leaving `--json` in the box's argv.
///
/// An unknown path yields nothing, which refuses nothing. The tests hold every
/// entry in `TRAILING_COMMANDS` to a real subcommand with real flags.
pub fn own_long_flags(path: &[&str]) -> Vec<String> {
    let mut root = Cli::command();
    root.build();
    let Some(command) = descend(&root, path) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for argument in command.get_arguments() {
        if argument.is_positional() {
            continue;
        }
        for long in argument
            .get_long_and_visible_aliases()
            .into_iter()
            .flatten()
            .chain(argument.get_all_aliases().into_iter().flatten())
        {
            let long = long.to_string();
            if !names.contains(&long) {
                names.push(long);
            }
        }
    }
    names
}

/// Walk a subcommand path from the root command.
fn descend<'a>(root: &'a clap::Command, path: &[&str]) -> Option<&'a clap::Command> {
    let mut current = root;
    for segment in path {
        current = current.find_subcommand(segment)?;
    }
    Some(current)
}

/// What the trailing positional is called in help, such as `<COMMAND>`.
fn trailing_value_name(path: &[&str]) -> String {
    let mut root = Cli::command();
    root.build();
    let named = descend(&root, path).and_then(|command| {
        command
            .get_arguments()
            .find(|argument| argument.is_trailing_var_arg_set())
            .and_then(|argument| argument.get_value_names().and_then(|names| names.first()))
            .map(|name| format!("<{name}>"))
    });
    named.unwrap_or_else(|| "the trailing argument".to_string())
}

/// The sentence the caller acts on.
fn refusal(path: &[&str], flag: &str) -> String {
    let value = trailing_value_name(path);
    format!(
        "`--{flag}` is a flag of `oa {command}`, and a flag written after {value} is read as \
         part of {value} rather than as a flag. Write `--{flag}` before {value}, or write `--` \
         before {value} to send `--{flag}` through on purpose.",
        command = path.join(" "),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// Parse an argv the way `main` does, then run the guard over it.
    fn guard(argv: &[&str]) -> Result<(), String> {
        let owned: Vec<String> = argv.iter().map(|a| a.to_string()).collect();
        let cli = Cli::try_parse_from(&owned).expect("the invocation must parse");
        let command = cli.command.expect("the invocation names a subcommand");
        check_argv(&command, &owned)
    }

    /// The invocation from the issue. The conversation id must not reach the
    /// box, and the refusal has to name the flag that was dropped.
    #[test]
    fn box_exec_refuses_a_trailing_conversation_flag() {
        let error = guard(&[
            "oa",
            "box",
            "exec",
            "bx_8bhkse3n",
            "echo hi",
            "--conversation",
            "3dd6d813-0000-4000-8000-000000000000",
        ])
        .expect_err("a trailing --conversation must be refused");
        assert!(
            error.contains("--conversation"),
            "the refusal must name the flag; it said: {error}"
        );
        assert!(
            error.contains("oa box exec"),
            "the refusal must name the subcommand whose flag it is; it said: {error}"
        );
        assert!(
            error.contains("before"),
            "the refusal must say where the flag goes; it said: {error}"
        );
    }

    /// `--flag=value` is the same flag.
    #[test]
    fn box_exec_refuses_the_joined_form() {
        let error = guard(&[
            "oa",
            "box",
            "exec",
            "bx_1",
            "echo hi",
            "--conversation=3dd6d813",
        ])
        .expect_err("--conversation=... must be refused too");
        assert!(error.contains("--conversation"), "it said: {error}");
    }

    /// Every flag of the subcommand, not one named in a list.
    #[test]
    fn box_exec_refuses_a_trailing_timeout_flag() {
        let error = guard(&["oa", "box", "exec", "bx_1", "sleep 30", "--timeout", "5"])
            .expect_err("a trailing --timeout must be refused");
        assert!(error.contains("--timeout"), "it said: {error}");
    }

    /// A global is a flag of the subcommand too: `oa box exec --json bx_1 ls`
    /// reads it, so writing it after the command is the same mistake.
    #[test]
    fn box_exec_refuses_a_trailing_global_flag() {
        let error = guard(&["oa", "box", "exec", "bx_1", "uptime", "--json"])
            .expect_err("a trailing --json must be refused");
        assert!(error.contains("--json"), "it said: {error}");
    }

    /// The remote program's own flags are none of `oa`'s business.
    #[test]
    fn a_flag_that_belongs_to_the_remote_command_passes() {
        guard(&["oa", "box", "exec", "bx_1", "ls", "--color=auto"])
            .expect("--color is not a flag of oa box exec");
        guard(&["oa", "box", "exec", "bx_1", "grep", "--color", "foo"])
            .expect("--color is not a flag of oa box exec");
        guard(&["oa", "box", "exec", "bx_1", "npm", "--registry", "http://x"])
            .expect("--registry is not a flag of oa box exec");
    }

    /// Short tokens are the remote program's. `-v` is `--verbose` on `oa`, and
    /// refusing `curl -v` over that would break more than it saves.
    #[test]
    fn short_tokens_pass() {
        guard(&["oa", "box", "exec", "bx_1", "rm", "-rf", "/tmp/x"])
            .expect("rm -rf must still run");
        guard(&[
            "oa",
            "box",
            "exec",
            "bx_1",
            "curl",
            "-v",
            "https://example.com",
        ])
        .expect("curl -v must still run");
    }

    /// `--` says where the boundary is, so the guard has nothing to say.
    #[test]
    fn an_explicit_separator_passes_anything_through() {
        guard(&[
            "oa",
            "box",
            "exec",
            "bx_1",
            "--",
            "mytool",
            "--conversation",
            "abc",
        ])
        .expect("`--` is the caller saying they meant it");
    }

    /// The correct invocation is not refused, and the flag is read.
    #[test]
    fn a_flag_before_the_command_is_read_and_passes() {
        let argv: Vec<String> = [
            "oa",
            "box",
            "exec",
            "--conversation",
            "abc",
            "bx_1",
            "echo hi",
        ]
        .iter()
        .map(|a| a.to_string())
        .collect();
        let cli = Cli::try_parse_from(&argv).expect("the invocation must parse");
        let command = cli.command.expect("a subcommand");
        match &command {
            Commands::Box(args) => match &args.action {
                BoxAction::Exec {
                    conversation,
                    command: trailing,
                    ..
                } => {
                    assert_eq!(conversation.as_deref(), Some("abc"));
                    assert_eq!(trailing, &vec!["echo hi".to_string()]);
                }
                other => panic!("expected box exec, got {other:?}"),
            },
            other => panic!("expected box, got {other:?}"),
        }
        check_argv(&command, &argv).expect("the correct invocation must pass");
    }

    /// `box run` has the same shape and the same bug.
    #[test]
    fn box_run_refuses_a_trailing_conversation_flag() {
        let error = guard(&[
            "oa",
            "box",
            "run",
            "bx_1",
            "cargo test",
            "--conversation",
            "abc",
        ])
        .expect_err("a trailing --conversation must be refused");
        assert!(error.contains("--conversation"), "it said: {error}");
        assert!(
            error.contains("oa box run"),
            "the refusal must name `oa box run`; it said: {error}"
        );
    }

    /// So does `memory add`, whose trailing argument is the memory itself.
    #[test]
    fn memory_add_refuses_a_trailing_supersedes_flag() {
        let error = guard(&[
            "oa",
            "memory",
            "add",
            "the box ids are short lived",
            "--supersedes",
            "mem_1",
        ])
        .expect_err("a trailing --supersedes must be refused");
        assert!(error.contains("--supersedes"), "it said: {error}");
        assert!(
            error.contains("oa memory add"),
            "the refusal must name `oa memory add`; it said: {error}"
        );
    }

    /// A memory that talks about some other program's flags is still a memory.
    #[test]
    fn memory_add_keeps_text_that_is_not_one_of_its_flags() {
        guard(&[
            "oa",
            "memory",
            "add",
            "run",
            "cargo",
            "--release",
            "for bench work",
        ])
        .expect("--release is not a flag of oa memory add");
    }

    /// A subcommand without a trailing positional is not touched.
    #[test]
    fn a_subcommand_without_trailing_arguments_is_untouched() {
        guard(&["oa", "box", "list", "--conversation", "abc"])
            .expect("box list parses --conversation as a flag, so there is nothing to guard");
    }

    /// The flag set comes out of clap, not out of a list written by hand.
    #[test]
    fn the_flag_set_is_read_from_the_command_tree() {
        let flags = own_long_flags(&["box", "exec"]);
        for expected in ["conversation", "timeout", "json", "verbose"] {
            assert!(
                flags.iter().any(|flag| flag == expected),
                "`--{expected}` is a flag of `oa box exec`, and the derived set is {flags:?}"
            );
        }
        assert!(
            !flags.iter().any(|flag| flag == "color"),
            "`--color` is not a flag of `oa box exec`, and the derived set is {flags:?}"
        );
    }

    /// Every `trailing_var_arg` in the tree is guarded.
    ///
    /// This is the part that does not rot: a fourth subcommand that captures
    /// the rest of the line fails here until it is listed.
    #[test]
    fn every_trailing_var_arg_in_the_tree_is_registered() {
        let mut root = Cli::command();
        root.build();
        let mut found: Vec<Vec<String>> = Vec::new();
        collect_trailing(&root, &mut Vec::new(), &mut found);
        assert!(
            !found.is_empty(),
            "the walk found no trailing_var_arg at all, so it is not testing anything"
        );
        for path in &found {
            let names: Vec<&str> = path.iter().map(|s| s.as_str()).collect();
            assert!(
                TRAILING_COMMANDS.contains(&names.as_slice()),
                "`oa {}` captures the rest of the line and is not in TRAILING_COMMANDS, \
                 so a flag written after its command is swallowed in silence",
                path.join(" ")
            );
        }
        for known in TRAILING_COMMANDS {
            let owned: Vec<String> = known.iter().map(|s| s.to_string()).collect();
            assert!(
                found.contains(&owned),
                "`oa {}` is registered but has no trailing_var_arg positional",
                known.join(" ")
            );
            assert!(
                !own_long_flags(known).is_empty(),
                "`oa {}` resolved to no flags, so the guard over it refuses nothing",
                known.join(" ")
            );
        }
    }

    fn collect_trailing(
        command: &clap::Command,
        path: &mut Vec<String>,
        found: &mut Vec<Vec<String>>,
    ) {
        if !path.is_empty()
            && command
                .get_arguments()
                .any(|argument| argument.is_trailing_var_arg_set())
        {
            found.push(path.clone());
        }
        for child in command.get_subcommands() {
            path.push(child.get_name().to_string());
            collect_trailing(child, path, found);
            path.pop();
        }
    }
}
