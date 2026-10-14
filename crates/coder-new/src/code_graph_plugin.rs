//! Code Graph (#11622): the `graph_*` tools, a built-in plugin on by
//! default that a person can turn off on the Plugins screen.
//!
//! The tools answer structure and impact questions from the repository's
//! code graph (`crates/code-graph`, `docs/code-graph/README.md`): the
//! graph published on the `code-graph` branch when it matches the
//! checkout, else a local build of a few seconds. They are offered where
//! the working directory is in a Git repository with Rust in it.
//!
//! Whether the plugin is on is a process-wide flag, as Token savers' is
//! ([`crate::token_savers`]): the app sets it when it loads its settings
//! and when the person toggles it; a headless run reads the saved
//! settings. [`ENV`] overrides both (`0` or `1`), for A/B runs.

use std::path::Path;
use std::sync::atomic::{AtomicU8, Ordering};

use serde_json::Value;

/// The plugin's ID.
pub const PLUGIN: &str = "code-graph";
/// What the Plugins screen says about it.
pub const DESCRIPTION: &str = "A map of this repository's code that Coder checks before opening files: where something is defined, what calls it, and what a change would affect.";
/// `0` turns the tools off and `1` on, whatever the settings say.
pub const ENV: &str = "OPENAGENTS_CODE_GRAPH";

static STATE: AtomicU8 = AtomicU8::new(UNSET);
const UNSET: u8 = 0;
const OFF: u8 = 1;
const ON: u8 = 2;

/// Records whether the plugin is on, for every later turn in this process.
pub fn set_enabled(enabled: bool) {
    STATE.store(if enabled { ON } else { OFF }, Ordering::Relaxed);
}

/// Whether the plugin is on.
#[must_use]
pub fn enabled() -> bool {
    match std::env::var(ENV).as_deref() {
        Ok("0") => return false,
        Ok("1") => return true,
        _ => {}
    }
    match STATE.load(Ordering::Relaxed) {
        ON => true,
        OFF => false,
        // A test never reads the person's own settings.
        _ if cfg!(test) => true,
        _ => crate::home::data_dir()
            .is_none_or(|dir| crate::bundled_settings::saved_enabled(&dir, PLUGIN)),
    }
}

/// Whether the tools are offered in `cwd`: the plugin is on and the folder
/// is in a Git checkout with a Cargo manifest at its top.
#[must_use]
pub fn offered(cwd: &Path) -> bool {
    enabled() && rust_checkout(cwd)
}

fn rust_checkout(cwd: &Path) -> bool {
    let mut dir = Some(cwd);
    while let Some(at) = dir {
        if at.join(".git").exists() {
            return at.join("Cargo.toml").is_file();
        }
        dir = at.parent();
    }
    false
}

/// Whether `name` is one of the tools.
#[must_use]
pub fn is_tool(name: &str) -> bool {
    code_graph::tools::is_tool(name)
}

/// The tools' definitions.
#[must_use]
pub fn definitions() -> Vec<Value> {
    code_graph::tools::definitions()
}

/// What the model is told about them.
pub const INSTRUCTIONS: &str = code_graph::tools::INSTRUCTIONS;

/// Run one tool off the async runtime: a first query may build the graph.
pub async fn execute(name: &str, arguments: Value, cwd: &Path) -> Result<Value, String> {
    let (name, cwd) = (name.to_owned(), cwd.to_path_buf());
    tokio::task::spawn_blocking(move || code_graph::tools::execute(&name, &arguments, &cwd))
        .await
        .map_err(|_| "The code graph query stopped.".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offered_only_in_a_rust_checkout() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!rust_checkout(dir.path()));
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        assert!(!rust_checkout(dir.path()));
        std::fs::write(dir.path().join("Cargo.toml"), "[workspace]\n").unwrap();
        std::fs::create_dir_all(dir.path().join("crates/x/src")).unwrap();
        assert!(rust_checkout(&dir.path().join("crates/x/src")));
        assert!(
            definitions()
                .iter()
                .all(|tool| is_tool(tool["function"]["name"].as_str().unwrap()))
        );
        assert_eq!(definitions().len(), 6);
    }
}
