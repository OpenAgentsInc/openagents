//! `coder export --account` (#11134): download everything on the signed-in
//! openagents.com account into one file, the same file Settings → Your data
//! → Export everything gives. `openagents coder export --account` runs the
//! same command ([`crate::programmatic`]).
//!
//! The sign-in is `coder login`'s; the file never holds a key, token, or
//! password, and is written only as a new file (`0600`).

use std::path::{Path, PathBuf};

use coder_sync::account_export::{self, Export};
use openagents_login::Saved;
use serde_json::{Value, json};

/// How to use it.
pub const USAGE: &str = "Usage: coder export --account [--output FILE]\n\nDownload everything on your openagents.com account into one file: your chats (each with a Markdown copy), projects, traces, computers, and settings. Keys, tokens, and passwords are never in it.\n\n--output FILE  Where to save it (default: openagents-export-DATE.json here). An existing file is never replaced.\n\nSign in first with coder login.";

/// What the command did.
#[derive(Debug)]
pub struct Outcome {
    pub path: PathBuf,
    pub chats: usize,
    pub traces: usize,
    pub bytes: usize,
    pub unavailable: Vec<String>,
}

fn count(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

impl Outcome {
    fn of(export: &Export, path: PathBuf) -> Self {
        Self {
            path,
            chats: export.chats,
            traces: export.traces,
            bytes: export.bytes.len(),
            unavailable: export.unavailable.clone(),
        }
    }

    /// The lines the terminal prints.
    #[must_use]
    pub fn text(&self) -> String {
        let mut lines = vec![format!(
            "Saved your account ({}, {}) to {}.",
            count(self.chats, "chat", "chats"),
            count(self.traces, "trace", "traces"),
            self.path.display()
        )];
        if !self.unavailable.is_empty() {
            lines.push(format!(
                "Not in this file, because openagents.com couldn't read it just now: {}. Run the command again later to get it.",
                self.unavailable.join(", ").replace('_', " ")
            ));
        }
        lines.push("Keys, tokens, and passwords are never in it.".into());
        lines.join("\n")
    }

    /// The same, as JSON.
    #[must_use]
    pub fn json(&self) -> Value {
        json!({
            "path": self.path,
            "chats": self.chats,
            "traces": self.traces,
            "bytes": self.bytes,
            "unavailable": self.unavailable,
        })
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The options after `export`: `--account` (required) and `--output FILE`.
fn options(args: &[String]) -> Result<Option<String>, String> {
    let mut account = false;
    let mut output = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--account" => account = true,
            "--output" | "-o" => {
                if output.is_some() {
                    return Err("Choose one --output file.".into());
                }
                output = Some(args.next().ok_or("--output needs a file.")?.clone());
            }
            other => return Err(format!("Unknown option {other}.\n\n{USAGE}")),
        }
    }
    if !account {
        return Err(USAGE.into());
    }
    Ok(output)
}

/// Run `export ARGS` with Coder's folder `dir`, saving under `cwd`.
///
/// # Errors
/// What went wrong, in words to show.
pub fn run(args: &[String], dir: &Path, cwd: &Path) -> Result<Outcome, String> {
    let output = options(args)?;
    let saved = Saved::load(dir)
        .filter(|saved| !saved.expired(now()))
        .ok_or_else(|| "Sign in first with coder login.".to_string())?;
    let export = account_export::download(&saved, &account_export::today())?;
    let path = cwd.join(output.unwrap_or_else(|| export.file_name.clone()));
    let path = account_export::save(&export, &path)?;
    Ok(Outcome::of(&export, path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| (*w).to_owned()).collect()
    }

    #[test]
    fn account_is_required_and_output_is_optional() {
        assert_eq!(options(&args(&["--account"])).unwrap(), None);
        assert_eq!(
            options(&args(&["--account", "--output", "me.json"])).unwrap(),
            Some("me.json".into())
        );
        assert_eq!(options(&args(&[])).unwrap_err(), USAGE);
        assert!(
            options(&args(&["--account", "--output"]))
                .unwrap_err()
                .contains("needs a file")
        );
        assert!(
            options(&args(&["--account", "--everything"]))
                .unwrap_err()
                .starts_with("Unknown option --everything.")
        );
    }

    #[test]
    fn signed_out_is_said_before_anything_is_asked() {
        let temp = tempfile::tempdir().unwrap();
        assert_eq!(
            run(&args(&["--account"]), temp.path(), temp.path()).unwrap_err(),
            "Sign in first with coder login."
        );
        assert!(std::fs::read_dir(temp.path()).unwrap().next().is_none());
    }

    #[test]
    fn the_outcome_names_the_file_and_what_was_left_for_later() {
        let outcome = Outcome {
            path: PathBuf::from("/tmp/openagents-export-2026-10-10.json"),
            chats: 1,
            traces: 3,
            bytes: 10,
            unavailable: vec!["claude_credential".into()],
        };
        let text = outcome.text();
        assert!(
            text.starts_with(
                "Saved your account (1 chat, 3 traces) to /tmp/openagents-export-2026-10-10.json."
            ),
            "{text}"
        );
        assert!(
            text.contains("couldn't read it just now: claude credential."),
            "{text}"
        );
        assert_eq!(outcome.json()["traces"], 3);
        let text = Outcome {
            unavailable: Vec::new(),
            ..outcome
        }
        .text();
        assert!(!text.contains("couldn't read"), "{text}");
    }
}
