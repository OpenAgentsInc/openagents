//! The Coder chats' transcripts as last shown, kept in memory and in the
//! app's encrypted store, so an open chat shows at once while its computer
//! is read again in the background.
//!
//! It is a display cache: a chat read from its computer always replaces
//! its copy, a copy of a turn the computer moved past is replaced by the
//! next turn's first read, and nothing here grants or starts anything.

use crate::conversation::Cached;
use coder_computers::cache::Cache;
use std::collections::HashMap;

/// The key of the list of kept chats, newest use last.
const INDEX: &str = "coder-transcripts";
/// The prefix of each kept chat's key; the task ID follows.
const PREFIX: &str = "coder-transcript-";
/// The most chats kept on disk.
pub const MAX_KEPT: usize = 32;
/// The most plaintext one kept chat may take; older rows go first.
const MAX_BYTES: usize = 160 * 1024;

/// Kept transcripts by task.
pub struct Transcripts {
    cache: Option<Cache>,
    memory: HashMap<String, Cached>,
    /// Kept tasks, least recently used first.
    index: Vec<String>,
}

impl Transcripts {
    /// Transcripts kept in `cache`. Without one they are kept only while
    /// the app runs.
    pub fn open(cache: Option<Cache>) -> Self {
        let index = cache
            .as_ref()
            .and_then(|cache| cache.read::<Vec<String>>(INDEX).ok().flatten())
            .unwrap_or_default();
        Self {
            cache,
            memory: HashMap::new(),
            index,
        }
    }

    /// The kept transcript of `task`, from memory or the store.
    pub fn get(&mut self, task: &str) -> Option<Cached> {
        if let Some(cached) = self.memory.get(task) {
            return Some(cached.clone());
        }
        let cached: Cached = self.cache.as_ref()?.read(&key(task)?).ok().flatten()?;
        self.memory.insert(task.to_owned(), cached.clone());
        Some(cached)
    }

    /// Keep `cached` as the transcript of `task`, trimmed to fit the store.
    pub fn put(&mut self, task: &str, mut cached: Cached) {
        if self.memory.get(task) == Some(&cached) {
            return;
        }
        fit(&mut cached);
        self.memory.insert(task.to_owned(), cached.clone());
        let (Some(cache), Some(name)) = (self.cache.as_ref(), key(task)) else {
            return;
        };
        if cache.write(&name, &cached).is_err() {
            return;
        }
        self.index.retain(|kept| kept != task);
        self.index.push(task.to_owned());
        while self.index.len() > MAX_KEPT {
            let dropped = self.index.remove(0);
            self.memory.remove(&dropped);
            if let Some(name) = key(&dropped) {
                let _ = cache.erase(&name);
            }
        }
        let _ = cache.write(INDEX, &self.index);
    }
}

/// A task's store key. A task ID is hexadecimal; anything else is not kept.
fn key(task: &str) -> Option<String> {
    (!task.is_empty() && task.len() <= 128 && task.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| format!("{PREFIX}{task}"))
}

/// Drop the oldest rows until the transcript fits one stored item; "Load
/// earlier" reads them again.
fn fit(cached: &mut Cached) {
    while serde_json::to_vec(cached).map_or(0, |bytes| bytes.len()) > MAX_BYTES
        && !cached.rows.is_empty()
    {
        let drop = cached.rows.len().div_ceil(4);
        cached.rows.drain(..drop);
        cached.previous = cached.rows.first().map(|row| row.offset);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversation::{CachedRow, Entry};
    use rust_native::MessageRole;

    fn cache(dir: &std::path::Path) -> Cache {
        let secret = secp256k1::SecretKey::from_byte_array([9; 32]).unwrap();
        Cache::open(dir, &secret).unwrap()
    }

    fn chat(source: &str) -> coder_history::Chat {
        coder_history::Chat {
            id: "chat".into(),
            harness: coder_history::Harness::Coder,
            native_id: Some("ab".repeat(32)),
            title: "Hello".into(),
            title_truncated: false,
            updated_at: None,
            archived: false,
            subagent: false,
            source_id: Some(source.into()),
            status: coder_history::SourceStatus::Available,
        }
    }

    fn transcript(rows: usize, text: &str) -> Cached {
        Cached {
            chat: chat("source"),
            rows: (0..rows as u64)
                .map(|index| CachedRow {
                    offset: index * 10,
                    end: index * 10 + 10,
                    part: 0,
                    entry: Entry::Message {
                        role: MessageRole::Assistant,
                        text: text.into(),
                    },
                })
                .collect(),
            previous: None,
            through: rows as u64 * 10,
        }
    }

    #[test]
    fn a_kept_transcript_survives_a_relaunch() {
        let temp = tempfile::tempdir().unwrap();
        let task = "cd".repeat(32);
        let kept = transcript(3, "Done.");
        Transcripts::open(Some(cache(temp.path()))).put(&task, kept.clone());
        let mut again = Transcripts::open(Some(cache(temp.path())));
        assert_eq!(again.get(&task), Some(kept));
        assert_eq!(again.get(&"ef".repeat(32)), None);
    }

    #[test]
    fn a_large_transcript_keeps_its_newest_rows_and_can_read_earlier() {
        let temp = tempfile::tempdir().unwrap();
        let task = "cd".repeat(32);
        let mut store = Transcripts::open(Some(cache(temp.path())));
        store.put(&task, transcript(200, &"x".repeat(2_000)));
        let kept = Transcripts::open(Some(cache(temp.path())))
            .get(&task)
            .unwrap();
        assert!(kept.rows.len() < 200 && !kept.rows.is_empty());
        assert_eq!(kept.rows.last().unwrap().end, 2_000);
        assert_eq!(kept.previous, Some(kept.rows[0].offset));
    }

    #[test]
    fn only_the_newest_chats_are_kept() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = Transcripts::open(Some(cache(temp.path())));
        let tasks: Vec<String> = (0..MAX_KEPT + 2).map(|i| format!("{i:064x}")).collect();
        for task in &tasks {
            store.put(task, transcript(1, "Hi."));
        }
        let mut again = Transcripts::open(Some(cache(temp.path())));
        assert_eq!(again.get(&tasks[0]), None);
        assert_eq!(again.get(&tasks[1]), None);
        assert!(again.get(&tasks[MAX_KEPT + 1]).is_some());
    }
}
