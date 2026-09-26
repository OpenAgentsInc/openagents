//! The command line: what `coder` was asked to do before it does it.
//!
//! Terminal and headless options, parsed without an argument-parser dependency.
//! The binary dispatches the local task inbox before parsing these options.

use std::fs;
use std::path::PathBuf;

use coder::program_authority;

/// The exit code a usage error takes. It sits outside the codes a turn
/// produces, so a script can tell "you called me wrong" from "the turn
/// declined".
pub const EXIT_USAGE: u8 = 64;

/// What the command line asked for.
#[derive(Debug, PartialEq, Eq)]
pub enum Invocation {
    /// Draw the terminal and talk.
    Interactive {
        /// Where this session's trace lands, when the caller named a file.
        trace: Option<PathBuf>,
        /// The program slugs this session may run, when `--programs`
        /// granted any.
        programs: Option<String>,
    },
    /// Run one turn without a terminal.
    Print(Print),
    /// Say which Coder this is: the repository, the commit, and whether
    /// the tree was dirty.
    Version,
    /// Report which door a turn would use and why.
    Doctor,
    /// Say what the flags are.
    Help,
}

/// One headless turn.
#[derive(Debug, PartialEq, Eq)]
pub struct Print {
    /// What to ask.
    pub prompt: String,
    /// Where the trace lands, when the caller named a file.
    pub trace: Option<PathBuf>,
    /// Report the turn as a JSON stream rather than as the reply text.
    pub json: bool,
    /// With `--json`, stream the reply's deltas too, as `delta` objects.
    pub json_deltas: bool,
    /// The program slugs this turn may run, when `--programs` granted any.
    pub programs: Option<String>,
}

/// The usage text, printed for `--help` and after a usage error.
pub const USAGE: &str = "\
coder — the Coder agent, in a terminal or in a script.

Usage:
  coder                       Open the terminal and start a conversation.
  coder -p <PROMPT>           Run one turn, write the reply to stdout, and exit.
  coder doctor                Show what would answer a turn, and why.
  coder task --help           Manage durable queued requests; runs no agent.
  coder --version             Show the repository, commit, and tree state.

Options:
  -p, --print <PROMPT>   Run one turn without the terminal.
      --prompt-file <FILE>
                         Read the prompt from a file. Implies --print.
      --trace <PATH>     Record this session to PATH. This overrides
                         CODER_TRACE.
      --json             With --print, write the turn as a JSON stream:
                         one object per turn event on stdout, then a
                         summary object.
      --json-deltas      With --json, also write each piece of the reply
                         as it streams in, as `delta` objects. Off by
                         default, because deltas can flood a pipe.
      --programs <SPEC>  Allow this session to run the named programs, as
                         CODER_PROGRAMS does: a comma-separated list of
                         program names, or `all`. If neither is set, a
                         program that a turn selects is refused instead of
                         run. See docs/coder/guides/program-authority.md.
  -V, --version          Show the repository, commit, and tree state.
  -h, --help             Show this text.

Exit codes with --print:
  0   The turn finished and the agent answered.
  1   The turn did not finish.
  2   The turn finished and the classifier declined it.
  64  The command line was wrong.

Environment variables choose what answers a turn. If Claude Code or
Codex is installed and signed in, Coder hands the turn to it with a
briefing that Jev prepares. CODER_DELEGATE sets when that happens: auto
(the default), always, or off. CODER_DELEGATE_AGENT picks claude-code or
codex, and CODER_DELEGATE_MODEL picks its model. If no agent is
available, Coder answers through an Open Responses endpoint instead.

CODER_DECISION_PROFILE and the other CODER_DECISION_* variables set the
decision service that classifies each turn. If they are unset, Coder
uses the hosted service that TYPESAFE_API_KEY unlocks. CODER_DOOR_KEY,
CODER_DOOR_URL, and CODER_MODEL point Coder at your own Open Responses
endpoint. CODER_WORKER and CODER_RELAY send the turn to a worker through
a Nostr relay. If none of these is set, a stub answers with a fixed
message. CODER_SHELL=off stops Coder from running commands.
CODER_PROGRAMS names the programs a session may run, and
CODER_PROGRAM_EFFECTS limits what they may do. CODER_MODEL takes a model
family, gemini or glm, or any model ID the gateway serves.";

/// Reads the command line.
///
/// `arguments` is everything after the program name.
///
/// # Errors
///
/// Returns a sentence naming what is wrong with the arguments. The caller
/// should print it, print [`USAGE`], and exit [`EXIT_USAGE`].
pub fn parse(arguments: &[String]) -> Result<Invocation, String> {
    // `doctor` is a subcommand only as the whole command line, so a prompt
    // that happens to be the word still reaches a turn with `-p`.
    if let [only] = arguments
        && only == "doctor"
    {
        return Ok(Invocation::Doctor);
    }
    let mut print = false;
    let mut json = false;
    let mut json_deltas = false;
    let mut trace: Option<PathBuf> = None;
    let mut prompt: Option<String> = None;
    let mut prompt_file: Option<PathBuf> = None;
    let mut programs: Option<String> = None;

    // Everything after a bare `--` is a prompt, whatever it starts with.
    let mut literal = false;
    let mut rest = arguments.iter();
    while let Some(argument) = rest.next() {
        if literal {
            if prompt.replace(argument.clone()).is_some() {
                return Err(
                    "give exactly one prompt, either as an argument or with --prompt-file"
                        .to_string(),
                );
            }
            continue;
        }
        let (flag, attached) = match argument.split_once('=') {
            Some((flag, value)) if flag.starts_with('-') => (flag, Some(value.to_string())),
            _ => (argument.as_str(), None),
        };
        // A flag that takes a value takes it from `--flag=value` or from
        // the next argument, and says so rather than swallowing the next
        // flag.
        let mut value = |flag: &str| -> Result<String, String> {
            match attached.clone() {
                Some(value) => Ok(value),
                None => rest
                    .next()
                    .filter(|next| !next.starts_with('-'))
                    .cloned()
                    .ok_or_else(|| format!("{flag} needs a value")),
            }
        };
        match flag {
            "-h" | "--help" => return Ok(Invocation::Help),
            "-V" | "--version" => return Ok(Invocation::Version),
            "-p" | "--print" => {
                print = true;
                if let Some(attached) = attached.clone()
                    && prompt.replace(attached).is_some()
                {
                    return Err(
                        "give exactly one prompt, either as an argument or with --prompt-file"
                            .to_string(),
                    );
                }
            }
            "--json" => json = true,
            "--json-deltas" => json_deltas = true,
            "--trace" => trace = Some(PathBuf::from(value("--trace")?)),
            "--prompt-file" => prompt_file = Some(PathBuf::from(value("--prompt-file")?)),
            "--programs" => {
                let spec = value("--programs")?;
                // A spec that names nothing grants nothing: say so at the
                // command line rather than as a session that refuses
                // every program it is asked about.
                let (_, notes) = program_authority::parse_programs(&spec);
                if !notes.is_empty() {
                    return Err(format!("--programs: {}", notes.join("; ")));
                }
                programs = Some(spec);
            }
            "--" => literal = true,
            other if other.starts_with('-') && other.len() > 1 => {
                return Err(format!("unknown option {other}"));
            }
            _ => {
                if prompt.replace(argument.clone()).is_some() {
                    return Err(
                        "give exactly one prompt, either as an argument or with --prompt-file"
                            .to_string(),
                    );
                }
            }
        }
    }

    // Reading a prompt from a file is only ever a headless thing to want,
    // so it implies the mode rather than erroring about it.
    if prompt_file.is_some() {
        print = true;
    }
    // The deltas flag tunes the stream, so it asks for the stream rather
    // than implying it — the same way --json asks for --print.
    if json_deltas && !json {
        return Err("--json-deltas needs --json".to_string());
    }
    if !print {
        if json {
            return Err("--json needs --print".to_string());
        }
        if let Some(prompt) = prompt {
            return Err(format!("a prompt needs --print: coder -p \"{prompt}\""));
        }
        return Ok(Invocation::Interactive { trace, programs });
    }

    let prompt = match (prompt, prompt_file) {
        (Some(_), Some(_)) => {
            return Err(
                "give exactly one prompt, either as an argument or with --prompt-file".to_string(),
            );
        }
        (Some(prompt), None) => prompt,
        (None, Some(path)) => fs::read_to_string(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?,
        (None, None) => return Err("--print needs a prompt".to_string()),
    };
    if prompt.trim().is_empty() {
        return Err("the prompt is empty".to_string());
    }
    Ok(Invocation::Print(Print {
        prompt,
        trace,
        json,
        json_deltas,
        programs,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_of(arguments: &[&str]) -> Result<Invocation, String> {
        let arguments: Vec<String> = arguments.iter().map(|a| (*a).to_string()).collect();
        parse(&arguments)
    }

    /// `doctor` alone is the subcommand, and a version flag anywhere
    /// answers with the version.
    #[test]
    fn doctor_and_version_are_their_own_invocations() {
        assert_eq!(parse_of(&["doctor"]).unwrap(), Invocation::Doctor);
        assert_eq!(parse_of(&["--version"]).unwrap(), Invocation::Version);
        assert_eq!(parse_of(&["-V"]).unwrap(), Invocation::Version);
        assert!(matches!(
            parse_of(&["-p", "doctor"]).unwrap(),
            Invocation::Print(Print { prompt, .. }) if prompt == "doctor"
        ));
    }

    /// No arguments draws the terminal.
    #[test]
    fn nothing_is_the_terminal() {
        assert_eq!(
            parse_of(&[]).unwrap(),
            Invocation::Interactive {
                trace: None,
                programs: None
            }
        );
    }

    /// The prompt is the free argument, wherever the flags sit around it.
    #[test]
    fn the_prompt_reads_from_either_side_of_the_flags() {
        let expected = Invocation::Print(Print {
            prompt: "count the crates".to_string(),
            trace: Some(PathBuf::from("/tmp/one.jsonl")),
            json: true,
            json_deltas: false,
            programs: None,
        });
        for arguments in [
            vec![
                "-p",
                "count the crates",
                "--json",
                "--trace",
                "/tmp/one.jsonl",
            ],
            vec!["--json", "--trace=/tmp/one.jsonl", "-p", "count the crates"],
            vec![
                "--print",
                "--trace",
                "/tmp/one.jsonl",
                "--json",
                "--",
                "count the crates",
            ],
        ] {
            assert_eq!(parse_of(&arguments).unwrap(), expected, "{arguments:?}");
        }
    }

    /// A prompt file reads the prompt and implies the mode.
    #[test]
    fn a_prompt_file_implies_print() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ask.txt");
        std::fs::write(&path, "one\ntwo\n").unwrap();
        let path = path.display().to_string();
        let Invocation::Print(print) = parse_of(&["--prompt-file", &path]).unwrap() else {
            panic!("expected print mode");
        };
        assert_eq!(print.prompt, "one\ntwo\n");
        assert!(!print.json);
        assert!(!print.json_deltas);
    }

    /// Deltas are opt-in and only mean something to the JSON stream, so
    /// the flag asks for `--json` rather than implying it.
    #[test]
    fn json_deltas_ask_for_the_stream() {
        let Invocation::Print(print) = parse_of(&["-p", "hi", "--json", "--json-deltas"]).unwrap()
        else {
            panic!("expected print mode");
        };
        assert!(print.json && print.json_deltas);
        let error = parse_of(&["-p", "hi", "--json-deltas"]).unwrap_err();
        assert!(error.contains("--json"), "{error}");
    }

    /// The wrong command line says what is wrong with it.
    #[test]
    fn a_wrong_command_line_says_so() {
        for (arguments, expected) in [
            (vec!["-p"], "--print needs a prompt"),
            (vec!["-p", "  "], "the prompt is empty"),
            (
                vec!["-p", "a", "b"],
                "give exactly one prompt, either as an argument or with --prompt-file",
            ),
            (vec!["--json"], "--json needs --print"),
            (
                vec!["-p", "a", "--json-deltas"],
                "--json-deltas needs --json",
            ),
            (vec!["-p", "a", "--trace"], "--trace needs a value"),
            (vec!["--verbose"], "unknown option --verbose"),
            (vec!["hello"], "a prompt needs --print"),
        ] {
            let error = parse_of(&arguments).unwrap_err();
            assert!(
                error.contains(expected),
                "{arguments:?} said {error:?}, wanted {expected:?}"
            );
        }
    }

    /// A named trace file works in the terminal too: the flag means the
    /// same thing in both modes.
    #[test]
    fn the_terminal_takes_a_named_trace() {
        assert_eq!(
            parse_of(&["--trace", "/tmp/session.jsonl"]).unwrap(),
            Invocation::Interactive {
                trace: Some(PathBuf::from("/tmp/session.jsonl")),
                programs: None,
            }
        );
    }

    /// The program grant is the same word in both modes, and a spec that
    /// grants nothing is a usage error rather than a session that refuses
    /// every program it meets.
    #[test]
    fn a_program_grant_reads_the_same_in_both_modes() {
        let Invocation::Print(print) =
            parse_of(&["-p", "run the list", "--programs", "burn-down"]).unwrap()
        else {
            panic!("expected print mode");
        };
        assert_eq!(print.programs.as_deref(), Some("burn-down"));
        assert_eq!(
            parse_of(&["--programs", "all"]).unwrap(),
            Invocation::Interactive {
                trace: None,
                programs: Some("all".to_string()),
            }
        );
        let error = parse_of(&["-p", "a", "--programs", "burn down"]).unwrap_err();
        assert!(error.contains("--programs"), "{error}");
    }
}
