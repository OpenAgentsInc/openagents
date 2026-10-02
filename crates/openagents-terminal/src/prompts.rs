//! The prompts you sent, so Up reaches them after a restart.
//!
//! One file, `prompts.json` in the client's chat home, holds the prompts
//! only, oldest first: no replies, no thread IDs. A scratch screen has its
//! own chat home, so it keeps its own.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The most prompts the file keeps; the oldest go first.
pub const PROMPTS_MAX: usize = 500;
/// A longer prompt is sent but not saved.
pub const PROMPT_BYTES_MAX: usize = 16 * 1024;

#[derive(Default, Serialize, Deserialize)]
struct File {
    #[serde(default)]
    prompts: Vec<String>,
}

fn path(home: &Path) -> PathBuf {
    home.join("prompts.json")
}

/// The saved prompts, oldest first.
pub fn read(home: &Path) -> Vec<String> {
    let mut prompts = std::fs::read(path(home))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<File>(&bytes).ok())
        .unwrap_or_default()
        .prompts;
    let over = prompts.len().saturating_sub(PROMPTS_MAX);
    prompts.drain(..over);
    prompts
}

/// Save `prompt` as the newest. The same prompt twice in a row is kept
/// once. A failure to write costs only the history, so it is not reported.
pub fn remember(home: &Path, prompt: &str) {
    if prompt.trim().is_empty() || prompt.len() > PROMPT_BYTES_MAX {
        return;
    }
    let mut prompts = read(home);
    if prompts.last().is_some_and(|last| last == prompt) {
        return;
    }
    prompts.push(prompt.to_owned());
    let over = prompts.len().saturating_sub(PROMPTS_MAX);
    prompts.drain(..over);
    if let Ok(bytes) = serde_json::to_vec(&File { prompts }) {
        let _ = std::fs::create_dir_all(home);
        let temporary = path(home).with_extension("json.tmp");
        if std::fs::write(&temporary, bytes).is_ok() {
            let _ = std::fs::rename(&temporary, path(home));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompts_survive_in_order_without_repeats() {
        let home = tempfile::tempdir().unwrap();
        assert!(read(home.path()).is_empty());
        remember(home.path(), "first");
        remember(home.path(), "second\nline");
        remember(home.path(), "second\nline");
        remember(home.path(), "  ");
        remember(home.path(), &"x".repeat(PROMPT_BYTES_MAX + 1));
        assert_eq!(read(home.path()), ["first", "second\nline"]);
        assert!(!home.path().join("prompts.json.tmp").exists());
    }

    #[test]
    fn the_oldest_prompts_go_first_and_a_bad_file_is_empty() {
        let home = tempfile::tempdir().unwrap();
        for index in 0..PROMPTS_MAX + 2 {
            remember(home.path(), &format!("p{index}"));
        }
        let prompts = read(home.path());
        assert_eq!(prompts.len(), PROMPTS_MAX);
        assert_eq!(prompts[0], "p2");
        std::fs::write(home.path().join("prompts.json"), b"not json").unwrap();
        assert!(read(home.path()).is_empty());
    }
}
