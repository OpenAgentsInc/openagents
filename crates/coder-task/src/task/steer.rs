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
//!
//! An accepted steer is not a consumed one (NIP-SESS, "Steering
//! capability"). Each task keeps a count of the messages taken from it, in
//! `<store>/local/<task>.steer.taken`, and [`add`] answers the place a
//! message holds in that count: the engine has read it once [`taken`]
//! reaches that place.

use std::io::Write;
use std::path::{Path, PathBuf};

/// The most bytes of one message.
pub const MAX_BYTES: usize = 32 * 1024;

fn path(store: &Path, task: &str) -> PathBuf {
    store.join("local").join(format!("{task}.steer.jsonl"))
}

fn taken_path(store: &Path, task: &str) -> PathBuf {
    store.join("local").join(format!("{task}.steer.taken"))
}

/// The messages waiting in `task`'s file, in the order they were sent.
fn waiting(store: &Path, task: &str) -> Vec<String> {
    std::fs::read_to_string(path(store, task))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str::<String>(line).ok())
        .filter(|message| !message.trim().is_empty())
        .collect()
}

/// Replace `task`'s waiting messages with `messages`, under its lock.
fn rewrite(store: &Path, task: &str, messages: &[String]) -> Result<(), String> {
    let at = path(store, task);
    if messages.is_empty() {
        return match std::fs::remove_file(&at) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.to_string()),
            _ => Ok(()),
        };
    }
    let mut body = String::new();
    for message in messages {
        body.push_str(&serde_json::to_string(message).map_err(|e| e.to_string())?);
        body.push('\n');
    }
    let mut file = crate::private::file(
        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true),
    )
    .open(&at)
    .map_err(|e| e.to_string())?;
    file.write_all(body.as_bytes()).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())
}

/// How many messages the engine has taken from `task`, ever.
#[must_use]
pub fn taken(store: &Path, task: &str) -> u64 {
    std::fs::read_to_string(taken_path(store, task))
        .ok()
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(0)
}

fn add_taken(store: &Path, task: &str, more: u64) {
    if more == 0 {
        return;
    }
    let total = taken(store, task).saturating_add(more);
    if let Ok(mut file) = crate::private::file(
        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true),
    )
    .open(taken_path(store, task))
    {
        let _ = writeln!(file, "{total}");
        let _ = file.sync_all();
    }
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

/// Leaves `text` for `task`'s running turn. Returns the message's place
/// in the count of messages taken from `task`: the engine has read it once
/// [`taken`] reaches it.
///
/// # Errors
/// The message is empty or too long, or cannot be written.
pub fn add(store: &Path, task: &str, text: &str) -> Result<u64, String> {
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
    file.sync_all().map_err(|e| e.to_string())?;
    let place = taken(store, task) + waiting(store, task).len() as u64;
    Ok(place)
}

/// Takes every message waiting for `task`, in the order they were sent,
/// and counts them as read.
#[must_use]
pub fn take(store: &Path, task: &str) -> Vec<String> {
    let at = path(store, task);
    if !at.exists() {
        return Vec::new();
    }
    let Ok(_held) = lock(store, task) else {
        return Vec::new();
    };
    let messages = waiting(store, task);
    let _ = std::fs::remove_file(&at);
    add_taken(store, task, messages.len() as u64);
    messages
}

/// Withdraws the first waiting message for `task` equal to `text`
/// without counting it as read, so it can reach the engine another way.
/// Returns whether one was waiting.
///
/// # Errors
/// The waiting messages cannot be rewritten.
pub fn withdraw(store: &Path, task: &str, text: &str) -> Result<bool, String> {
    if !path(store, task).exists() {
        return Ok(false);
    }
    let _held = lock(store, task)?;
    let mut messages = waiting(store, task);
    let Some(at) = messages.iter().position(|message| message == text.trim()) else {
        return Ok(false);
    };
    messages.remove(at);
    rewrite(store, task, &messages)?;
    Ok(true)
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

    #[test]
    fn a_message_is_read_once_the_taken_count_reaches_its_place() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(taken(dir.path(), "t1"), 0);
        assert_eq!(add(dir.path(), "t1", "First.").unwrap(), 1);
        assert_eq!(add(dir.path(), "t1", "Second.").unwrap(), 2);
        // Accepted, not yet read.
        assert_eq!(taken(dir.path(), "t1"), 0);
        assert_eq!(take(dir.path(), "t1").len(), 2);
        assert_eq!(taken(dir.path(), "t1"), 2);
        assert_eq!(add(dir.path(), "t1", "Third.").unwrap(), 3);
        assert_eq!(taken(dir.path(), "t1"), 2);
        assert_eq!(take(dir.path(), "t1"), ["Third."]);
        assert_eq!(taken(dir.path(), "t1"), 3);
    }

    #[test]
    fn a_withdrawn_message_is_not_counted_as_read() {
        let dir = tempfile::tempdir().unwrap();
        add(dir.path(), "t1", "Keep.").unwrap();
        add(dir.path(), "t1", "Withdraw.").unwrap();
        assert!(withdraw(dir.path(), "t1", "Withdraw.").unwrap());
        assert!(!withdraw(dir.path(), "t1", "Withdraw.").unwrap());
        assert_eq!(take(dir.path(), "t1"), ["Keep."]);
        assert_eq!(taken(dir.path(), "t1"), 1);
        assert!(!withdraw(dir.path(), "t1", "Keep.").unwrap());
        // The next message takes the place after what was read.
        assert_eq!(add(dir.path(), "t1", "Next.").unwrap(), 2);
    }
}
