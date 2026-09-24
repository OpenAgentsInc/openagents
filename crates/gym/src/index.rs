//! The startup index (issue #9595): parsed records kept between runs of
//! the Gym, so opening it reads what changed and not the whole history.
//!
//! An [`Index`] holds one entry per unit a reader parses: a Terminal-Bench
//! attempt, a run, an attempt's timeline summary. Each entry keeps the
//! files its parse read, the size and modification time each had, and a
//! SHA-256 digest over those stamps. A lookup takes the stamps again and
//! uses the entry only when the digest still matches; otherwise the
//! reader parses the unit again and the entry is replaced. A unit the
//! reader no longer finds is dropped when the index is saved, so a
//! deleted run leaves the index with it.
//!
//! A reader doesn't list its inputs by hand. While [`recording`] runs a
//! parse, every [`touch`] the reader's file helpers make is recorded with
//! the file's stamp at that moment, taken before the file is read. A file
//! that was absent is recorded as absent, so one that appears later
//! invalidates the entry too.
//!
//! The index lives in `~/.openagents/gym/index/`, one JSON file per
//! reader and source set. Each file names the build that wrote it: a
//! different executable (a rebuild, an upgrade) discards the file, so a
//! change to a parser never reads an entry an older parser wrote. The
//! index is only a cache: deleting the directory costs one full parse.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The index file's schema.
pub const SCHEMA: &str = "openagents.gym.index.v1";

/// `~/.openagents/gym/index`.
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".openagents/gym/index"))
}

thread_local! {
    /// The files the parse in progress has touched, with their stamps.
    static TOUCHED: RefCell<Option<Vec<(PathBuf, String)>>> = const { RefCell::new(None) };
}

/// Records that the parse in progress reads, or checks for, `path`. A
/// no-op outside [`recording`].
pub fn touch(path: &Path) {
    TOUCHED.with(|touched| {
        if let Some(touched) = touched.borrow_mut().as_mut()
            && !touched.iter().any(|(seen, _)| seen == path)
        {
            touched.push((path.to_path_buf(), stamp(path)));
        }
    });
}

/// Runs `read` and returns what it returned and the files it touched,
/// each with the stamp it had when it was first touched. A recording
/// inside another adds its files to the outer one as well.
pub fn recording<T>(read: impl FnOnce() -> T) -> (T, Vec<(PathBuf, String)>) {
    let outer = TOUCHED.with(|touched| touched.borrow_mut().replace(Vec::new()));
    let value = read();
    let inputs = TOUCHED.with(|touched| {
        let inputs = touched.borrow_mut().take().unwrap_or_default();
        if let Some(mut outer) = outer {
            for input in &inputs {
                if !outer.iter().any(|(seen, _)| seen == &input.0) {
                    outer.push(input.clone());
                }
            }
            *touched.borrow_mut() = Some(outer);
        }
        inputs
    });
    (value, inputs)
}

/// A file's stamp: its kind, size, and modification time in nanoseconds,
/// or `-` when it doesn't exist.
#[must_use]
pub fn stamp(path: &Path) -> String {
    let Ok(meta) = std::fs::metadata(path) else {
        return "-".to_owned();
    };
    let modified = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |elapsed| elapsed.as_nanos());
    let kind = if meta.is_dir() { 'd' } else { 'f' };
    format!("{kind}{}:{modified}", meta.len())
}

/// The digest of `inputs`' paths and stamps.
fn digest(inputs: &[(PathBuf, String)]) -> String {
    let mut hasher = Sha256::new();
    for (path, stamp) in inputs {
        hasher.update(path.as_os_str().as_encoded_bytes());
        hasher.update([0]);
        hasher.update(stamp.as_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())
}

/// Which executable wrote an index: its path's stamp. A rebuilt binary
/// reads nothing an older one wrote.
fn build() -> String {
    std::env::current_exe()
        .map(|exe| format!("{}@{}", exe.display(), stamp(&exe)))
        .unwrap_or_default()
}

/// One parsed unit and what it was parsed from.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Entry<T> {
    /// The files the parse touched.
    inputs: Vec<PathBuf>,
    /// The digest of `inputs` and their stamps when they were read.
    digest: String,
    value: T,
}

/// An index file as it is written.
#[derive(Serialize, Deserialize)]
struct File<T> {
    schema: String,
    build: String,
    entries: HashMap<String, Entry<T>>,
}

/// How a load went: entries used as they were, and units parsed again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub hits: usize,
    pub parsed: usize,
}

/// One reader's index: parsed units by key.
#[derive(Clone, Debug)]
pub struct Index<T> {
    /// The file, or `None` for an index that keeps nothing.
    path: Option<PathBuf>,
    entries: HashMap<String, Entry<T>>,
    used: HashSet<String>,
    changed: bool,
    pub stats: Stats,
}

impl<T> Default for Index<T> {
    fn default() -> Self {
        Index {
            path: None,
            entries: HashMap::new(),
            used: HashSet::new(),
            changed: false,
            stats: Stats::default(),
        }
    }
}

impl<T: Clone + Serialize + DeserializeOwned> Index<T> {
    /// The index `name` in `dir`, for the sources `scope` names: two
    /// source sets never share a file. With no `dir`, an index that
    /// parses every unit and keeps nothing.
    #[must_use]
    pub fn open(dir: Option<&Path>, name: &str, scope: &str) -> Self {
        let Some(dir) = dir else {
            return Index::default();
        };
        let scope = format!("{:x}", Sha256::digest(scope.as_bytes()));
        let path = dir.join(format!("{name}-{}.json", &scope[..16]));
        let entries = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<File<T>>(&bytes).ok())
            .filter(|file| file.schema == SCHEMA && file.build == build())
            .map(|file| file.entries)
            .unwrap_or_default();
        Index {
            path: Some(path),
            entries,
            ..Index::default()
        }
    }

    /// The file the index is kept in.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// The unit `key`: the entry when its files are as they were, or else
    /// `read`'s value, which the index keeps when `keep` holds for it. A
    /// unit still changing, such as a running run, isn't kept.
    pub fn get_or_read(
        &mut self,
        key: &str,
        read: impl FnOnce() -> T,
        keep: impl FnOnce(&T) -> bool,
    ) -> T {
        self.used.insert(key.to_owned());
        if let Some(entry) = self.entries.get(key)
            && digest(
                &entry
                    .inputs
                    .iter()
                    .map(|path| (path.clone(), stamp(path)))
                    .collect::<Vec<_>>(),
            ) == entry.digest
        {
            self.stats.hits += 1;
            // Report the entry's inputs to an enclosing recording.
            for input in &entry.inputs {
                touch(input);
            }
            return entry.value.clone();
        }
        self.stats.parsed += 1;
        let (value, inputs) = recording(read);
        if self.path.is_some() && keep(&value) {
            let digest = digest(&inputs);
            self.entries.insert(
                key.to_owned(),
                Entry {
                    inputs: inputs.into_iter().map(|(path, _)| path).collect(),
                    digest,
                    value: value.clone(),
                },
            );
            self.changed = true;
        } else if self.entries.remove(key).is_some() {
            self.changed = true;
        }
        value
    }

    /// Marks `key` as still present without reading it, for a unit the
    /// caller holds already.
    pub fn keep(&mut self, key: &str) {
        self.used.insert(key.to_owned());
    }

    /// Writes the index when it changed, dropping every entry no lookup
    /// or [`Index::keep`] asked for since the last save: those units are
    /// gone. The write is atomic: a temporary file renamed over the old.
    pub fn save(&mut self) -> Result<(), String> {
        let used = std::mem::take(&mut self.used);
        let before = self.entries.len();
        self.entries.retain(|key, _| used.contains(key));
        let changed = std::mem::take(&mut self.changed) || self.entries.len() != before;
        let Some(path) = &self.path else {
            return Ok(());
        };
        if !changed && path.is_file() {
            return Ok(());
        }
        let file = File {
            schema: SCHEMA.to_owned(),
            build: build(),
            entries: std::mem::take(&mut self.entries),
        };
        let written = serde_json::to_vec(&file).map_err(|error| error.to_string());
        self.entries = file.entries;
        let bytes = written?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let temporary = path.with_extension(format!("json.{}.tmp", std::process::id()));
        std::fs::write(&temporary, bytes)
            .and_then(|()| std::fs::rename(&temporary, path))
            .map_err(|error| format!("{}: {error}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        std::fs::write(path, text).unwrap();
    }

    /// A reader that parses a file's first line and touches it.
    fn first_line(path: &Path) -> String {
        touch(path);
        std::fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .next()
            .unwrap_or_default()
            .to_owned()
    }

    #[test]
    fn an_unchanged_file_is_read_from_the_index_and_a_changed_one_again() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        write(&file, "one\n");
        let index_dir = dir.path().join("index");
        let mut index: Index<String> = Index::open(Some(&index_dir), "test", "scope");
        let read = index.get_or_read("a", || first_line(&file), |_| true);
        assert_eq!(read, "one");
        index.save().unwrap();

        let mut index: Index<String> = Index::open(Some(&index_dir), "test", "scope");
        // The reader isn't called: the entry answers.
        let read = index.get_or_read("a", || unreachable!(), |_| true);
        assert_eq!(read, "one");
        assert_eq!(index.stats, Stats { hits: 1, parsed: 0 });

        // A different size and time: the unit is parsed again.
        write(&file, "two lines\nnow\n");
        let read = index.get_or_read("a", || first_line(&file), |_| true);
        assert_eq!(read, "two lines");
        assert_eq!(index.stats, Stats { hits: 1, parsed: 1 });
    }

    #[test]
    fn a_file_that_appears_invalidates_the_entry_that_saw_it_absent() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("late.txt");
        let mut index: Index<String> = Index::open(Some(dir.path()), "test", "scope");
        assert_eq!(index.get_or_read("a", || first_line(&file), |_| true), "");
        write(&file, "here\n");
        assert_eq!(
            index.get_or_read("a", || first_line(&file), |_| true),
            "here"
        );
    }

    #[test]
    fn a_unit_not_asked_for_is_dropped_and_an_unkept_one_is_never_stored() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a"), dir.path().join("b"));
        write(&a, "a\n");
        write(&b, "b\n");
        let mut index: Index<String> = Index::open(Some(dir.path()), "test", "scope");
        index.get_or_read("a", || first_line(&a), |_| true);
        index.get_or_read("b", || first_line(&b), |_| true);
        index.get_or_read("running", || "partial".to_owned(), |_| false);
        index.save().unwrap();

        let mut index: Index<String> = Index::open(Some(dir.path()), "test", "scope");
        assert_eq!(index.entries.len(), 2);
        index.get_or_read("a", || unreachable!(), |_| true);
        index.save().unwrap();
        let index: Index<String> = Index::open(Some(dir.path()), "test", "scope");
        assert_eq!(index.entries.keys().collect::<Vec<_>>(), ["a"]);
    }

    #[test]
    fn two_scopes_keep_two_files_and_no_dir_keeps_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let one: Index<String> = Index::open(Some(dir.path()), "runs", "one");
        let two: Index<String> = Index::open(Some(dir.path()), "runs", "two");
        assert_ne!(one.path(), two.path());
        let mut none: Index<String> = Index::open(None, "runs", "one");
        none.get_or_read("a", || "a".to_owned(), |_| true);
        assert!(none.entries.is_empty());
        none.save().unwrap();
    }

    #[test]
    fn a_nested_recording_reports_its_files_to_the_outer_one() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a"), dir.path().join("b"));
        let ((), outer) = recording(|| {
            touch(&a);
            let ((), inner) = recording(|| touch(&b));
            assert_eq!(inner.len(), 1);
        });
        let paths: Vec<_> = outer.into_iter().map(|(path, _)| path).collect();
        assert_eq!(paths, [a, b]);
    }
}
