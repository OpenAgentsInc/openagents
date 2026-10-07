//! `openagents scratch`: the calling session's durable scratch directory
//! (`coder_lease::scratch`), created private to this user and printed, so
//! an agent keeps captures, scripts, and notes somewhere a reboot doesn't
//! erase. `docs/coder/guides/scratch.md` is the guide.

use std::path::PathBuf;

#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};
use coder_lease::scratch;
use serde_json::{Value, json};

use crate::{Args, Output};

pub(crate) const USAGE: &str = "usage: openagents scratch [--session SESSION]
Create this session's durable scratch directory, private to you (mode
0700), and print its path: ~/.openagents/scratch/<session>/. Keep
captures, scripts, and notes there rather than in /tmp, which a reboot
clears; evidence a check needs belongs in the repository or the task
store. The session is --session SESSION; else the directory
$OPENAGENTS_SCRATCH names, which a lease or a Coder delegation sets; else
the session the lease broker detects: $OPENAGENTS_SESSION, the agent's own
session variable, or the nearest agent process above this one. The session
becomes a directory name of letters, digits, `.`, `_`, and `-`.
$OPENAGENTS_SCRATCH_ROOT moves the root. The disk cleanup rule removes
the scratch of a session that ended at least seven days ago.";

/// What the command does, for the chat router's command tree
/// (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[Declared::computer("", Effect::LocalWrite)];

pub fn run(output: &Output, words: &[String]) -> u8 {
    if let Some(first) = words.first()
        && matches!(first.as_str(), "--help" | "-h" | "help")
    {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(words, &[]) {
        Ok(args) => args,
        Err(message) => return output.usage("scratch", &message, USAGE),
    };
    if let Some(word) = args.positional().first() {
        return output.usage("scratch", &format!("unexpected word `{word}`"), USAGE);
    }
    if let Some(name) = args
        .option_names()
        .into_iter()
        .find(|name| *name != "session")
    {
        return output.usage("scratch", &format!("unknown option `--{name}`"), USAGE);
    }
    let given = args.option("session").map(str::to_owned);
    if given.as_deref().is_some_and(str::is_empty) {
        return output.usage("scratch", "--session needs a session name", USAGE);
    }
    match locate(given) {
        Ok(value) => {
            output.emit(&value, |value| {
                value["path"].as_str().unwrap_or_default().to_owned()
            });
            0
        }
        Err(message) => output.fail("scratch", &message),
    }
}

/// Creates the scratch directory and describes it: its path, session, and
/// root.
fn locate(given: Option<String>) -> Result<Value, String> {
    let inherited = std::env::var_os(scratch::SCRATCH_VAR)
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from);
    if given.is_none()
        && let Some(dir) = inherited
    {
        scratch::make_private(&dir)
            .map_err(|error| format!("{} can't be used: {error}", dir.display()))?;
        return Ok(json!({
            "path": dir.display().to_string(),
            "session": scratch::session_of(&dir),
            "root": dir.parent().map(|root| root.display().to_string()),
            "from": scratch::SCRATCH_VAR,
        }));
    }
    let (session, from) = match given {
        Some(session) => (session, "--session"),
        None => (
            coder_lease::Session::detect(&|name| std::env::var(name).ok(), &coder_lease::ancestors)
                .id,
            "detected",
        ),
    };
    let root = scratch::root_from_env()?;
    let path = scratch::ensure(&root, &session).map_err(|error| {
        format!(
            "the scratch directory could not be made under {}: {error}",
            root.display()
        )
    })?;
    Ok(json!({
        "path": path.display().to_string(),
        "session": session,
        "root": root.display().to_string(),
        "from": from,
    }))
}
