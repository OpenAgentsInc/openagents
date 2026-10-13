//! The worktree guard (#11509): one task session's tools held to its
//! worktree, as a floor every surface registers the same way. It was
//! written three times before (the issue run's permission handler, the
//! briefed agent's, the Solver's Devin boundary); this is the one copy.
//!
//! A [`Guard`] names the worktree, the tools the session was offered
//! (none named: any), whether it may change files, and the folders outside
//! the worktree it may also read (a squeezed command's full output). A file
//! tool whose path leaves the worktree, through `..` or a symbolic link, is
//! refused; so is a tool the session was not offered, and any write by a
//! session that may not write.

use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use crate::events::{Call, Event};
use crate::outcome::{HookOutcome, Permission};
use crate::registry::{Hook, Versions};

/// The built-in tools that name a file, as Claude Code names them (an ACP
/// agent's calls reach the hooks in the same names, [`crate::acp::call`]).
pub const FILE_TOOLS: [&str; 7] = [
    "Read",
    "Edit",
    "MultiEdit",
    "Write",
    "NotebookEdit",
    "Glob",
    "Grep",
];

/// The tools that change files.
pub const WRITE_TOOLS: [&str; 4] = ["Edit", "MultiEdit", "Write", "NotebookEdit"];

/// The input fields a file tool names its path in.
pub const PATH_FIELDS: [&str; 3] = ["file_path", "path", "notebook_path"];

/// The field [`crate::acp::call`] lists a writing ACP tool's every path in
/// (a move names two).
pub const WRITTEN_PATHS: &str = "written_paths";

/// The guard's ID in a session's registry.
pub const ID: &str = "worktree";

/// The guard's version in the hook set digest.
pub const POLICY: &str = "coder-worktree-v1";

/// What a call outside the worktree reads.
pub const OUTSIDE: &str =
    "That path is outside the task's worktree. Work only inside the worktree.";
/// What a call to a tool the session wasn't offered reads.
pub const NOT_OFFERED: &str = "That tool is not available in this task.";
/// What a write by a session that may not write reads.
pub const READ_ONLY: &str = "This task may not change files.";

/// `path` made absolute against `root` and normalized lexically.
#[must_use]
pub fn resolve(root: &Path, path: &str) -> PathBuf {
    let joined = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        root.join(path)
    };
    let mut out = PathBuf::new();
    for part in joined.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// `path` as an absolute, resolved name: relative to `base`, `.` and `..`
/// taken lexically, and the longest part that exists resolved through its
/// symbolic links, so `/tmp` and `/private/tmp` compare equal.
#[must_use]
pub fn resolved(base: &Path, path: &str) -> PathBuf {
    let lexical = resolve(base, path);
    let mut existing = lexical.as_path();
    let mut rest = Vec::new();
    loop {
        if let Ok(real) = existing.canonicalize() {
            let mut out = real;
            for part in rest.iter().rev() {
                out.push(part);
            }
            return out;
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_owned());
                existing = parent;
            }
            _ => return lexical,
        }
    }
}

/// The worktree-relative path a tool input names, read lexically:
/// `Some(None)` when it names none, `None` when the path leaves `root`.
/// For showing and counting a call; [`contains`] is the check.
#[must_use]
pub fn relative(root: &Path, input: &Value) -> Option<Option<String>> {
    let Some(path) = PATH_FIELDS.iter().find_map(|key| input[*key].as_str()) else {
        return Some(None);
    };
    let resolved = resolve(root, path);
    let relative = resolved.strip_prefix(root).ok()?;
    Some(Some(relative.to_string_lossy().into_owned()))
}

/// Whether `path` lies inside `root`, both lexically and once symbolic
/// links are followed: the longest existing part of it (the path itself
/// when it exists) must resolve inside `root`. A `~` path is never inside.
#[must_use]
pub fn contains(root: &Path, path: &str) -> bool {
    if path.starts_with('~') {
        return false;
    }
    let lexical = resolve(root, path);
    let real_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let mut existing = lexical.as_path();
    while existing.symlink_metadata().is_err() {
        match existing.parent() {
            Some(parent) => existing = parent,
            None => return false,
        }
    }
    existing
        .canonicalize()
        .is_ok_and(|real| real.starts_with(&real_root))
}

/// One session's guard.
#[derive(Clone, Debug)]
pub struct Guard {
    /// The worktree.
    pub root: PathBuf,
    /// The tools the session was offered, by name; an MCP server's name
    /// (`mcp__oa`) admits its tools. `None` admits every tool.
    pub tools: Option<Vec<String>>,
    /// Whether the session may change files.
    pub writes: bool,
    /// Folders outside the worktree the session may also read.
    pub readable: Vec<PathBuf>,
    /// Whether the session may read anywhere: only its writes are held to
    /// the worktree ([`Guard::writes`]).
    pub reads_anywhere: bool,
    /// Folders outside the worktree the session may also write, such as
    /// its boundary's scratch.
    pub writable: Vec<PathBuf>,
}

impl Guard {
    /// A guard on `root` admitting every tool, writes included.
    #[must_use]
    pub fn new(root: &Path) -> Self {
        Guard {
            root: root.to_path_buf(),
            tools: None,
            writes: true,
            readable: Vec::new(),
            reads_anywhere: false,
            writable: Vec::new(),
        }
    }

    /// Also lets the session write under `dir`.
    #[must_use]
    pub fn writing(mut self, dir: &Path) -> Self {
        self.writable.push(resolved(dir, "."));
        self
    }

    /// Whether the session may write `path` (relative to the worktree).
    fn may_write(&self, path: &str) -> bool {
        contains(&self.root, path)
            || (!path.starts_with('~')
                && self
                    .writable
                    .iter()
                    .any(|dir| resolved(&self.root, path).starts_with(dir)))
    }

    /// A guard on `root` holding only writes to it (#11527): a session
    /// with full access, whose reads go where it likes, still writes only
    /// in its worktree through the file tools.
    #[must_use]
    pub fn writes(root: &Path) -> Self {
        Guard {
            reads_anywhere: true,
            ..Guard::new(root)
        }
    }

    /// Admits only `tools`.
    #[must_use]
    pub fn offering(mut self, tools: Vec<String>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Refuses every write.
    #[must_use]
    pub fn read_only(mut self) -> Self {
        self.writes = false;
        self
    }

    /// Also lets the session read under `dir`.
    #[must_use]
    pub fn reading(mut self, dir: PathBuf) -> Self {
        self.readable.push(dir);
        self
    }

    fn offered(&self, tool: &str) -> bool {
        self.tools.as_ref().is_none_or(|tools| {
            tools
                .iter()
                .any(|name| tool == name || tool.starts_with(&format!("{name}__")))
        })
    }

    /// Why `tool` with `input` is refused, or `None` when it may run.
    #[must_use]
    pub fn refusal(&self, tool: &str, input: &Value) -> Option<&'static str> {
        if !self.offered(tool) {
            return Some(NOT_OFFERED);
        }
        let written: Vec<&str> = input[WRITTEN_PATHS]
            .as_array()
            .map(|paths| paths.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let writes = WRITE_TOOLS.contains(&tool) || !written.is_empty();
        if writes && !self.writes {
            return Some(READ_ONLY);
        }
        if written.iter().any(|path| !self.may_write(path)) {
            return Some(OUTSIDE);
        }
        if !FILE_TOOLS.contains(&tool) {
            return None;
        }
        let mut paths: Vec<&str> = PATH_FIELDS
            .iter()
            .filter_map(|key| input[*key].as_str())
            .take(1)
            .collect();
        // A `Glob` pattern that leaves the worktree names where it reads.
        if tool == "Glob"
            && let Some(pattern) = input["pattern"].as_str()
            && (pattern.starts_with('/') || pattern.starts_with('~') || pattern.contains(".."))
        {
            paths.push(pattern);
        }
        let writing = WRITE_TOOLS.contains(&tool);
        let refused = paths.into_iter().any(|path| {
            if (writing && self.may_write(path))
                || (!writing && (contains(&self.root, path) || self.reads_anywhere))
            {
                return false;
            }
            let readable = tool == "Read"
                && self
                    .readable
                    .iter()
                    .any(|dir| resolve(dir, path).starts_with(dir) || contains(dir, path));
            !readable
        });
        refused.then_some(OUTSIDE)
    }

    /// The guard as a `PreToolUse` floor: a refused call is denied with
    /// its reason; a guard that fails denies.
    #[must_use]
    pub fn hook(self) -> Hook {
        Hook::floor(
            ID,
            Event::PreToolUse,
            Permission::deny(OUTSIDE),
            move |call: &Call| {
                let tool = call.tool.as_deref().unwrap_or_default();
                Ok(match self.refusal(tool, &call.input) {
                    Some(reason) => HookOutcome::deny(reason),
                    None => HookOutcome::none(),
                })
            },
        )
        .pinned(Versions {
            question_set: None,
            policy: Some(POLICY.to_owned()),
            model: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_path_leaving_the_worktree_is_refused_through_dots_and_links() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("tree");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::os::unix::fs::symlink(dir.path(), root.join("out")).unwrap();
        let guard = Guard::new(&root);
        let refused = |tool: &str, path: &str| guard.refusal(tool, &json!({"file_path": path}));
        assert_eq!(refused("Edit", "src/lib.rs"), None);
        assert_eq!(refused("Write", "src/new/file.rs"), None);
        assert_eq!(refused("Edit", "../elsewhere.rs"), Some(OUTSIDE));
        assert_eq!(refused("Read", "/etc/hosts"), Some(OUTSIDE));
        assert_eq!(refused("Edit", "out/escaped.rs"), Some(OUTSIDE));
        assert_eq!(refused("Read", "~/.ssh/id_rsa"), Some(OUTSIDE));
        assert_eq!(guard.refusal("Grep", &json!({"pattern": "x"})), None);
    }

    #[test]
    fn a_write_guard_reads_anywhere_and_holds_writes_and_glob_patterns() {
        let dir = tempfile::tempdir().unwrap();
        let writes = Guard::writes(dir.path());
        assert_eq!(
            writes.refusal("Read", &json!({"file_path": "/etc/hosts"})),
            None
        );
        assert_eq!(writes.refusal("Glob", &json!({"pattern": "/etc/*"})), None);
        assert_eq!(
            writes.refusal("Write", &json!({"file_path": "/etc/hosts"})),
            Some(OUTSIDE)
        );
        assert_eq!(writes.refusal("Edit", &json!({"file_path": "a.rs"})), None);
        let strict = Guard::new(dir.path());
        assert_eq!(
            strict.refusal("Glob", &json!({"pattern": "../**/*.rs"})),
            Some(OUTSIDE)
        );
        assert_eq!(
            strict.refusal("Glob", &json!({"pattern": "src/**/*.rs"})),
            None
        );
        let scratch = tempfile::tempdir().unwrap();
        let with_scratch = Guard::writes(dir.path()).writing(scratch.path());
        let inside = scratch.path().join("x").display().to_string();
        assert_eq!(
            with_scratch.refusal("Write", &json!({"file_path": inside})),
            None
        );
        let moved = json!({"file_path": "a.rs", WRITTEN_PATHS: ["a.rs", inside]});
        assert_eq!(with_scratch.refusal("Edit", &moved), None);
    }

    #[test]
    fn tools_writes_and_extra_reading_are_held_to_what_the_session_got() {
        let dir = tempfile::tempdir().unwrap();
        let spill = tempfile::tempdir().unwrap();
        let spill_dir = spill.path().canonicalize().unwrap();
        let guard = Guard::new(dir.path())
            .offering(vec!["Read".into(), "Edit".into(), "mcp__oa".into()])
            .reading(spill_dir.clone());
        assert_eq!(
            guard.refusal("Bash", &json!({"command": "ls"})),
            Some(NOT_OFFERED)
        );
        assert_eq!(guard.refusal("mcp__oa__verify", &json!({})), None);
        let full = spill_dir.join("out.txt").display().to_string();
        assert_eq!(guard.refusal("Read", &json!({"file_path": full})), None);
        assert_eq!(
            guard.refusal("Edit", &json!({"file_path": full})),
            Some(OUTSIDE)
        );
        let read_only = Guard::new(dir.path()).read_only();
        assert_eq!(
            read_only.refusal("Edit", &json!({"file_path": "a.rs"})),
            Some(READ_ONLY)
        );
        assert_eq!(
            read_only.refusal("Read", &json!({"file_path": "a.rs"})),
            None
        );
        // An ACP move names both its paths.
        let moved = json!({"file_path": "a.rs", WRITTEN_PATHS: ["a.rs", "/tmp/b.rs"]});
        assert_eq!(
            Guard::new(dir.path()).refusal("Edit", &moved),
            Some(OUTSIDE)
        );
        assert_eq!(read_only.refusal("Other", &moved), Some(READ_ONLY));
    }
}
