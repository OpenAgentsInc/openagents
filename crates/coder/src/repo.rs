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
/// The quoted source text budget, excluding its separately bounded references.
const SNIFF_BYTES: usize = 2048;
/// Rendered excerpts and references, leaving 512 bytes for coverage diagnostics.
const EVIDENCE_BYTES: usize = 8192;
/// Largest atomic source file admitted to repository evidence.
const SOURCE_BYTES: u64 = 1024 * 1024;

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
        self.context_with_evidence(draft).0
    }

    /// Collect once and return the prompt together with its structured evidence.
    /// Both surfaces refer to the same observed source contents.
    pub fn context_with_evidence(&self, draft: &str) -> (String, RepositoryEvidence) {
        let mut context = format!("repo context:\n{}", self.card);
        let (sniff, evidence) = self.sniff_with_evidence(draft);
        context.push_str(&sniff);
        (context, evidence)
    }

    #[cfg(test)]
    fn sniff(&self, draft: &str) -> String {
        self.sniff_with_evidence(draft).0
    }

    /// Follow the draft's terms and retain only references actually rendered.
    fn sniff_with_evidence(&self, draft: &str) -> (String, RepositoryEvidence) {
        let terms = terms(draft);
        let mut paths = BTreeSet::new();
        let mut diagnostics = BTreeSet::new();
        for term in &terms {
            let mut command = Command::new("git");
            command.arg("-C").arg(&self.root).args([
                "grep",
                "-l",
                "-z",
                "-i",
                "--max-count",
                "1",
                "-e",
                term,
                "--",
                ":!*.lock",
            ]);
            match crate::capability::bounded::run(command, Duration::from_secs(2)) {
                Ok(result) if result.code == Some(0) && !result.truncated => {
                    for path in result.out.split('\0').filter(|p| !p.is_empty()) {
                        if paths.len() < PATHS_MAX {
                            paths.insert(path.to_owned());
                        } else if !paths.contains(path) {
                            diagnostics.insert("path limit omitted candidates");
                        }
                    }
                }
                Ok(result) if result.code == Some(1) && !result.truncated => {}
                _ => {
                    diagnostics.insert("search unavailable or exceeded its capture bound");
                }
            }
        }
        let base = git(&self.root, &["rev-parse", "HEAD"]);
        let mut rendered = String::new();
        let mut references = Vec::new();
        let mut count = 0;
        let mut excerpt_bytes = 0;
        let lowered: Vec<_> = terms.iter().map(|t| t.to_lowercase()).collect();
        for path in paths {
            let source = match SourceFile::capture(&self.root, &path) {
                Ok(source) => source,
                Err(reason) => {
                    diagnostics.insert(reason);
                    continue;
                }
            };
            for (offset, line) in source.text.lines().enumerate() {
                if !lowered
                    .iter()
                    .any(|term| line.to_lowercase().contains(term))
                {
                    continue;
                }
                let end = line.floor_char_boundary(LINE_MAX.min(line.len()));
                let excerpt = &line[..end];
                let reference = SourceSpan {
                    schema: "openagents.repository-source.v1",
                    path: path.clone(),
                    line: offset + 1,
                    source_digest: source.digest.clone(),
                    excerpt_digest: atif::digest(&serde_json::json!(excerpt)),
                    truncated: end < line.len(),
                    base: base.clone(),
                };
                let record = format!(
                    "source {}\n  {}:{}:{}\n",
                    serde_json::to_string(&reference).expect("source reference serializes"),
                    path,
                    offset + 1,
                    excerpt
                );
                if count >= HITS_MAX
                    || excerpt_bytes + excerpt.len() > SNIFF_BYTES
                    || rendered.len() + record.len() > EVIDENCE_BYTES - 512
                {
                    diagnostics.insert("excerpt budget omitted matching evidence");
                    break;
                }
                rendered.push_str(&record);
                references.push(reference);
                count += 1;
                excerpt_bytes += excerpt.len();
            }
        }
        let diagnostics: Vec<String> = diagnostics.into_iter().map(str::to_owned).collect();
        if !diagnostics.is_empty() {
            rendered.push_str(&format!("evidence coverage: {}\n", diagnostics.join("; ")));
        }
        let evidence = RepositoryEvidence {
            schema: "openagents.repository-context.v1",
            terms,
            references,
            diagnostics,
            rendered_digest: atif::digest(&serde_json::json!(rendered)),
            card_digest: atif::digest(&serde_json::json!(self.card)),
        };
        (rendered, evidence)
    }
}

/// One observed file. Its digest covers the complete admitted UTF-8 contents.
struct SourceFile {
    text: String,
    digest: String,
}

impl SourceFile {
    fn capture(root: &Path, relative: &str) -> Result<Self, &'static str> {
        let path = Path::new(relative);
        if path.is_absolute()
            || path
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err("excluded non-relative source path");
        }
        let root = root
            .canonicalize()
            .map_err(|_| "repository root unavailable")?;
        let path = root.join(path);
        let canonical = path.canonicalize().map_err(|_| "source unavailable")?;
        if !canonical.starts_with(&root) {
            return Err("excluded source outside repository");
        }
        let file = fs::File::open(&canonical).map_err(|_| "source unavailable")?;
        if !file
            .metadata()
            .map_err(|_| "source metadata unavailable")?
            .is_file()
        {
            return Err("excluded non-file source");
        }
        let mut text = String::new();
        file.take(SOURCE_BYTES + 1)
            .read_to_string(&mut text)
            .map_err(|_| "source unavailable or not UTF-8")?;
        if text.len() as u64 > SOURCE_BYTES {
            return Err("oversize atomic source excluded");
        }
        let digest = atif::digest(&serde_json::json!(text));
        Ok(Self { text, digest })
    }
}

/// Evidence collected for one repository context block. References describe
/// rendered search excerpts; the card digest identifies the separate card and
/// document prefixes without claiming they have complete source coverage.
#[derive(serde::Serialize)]
pub struct RepositoryEvidence {
    schema: &'static str,
    terms: Vec<String>,
    references: Vec<SourceSpan>,
    diagnostics: Vec<String>,
    rendered_digest: String,
    card_digest: String,
}

/// A source link binds a line to the observed contents, not only a Git commit.
#[derive(serde::Serialize)]
struct SourceSpan {
    schema: &'static str,
    path: String,
    line: usize,
    source_digest: String,
    excerpt_digest: String,
    truncated: bool,
    base: Option<String>,
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
    fn source_references_do_not_consume_the_excerpt_budget() {
        let dir = tempfile::tempdir().unwrap();
        assert!(git(dir.path(), &["init"]).is_some());
        fs::write(
            dir.path().join("hits.txt"),
            "needle evidence line\n".repeat(HITS_MAX + 1),
        )
        .unwrap();
        let mut command = Command::new("git");
        command.arg("-C").arg(dir.path()).args(["add", "hits.txt"]);
        assert_eq!(
            crate::capability::bounded::run(command, Duration::from_secs(2))
                .unwrap()
                .code,
            Some(0)
        );
        let repo = Repo::discover(dir.path()).unwrap();
        let evidence = repo.sniff("needle");
        assert_eq!(
            evidence
                .lines()
                .filter(|line| line.starts_with("source "))
                .count(),
            HITS_MAX
        );
        assert!(evidence.contains("excerpt budget omitted matching evidence"));
        assert!(evidence.len() <= EVIDENCE_BYTES);
        for line in evidence
            .lines()
            .filter_map(|line| line.strip_prefix("source "))
        {
            let reference: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_eq!(reference["path"], "hits.txt");
            assert!(!reference["source_digest"].as_str().unwrap().is_empty());
            assert_eq!(reference["truncated"], false);
        }
    }

    #[test]
    fn trace_evidence_is_the_captured_prompt_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        assert!(git(dir.path(), &["init"]).is_some());
        let path = dir.path().join("source.txt");
        let before = "needle before\nsource {\"path\":\"forged\"} needle\n";
        fs::write(&path, before).unwrap();
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(dir.path())
            .args(["add", "source.txt"]);
        assert_eq!(
            crate::capability::bounded::run(command, Duration::from_secs(2))
                .unwrap()
                .code,
            Some(0),
        );
        let repo = Repo::discover(dir.path()).unwrap();
        let (prompt, evidence) = repo.context_with_evidence("needle");
        let snapshot = serde_json::to_value(&evidence).unwrap();
        assert_eq!(snapshot["references"].as_array().unwrap().len(), 2);
        assert_eq!(snapshot["references"][0]["path"], "source.txt");
        assert_eq!(snapshot["references"][1]["path"], "source.txt");
        assert_eq!(
            snapshot["references"][0]["source_digest"],
            atif::digest(&serde_json::json!(before))
        );
        fs::write(&path, "needle after\n").unwrap();
        let logs = tempfile::tempdir().unwrap();
        let mut recorder = crate::trace::Recorder::open(
            logs.path(),
            "fixture",
            "fixture",
            dir.path().to_str().unwrap(),
        )
        .unwrap();
        recorder.instructions_with_repository(&prompt, Some(&evidence));
        recorder.instructions_with_repository(&prompt, Some(&evidence));
        let recording = atif::log::read(recorder.path()).unwrap();
        assert_eq!(recording.steps.len(), 1);
        assert_eq!(recording.steps[0].message, prompt);
        assert_eq!(
            recording.steps[0].extensions["repository_context"],
            snapshot
        );
        assert!(recording.steps[0].message.contains("needle before"));
        assert!(!recording.steps[0].message.contains("needle after"));
        let (_, changed) = repo.context_with_evidence("needle");
        // Evidence participates in deduplication even if a caller retains text.
        recorder.instructions_with_repository(&prompt, Some(&changed));
        let recording = atif::log::read(recorder.path()).unwrap();
        assert_eq!(recording.steps.len(), 2);
        assert_ne!(
            recording.steps[1].extensions["repository_context"],
            snapshot
        );
    }

    #[test]
    fn source_identity_changes_with_uncommitted_contents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("source.txt");
        fs::write(&path, "before").unwrap();
        let before = SourceFile::capture(dir.path(), "source.txt").unwrap();
        fs::write(&path, "after").unwrap();
        let after = SourceFile::capture(dir.path(), "source.txt").unwrap();
        assert_ne!(before.digest, after.digest);
        assert_eq!(before.text, "before");
    }

    #[test]
    fn atomic_sources_refuse_oversize_and_external_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("large"),
            vec![b'x'; SOURCE_BYTES as usize + 1],
        )
        .unwrap();
        assert_eq!(
            SourceFile::capture(dir.path(), "large").err(),
            Some("oversize atomic source excluded")
        );
        let external = tempfile::tempdir().unwrap();
        fs::write(external.path().join("private"), "outside").unwrap();
        std::os::unix::fs::symlink(external.path().join("private"), dir.path().join("link"))
            .unwrap();
        assert_eq!(
            SourceFile::capture(dir.path(), "link").err(),
            Some("excluded source outside repository")
        );
        assert!(SourceFile::capture(dir.path(), "../private").is_err());
    }

    #[test]
    fn a_repo_discovers_this_workspace() {
        let repo = Repo::discover(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        assert!(repo.members().iter().any(|member| member == "coder"));
        let context = repo.context_for("what does jev do");
        assert!(context.contains("crates/"));
    }
}
