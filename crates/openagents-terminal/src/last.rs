//! The last thread the screen had open in each folder, so `openagents
//! terminal` in a folder opens where it left off there.
//!
//! One small file, `terminal.json` in the client's chat home, maps each
//! folder's absolute path to a thread ID. It holds no message text. A
//! thread that is gone by the next start opens a new one instead.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The most folders the file remembers; the oldest entries go first.
const FOLDERS_MAX: usize = 256;

#[derive(Default, Serialize, Deserialize)]
struct File {
    /// Folder → (thread, when it was last opened, in Unix seconds).
    #[serde(default)]
    folders: BTreeMap<String, (String, u64)>,
}

fn path(home: &Path) -> PathBuf {
    home.join("terminal.json")
}

fn load(home: &Path) -> File {
    std::fs::read(path(home))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// The last thread opened in `folder`.
pub fn read(home: &Path, folder: &Path) -> Option<String> {
    load(home)
        .folders
        .remove(&folder.display().to_string())
        .map(|(thread, _)| thread)
        .filter(|thread| openagents_chat::client::thread_id(thread))
}

/// Remember `thread` as the last one opened in `folder`. A failure to write
/// costs only the resume, so it is not reported.
pub fn remember(home: &Path, folder: &Path, thread: &str, now: u64) {
    let mut file = load(home);
    file.folders
        .insert(folder.display().to_string(), (thread.to_owned(), now));
    while file.folders.len() > FOLDERS_MAX {
        let oldest = file
            .folders
            .iter()
            .min_by_key(|(_, (_, at))| *at)
            .map(|(folder, _)| folder.clone());
        match oldest {
            Some(folder) => file.folders.remove(&folder),
            None => break,
        };
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(&file) {
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
    fn each_folder_keeps_its_own_last_thread() {
        let home = tempfile::tempdir().unwrap();
        let (a, b) = (Path::new("/work/a"), Path::new("/work/b"));
        assert_eq!(read(home.path(), a), None);
        remember(home.path(), a, &"a".repeat(32), 1);
        remember(home.path(), b, &"b".repeat(32), 2);
        remember(home.path(), a, &"c".repeat(32), 3);
        assert_eq!(read(home.path(), a), Some("c".repeat(32)));
        assert_eq!(read(home.path(), b), Some("b".repeat(32)));
    }

    #[test]
    fn a_bad_file_or_id_resumes_nothing() {
        let home = tempfile::tempdir().unwrap();
        std::fs::write(home.path().join("terminal.json"), b"not json").unwrap();
        assert_eq!(read(home.path(), Path::new("/x")), None);
        remember(home.path(), Path::new("/x"), "not-an-id", 1);
        assert_eq!(read(home.path(), Path::new("/x")), None);
    }

    #[test]
    fn the_oldest_folders_are_forgotten_first() {
        let home = tempfile::tempdir().unwrap();
        for at in 0..(FOLDERS_MAX as u64 + 3) {
            remember(
                home.path(),
                Path::new(&format!("/f{at}")),
                &"d".repeat(32),
                at,
            );
        }
        assert_eq!(read(home.path(), Path::new("/f0")), None);
        assert!(read(home.path(), Path::new(&format!("/f{}", FOLDERS_MAX + 2))).is_some());
    }
}
