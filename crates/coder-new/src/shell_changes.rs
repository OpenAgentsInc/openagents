//! The files a shell command changed, drawn as diffs.
//!
//! Claude Code's Edit and Write carry their own patch, but a model that
//! edits through Bash (`sed -i`, a heredoc, a script) changes files the
//! transcript never sees. A turn keeps a [`Snapshot`] of the checkout's
//! changed files; when a Bash call ends, [`Snapshot::changes`] compares the
//! files now against it, and each file that moved shows as an Edit (or a
//! Write, for a new file) with its diff.
//!
//! The checkout is shared: another agent, a person, or a `git pull` can
//! change files while a command runs. A diff shows as the command's only
//! when the file last changed while the command ran
//! ([`Snapshot::changes_during`]); anything else moved without this chat
//! and is not drawn as its edit (#11506).

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

use serde_json::{Value, json};

/// A file larger than this is not read or diffed.
const MAX_BYTES: u64 = 1 << 20;

/// A checkout with more changed paths than this is not tracked.
const MAX_PATHS: usize = 2_000;

/// At most this many files show after one command.
pub const MAX_SHOWN: usize = 20;

/// How long before Coder saw a command's call the command may already
/// have written: the call reaches Coder through a pipe after it started.
const START_SLACK: Duration = Duration::from_millis(500);

/// What a file held when a snapshot was taken.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Content {
    Absent,
    Text(String),
    /// Too large, binary, or unreadable: never diffed.
    Opaque,
}

/// The checkout's changed files and their contents at one moment.
#[derive(Debug)]
pub struct Snapshot {
    root: PathBuf,
    /// Where the turn's directory sits inside `root`.
    cwd: PathBuf,
    head: Option<String>,
    files: HashMap<String, Content>,
}

/// One file a command changed.
#[derive(Debug)]
pub struct Change {
    /// The path as the transcript shows it: from the turn's directory when
    /// the file is inside it, else from the checkout's root.
    pub path: String,
    pub created: bool,
    pub diff: crate::file_tools::Diff,
}

impl Change {
    /// The tool name and output Coder's Edit and Write widgets draw.
    #[must_use]
    pub fn event(&self) -> crate::bundled_runtime::RuntimeEvent {
        let name = if self.created { "Write" } else { "Edit" };
        crate::bundled_runtime::RuntimeEvent::Tool {
            name: name.into(),
            input: json!({"path": self.path}),
            output: json!({
                "path": self.path,
                "diff": self.diff.text,
                "added": self.diff.added,
                "removed": self.diff.removed,
            }),
            running: false,
        }
    }
}

fn git(root: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let output = Command::new("git")
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(root)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

fn text(bytes: Vec<u8>) -> Content {
    if bytes.len() as u64 > MAX_BYTES || bytes.contains(&0) {
        return Content::Opaque;
    }
    String::from_utf8(bytes).map_or(Content::Opaque, Content::Text)
}

fn read(path: &Path) -> Content {
    match std::fs::metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Content::Absent,
        Err(_) => Content::Opaque,
        Ok(meta) if !meta.is_file() || meta.len() > MAX_BYTES => Content::Opaque,
        Ok(_) => std::fs::read(path).map_or(Content::Opaque, text),
    }
}

/// The checkout-relative paths `git status` reports changed or untracked.
fn changed_paths(root: &Path) -> Option<BTreeSet<String>> {
    let out = git(
        root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?;
    let mut paths = BTreeSet::new();
    let mut fields = out.split(|b| *b == 0).filter(|f| !f.is_empty());
    while let Some(field) = fields.next() {
        if field.len() < 4 {
            continue;
        }
        let (status, path) = (&field[..2], &field[3..]);
        paths.insert(String::from_utf8_lossy(path).into_owned());
        // A rename or copy names its source in the next field.
        if status.contains(&b'R') || status.contains(&b'C') {
            if let Some(source) = fields.next() {
                paths.insert(String::from_utf8_lossy(source).into_owned());
            }
        }
        if paths.len() > MAX_PATHS {
            return None;
        }
    }
    Some(paths)
}

impl Snapshot {
    /// The changed files of the checkout holding `cwd`, or `None` outside
    /// a Git checkout or in one with too many changes to track.
    #[must_use]
    pub fn take(cwd: &Path) -> Option<Self> {
        let top = git(cwd, &["rev-parse", "--show-toplevel"])?;
        let root = PathBuf::from(String::from_utf8_lossy(&top).trim());
        let root = root.canonicalize().unwrap_or(root);
        let here = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
        let cwd = here
            .strip_prefix(&root)
            .map(Path::to_path_buf)
            .unwrap_or_default();
        Self::at(root, cwd)
    }

    fn at(root: PathBuf, cwd: PathBuf) -> Option<Self> {
        let head = git(&root, &["rev-parse", "--verify", "-q", "HEAD"])
            .map(|out| String::from_utf8_lossy(&out).trim().to_owned());
        let files = changed_paths(&root)?
            .into_iter()
            .map(|path| {
                let content = read(&root.join(&path));
                (path, content)
            })
            .collect();
        Some(Self {
            root,
            cwd,
            head,
            files,
        })
    }

    /// What `path` held at this snapshot: its saved contents when it was
    /// changed then, else its contents at the snapshot's commit.
    fn before(&self, path: &str) -> Content {
        if let Some(content) = self.files.get(path) {
            return content.clone();
        }
        match &self.head {
            None => Content::Absent,
            Some(head) => {
                git(&self.root, &["show", &format!("{head}:{path}")]).map_or(Content::Absent, text)
            }
        }
    }

    /// Saves `path`'s current contents, after a tool that draws its own
    /// diff (Edit, Write) changed it, so the next command does not show it
    /// again.
    pub fn refresh(&mut self, path: &Path) {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(&self.cwd).join(path)
        };
        let resolved = absolute
            .parent()
            .and_then(|parent| parent.canonicalize().ok())
            .zip(absolute.file_name())
            .map_or(absolute.clone(), |(parent, name)| parent.join(name));
        if let Ok(relative) = resolved.strip_prefix(&self.root) {
            let key = relative.to_string_lossy().into_owned();
            self.files.insert(key, read(&resolved));
        }
    }

    /// The files that changed since this snapshot, and a snapshot of now
    /// for the next command to compare against.
    #[must_use]
    pub fn changes(self) -> (Vec<Change>, Option<Self>) {
        self.compare(None)
    }

    /// The files a command that ran from `from` to `to` changed since this
    /// snapshot: only files last written in that window, so an edit another
    /// agent made in the shared checkout before or after it is not the
    /// command's. A file removed counts when its directory changed in the
    /// window. The snapshot of now covers every file, so what was left out
    /// does not show after the next command either.
    #[must_use]
    pub fn changes_during(self, from: SystemTime, to: SystemTime) -> (Vec<Change>, Option<Self>) {
        self.compare(Some((from.checked_sub(START_SLACK).unwrap_or(from), to)))
    }

    /// When `path` (or, once removed, its nearest directory) last changed.
    fn modified(&self, path: &str) -> Option<SystemTime> {
        let mut at = self.root.join(path);
        loop {
            if let Ok(time) = std::fs::symlink_metadata(&at).and_then(|meta| meta.modified()) {
                return Some(time);
            }
            if !at.pop() || !at.starts_with(&self.root) {
                return None;
            }
        }
    }

    fn compare(self, window: Option<(SystemTime, SystemTime)>) -> (Vec<Change>, Option<Self>) {
        let Some(now) = Self::at(self.root.clone(), self.cwd.clone()) else {
            return (Vec::new(), None);
        };
        let paths: BTreeSet<&String> = self.files.keys().chain(now.files.keys()).collect();
        let mut changes = Vec::new();
        for path in paths {
            if changes.len() == MAX_SHOWN {
                break;
            }
            if let Some((from, to)) = window
                && !self
                    .modified(path)
                    .is_some_and(|time| time >= from && time <= to)
            {
                continue;
            }
            let old = self.before(path);
            let new = match now.files.get(path) {
                Some(content) => content.clone(),
                // Clean now: what the checkout's commit holds.
                None => read(&self.root.join(path)),
            };
            let (Content::Text(_) | Content::Absent, Content::Text(_) | Content::Absent) =
                (&old, &new)
            else {
                continue;
            };
            if old == new {
                continue;
            }
            let as_str = |content: &Content| match content {
                Content::Text(text) => text.clone(),
                _ => String::new(),
            };
            let diff = crate::file_tools::unified(&as_str(&old), &as_str(&new));
            if diff.text.is_empty() {
                continue;
            }
            let shown = Path::new(path)
                .strip_prefix(&self.cwd)
                .map_or_else(|_| path.clone(), |p| p.to_string_lossy().into_owned());
            changes.push(Change {
                path: shown,
                created: old == Content::Absent,
                diff,
            });
        }
        (changes, Some(now))
    }
}

/// The path an Edit or Write call names, for [`Snapshot::refresh`].
#[must_use]
pub fn edited_path(input: &Value) -> Option<PathBuf> {
    ["file_path", "path"]
        .iter()
        .find_map(|key| input[*key].as_str())
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    fn repo() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        run(&root, &["init", "-q"]);
        run(&root, &["config", "user.email", "t@example.com"]);
        run(&root, &["config", "user.name", "t"]);
        std::fs::create_dir(root.join("src")).unwrap();
        std::fs::write(root.join("src/a.rs"), "one\ntwo\nthree\n").unwrap();
        std::fs::write(root.join("dirty.rs"), "x\n").unwrap();
        run(&root, &["add", "."]);
        run(&root, &["commit", "-q", "-m", "init"]);
        std::fs::write(root.join("dirty.rs"), "x\ny\n").unwrap();
        (dir, root)
    }

    #[test]
    fn a_shell_edit_a_new_file_and_a_removal_show_as_diffs() {
        let (_dir, root) = repo();
        let before = Snapshot::take(&root).unwrap();
        // What a `sed -i`, a heredoc and an `rm` would do.
        std::fs::write(root.join("src/a.rs"), "one\n2\nthree\n").unwrap();
        std::fs::write(root.join("src/new.rs"), "fresh\n").unwrap();
        std::fs::write(root.join("dirty.rs"), "x\ny\nz\n").unwrap();
        let (changes, after) = before.changes();
        let by_path: HashMap<&str, &Change> =
            changes.iter().map(|c| (c.path.as_str(), c)).collect();
        assert_eq!(by_path.len(), 3, "{changes:?}");
        let edited = by_path["src/a.rs"];
        assert!(!edited.created);
        assert!(
            edited.diff.text.contains("-two\n+2\n"),
            "{}",
            edited.diff.text
        );
        assert!(by_path["src/new.rs"].created);
        // A file already changed before the command shows only the new part.
        let dirty = by_path["dirty.rs"];
        assert_eq!((dirty.diff.added, dirty.diff.removed), (1, 0));
        // The next command compares against now: nothing moved.
        assert!(after.unwrap().changes().0.is_empty());
    }

    #[test]
    fn a_file_an_edit_tool_already_drew_does_not_show_again() {
        let (_dir, root) = repo();
        let mut before = Snapshot::take(&root.join("src")).unwrap();
        std::fs::write(root.join("src/a.rs"), "edited\n").unwrap();
        before.refresh(Path::new("a.rs"));
        std::fs::write(root.join("src/b.rs"), "b\n").unwrap();
        let (changes, _) = before.changes();
        // Paths read from the turn's directory.
        let paths: Vec<&str> = changes.iter().map(|c| c.path.as_str()).collect();
        assert_eq!(paths, ["b.rs"]);
    }

    #[test]
    fn a_commit_or_a_revert_shows_only_real_content_moves() {
        let (_dir, root) = repo();
        let before = Snapshot::take(&root).unwrap();
        run(&root, &["commit", "-q", "-am", "keep"]);
        assert!(before.changes().0.is_empty());
        std::fs::write(root.join("src/a.rs"), "changed\n").unwrap();
        let before = Snapshot::take(&root).unwrap();
        run(&root, &["checkout", "-q", "--", "src/a.rs"]);
        let (changes, _) = before.changes();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].diff.text.contains("+two\n"));
    }

    /// A file another agent wrote in the shared checkout before or after a
    /// command ran is not drawn as the command's edit; one written while
    /// it ran is, and neither shows again after the next command (#11506).
    #[test]
    fn only_files_written_while_the_command_ran_are_its_changes() {
        let (_dir, root) = repo();
        let before = Snapshot::take(&root).unwrap();
        let set = |path: &str, at: SystemTime| {
            let file = std::fs::File::options()
                .write(true)
                .open(root.join(path))
                .unwrap();
            file.set_modified(at).unwrap();
        };
        let start = SystemTime::now() + Duration::from_secs(60);
        let end = start + Duration::from_secs(5);
        // Another agent, a minute before the command started.
        std::fs::write(root.join("dirty.rs"), "someone else\n").unwrap();
        set("dirty.rs", start - Duration::from_secs(60));
        // The command itself.
        std::fs::write(root.join("src/a.rs"), "one\n2\nthree\n").unwrap();
        set("src/a.rs", start + Duration::from_secs(1));
        // Another agent, after the command ended.
        std::fs::write(root.join("src/later.rs"), "later\n").unwrap();
        set("src/later.rs", end + Duration::from_secs(30));
        let (changes, after) = before.changes_during(start, end);
        let paths: Vec<&str> = changes.iter().map(|c| c.path.as_str()).collect();
        assert_eq!(paths, ["src/a.rs"]);
        assert!(after.unwrap().changes().0.is_empty());
    }

    #[test]
    fn outside_a_checkout_nothing_is_tracked() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Snapshot::take(dir.path()).is_none());
    }
}
