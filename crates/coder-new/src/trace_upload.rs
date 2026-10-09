//! `coder trace upload` and `coder trace list` (#11109): send a saved chat,
//! or any ATIF file, to the signed-in openagents.com account as a trace,
//! private unless shared. `openagents coder trace …` and `openagents trace
//! …` run the same commands ([`crate::programmatic`]).
//!
//! The trace is redacted here first ([`coder_sync::traces::prepare`]) and
//! the command says what was left out. The sign-in is `coder login`'s.
//!
//! A whole orchestration goes up as a tree (#11178): `--claude-session`
//! converts a Claude Code session and every agent it started
//! ([`coder_sync::claude_session`]), and a saved chat or file that carries
//! its agents inline (`subagent_trajectories`) is split the same way. The
//! main conversation becomes the trace; each agent is saved under its
//! parent, and the trace's page shows the tree.

use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

use coder_sync::claude_session;
use coder_sync::traces::{self, Trace, TreeUploaded, Uploaded};
use openagents_login::Saved;
use serde_json::{Value, json};

use crate::sessions;

/// How to use it.
pub const USAGE: &str = "Usage: coder trace upload [SESSION_ID | --last | --file PATH | --claude-session ID|PATH] [--share]\n       coder trace list\n\nupload  Send a saved chat, an ATIF file, or a Claude Code session (.jsonl) to your openagents.com account.\n        --claude-session (or --file SESSION.jsonl) sends a Claude Code session with every agent it started.\n        Passwords, keys, home folder names, and email addresses are taken out first.\n        It's private unless you add --share, which gives it a public link.\nlist    Show the traces on your account.\n\nSign in first with coder login.";

/// What a command did, for the terminal or as JSON.
#[derive(Debug)]
pub enum Outcome {
    Uploaded {
        uploaded: Uploaded,
        left_out: Option<String>,
    },
    /// A main conversation and its agents.
    UploadedTree {
        uploaded: TreeUploaded,
        left_out: Option<String>,
    },
    Listed(Vec<Trace>),
}

fn count(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

fn where_it_is(trace: &Trace, lines: &mut Vec<String>) {
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
}

impl Outcome {
    /// The lines the terminal prints.
    #[must_use]
    pub fn text(&self) -> String {
        match self {
            Self::Uploaded { uploaded, left_out } => {
                let mut lines: Vec<String> = left_out.iter().cloned().collect();
                let trace = &uploaded.trace;
                let steps = count(trace.steps, "step", "steps");
                lines.push(if uploaded.existing {
                    format!("\"{}\" ({steps}) is already on your account.", trace.title)
                } else {
                    format!("Uploaded \"{}\" ({steps}).", trace.title)
                });
                where_it_is(trace, &mut lines);
                lines.join("\n")
            }
            Self::UploadedTree { uploaded, left_out } => {
                let mut lines: Vec<String> = left_out.iter().cloned().collect();
                let trace = &uploaded.trace;
                lines.push(format!(
                    "Uploaded \"{}\" ({}) with {}.",
                    trace.title,
                    count(trace.steps, "step", "steps"),
                    count(uploaded.agents, "agent", "agents")
                ));
                if !uploaded.failed.is_empty() {
                    lines.push(format!(
                        "{} couldn't be saved:",
                        count(uploaded.failed.len(), "agent", "agents")
                    ));
                    for failed in uploaded.failed.iter().take(10) {
                        lines.push(format!("  {failed}"));
                    }
                    lines.push("Run the same command again to retry them.".into());
                }
                where_it_is(trace, &mut lines);
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
            Self::UploadedTree { uploaded, left_out } => json!({
                "trace": trace(&uploaded.trace),
                "existing": uploaded.existing,
                "agents": uploaded.agents,
                "failed": uploaded.failed,
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

/// What to send.
#[derive(Debug)]
enum Source {
    Document(Value),
    /// A Claude Code session's main file.
    Claude(PathBuf),
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map_or_else(PathBuf::new, PathBuf::from)
}

/// The trace to send, from the arguments after `upload`.
fn source(args: &[String], dir: &Path, cwd: &Path) -> Result<(Source, bool), String> {
    let mut share = false;
    let mut chosen: Option<Source> = None;
    let mut args = args.iter();
    let choose = |chosen: &mut Option<Source>, value: Source| {
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
                choose(&mut chosen, Source::Document(store.read(&last.id)?))?;
            }
            "--file" => {
                let path = cwd.join(args.next().ok_or("--file needs a path.")?);
                // A Claude Code session file (#11154) goes up with its
                // agents, the same as --claude-session.
                let source = if claude_session::is_session_file(&path) {
                    Source::Claude(path)
                } else {
                    Source::Document(sessions::read_document(&path)?)
                };
                choose(&mut chosen, source)?;
            }
            "--claude-session" => {
                let session = args
                    .next()
                    .ok_or("--claude-session needs a session id or folder.")?;
                choose(
                    &mut chosen,
                    Source::Claude(claude_session::find(session, &home(), cwd)?),
                )?;
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
                choose(&mut chosen, Source::Document(document))?;
            }
            other => return Err(format!("Unknown option {other}.\n\n{USAGE}")),
        }
    }
    let document = chosen.ok_or_else(|| {
        "Choose what to upload: --last for your latest chat, a session id, --file PATH, or --claude-session ID."
            .to_string()
    })?;
    Ok((document, share))
}

fn signed_in(dir: &Path) -> Result<Saved, String> {
    Saved::load(dir)
        .filter(|saved| !saved.expired(now()))
        .ok_or_else(|| "Sign in first with coder login.".to_string())
}

fn has_agents(document: &Value) -> bool {
    document
        .get("subagent_trajectories")
        .and_then(Value::as_array)
        .is_some_and(|children| !children.is_empty())
}

/// Run `trace ARGS` with Coder's folder `dir`, reading files from `cwd`.
///
/// # Errors
/// What went wrong, in words to show.
pub fn run(args: &[String], dir: &Path, cwd: &Path) -> Result<Outcome, String> {
    match args.split_first() {
        Some((command, rest)) if command == "upload" => {
            let (source, share) = source(rest, dir, cwd)?;
            let saved = signed_in(dir)?;
            let screen = secret_screen::Screen::host();
            let (nodes, left_out) = match source {
                Source::Claude(main) => claude_session::convert(&main, &screen)?,
                Source::Document(document) if has_agents(&document) => (
                    claude_session::split_inline(document),
                    secret_screen::Counts::new(),
                ),
                Source::Document(document) => {
                    let prepared = traces::prepare(document, &screen)?;
                    let uploaded = traces::upload(&saved, &prepared, share)?;
                    return Ok(Outcome::Uploaded {
                        uploaded,
                        left_out: traces::left_out_text(&prepared.left_out),
                    });
                }
            };
            let tree = traces::prepare_tree(nodes, &screen, left_out)?;
            let terminal = std::io::stderr().is_terminal();
            let uploaded = traces::upload_tree(&saved, &tree, share, &mut |done, total| {
                if terminal && total > 1 {
                    eprint!("\rUploading {done} of {total}…");
                    if done == total {
                        eprintln!();
                    }
                    let _ = std::io::stderr().flush();
                }
            })?;
            Ok(Outcome::UploadedTree {
                uploaded,
                left_out: traces::left_out_text(&tree.left_out),
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
        let Source::Document(found) = found else {
            panic!("not a document");
        };
        assert_eq!(found["session_id"], "chat-1");
        assert!(!has_agents(&found));
        assert!(has_agents(
            &json!({"subagent_trajectories": [{"steps": []}]})
        ));
        assert!(source(&args(&["--last"]), &dir, temp.path()).is_ok());
        assert!(source(&args(&["--file", "t.json"]), &dir, temp.path()).is_ok());
        std::fs::create_dir_all(temp.path().join("s")).unwrap();
        std::fs::write(temp.path().join("s.jsonl"), "{}").unwrap();
        assert!(matches!(
            source(&args(&["--claude-session", "s"]), &dir, temp.path()).unwrap().0,
            Source::Claude(path) if path.ends_with("s.jsonl")
        ));
        assert!(matches!(
            source(&args(&["--file", "s.jsonl"]), &dir, temp.path())
                .unwrap()
                .0,
            Source::Claude(_)
        ));
        assert!(
            source(&args(&["--claude-session"]), &dir, temp.path())
                .unwrap_err()
                .contains("needs")
        );
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
        let tree = Outcome::UploadedTree {
            uploaded: TreeUploaded {
                trace: trace.clone(),
                existing: false,
                agents: 3,
                failed: vec!["Agent b: openagents.com refused the trace (400).".into()],
            },
            left_out: None,
        };
        let text = tree.text();
        assert!(
            text.starts_with(
                "Uploaded \"Fix it\" (3 steps) with 3 agents.\n1 agent couldn't be saved:"
            ),
            "{text}"
        );
        assert_eq!(tree.json()["agents"], 3);
        let listed = Outcome::Listed(vec![trace]).text();
        assert!(
            listed.contains("Fix it  (3 steps, 2026-10-09, private)"),
            "{listed}"
        );
        assert!(Outcome::Listed(Vec::new()).text().contains("--last"));
        for text in [private, listed, text] {
            assert!(oa_copy::violations(&text, &[]).is_empty(), "{text}");
        }
    }
}
