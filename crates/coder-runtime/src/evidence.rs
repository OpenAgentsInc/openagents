//! Evidence candidates: what the host observed, bounded and digested,
//! before anything ranks it.
//!
//! A later decision function will rank repository evidence — which files
//! a review, a repair, or a plan should stand on. Ranking is a judgment;
//! building the set it ranks over is not. This module builds that set the
//! only way such a set can be trusted: from deterministic, bounded
//! observations of the tree the caller named, never from a path a model
//! invented. A candidate is a file's bytes, a path's presence, a bounded
//! search hit, or a diff or test reference the caller declared. The
//! builder runs no command, asks no model, and reaches for nothing off
//! the disk.
//!
//! # A candidate is a versioned observation
//!
//! Every candidate carries the same identity: the stable repo-relative
//! path, the line span it covers when it covers one, the base commit the
//! caller observed it against, the SHA-256 of the bytes read, and how
//! many bytes those were. [`Readness`] says how much of the content the
//! candidate holds — `full`, `truncated`, or `refused` — so a candidate
//! is never silently empty: either it carries bytes and the digest that
//! names them, or it carries the reason it does not. The digests are
//! what make two runs comparable — the same path at the same base with
//! the same digest is the same evidence, and a changed digest is a
//! changed file, whatever a model later says about it.
//!
//! # A path can be named without being read
//!
//! The request declares a **read set**: the paths whose content may be
//! admitted. A read asked for a path outside the set is not a failed
//! read — it is a refused entry, a candidate that names the path and its
//! presence while holding no byte of it. Existence checks, search hits,
//! and declared references name paths the same way. Naming is a
//! disclosure too, so every refusal records why the content was
//! withheld: a reviewer can see what was hidden and on whose say-so.
//!
//! # Omission is evidence too
//!
//! The bounds are stated, not discovered: `max_files`, `max_bytes` per
//! file, and `max_total_bytes` across the set. An atomic input that
//! cannot fit — a file that stands alone too large, a read the file
//! count or the byte total no longer reaches — is rejected whole and
//! recorded in `omitted` with the bound that excluded it, never shortened
//! quietly. Truncation happens only where the caller declared it
//! acceptable, and the candidate then records the span it retained. An
//! answer that dropped an input without a record would be claiming a
//! coverage it did not earn.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A line span, one-based and inclusive: `Span { start: 4, end: 9 }`
/// names lines four through nine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// The bounds every read in one [`Request`] shares.
///
/// Existence checks, search hits, and declared references name paths
/// without reading them and spend nothing here; the bounds govern
/// admitted content only. A search carries bounds of its own for the
/// scan it performs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct Bounds {
    /// Files whose content the build may admit at most.
    pub max_files: usize,
    /// Bytes one file may contribute at most.
    pub max_bytes_per_file: u64,
    /// Bytes the whole set may admit at most.
    pub max_total_bytes: u64,
}

/// How much of a candidate's content was admitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Readness {
    /// Every byte of what was asked for, within the bounds.
    Full,
    /// The caller declared truncation acceptable; the candidate records
    /// the span it retained.
    Truncated,
    /// The path was named but no byte of it was admitted — outside the
    /// declared read set, absent from the tree, or asked for in a way
    /// that carries no content.
    Refused,
}

impl Readness {
    /// The word a record spells this with.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Readness::Full => "full",
            Readness::Truncated => "truncated",
            Readness::Refused => "refused",
        }
    }
}

/// Which observation produced a candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Observation {
    /// A declared file read — the only observation that admits content.
    Read,
    /// A path's presence, checked and recorded.
    Exists,
    /// A bounded search hit: a path and the line its first match fell on.
    Search,
    /// A diff or test reference the caller declared.
    Reference,
}

impl Observation {
    /// The word a record spells this with.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Observation::Read => "read",
            Observation::Exists => "exists",
            Observation::Search => "search",
            Observation::Reference => "reference",
        }
    }
}

/// The bound an [`Omitted`] names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Omission {
    /// The input cannot fit the per-file byte bound, and the caller did
    /// not declare truncation acceptable — or no whole line of it fits.
    Oversize,
    /// The file count bound was already spent.
    OverCount,
    /// The total byte bound was already spent.
    OverTotal,
    /// The input was denied at admission: outside the declared read set,
    /// a path no repository contains, or a span that names no lines.
    Denied,
}

impl Omission {
    /// The word a record spells this with.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Omission::Oversize => "oversize",
            Omission::OverCount => "over-count",
            Omission::OverTotal => "over-total",
            Omission::Denied => "denied",
        }
    }
}

/// One input the bounds excluded, and why.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Omitted {
    /// The path the input named — repo-relative when the path was one,
    /// the caller's own spelling when it was not.
    pub path: String,
    /// Which bound excluded it.
    pub reason: Omission,
}

/// One observed path: what the build can prove about it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Candidate {
    /// The stable repo-relative path. Always relative — an input that
    /// cannot be named relative to the root is an omission, not a
    /// candidate.
    pub path: String,
    /// The line span this candidate covers, when it covers one: the span
    /// the caller declared, the line a search hit fell on, or the span a
    /// truncated read retained. `None` means the whole file.
    pub span: Option<Span>,
    /// The base commit the caller observed this request against.
    pub base: Option<String>,
    /// The SHA-256 of the bytes read — `None` when no byte was admitted,
    /// which is every candidate whose `readness` is `refused`.
    pub digest: Option<String>,
    /// How many bytes the digest covers.
    pub bytes: u64,
    /// How much of the content this candidate carries.
    pub readness: Readness,
    /// Which observation produced the candidate.
    pub observation: Observation,
    /// Whether the path was present in the tree when observed.
    pub present: bool,
    /// Why content was withheld, when it was — the disclosure decision a
    /// reviewer reads to see what stayed hidden and why.
    pub withheld: Option<String>,
}

/// One file the caller asks to read.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct FileAsk {
    /// The repo-relative path.
    pub path: String,
    /// The line span to read, or `None` for the whole file.
    pub span: Option<Span>,
    /// Whether the caller accepts a truncated read when the content does
    /// not fit the bounds. Without it an input that cannot fit is
    /// rejected whole — never quietly shortened.
    pub allow_truncate: bool,
}

impl FileAsk {
    /// The whole file, atomic: it fits the bounds or it is rejected.
    #[must_use]
    pub fn read(path: impl Into<String>) -> Self {
        FileAsk {
            path: path.into(),
            span: None,
            allow_truncate: false,
        }
    }

    /// One line span, atomic by default like a whole-file ask.
    #[must_use]
    pub fn lines(path: impl Into<String>, start: usize, end: usize) -> Self {
        FileAsk {
            path: path.into(),
            span: Some(Span { start, end }),
            allow_truncate: false,
        }
    }

    /// Declares truncation acceptable for this ask.
    #[must_use]
    pub fn allowing_truncation(mut self) -> Self {
        self.allow_truncate = true;
        self
    }
}

/// A bounded search: declared terms, answered as paths.
///
/// The walk is sorted, skips dot-entries, and does not follow links. A
/// hit names the path and the line the first match fell on — a search
/// admits no content, whichever set the path sits in, because naming was
/// all the caller asked for. Content for a hit comes from a read the
/// request also declares, under the read set.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct Search {
    /// The terms to match, as case-insensitive substrings of a line. A
    /// file whose any line holds any term is a hit.
    pub terms: Vec<String>,
    /// Files the walk may open at most.
    pub max_files: usize,
    /// Bytes one file may hold at most; a larger file is omitted rather
    /// than partially scanned.
    pub max_bytes: u64,
    /// Paths the search may name at most.
    pub max_hits: usize,
}

/// A diff or test reference the caller declares: a path and the span it
/// points at, recorded rather than derived.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Reference {
    /// The repo-relative path the reference names.
    pub path: String,
    /// The span it points at, when it names one.
    pub span: Option<Span>,
    /// What produced it — `diff`, `test`, or the caller's own word.
    pub kind: String,
}

impl Reference {
    /// A reference a diff declares, as a hunk header would spell it.
    #[must_use]
    pub fn diff(path: impl Into<String>, span: Option<Span>) -> Self {
        Reference {
            path: path.into(),
            span,
            kind: "diff".to_string(),
        }
    }

    /// A reference a test failure declares.
    #[must_use]
    pub fn test(path: impl Into<String>, span: Option<Span>) -> Self {
        Reference {
            path: path.into(),
            span,
            kind: "test".to_string(),
        }
    }
}

/// What a build may observe: the declared inputs and the bounds every
/// one of them shares.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Request {
    /// The base commit the caller observed these inputs against, stamped
    /// on every candidate so two runs can be compared.
    pub base: Option<String>,
    /// The paths whose content may be admitted — the read set. A path
    /// not named here can still be named as a candidate; its bytes stay
    /// unread.
    pub read_set: BTreeSet<String>,
    /// Files to read under the read set and the bounds.
    pub reads: Vec<FileAsk>,
    /// Paths to check for presence only.
    pub exists: Vec<String>,
    /// Searches to run, each bounded as it declares.
    pub searches: Vec<Search>,
    /// Diff and test references the caller declares.
    pub references: Vec<Reference>,
    /// The bounds every read shares.
    pub bounds: Bounds,
}

/// What a build produced: the candidates it could observe, and every
/// input the bounds excluded.
#[derive(Clone, Debug, Default)]
pub struct Candidates {
    /// The observed paths, ordered by path and span.
    pub candidates: Vec<Candidate>,
    /// Every input the bounds excluded, with why. Omission is evidence
    /// too — a reviewer sees what was asked for and not admitted.
    pub omitted: Vec<Omitted>,
}

impl Candidates {
    /// Builds the candidate set one request asks for.
    ///
    /// The observation kinds run in a fixed order so the answer does not
    /// depend on the order the caller listed them: reads first, because
    /// the strongest observation a path gets is the one that stands;
    /// then existence checks, declared references, and bounded searches.
    /// Candidates come out ordered by path and span; `omitted` lists the
    /// inputs the bounds excluded in the order they were considered.
    ///
    /// The build fails nothing. A request with nothing in it yields an
    /// empty answer rather than an error, and everything the bounds or
    /// the tree refused is recorded rather than raised.
    #[must_use]
    pub fn of(root: &Path, request: &Request) -> Self {
        let mut build = Build {
            root: root.canonicalize().ok(),
            read_set: request
                .read_set
                .iter()
                .filter_map(|path| relative(path))
                .collect(),
            request,
            candidates: BTreeMap::new(),
            omitted: Vec::new(),
            files: 0,
            total: 0,
        };

        let mut reads: Vec<(String, FileAsk)> = Vec::new();
        for ask in &request.reads {
            match relative(&ask.path) {
                Some(path) => reads.push((path, ask.clone())),
                None => build.omit(&ask.path, Omission::Denied),
            }
        }
        reads.sort_by(|a, b| (&a.0, a.1.span).cmp(&(&b.0, b.1.span)));
        reads.dedup_by(|a, b| a.0 == b.0 && a.1.span == b.1.span);
        for (path, ask) in reads {
            build.read(&path, ask);
        }

        let mut exists: Vec<String> = Vec::new();
        for declared in &request.exists {
            match relative(declared) {
                Some(path) => exists.push(path),
                None => build.omit(declared, Omission::Denied),
            }
        }
        exists.sort();
        exists.dedup();
        for path in exists {
            build.exists(&path);
        }

        let mut references: Vec<(String, Reference)> = Vec::new();
        for declared in &request.references {
            match relative(&declared.path) {
                Some(path) => references.push((path, declared.clone())),
                None => build.omit(&declared.path, Omission::Denied),
            }
        }
        references.sort_by(|a, b| (&a.0, a.1.span, &a.1.kind).cmp(&(&b.0, b.1.span, &b.1.kind)));
        references.dedup_by(|a, b| a.0 == b.0 && a.1.span == b.1.span && a.1.kind == b.1.kind);
        for (path, declared) in references {
            build.reference(&path, declared);
        }

        for search in &request.searches {
            build.search(search);
        }

        Candidates {
            candidates: build.candidates.into_values().collect(),
            omitted: build.omitted,
        }
    }

    /// How many candidates the build admitted.
    #[must_use]
    pub fn len(&self) -> usize {
        self.candidates.len()
    }

    /// Whether the build observed nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }
}

/// One build in progress: the resolved root, the normalized read set,
/// and the bounds' running account.
struct Build<'a> {
    root: Option<PathBuf>,
    read_set: BTreeSet<String>,
    request: &'a Request,
    candidates: BTreeMap<(String, Option<Span>), Candidate>,
    omitted: Vec<Omitted>,
    files: usize,
    total: u64,
}

impl Build<'_> {
    /// The canonical path a repo-relative name resolves to, when it
    /// resolves inside the root — a link that retargets outside it does
    /// not resolve.
    fn resolve(&self, path: &str) -> Option<PathBuf> {
        let root = self.root.as_ref()?;
        let canonical = root.join(path).canonicalize().ok()?;
        canonical.starts_with(root).then_some(canonical)
    }

    /// Whether a repo-relative path is present in the tree.
    fn present(&self, path: &str) -> bool {
        self.resolve(path).is_some()
    }

    /// Records an input the bounds excluded.
    fn omit(&mut self, path: &str, reason: Omission) {
        self.omitted.push(Omitted {
            path: path.to_string(),
            reason,
        });
    }

    /// Records a candidate. The first observation of a path at a span
    /// stands — reads run first, so content already admitted is never
    /// displaced by a weaker naming.
    fn admit(&mut self, candidate: Candidate) {
        self.candidates
            .entry((candidate.path.clone(), candidate.span))
            .or_insert(candidate);
    }

    /// Records a named entry: a path with no content admitted.
    fn refused(&mut self, path: &str, span: Option<Span>, observation: Observation, why: &str) {
        self.admit(Candidate {
            path: path.to_string(),
            span,
            base: self.request.base.clone(),
            digest: None,
            bytes: 0,
            readness: Readness::Refused,
            observation,
            present: self.present(path),
            withheld: Some(why.to_string()),
        });
    }

    /// Records an admitted read and charges it to the bounds.
    fn admitted(&mut self, path: &str, span: Option<Span>, bytes: Vec<u8>, readness: Readness) {
        self.files += 1;
        let bytes_len = bytes.len() as u64;
        self.total += bytes_len;
        self.admit(Candidate {
            path: path.to_string(),
            span,
            base: self.request.base.clone(),
            digest: Some(crate::capability::digest_bytes(&bytes)),
            bytes: bytes_len,
            readness,
            observation: Observation::Read,
            present: true,
            withheld: None,
        });
    }

    /// One file ask: path, grant, presence, count, bytes — in that
    /// order, each refusal recorded where it falls.
    fn read(&mut self, path: &str, ask: FileAsk) {
        if let Some(span) = ask.span
            && (span.start == 0 || span.start > span.end)
        {
            return self.omit(path, Omission::Denied);
        }
        if !self.read_set.contains(path) {
            self.refused(
                path,
                ask.span,
                Observation::Read,
                "outside the declared read set; named, never read",
            );
            return self.omit(path, Omission::Denied);
        }
        let Some(full) = self.resolve(path) else {
            return self.refused(
                path,
                ask.span,
                Observation::Read,
                "no such path under the root",
            );
        };
        if !full.is_file() {
            return self.refused(path, ask.span, Observation::Read, "not a regular file");
        }
        if self.files >= self.request.bounds.max_files {
            return self.omit(path, Omission::OverCount);
        }
        match ask.span {
            Some(span) => self.read_span(path, &full, span, ask.allow_truncate),
            None => self.read_whole(path, &full, ask.allow_truncate),
        }
    }

    /// A whole-file ask: the file is the atomic unit unless the caller
    /// declared truncation.
    fn read_whole(&mut self, path: &str, full: &Path, allow_truncate: bool) {
        let bounds = self.request.bounds;
        let len = full.metadata().map(|meta| meta.len()).unwrap_or(0);
        let remaining = bounds.max_total_bytes.saturating_sub(self.total);
        if len <= bounds.max_bytes_per_file && len <= remaining {
            match read_up_to(full, len) {
                Some(bytes) => self.admitted(path, None, bytes, Readness::Full),
                None => self.refused(path, None, Observation::Read, "unreadable"),
            }
            return;
        }
        if !allow_truncate {
            let reason = match len > bounds.max_bytes_per_file {
                true => Omission::Oversize,
                false => Omission::OverTotal,
            };
            return self.omit(path, reason);
        }
        self.retain(
            path,
            full,
            None,
            1,
            bounds.max_bytes_per_file.min(remaining),
        );
    }

    /// A declared span: every line the file has inside it is the atomic
    /// unit, unless the caller declared truncation.
    fn read_span(&mut self, path: &str, full: &Path, span: Span, allow_truncate: bool) {
        let bounds = self.request.bounds;
        let Some((content, covered)) = read_lines(full, span.start, span.end) else {
            return self.refused(path, Some(span), Observation::Read, "unreadable");
        };
        if covered == 0 {
            return self.refused(
                path,
                Some(span),
                Observation::Read,
                "the declared span starts beyond the file's end",
            );
        }
        let remaining = bounds.max_total_bytes.saturating_sub(self.total);
        let len = content.len() as u64;
        if len <= bounds.max_bytes_per_file && len <= remaining {
            let end = span.start + covered - 1;
            return self.admitted(
                path,
                Some(Span {
                    start: span.start,
                    end,
                }),
                content,
                Readness::Full,
            );
        }
        if !allow_truncate {
            let reason = match len > bounds.max_bytes_per_file {
                true => Omission::Oversize,
                false => Omission::OverTotal,
            };
            return self.omit(path, reason);
        }
        self.retain(
            path,
            full,
            Some(span),
            span.start,
            bounds.max_bytes_per_file.min(remaining),
        );
    }

    /// The truncation path: keep whole lines inside the byte budget and
    /// record the span they cover. A first line that alone cannot fit is
    /// an atomic input rejected, not a candidate holding nothing.
    fn retain(&mut self, path: &str, full: &Path, span: Option<Span>, start: usize, budget: u64) {
        let content = match span {
            Some(span) => match read_lines(full, span.start, span.end) {
                Some((content, covered)) if covered > 0 => content,
                _ => {
                    return self.refused(
                        path,
                        Some(span),
                        Observation::Read,
                        "the declared span starts beyond the file's end",
                    );
                }
            },
            None => match read_up_to(full, budget) {
                // The prefix can end mid-line, and the retained span
                // counts complete lines only — drop the partial tail.
                Some(bytes) => match bytes.iter().rposition(|byte| *byte == b'\n') {
                    Some(end) => bytes[..=end].to_vec(),
                    None => Vec::new(),
                },
                None => return self.refused(path, None, Observation::Read, "unreadable"),
            },
        };
        let (kept, lines) = retain_lines(&content, budget);
        if lines == 0 {
            // No complete line fit. The tighter bound is the one that
            // excluded the first line: `budget` is the smaller of the
            // per-file bound and what remained of the total.
            let reason = match self.request.bounds.max_bytes_per_file <= budget {
                true => Omission::Oversize,
                false => Omission::OverTotal,
            };
            return self.omit(path, reason);
        }
        self.admitted(
            path,
            Some(Span {
                start,
                end: start + lines - 1,
            }),
            kept,
            Readness::Truncated,
        );
    }

    /// A path checked for presence — named, never read.
    fn exists(&mut self, path: &str) {
        self.refused(
            path,
            None,
            Observation::Exists,
            "named for existence; no content was requested",
        );
    }

    /// A reference the caller declared — named, never read.
    fn reference(&mut self, path: &str, declared: Reference) {
        if let Some(span) = declared.span
            && (span.start == 0 || span.start > span.end)
        {
            return self.omit(path, Omission::Denied);
        }
        self.refused(
            path,
            declared.span,
            Observation::Reference,
            &format!(
                "a {} reference names the path; no content was requested",
                declared.kind
            ),
        );
    }

    /// A bounded search: a sorted walk, a scan bound, a hit bound — and
    /// every hit a named entry, never a read.
    fn search(&mut self, search: &Search) {
        let terms: Vec<String> = search
            .terms
            .iter()
            .map(|term| term.to_lowercase())
            .collect();
        let Some(root) = self.root.clone() else {
            return;
        };
        if terms.is_empty() || search.max_hits == 0 {
            return;
        }
        let mut scanned = 0;
        let mut hits = 0;
        self.walk(&root, search, &terms, &mut scanned, &mut hits);
    }

    /// One directory of the walk, in sorted order. `false` stops the
    /// walk: the scan bound spent, and the first file it excluded is
    /// already recorded.
    fn walk(
        &mut self,
        dir: &Path,
        search: &Search,
        terms: &[String],
        scanned: &mut usize,
        hits: &mut usize,
    ) -> bool {
        let mut entries: Vec<PathBuf> = match fs::read_dir(dir) {
            Ok(read) => read
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .collect(),
            Err(_) => return true,
        };
        entries.sort();
        for entry in entries {
            if entry
                .file_name()
                .map(|name| name.to_string_lossy().starts_with('.'))
                .unwrap_or(true)
            {
                continue;
            }
            let Ok(meta) = fs::symlink_metadata(&entry) else {
                continue;
            };
            if meta.is_dir() {
                if !self.walk(&entry, search, terms, scanned, hits) {
                    return false;
                }
                continue;
            }
            if !meta.is_file() {
                continue;
            }
            let path = rel_of(self.root.as_deref().unwrap_or(dir), &entry);
            if *scanned >= search.max_files {
                self.omit(&path, Omission::OverCount);
                return false;
            }
            *scanned += 1;
            if meta.len() > search.max_bytes {
                self.omit(&path, Omission::Oversize);
                continue;
            }
            let Some(line) = first_match(&entry, terms) else {
                continue;
            };
            if *hits >= search.max_hits {
                self.omit(&path, Omission::OverCount);
                continue;
            }
            *hits += 1;
            self.admit(Candidate {
                path,
                span: Some(Span {
                    start: line,
                    end: line,
                }),
                base: self.request.base.clone(),
                digest: None,
                bytes: 0,
                readness: Readness::Refused,
                observation: Observation::Search,
                present: true,
                withheld: Some(
                    "a bounded search names the path; no content was requested".to_string(),
                ),
            });
        }
        true
    }
}

/// The repo-relative form of a declared path, or `None` when no
/// repository can contain it: absolute paths and parent steps are
/// refused, the same rule a source's file path follows.
fn relative(path: &str) -> Option<String> {
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        return None;
    }
    let mut parts = Vec::new();
    for component in candidate.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            _ => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// The repo-relative spelling of a path already under the root.
fn rel_of(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<String>>()
        .join("/")
}

/// Up to `limit` bytes of `path` — the digest names exactly what was
/// read, so a file that changed under the read still cannot masquerade.
fn read_up_to(path: &Path, limit: u64) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(limit)
        .read_to_end(&mut bytes)
        .ok()?;
    Some(bytes)
}

/// The bytes of lines `start..=end` of `path`, and how many of them the
/// file had to give — zero means the span starts past the file's end.
fn read_lines(path: &Path, start: usize, end: usize) -> Option<(Vec<u8>, usize)> {
    let mut reader = BufReader::new(fs::File::open(path).ok()?);
    let mut content = Vec::new();
    let mut line = Vec::new();
    let mut covered = 0;
    for n in 1..=end {
        line.clear();
        if reader.read_until(b'\n', &mut line).ok()? == 0 {
            break;
        }
        if n >= start {
            content.extend_from_slice(&line);
            covered += 1;
        }
    }
    Some((content, covered))
}

/// The whole lines of `content` that fit in `budget`, and how many that
/// was. A line never splits — the retained span stays a span.
fn retain_lines(content: &[u8], budget: u64) -> (Vec<u8>, usize) {
    let mut kept = Vec::new();
    let mut lines = 0;
    for line in content.split_inclusive(|byte| *byte == b'\n') {
        if kept.len() as u64 + line.len() as u64 > budget {
            break;
        }
        kept.extend_from_slice(line);
        lines += 1;
    }
    (kept, lines)
}

/// The first line of `path` holding any of `terms`, compared
/// case-insensitively — the search's whole disclosure.
fn first_match(path: &Path, terms: &[String]) -> Option<usize> {
    let mut reader = BufReader::new(fs::File::open(path).ok()?);
    let mut line = Vec::new();
    let mut n = 0;
    loop {
        line.clear();
        if reader.read_until(b'\n', &mut line).ok()? == 0 {
            return None;
        }
        n += 1;
        let text = String::from_utf8_lossy(&line).to_lowercase();
        if terms.iter().any(|term| text.contains(term)) {
            return Some(n);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A request generous enough to read anything: four files, a
    /// kilobyte each, a page in total.
    fn open() -> Request {
        Request {
            base: Some("base-commit".to_string()),
            read_set: BTreeSet::new(),
            reads: Vec::new(),
            exists: Vec::new(),
            searches: Vec::new(),
            references: Vec::new(),
            bounds: Bounds {
                max_files: 4,
                max_bytes_per_file: 1024,
                max_total_bytes: 4096,
            },
        }
    }

    #[test]
    fn a_file_under_bounds_yields_a_full_candidate_with_its_digest() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), b"hello\n").unwrap();
        let mut request = open();
        request.read_set.insert("a.txt".to_string());
        request.reads.push(FileAsk::read("a.txt"));

        let out = Candidates::of(dir.path(), &request);

        assert!(out.omitted.is_empty(), "{:?}", out.omitted);
        assert_eq!(out.candidates.len(), 1);
        let candidate = &out.candidates[0];
        assert_eq!(candidate.path, "a.txt");
        assert_eq!(candidate.readness, Readness::Full);
        assert_eq!(
            candidate.digest.as_deref(),
            Some(crate::capability::digest_bytes(b"hello\n").as_str()),
            "the digest names the exact bytes read"
        );
        assert_eq!(candidate.bytes, 6);
        assert_eq!(candidate.base.as_deref(), Some("base-commit"));
        assert_eq!(candidate.observation, Observation::Read);
        assert!(candidate.present);
        assert!(candidate.span.is_none());
        assert!(candidate.withheld.is_none());
    }

    #[test]
    fn an_oversize_atomic_input_is_rejected_into_omitted() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("big.txt"), "x".repeat(100)).unwrap();
        let mut request = open();
        request.bounds.max_bytes_per_file = 10;
        request.read_set.insert("big.txt".to_string());
        request.reads.push(FileAsk::read("big.txt"));

        let out = Candidates::of(dir.path(), &request);

        assert!(out.candidates.is_empty(), "{:?}", out.candidates);
        assert_eq!(
            out.omitted,
            vec![Omitted {
                path: "big.txt".to_string(),
                reason: Omission::Oversize,
            }],
            "an atomic input that cannot fit is rejected, never shortened"
        );
    }

    #[test]
    fn declared_truncation_retains_the_declared_span() {
        let dir = tempfile::tempdir().unwrap();
        let text = "aaaa1111\nbbbb2222\ncccc3333\ndddd4444\neeee5555\n";
        fs::write(dir.path().join("f.txt"), text).unwrap();
        let mut request = open();
        request.bounds.max_bytes_per_file = 25;
        request.read_set.insert("f.txt".to_string());
        request
            .reads
            .push(FileAsk::read("f.txt").allowing_truncation());
        request
            .reads
            .push(FileAsk::lines("f.txt", 2, 4).allowing_truncation());

        let out = Candidates::of(dir.path(), &request);

        assert!(out.omitted.is_empty(), "{:?}", out.omitted);
        assert_eq!(out.candidates.len(), 2);
        let whole = &out.candidates[0];
        assert_eq!(whole.path, "f.txt");
        assert_eq!(whole.readness, Readness::Truncated);
        assert_eq!(
            whole.span,
            Some(Span { start: 1, end: 2 }),
            "two whole lines fit the 25-byte bound; the retained span says so"
        );
        assert_eq!(whole.bytes, 18);
        assert_eq!(
            whole.digest.as_deref(),
            Some(crate::capability::digest_bytes(b"aaaa1111\nbbbb2222\n").as_str())
        );
        let part = &out.candidates[1];
        assert_eq!(
            part.span,
            Some(Span { start: 2, end: 3 }),
            "the declared span 2..4 retained what the bound allowed"
        );
        assert_eq!(part.readness, Readness::Truncated);
        assert_eq!(
            part.digest.as_deref(),
            Some(crate::capability::digest_bytes(b"bbbb2222\ncccc3333\n").as_str())
        );
    }

    #[test]
    fn a_denied_path_yields_a_refused_entry_not_content() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("secret.txt"), b"not for you\n").unwrap();
        let mut request = open();
        request.reads.push(FileAsk::read("secret.txt"));

        let out = Candidates::of(dir.path(), &request);

        assert_eq!(out.candidates.len(), 1);
        let candidate = &out.candidates[0];
        assert_eq!(candidate.path, "secret.txt");
        assert_eq!(candidate.readness, Readness::Refused);
        assert_eq!(candidate.digest, None, "no byte was admitted");
        assert_eq!(candidate.bytes, 0);
        assert!(candidate.present, "naming is not reading");
        assert!(
            candidate
                .withheld
                .as_deref()
                .is_some_and(|why| why.contains("read set")),
            "the disclosure decision is recorded: {:?}",
            candidate.withheld
        );
        assert_eq!(
            out.omitted,
            vec![Omitted {
                path: "secret.txt".to_string(),
                reason: Omission::Denied,
            }],
            "the read input the read set excluded is an omission too"
        );
    }

    #[test]
    fn bounds_account_across_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "a".repeat(10)).unwrap();
        fs::write(dir.path().join("b.txt"), "b".repeat(10)).unwrap();
        fs::write(dir.path().join("c.txt"), "c".repeat(5)).unwrap();
        fs::write(dir.path().join("d.txt"), "d".repeat(10)).unwrap();
        let mut request = open();
        request.bounds = Bounds {
            max_files: 3,
            max_bytes_per_file: 100,
            max_total_bytes: 25,
        };
        for path in ["a.txt", "b.txt", "c.txt", "d.txt"] {
            request.read_set.insert(path.to_string());
            request.reads.push(FileAsk::read(path));
        }

        let out = Candidates::of(dir.path(), &request);

        let full: Vec<&str> = out
            .candidates
            .iter()
            .filter(|candidate| candidate.readness == Readness::Full)
            .map(|candidate| candidate.path.as_str())
            .collect();
        assert_eq!(full, ["a.txt", "b.txt", "c.txt"]);
        assert_eq!(
            out.omitted,
            vec![Omitted {
                path: "d.txt".to_string(),
                reason: Omission::OverCount,
            }],
            "the fourth read fell off the file bound after three were spent"
        );

        // The same four, against a total the last cannot reach.
        request.bounds = Bounds {
            max_files: 10,
            max_bytes_per_file: 100,
            max_total_bytes: 15,
        };
        let out = Candidates::of(dir.path(), &request);
        let full: Vec<&str> = out
            .candidates
            .iter()
            .filter(|candidate| candidate.readness == Readness::Full)
            .map(|candidate| candidate.path.as_str())
            .collect();
        assert_eq!(full, ["a.txt", "c.txt"]);
        assert_eq!(
            out.omitted,
            vec![
                Omitted {
                    path: "b.txt".to_string(),
                    reason: Omission::OverTotal,
                },
                Omitted {
                    path: "d.txt".to_string(),
                    reason: Omission::OverTotal,
                },
            ],
            "b could not fit the remaining five bytes, and nothing was left for d"
        );
    }

    #[test]
    fn an_empty_request_yields_empty_candidates() {
        let dir = tempfile::tempdir().unwrap();
        let out = Candidates::of(dir.path(), &Request::default());
        assert!(out.candidates.is_empty());
        assert!(out.omitted.is_empty());
        assert!(out.is_empty());
    }

    #[test]
    fn candidates_order_by_path() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "a\n").unwrap();
        fs::write(dir.path().join("z.txt"), "z\n").unwrap();
        fs::write(dir.path().join("m.txt"), "m\n").unwrap();
        let mut request = open();
        // Declared out of order, on purpose: the answer must not depend
        // on the order the caller listed them.
        request.exists.push("z.txt".to_string());
        request
            .references
            .push(Reference::diff("m.txt", Some(Span { start: 3, end: 5 })));
        request.exists.push("a.txt".to_string());

        let out = Candidates::of(dir.path(), &request);

        let paths: Vec<&str> = out
            .candidates
            .iter()
            .map(|candidate| candidate.path.as_str())
            .collect();
        assert_eq!(paths, ["a.txt", "m.txt", "z.txt"]);
        assert!(out.omitted.is_empty());
    }

    #[test]
    fn existence_and_references_name_without_reading() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("present.txt"), "here\n").unwrap();
        let mut request = open();
        request.exists.push("present.txt".to_string());
        request.exists.push("absent.txt".to_string());
        request.references.push(Reference::test(
            "present.txt",
            Some(Span { start: 4, end: 6 }),
        ));

        let out = Candidates::of(dir.path(), &request);

        assert_eq!(out.candidates.len(), 3);
        let absent = &out.candidates[0];
        assert_eq!(absent.path, "absent.txt");
        assert!(!absent.present, "the tree refused it, not the bounds");
        assert!(out.omitted.is_empty());
        let present = &out.candidates[1];
        assert_eq!(present.path, "present.txt");
        assert_eq!(present.observation, Observation::Exists);
        assert!(present.present);
        assert_eq!(present.readness, Readness::Refused);
        assert_eq!(present.digest, None);
        let reference = &out.candidates[2];
        assert_eq!(reference.observation, Observation::Reference);
        assert_eq!(reference.span, Some(Span { start: 4, end: 6 }));
        assert!(
            reference
                .withheld
                .as_deref()
                .is_some_and(|why| why.contains("test"))
        );
    }

    #[test]
    fn a_bounded_search_names_hits_without_opening_them() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("hit.txt"), "first\nneedle here\nlast\n").unwrap();
        fs::write(dir.path().join("miss.txt"), "nothing\n").unwrap();
        fs::write(
            dir.path().join("big.txt"),
            format!("needle\n{}", "x".repeat(64)),
        )
        .unwrap();
        let mut request = open();
        request.read_set.insert("hit.txt".to_string());
        request.searches.push(Search {
            terms: vec!["NEEDLE".to_string()],
            max_files: 10,
            max_bytes: 64,
            max_hits: 10,
        });

        let out = Candidates::of(dir.path(), &request);

        assert_eq!(out.candidates.len(), 1);
        let hit = &out.candidates[0];
        assert_eq!(hit.path, "hit.txt");
        assert_eq!(hit.observation, Observation::Search);
        assert_eq!(
            hit.span,
            Some(Span { start: 2, end: 2 }),
            "the hit names where the first match fell"
        );
        assert_eq!(
            hit.readness,
            Readness::Refused,
            "a search names a path even inside the read set; content is a read's job"
        );
        assert_eq!(hit.digest, None);
        assert_eq!(
            out.omitted,
            vec![Omitted {
                path: "big.txt".to_string(),
                reason: Omission::Oversize,
            }],
            "a file too large to scan is omitted, not partially read"
        );
    }
}
