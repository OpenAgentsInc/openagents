//! The file panel's plain values: what a file opens as, the tree's rows,
//! git badges, and sizes, kept apart from drawing so they test natively.

use std::collections::{HashMap, HashSet};

use openagents_proto::{
    CheckoutGitStatus, GitFileState, WorkspaceEntry, WorkspaceEntryKind, WorkspaceFileText,
    WorkspaceTextEncoding,
};

/// How a file opens in the panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opens {
    /// Drawn as Markdown, with its source a click away.
    Markdown,
    /// Read through `ReadWorkspaceImage` and drawn.
    Image,
    /// Read through `ReadWorkspaceFile`; text shows, anything else gets the
    /// name and size card with Download.
    Text,
}

/// The images the browser draws, by extension.
const IMAGES: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "avif"];
const MARKDOWN: &[&str] = &["md", "markdown", "mdx"];

fn extension(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => ext.to_ascii_lowercase(),
        _ => String::new(),
    }
}

pub fn opens(path: &str) -> Opens {
    let ext = extension(path);
    if IMAGES.contains(&ext.as_str()) {
        Opens::Image
    } else if MARKDOWN.contains(&ext.as_str()) {
        Opens::Markdown
    } else {
        Opens::Text
    }
}

/// Whether a read file has text to show (and so to edit), or only a card.
pub fn has_text(file: &WorkspaceFileText) -> bool {
    file.text.is_some()
        && matches!(
            file.encoding,
            WorkspaceTextEncoding::Utf8 | WorkspaceTextEncoding::Utf8Bom
        )
}

/// A size as people read it: `812 bytes`, `4.2 KB`, `18 MB`.
pub fn size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if bytes < 1000 {
        return if bytes == 1 {
            "1 byte".to_owned()
        } else {
            format!("{bytes} bytes")
        };
    }
    let mut value = bytes as f64 / 1000.;
    let mut unit = 0;
    while value >= 1000. && unit + 1 < UNITS.len() {
        value /= 1000.;
        unit += 1;
    }
    if value < 10. {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.0} {}", UNITS[unit])
    }
}

/// A tree row: an entry and how deep it sits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub path: String,
    pub name: String,
    pub dir: bool,
    pub depth: usize,
    pub size: Option<u64>,
    pub open: bool,
}

/// Directories first, then files, each by name without case.
pub fn sort(entries: &mut [WorkspaceEntry]) {
    entries.sort_by(|a, b| {
        let dir = |e: &WorkspaceEntry| e.kind != WorkspaceEntryKind::Directory;
        dir(a)
            .cmp(&dir(b))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
    });
}

/// The tree's visible rows: `listed` holds each read directory's entries
/// (the root under `""`), and `expanded` the open directories.
pub fn rows(
    listed: &HashMap<String, Vec<WorkspaceEntry>>,
    expanded: &HashSet<String>,
    hidden: bool,
) -> Vec<Row> {
    let mut out = Vec::new();
    fn walk(
        dir: &str,
        depth: usize,
        listed: &HashMap<String, Vec<WorkspaceEntry>>,
        expanded: &HashSet<String>,
        hidden: bool,
        out: &mut Vec<Row>,
    ) {
        let Some(entries) = listed.get(dir) else {
            return;
        };
        for entry in entries {
            if !hidden && entry.name.starts_with('.') {
                continue;
            }
            let is_dir = entry.kind == WorkspaceEntryKind::Directory;
            let open = is_dir && expanded.contains(&entry.path);
            out.push(Row {
                path: entry.path.clone(),
                name: entry.name.clone(),
                dir: is_dir,
                depth,
                size: entry.size,
                open,
            });
            if open && depth < 64 {
                walk(&entry.path, depth + 1, listed, expanded, hidden, out);
            }
        }
    }
    walk("", 0, listed, expanded, hidden, &mut out);
    out
}

/// One letter for a file's git state, as the desktop's file tree shows it.
pub fn badge(state: GitFileState) -> Option<char> {
    match state {
        GitFileState::Unchanged => None,
        GitFileState::Added => Some('A'),
        GitFileState::Modified | GitFileState::TypeChanged => Some('M'),
        GitFileState::Deleted => Some('D'),
        GitFileState::Renamed | GitFileState::Copied => Some('R'),
        GitFileState::Unmerged => Some('U'),
        GitFileState::Untracked => Some('U'),
    }
}

/// Each changed path's badge, and each directory above one a dot (`•`).
pub fn badges(status: &CheckoutGitStatus) -> HashMap<String, char> {
    let mut out = HashMap::new();
    for file in &status.files {
        let state = if file.worktree != GitFileState::Unchanged {
            file.worktree
        } else {
            file.index
        };
        let Some(letter) = badge(state) else {
            continue;
        };
        out.insert(file.path.clone(), letter);
        let mut parent = file.path.as_str();
        while let Some((dir, _)) = parent.rsplit_once('/') {
            out.entry(dir.to_owned()).or_insert('•');
            parent = dir;
        }
    }
    out
}

/// The directory a path sits in (`""` at the root).
pub fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// `name` beside `path` (a rename in place).
pub fn sibling(path: &str, name: &str) -> String {
    match parent(path) {
        "" => name.to_owned(),
        dir => format!("{dir}/{name}"),
    }
}

/// A name a person may give a file: no separators, not `.` or `..`, no
/// control characters.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\'])
        && !name.chars().any(char::is_control)
        && name.len() <= 255
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_proto::GitFileStatus;

    fn entry(path: &str, dir: bool) -> WorkspaceEntry {
        WorkspaceEntry {
            mutation_revision: None,
            path: path.into(),
            name: path.rsplit('/').next().unwrap().into(),
            kind: if dir {
                WorkspaceEntryKind::Directory
            } else {
                WorkspaceEntryKind::File
            },
            size: (!dir).then_some(10),
            modified_at: None,
            ignored: false,
            read_only: false,
        }
    }

    #[test]
    fn files_open_by_their_extension() {
        assert_eq!(opens("README.md"), Opens::Markdown);
        assert_eq!(opens("docs/a.MARKDOWN"), Opens::Markdown);
        assert_eq!(opens("logo.PNG"), Opens::Image);
        assert_eq!(opens("src/main.rs"), Opens::Text);
        assert_eq!(opens(".png"), Opens::Text);
        assert_eq!(opens("build.zip"), Opens::Text);
    }

    #[test]
    fn sizes_read_plainly() {
        assert_eq!(size(1), "1 byte");
        assert_eq!(size(812), "812 bytes");
        assert_eq!(size(4_200), "4.2 KB");
        assert_eq!(size(18_000_000), "18 MB");
        assert_eq!(size(2_500_000_000), "2.5 GB");
    }

    #[test]
    fn the_tree_lists_open_directories_and_hides_dot_files_when_asked() {
        let mut root = vec![
            entry("b.txt", false),
            entry("src", true),
            entry(".git", true),
        ];
        sort(&mut root);
        assert_eq!(root[0].name, ".git");
        assert_eq!(root[1].name, "src");
        let listed = HashMap::from([
            (String::new(), root),
            ("src".to_owned(), vec![entry("src/main.rs", false)]),
        ]);
        let mut expanded = HashSet::new();
        let names = |rows: Vec<Row>| rows.into_iter().map(|r| r.path).collect::<Vec<_>>();
        assert_eq!(names(rows(&listed, &expanded, false)), ["src", "b.txt"]);
        expanded.insert("src".to_owned());
        let shown = rows(&listed, &expanded, false);
        assert_eq!(shown[1].depth, 1);
        assert_eq!(names(shown), ["src", "src/main.rs", "b.txt"]);
        assert_eq!(rows(&listed, &expanded, true).len(), 4);
    }

    #[test]
    fn badges_mark_changed_files_and_their_folders() {
        let status = CheckoutGitStatus {
            checkout_id: "c".into(),
            device_id: "d".into(),
            revision: "r".into(),
            complete: true,
            files: vec![
                GitFileStatus {
                    path: "src/app/main.rs".into(),
                    old_path: None,
                    index: GitFileState::Unchanged,
                    worktree: GitFileState::Modified,
                },
                GitFileStatus {
                    path: "new.txt".into(),
                    old_path: None,
                    index: GitFileState::Added,
                    worktree: GitFileState::Unchanged,
                },
            ],
        };
        let badges = badges(&status);
        assert_eq!(badges["src/app/main.rs"], 'M');
        assert_eq!(badges["src/app"], '•');
        assert_eq!(badges["src"], '•');
        assert_eq!(badges["new.txt"], 'A');
    }

    #[test]
    fn renames_stay_beside_the_file() {
        assert_eq!(sibling("src/a.rs", "b.rs"), "src/b.rs");
        assert_eq!(sibling("a.rs", "b.rs"), "b.rs");
        assert!(valid_name("b.rs"));
        assert!(!valid_name("../b"));
        assert!(!valid_name(".."));
        assert!(!valid_name(""));
    }
}
