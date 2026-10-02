//! Messages the person sends a running turn: steering.
//!
//! A message for a running turn waits beside the task, in
//! `<store>/local/<task>.steer.jsonl`, one JSON string per line, until the
//! engine reads it. Microcoder's loop reads waiting messages at the start
//! of each step and before it lets a step finish
//! (`microcoder_loop::env::Env::steering`), records each in the turn's
//! trace as the person's message, and every later step's prompt carries
//! it. Taking messages removes them, under a lock that adding takes too,
//! so a message is read once.
//!
//! A message the turn did not read before it ended, and a message for a
//! whole coding agent (Grok Build, OpenCode, Devin), which reads its
//! instructions only when a turn starts, start the next turn instead
//! ([`super::local::Local::steer`]).

use std::io::Write;
use std::path::{Path, PathBuf};

/// The most bytes of one message.
pub const MAX_BYTES: usize = 32 * 1024;

fn path(store: &Path, task: &str) -> PathBuf {
    store.join("local").join(format!("{task}.steer.jsonl"))
}

fn lock(store: &Path, task: &str) -> Result<std::fs::File, String> {
    let dir = store.join("local");
    crate::private::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = crate::private::file(
        std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true),
    )
    .open(dir.join(format!("{task}.steer.lock")))
    .map_err(|e| e.to_string())?;
    file.lock().map_err(|e| e.to_string())?;
    Ok(file)
}

/// Leaves `text` for `task`'s running turn.
///
/// # Errors
/// The message is empty or too long, or cannot be written.
pub fn add(store: &Path, task: &str, text: &str) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() || text.len() > MAX_BYTES {
        return Err("A message for Coder is 1 to 32,768 bytes.".into());
    }
    let _held = lock(store, task)?;
    let line = serde_json::to_string(text).map_err(|e| e.to_string())?;
    let mut file = crate::private::file(std::fs::OpenOptions::new().create(true).append(true))
        .open(path(store, task))
        .map_err(|e| e.to_string())?;
    writeln!(file, "{line}").map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())
}

/// Takes every message waiting for `task`, in the order they were sent.
#[must_use]
pub fn take(store: &Path, task: &str) -> Vec<String> {
    let at = path(store, task);
    if !at.exists() {
        return Vec::new();
    }
    let Ok(_held) = lock(store, task) else {
        return Vec::new();
    };
    let text = std::fs::read_to_string(&at).unwrap_or_default();
    let _ = std::fs::remove_file(&at);
    text.lines()
        .filter_map(|line| serde_json::from_str::<String>(line).ok())
        .filter(|message| !message.trim().is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_are_read_once_in_the_order_they_were_sent() {
        let dir = tempfile::tempdir().unwrap();
        assert!(take(dir.path(), "t1").is_empty());
        add(dir.path(), "t1", "Use tabs.\nNot spaces.").unwrap();
        add(dir.path(), "t1", "  And keep the README.  ").unwrap();
        add(dir.path(), "t2", "Another task.").unwrap();
        assert!(add(dir.path(), "t1", "   ").is_err());
        assert_eq!(
            take(dir.path(), "t1"),
            ["Use tabs.\nNot spaces.", "And keep the README."]
        );
        assert!(take(dir.path(), "t1").is_empty());
        assert_eq!(take(dir.path(), "t2"), ["Another task."]);
    }
}
