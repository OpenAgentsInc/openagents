//! The repository the shell sits in, bounded and cheap.
//!
//! The agent has no tools — it cannot open files on demand — so the card
//! and the sniff are how it looks around. The card names the repo, its
//! workspace members, its docs, and its top level; the sniff takes the
//! draft's terms to `git grep` and brings back the paths and lines they
//! touch. A question about the project is answered from the project, not
//! guessed at, and a question the context cannot cover says so.

use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// The workspace members a card lists at most.
const MEMBERS_MAX: usize = 24;
/// The top-level entries a card lists at most.
const ENTRIES_MAX: usize = 40;
/// The head of `AGENTS.md` or `README.md` a card carries.
const DOC_HEAD_BYTES: usize = 1200;
/// The draft terms a sniff follows.
const TERMS_MAX: usize = 4;
/// The paths a sniff may bring back.
const PATHS_MAX: usize = 8;
/// The grep hits a sniff may quote.
const HITS_MAX: usize = 10;
/// One quoted line's width.
const LINE_MAX: usize = 160;
/// The whole sniff block's size.
const SNIFF_BYTES: usize = 2048;

/// Words too common or structural to sniff for.
const STOP: &[&str] = &[
    "the", "and", "for", "are", "how", "what", "does", "this", "that", "with", "use", "using",
    "our", "your", "you", "can", "have", "from", "into", "project", "repo", "code", "work", "tell",
    "show", "describe", "explain", "about", "why", "when", "where", "which",
];

/// A repository the shell can describe: its root, and the prebuilt card.
pub struct Repo {
    root: PathBuf,
    card: String,
    /// The member names — `coder`, `jev`, `nostr` — for the classify
    /// state, so the router knows the project's own vocabulary.
    members: Vec<String>,
}

impl Repo {
    /// The repo `dir` sits in — the git root when there is one, the
    /// directory itself otherwise. Always `Some`: even a bare directory
    /// is worth naming.
    pub fn discover(dir: &Path) -> Option<Self> {
        let root = git(dir, &["rev-parse", "--show-toplevel"])
            .map(PathBuf::from)
            .unwrap_or_else(|| dir.to_path_buf());
        let branch =
            git(&root, &["rev-parse", "--abbrev-ref", "HEAD"]).filter(|name| name != "HEAD");
        let name = root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.display().to_string());

        // Members are the crates/ directories — the repo's own words.
        let mut members: Vec<String> = fs::read_dir(root.join("crates"))
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                let entry = entry.ok()?;
                entry
                    .file_type()
                    .ok()?
                    .is_dir()
                    .then(|| entry.file_name().to_string_lossy().into_owned())
            })
            .collect();
        members.sort();
        members.truncate(MEMBERS_MAX);

        // Top level: directories first, then files, each bounded.
        let mut entries: Vec<String> = fs::read_dir(&root)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') {
                    return None;
                }
                let dir = entry.file_type().ok()?.is_dir();
                Some(if dir { format!("{name}/") } else { name })
            })
            .collect();
        entries.sort();
        entries.truncate(ENTRIES_MAX);

        let mut card = format!("repo: {name}");
        if let Some(branch) = &branch {
            card.push_str(&format!(" (branch {branch})"));
        }
        card.push('\n');
        if !members.is_empty() {
            card.push_str(&format!("members: {}\n", members.join(", ")));
        }
        if !entries.is_empty() {
            card.push_str(&format!("top level: {}\n", entries.join(" ")));
        }
        for doc in ["AGENTS.md", "README.md"] {
            if let Some(head) = doc_head(&root.join(doc)) {
                card.push_str(&format!("{doc}:\n{head}\n"));
            }
        }

        Some(Self {
            root,
            card,
            members,
        })
    }

    /// The directory the repo sits in, which a trace records as the place
    /// the session ran.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The member names, for the classify state.
    pub fn members(&self) -> &[String] {
        &self.members
    }

    /// The prompt block for `draft`: the card plus whatever the draft's
    /// terms turn up in the tree.
    pub fn context_for(&self, draft: &str) -> String {
        let mut context = format!("repo context:\n{}", self.card);
        let sniff = self.sniff(draft);
        if !sniff.is_empty() {
            context.push_str(&sniff);
        }
        context
    }

    /// Follows the draft's terms through `git grep`: the paths they hit
    /// and a few quoted lines. Empty outside a git repo or on no hits.
    fn sniff(&self, draft: &str) -> String {
        if git(&self.root, &["rev-parse", "--git-dir"]).is_none() {
            return String::new();
        }
        let mut paths: BTreeSet<String> = BTreeSet::new();
        let mut hits: Vec<String> = Vec::new();
        let mut bytes = 0usize;
        for term in terms(draft) {
            let Some(listed) = git(
                &self.root,
                &[
                    "grep",
                    "-l",
                    "-i",
                    "--max-count",
                    "1",
                    "-e",
                    &term,
                    "--",
                    ":!*.lock",
                ],
            ) else {
                continue;
            };
            for path in listed.lines().take(PATHS_MAX) {
                paths.insert(path.to_string());
                if paths.len() >= PATHS_MAX {
                    break;
                }
            }
            for path in paths.iter().cloned().collect::<Vec<_>>() {
                if hits.len() >= HITS_MAX || bytes >= SNIFF_BYTES {
                    break;
                }
                let Some(hit) = git(
                    &self.root,
                    &[
                        "grep",
                        "-n",
                        "-i",
                        "--max-count",
                        "1",
                        "-e",
                        &term,
                        "--",
                        &path,
                    ],
                ) else {
                    continue;
                };
                for line in hit.lines().take(2) {
                    let line = &line[..line.floor_char_boundary(LINE_MAX.min(line.len()))];
                    if bytes + line.len() > SNIFF_BYTES || hits.len() >= HITS_MAX {
                        break;
                    }
                    bytes += line.len();
                    hits.push(line.to_string());
                }
            }
        }
        if paths.is_empty() {
            return String::new();
        }
        let mut out = format!(
            "sniff: {}\n",
            paths.iter().cloned().collect::<Vec<_>>().join(", ")
        );
        for hit in hits {
            out.push_str(&format!("  {hit}\n"));
        }
        out
    }
}

/// The draft's searchable terms: identifier-shaped words of three letters
/// or more, minus the stop list, first four distinct.
fn terms(draft: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for word in draft.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-')) {
        let word = word.trim_matches(|c: char| c == '-' || c == '_');
        if word.len() < 3 || STOP.contains(&word.to_lowercase().as_str()) {
            continue;
        }
        if seen.insert(word.to_lowercase()) {
            out.push(word.to_string());
            if out.len() >= TERMS_MAX {
                break;
            }
        }
    }
    out
}

/// The first bounded bytes of a doc file, whole lines only.
fn doc_head(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    // A bounded prefix must not allocate the rest of a large document.
    let mut bytes = Vec::new();
    file.take((DOC_HEAD_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    let complete = match std::str::from_utf8(&bytes) {
        Ok(text) => text,
        Err(error) if error.error_len().is_none() => {
            std::str::from_utf8(&bytes[..error.valid_up_to()]).ok()?
        }
        Err(_) => return None,
    };
    let text = complete;
    let mut head = String::new();
    for line in text.lines() {
        if head.len() + line.len() + 1 > DOC_HEAD_BYTES {
            break;
        }
        head.push_str(line);
        head.push('\n');
    }
    (!head.is_empty()).then_some(head)
}

/// One `git` call at `dir`, trimmed stdout on success.
fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(dir).args(args);
    let output = crate::capability::bounded::run(command, Duration::from_secs(2)).ok()?;
    // A partial path or grep record must not masquerade as complete evidence.
    if output.code != Some(0) || output.truncated {
        return None;
    }
    let text = output.out.trim().to_string();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terms_pick_identifiers_and_skip_stop_words() {
        let found = terms("how do we use jev in this project");
        assert_eq!(found, vec!["jev".to_string()]);
        let found = terms("describe the relay auth handshake");
        assert!(found.contains(&"relay".to_string()));
        assert!(found.contains(&"auth".to_string()));
        assert!(found.contains(&"handshake".to_string()));
        assert!(!found.contains(&"the".to_string()));
    }

    #[test]
    fn document_prefix_handles_a_split_character_and_large_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("README.md");
        let text = format!(
            "heading\n{}é\n{}",
            "a".repeat(DOC_HEAD_BYTES - 8),
            "z".repeat(100_000)
        );
        fs::write(&path, text).unwrap();
        assert_eq!(doc_head(&path).as_deref(), Some("heading\n"));
    }

    #[test]
    fn grep_excerpt_does_not_split_utf8() {
        let dir = tempfile::tempdir().unwrap();
        assert!(git(dir.path(), &["init"]).is_some());
        let line = format!("needle {}", "é".repeat(100));
        fs::write(dir.path().join("sample.txt"), line).unwrap();
        // Indexing is enough: grep reads the working tree without a commit.
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(dir.path())
            .args(["add", "sample.txt"]);
        let result = crate::capability::bounded::run(command, Duration::from_secs(2)).unwrap();
        assert_eq!(result.code, Some(0));
        let repo = Repo::discover(dir.path()).unwrap();
        let context = repo.context_for("needle");
        assert!(context.contains("sample.txt:1:needle"));
        assert!(!context.contains('\u{fffd}'));
    }

    #[test]
    fn oversized_git_output_is_not_treated_as_complete() {
        let dir = tempfile::tempdir().unwrap();
        assert!(git(dir.path(), &["init"]).is_some());
        fs::write(dir.path().join("large.txt"), "x".repeat(100_000)).unwrap();
        assert!(git(dir.path(), &["hash-object", "-w", "large.txt"]).is_some());
        let digest = git(dir.path(), &["hash-object", "large.txt"]).unwrap();
        assert!(git(dir.path(), &["cat-file", "blob", &digest]).is_none());
    }

    #[test]
    fn a_repo_discovers_this_workspace() {
        let repo = Repo::discover(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        assert!(repo.members().iter().any(|member| member == "coder"));
        let context = repo.context_for("what does jev do");
        assert!(context.contains("crates/"));
    }
}
