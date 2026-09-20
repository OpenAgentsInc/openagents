//! The command line: what `coder` was asked to do before it does it.
//!
//! Two modes and four flags, parsed by hand. The crate takes no argument
//! parser as a dependency, and a surface this size does not earn one.

use std::fs;
use std::path::PathBuf;

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
    },
    /// Run one turn without a terminal.
    Print(Print),
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
    /// Report the turn as one JSON object rather than as the reply text.
    pub json: bool,
}

/// The usage text, printed for `--help` and after a usage error.
pub const USAGE: &str = "\
coder — the Coder agent, in a terminal or in a script.

Usage:
  coder                       Draw the terminal and talk.
  coder -p <PROMPT>           Run one turn, write the reply to stdout, exit.

Options:
  -p, --print <PROMPT>   Run one turn without a terminal.
      --prompt-file <FILE>
                         Read the prompt from a file. Implies --print.
      --trace <PATH>     Write this session's trace to PATH. Recording a
                         named file outranks CODER_TRACE.
      --json             With --print, report the turn as one JSON object:
                         the reply, the trace path, and how it finished.
  -h, --help             Show this text.

Exit codes with --print:
  0   The turn finished and the agent answered.
  1   The turn did not finish.
  2   The turn finished and the router declined it.
  64  The command line was wrong.

The environment picks the door. TYPESAFE_API_KEY turns classify on;
CODER_DOOR_KEY, CODER_DOOR_URL, and CODER_MODEL name an own-key door;
CODER_WORKER and CODER_RELAY route the turn through the relay; with none
of them set the stub door answers.";

/// Reads the command line.
///
/// `arguments` is everything after the program name.
///
/// # Errors
///
/// Returns a sentence naming what is wrong with the arguments. The caller
/// should print it, print [`USAGE`], and exit [`EXIT_USAGE`].
pub fn parse(arguments: &[String]) -> Result<Invocation, String> {
    let mut print = false;
    let mut json = false;
    let mut trace: Option<PathBuf> = None;
    let mut prompt: Option<String> = None;
    let mut prompt_file: Option<PathBuf> = None;

    // Everything after a bare `--` is a prompt, whatever it starts with.
    let mut literal = false;
    let mut rest = arguments.iter();
    while let Some(argument) = rest.next() {
        if literal {
            if prompt.replace(argument.clone()).is_some() {
                return Err("one prompt, or --prompt-file".to_string());
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
            "-p" | "--print" => {
                print = true;
                if let Some(attached) = attached.clone()
                    && prompt.replace(attached).is_some()
                {
                    return Err("one prompt, or --prompt-file".to_string());
                }
            }
            "--json" => json = true,
            "--trace" => trace = Some(PathBuf::from(value("--trace")?)),
            "--prompt-file" => prompt_file = Some(PathBuf::from(value("--prompt-file")?)),
            "--" => literal = true,
            other if other.starts_with('-') && other.len() > 1 => {
                return Err(format!("unknown option {other}"));
            }
            _ => {
                if prompt.replace(argument.clone()).is_some() {
                    return Err("one prompt, or --prompt-file".to_string());
                }
            }
        }
    }

    // Reading a prompt from a file is only ever a headless thing to want,
    // so it implies the mode rather than erroring about it.
    if prompt_file.is_some() {
        print = true;
    }
    if !print {
        if json {
            return Err("--json needs --print".to_string());
        }
        if let Some(prompt) = prompt {
            return Err(format!("a prompt needs --print: coder -p \"{prompt}\""));
        }
        return Ok(Invocation::Interactive { trace });
    }

    let prompt = match (prompt, prompt_file) {
        (Some(_), Some(_)) => return Err("one prompt, or --prompt-file".to_string()),
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
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_of(arguments: &[&str]) -> Result<Invocation, String> {
        let arguments: Vec<String> = arguments.iter().map(|a| (*a).to_string()).collect();
        parse(&arguments)
    }

    /// No arguments draws the terminal.
    #[test]
    fn nothing_is_the_terminal() {
        assert_eq!(
            parse_of(&[]).unwrap(),
            Invocation::Interactive { trace: None }
        );
    }

    /// The prompt is the free argument, wherever the flags sit around it.
    #[test]
    fn the_prompt_reads_from_either_side_of_the_flags() {
        let expected = Invocation::Print(Print {
            prompt: "count the crates".to_string(),
            trace: Some(PathBuf::from("/tmp/one.jsonl")),
            json: true,
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
    }

    /// The wrong command line says what is wrong with it.
    #[test]
    fn a_wrong_command_line_says_so() {
        for (arguments, expected) in [
            (vec!["-p"], "--print needs a prompt"),
            (vec!["-p", "  "], "the prompt is empty"),
            (vec!["-p", "a", "b"], "one prompt, or --prompt-file"),
            (vec!["--json"], "--json needs --print"),
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
            }
        );
    }
}
