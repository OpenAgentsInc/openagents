//! File watcher for detecting external changes to vault files

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Events detected by the file watcher
#[derive(Debug, Clone)]
pub enum FileChange {
    /// A file was modified
    Modified(PathBuf),
    /// A file was created
    Created(PathBuf),
    /// A file was deleted
    Deleted(PathBuf),
}

/// Shared state for background file watching
struct WatcherState {
    /// Pending changes to process
    pending_changes: Vec<FileChange>,
}

/// File watcher for vault directory
pub struct FileWatcher {
    state: Arc<Mutex<WatcherState>>,
    /// Keep watcher alive
    _watcher: RecommendedWatcher,
}

impl FileWatcher {
    /// Create a new file watcher for the given directory
    pub fn new(watch_path: PathBuf) -> Result<Self, String> {
        let state = Arc::new(Mutex::new(WatcherState {
            pending_changes: Vec::new(),
        }));

        let state_clone = Arc::clone(&state);

        // Create the watcher with a callback that updates shared state
        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            match res {
                Ok(event) => {
                    if let Ok(mut s) = state_clone.lock() {
                        // Convert notify events to our FileChange type
                        for path in event.paths {
                            // Only care about .md files
                            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                                continue;
                            }

                            // Skip files in .archive folder
                            if path.components().any(|c| c.as_os_str() == ".archive") {
                                continue;
                            }

                            use notify::EventKind;
                            let change = match event.kind {
                                EventKind::Create(_) => FileChange::Created(path),
                                EventKind::Modify(_) => FileChange::Modified(path),
                                EventKind::Remove(_) => FileChange::Deleted(path),
                                _ => continue,
                            };
                            s.pending_changes.push(change);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("File watcher error: {}", e);
                }
            }
        })
        .map_err(|e| format!("Failed to create watcher: {}", e))?;

        // Start watching the directory (non-recursive to avoid .archive)
        watcher
            .watch(&watch_path, RecursiveMode::NonRecursive)
            .map_err(|e| format!("Failed to watch directory: {}", e))?;

        tracing::info!("File watcher started for {:?}", watch_path);

        Ok(Self {
            state,
            _watcher: watcher,
        })
    }

    /// Take any pending file changes (call from main loop)
    pub fn take_changes(&mut self) -> Vec<FileChange> {
        if let Ok(mut s) = self.state.lock() {
            std::mem::take(&mut s.pending_changes)
        } else {
            Vec::new()
        }
    }
}
