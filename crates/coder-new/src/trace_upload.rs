//! `coder trace upload` and `coder trace list` (#11109): send a saved chat,
//! or any ATIF file, to the signed-in openagents.com account as a trace,
//! private unless shared. `openagents coder trace …` and `openagents trace
//! …` run the same commands ([`crate::programmatic`]).
//!
//! The trace is redacted here first ([`coder_sync::traces::prepare`]) and
//! the command says what was left out. The sign-in is `coder login`'s.

use std::path::Path;

use coder_sync::traces::{self, Trace, Uploaded};
use openagents_login::Saved;
use serde_json::{Value, json};

use crate::sessions;

/// How to use it.
pub const USAGE: &str = "Usage: coder trace upload [SESSION_ID | --last | --file PATH] [--share]\n       coder trace list\n\nupload  Send a saved chat (or an ATIF file) to your openagents.com account.\n        Passwords, keys, home folder names, and email addresses are taken out first.\n        It's private unless you add --share, which gives it a public link.\nlist    Show the traces on your account.\n\nSign in first with coder login.";

/// What a command did, for the terminal or as JSON.
#[derive(Debug)]
pub enum Outcome {
    Uploaded {
        uploaded: Uploaded,
        left_out: Option<String>,
    },
    Listed(Vec<Trace>),
}

impl Outcome {
    /// The lines the terminal prints.
    #[must_use]
    pub fn text(&self) -> String {
        match self {
            Self::Uploaded { uploaded, left_out } => {
                let mut lines = Vec::new();
                if let Some(left_out) = left_out {
                    lines.push(left_out.clone());
                }
                let trace = &uploaded.trace;
                let steps = if trace.steps == 1 {
                    "1 step".to_owned()
                } else {
                    format!("{} steps", trace.steps)
                };
                lines.push(if uploaded.existing {
                    format!("\"{}\" ({steps}) is already on your account.", trace.title)
                } else {
                    format!("Uploaded \"{}\" ({steps}).", trace.title)
                });
                match &trace.share_url {
                    Some(public) if trace.shared => {
                        lines.push("Shared: anyone with this link can see it.".into());
                        lines.push(public.clone());
                    }
                    _ => {
                        lines.push("Private: only you can see it, signed in.".into());
                        lines.push(trace.url.clone());
                    }
                }
                lines.join("\n")
            }
            Self::Listed(listed) if listed.is_empty() => {
                "No traces yet. Upload your latest chat with coder trace upload --last.".into()
            }
            Self::Listed(listed) => listed
                .iter()
                .map(|trace| {
                    let day = atif::iso(trace.uploaded_unix.saturating_mul(1000));
                    format!(
                        "{}  ({} steps, {}, {})\n  {}",
                        trace.title,
                        trace.steps,
                        &day[..10],
                        if trace.shared { "shared" } else { "private" },
                        trace.share_url.as_ref().unwrap_or(&trace.url)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }

    /// The same, as JSON.
    #[must_use]
    pub fn json(&self) -> Value {
        let trace = |trace: &Trace| {
            json!({
                "id": trace.id, "title": trace.title, "steps": trace.steps,
                "uploaded_unix": trace.uploaded_unix, "shared": trace.shared,
                "url": trace.url, "share_url": trace.share_url,
            })
        };
        match self {
            Self::Uploaded { uploaded, left_out } => json!({
                "trace": trace(&uploaded.trace),
                "existing": uploaded.existing,
                "left_out": left_out,
            }),
            Self::Listed(listed) => json!({"traces": listed.iter().map(trace).collect::<Vec<_>>()}),
        }
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The trace to send, from the arguments after `upload`.
fn source(args: &[String], dir: &Path, cwd: &Path) -> Result<(Value, bool), String> {
    let mut share = false;
    let mut chosen: Option<Value> = None;
    let mut args = args.iter();
    let choose = |chosen: &mut Option<Value>, value: Value| {
        if chosen.is_some() {
            return Err("Choose one chat or file to upload.".to_string());
        }
        *chosen = Some(value);
        Ok(())
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--share" => share = true,
            "--last" => {
                let store = sessions::Store::under(dir);
                let last = store
                    .recent(1)?
                    .into_iter()
                    .next()
                    .ok_or("You have no saved chats yet.")?;
                choose(&mut chosen, store.read(&last.id)?)?;
            }
            "--file" => {
                let path = args.next().ok_or("--file needs a path.")?;
                choose(&mut chosen, sessions::read_document(&cwd.join(path))?)?;
            }
            id if !id.starts_with('-') => {
                let store = sessions::Store::under(dir);
                store.path(id)?;
                let document = store.read(id).map_err(|error| {
                    if store.path(id).is_ok_and(|path| !path.exists()) {
                        format!("No saved chat has the id {id}.")
                    } else {
                        error
                    }
                })?;
                choose(&mut chosen, document)?;
            }
            other => return Err(format!("Unknown option {other}.\n\n{USAGE}")),
        }
    }
    let document = chosen.ok_or_else(|| {
        "Choose what to upload: --last for your latest chat, a session id, or --file PATH."
            .to_string()
    })?;
    Ok((document, share))
}

fn signed_in(dir: &Path) -> Result<Saved, String> {
    Saved::load(dir)
        .filter(|saved| !saved.expired(now()))
        .ok_or_else(|| "Sign in first with coder login.".to_string())
}

/// Run `trace ARGS` with Coder's folder `dir`, reading files from `cwd`.
///
/// # Errors
/// What went wrong, in words to show.
pub fn run(args: &[String], dir: &Path, cwd: &Path) -> Result<Outcome, String> {
    match args.split_first() {
        Some((command, rest)) if command == "upload" => {
            let (document, share) = source(rest, dir, cwd)?;
            let saved = signed_in(dir)?;
            let prepared = traces::prepare(document, &secret_screen::Screen::host())?;
            let uploaded = traces::upload(&saved, &prepared, share)?;
            Ok(Outcome::Uploaded {
                uploaded,
                left_out: traces::left_out_text(&prepared.left_out),
            })
        }
        Some((command, rest)) if command == "list" && rest.is_empty() => {
            Ok(Outcome::Listed(traces::list(&signed_in(dir)?)?))
        }
        _ => Err(USAGE.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| (*w).to_owned()).collect()
    }

    #[test]
    fn sources_are_chosen_once_and_signed_in_is_needed() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("state");
        let document = json!({"schema_version": "ATIF-v1.8", "session_id": "chat-1",
            "steps": [{"step_id": 1, "source": "user", "message": "Fix it"}]});
        sessions::Store::under(&dir)
            .save("chat-1", &document)
            .unwrap();
        std::fs::write(temp.path().join("t.json"), document.to_string()).unwrap();
        let (found, share) = source(&args(&["chat-1", "--share"]), &dir, temp.path()).unwrap();
        assert!(share);
        assert_eq!(found["session_id"], "chat-1");
        assert!(source(&args(&["--last"]), &dir, temp.path()).is_ok());
        assert!(source(&args(&["--file", "t.json"]), &dir, temp.path()).is_ok());
        assert!(
            source(&args(&["--last", "chat-1"]), &dir, temp.path())
                .unwrap_err()
                .contains("one")
        );
        assert!(
            source(&args(&["nope"]), &dir, temp.path())
                .unwrap_err()
                .contains("No saved chat")
        );
        assert!(
            source(&args(&[]), &dir, temp.path())
                .unwrap_err()
                .contains("--last")
        );
        assert_eq!(
            run(&args(&["upload", "--last"]), &dir, temp.path()).unwrap_err(),
            "Sign in first with coder login."
        );
        assert!(
            run(&args(&["nope"]), &dir, temp.path())
                .unwrap_err()
                .contains("Usage")
        );
    }

    #[test]
    fn the_terminal_says_where_it_went() {
        let trace = Trace {
            id: "t1".into(),
            title: "Fix it".into(),
            steps: 3,
            uploaded_unix: 1_791_504_000,
            shared: false,
            url: "https://openagents.com/settings/traces/t1".into(),
            share_url: None,
        };
        let private = Outcome::Uploaded {
            uploaded: Uploaded {
                trace: trace.clone(),
                existing: false,
            },
            left_out: Some("Left out before upload: 1 password or key.".into()),
        }
        .text();
        assert_eq!(
            private,
            "Left out before upload: 1 password or key.\nUploaded \"Fix it\" (3 steps).\nPrivate: only you can see it, signed in.\nhttps://openagents.com/settings/traces/t1"
        );
        let shared = Outcome::Uploaded {
            uploaded: Uploaded {
                trace: Trace {
                    shared: true,
                    share_url: Some("https://openagents.com/trace/t1".into()),
                    ..trace.clone()
                },
                existing: true,
            },
            left_out: None,
        };
        assert!(
            shared
                .text()
                .ends_with("anyone with this link can see it.\nhttps://openagents.com/trace/t1")
        );
        assert_eq!(
            shared.json()["trace"]["share_url"],
            "https://openagents.com/trace/t1"
        );
        let listed = Outcome::Listed(vec![trace]).text();
        assert!(
            listed.contains("Fix it  (3 steps, 2026-10-09, private)"),
            "{listed}"
        );
        assert!(Outcome::Listed(Vec::new()).text().contains("--last"));
        for text in [private, listed] {
            assert!(oa_copy::violations(&text, &[]).is_empty(), "{text}");
        }
    }
}
