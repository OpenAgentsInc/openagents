//! Codebase knowledge: an embedding index of this public repository's
//! documentation and doc comments at one pinned commit.
//!
//! The chat router's `codebase.kb` route answers questions about how the
//! OpenAgents software is built from this index
//! (`docs/coder/design/2026-09-28-chat-router.md`, "Codebase knowledge").
//! Two kinds of text are indexed, both read from Git at the pinned commit,
//! never from a working tree:
//!
//! - Markdown documents, chunked by heading ([`chunk_markdown`]).
//! - Rust doc comments: each `//!` module block and each `///` item block
//!   with the item line it documents ([`chunk_rust`]).
//!
//! [`included`] names what is read. Retained transcripts, benchmark
//! evidence, upstream NIP copies, the coding knowledge base (which has its
//! own index), fixtures, and vendored third-party source are left out. Only
//! this repository is indexed; no private repository ever is.
//!
//! Each chunk keeps its path and its line range, so an answer can cite
//! `path:start-end` at the commit the index names, and a reader can check
//! the citation against that commit.
//!
//! Vectors are `text-embedding-3-small`'s, reduced to [`DIMENSIONS`] by
//! keeping the leading components and normalizing again (the model was
//! trained so that this is what its `dimensions` parameter does), then
//! quantized to one signed byte per component with one scale per vector.
//! The file is gzip-compressed. [`Index::nearest`] ranks chunks by cosine
//! similarity to a query vector reduced the same way. Retrieval here is
//! embedding search only; no keyword ranking chooses what is read.
//!
//! [`build`] embeds only the chunks whose text is new since a previous
//! index, so refreshing to a newer commit re-embeds what changed.
//! `scripts/build-codebase-kb.sh` is the operator's path; read
//! `docs/coder/design/codebase-kb.md` for where the file lives and how it is
//! refreshed.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::search::Embed;

/// The index file's schema.
pub const SCHEMA: &str = "openagents.codebase-kb.v1";

/// The first bytes of an index file, before gzip.
const MAGIC: &[u8] = b"OACKB1\n";

/// Components kept of each vector.
pub const DIMENSIONS: usize = 256;

/// The most characters in one chunk, except a single line longer than
/// this, which is kept whole.
pub const CHUNK_CHARS: usize = 1_600;

/// The fewest characters of prose a doc comment block needs to be
/// indexed. Shorter blocks ("Returns the name.") answer no question the
/// item's name does not.
pub const MIN_DOC_CHARS: usize = 120;

/// The fewest characters of text under a heading for a Markdown chunk to
/// be indexed.
pub const MIN_SECTION_CHARS: usize = 40;

/// Inputs per embeddings call while building.
pub const BATCH: usize = 96;

/// Characters of a chunk that are embedded, at most.
const EMBED_CHARS: usize = 6_000;

/// Where the text of a chunk comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// A Markdown document.
    Doc,
    /// Rust doc comments.
    Code,
}

/// One indexed piece of text and where it is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chunk {
    /// The path in the repository.
    pub path: String,
    /// The first line, counted from 1.
    pub start: u32,
    /// The last line, inclusive.
    pub end: u32,
    pub source: Source,
    /// The heading path of a document chunk, or the documented item's line
    /// (or `module`) for a doc comment.
    pub title: String,
    /// The lines as they read at the commit, comment markers removed for a
    /// doc comment.
    pub text: String,
}

impl Chunk {
    /// `path:start-end`, or `path:line` for one line.
    #[must_use]
    pub fn cite(&self) -> String {
        if self.start == self.end {
            format!("{}:{}", self.path, self.start)
        } else {
            format!("{}:{}-{}", self.path, self.start, self.end)
        }
    }

    /// What is embedded: the path and title, then the text.
    #[must_use]
    pub fn embed_text(&self) -> String {
        let text: String = self.text.chars().take(EMBED_CHARS).collect();
        format!("{} — {}\n\n{text}", self.path, self.title)
    }

    /// The digest of [`Chunk::embed_text`], which keys vector reuse.
    #[must_use]
    pub fn digest(&self) -> String {
        crate::digest(self.embed_text().as_bytes())
    }
}

/// Whether `path` is read into the index.
#[must_use]
pub fn included(path: &str) -> bool {
    const LEFT_OUT: &[&str] = &[
        "docs/transcripts/",
        "bench/",
        "nips/official/",
        "knowledge/",
        "target/",
    ];
    if LEFT_OUT.iter().any(|prefix| path.starts_with(prefix)) {
        return false;
    }
    if path.contains("/fixtures/") || path.contains("/vendor/") || path.contains("/target/") {
        return false;
    }
    path.ends_with(".md") || path.ends_with(".rs")
}

/// The chunks of one file, by its extension.
#[must_use]
pub fn chunk_file(path: &str, text: &str) -> Vec<Chunk> {
    if path.ends_with(".md") {
        chunk_markdown(path, text)
    } else if path.ends_with(".rs") {
        chunk_rust(path, text)
    } else {
        Vec::new()
    }
}

/// A Markdown document's chunks: one per heading's section, split at
/// blank lines when a section is longer than [`CHUNK_CHARS`]. A `#` inside
/// a fenced block is not a heading.
#[must_use]
pub fn chunk_markdown(path: &str, text: &str) -> Vec<Chunk> {
    let lines: Vec<&str> = text.lines().collect();
    let mut sections: Vec<(Vec<String>, usize, usize)> = Vec::new();
    let mut headings: Vec<(usize, String)> = Vec::new();
    let mut fence: Option<&str> = None;
    let mut start = 0;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if let Some(marker) = fence {
            if trimmed.starts_with(marker) {
                fence = None;
            }
            continue;
        }
        if trimmed.starts_with("```") {
            fence = Some("```");
            continue;
        }
        if trimmed.starts_with("~~~") {
            fence = Some("~~~");
            continue;
        }
        if let Some(level) = heading_level(line) {
            if i > start {
                sections.push((titles(&headings), start, i - 1));
            }
            let name = line[level..]
                .trim()
                .trim_end_matches('#')
                .trim()
                .to_string();
            headings.retain(|(l, _)| *l < level);
            headings.push((level, name));
            start = i;
        }
    }
    if start < lines.len() {
        sections.push((titles(&headings), start, lines.len() - 1));
    }
    let mut chunks = Vec::new();
    for (title, first, last) in sections {
        let title = if title.is_empty() {
            path.rsplit('/').next().unwrap_or(path).to_string()
        } else {
            title.join(" > ")
        };
        for (a, b) in pieces(&lines, first, last) {
            let body: String = lines[a..=b].join("\n");
            let prose = lines[a..=b]
                .iter()
                .filter(|line| heading_level(line).is_none())
                .map(|line| line.trim().len())
                .sum::<usize>();
            if prose < MIN_SECTION_CHARS {
                continue;
            }
            chunks.push(Chunk {
                path: path.to_string(),
                start: line_number(a),
                end: line_number(b),
                source: Source::Doc,
                title: title.clone(),
                text: body,
            });
        }
    }
    chunks
}

fn titles(headings: &[(usize, String)]) -> Vec<String> {
    headings.iter().map(|(_, name)| name.clone()).collect()
}

fn line_number(index: usize) -> u32 {
    u32::try_from(index + 1).unwrap_or(u32::MAX)
}

/// The level of an ATX heading (`#` to `######` then a space), or `None`.
fn heading_level(line: &str) -> Option<usize> {
    let hashes = line.bytes().take_while(|b| *b == b'#').count();
    ((1..=6).contains(&hashes) && line[hashes..].starts_with(' ')).then_some(hashes)
}

/// Line ranges covering `first..=last`, each at most [`CHUNK_CHARS`] where
/// a blank line allows it, and a line range otherwise.
fn pieces(lines: &[&str], first: usize, last: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut a = first;
    let mut size = 0;
    let mut last_blank: Option<usize> = None;
    let mut i = first;
    while i <= last {
        size += lines[i].len() + 1;
        if lines[i].trim().is_empty() {
            last_blank = Some(i);
        }
        if size > CHUNK_CHARS && i > a {
            let cut = match last_blank {
                Some(blank) if blank > a => blank,
                _ => i - 1,
            };
            out.push((a, cut));
            a = cut + 1;
            size = lines[a..=i].iter().map(|l| l.len() + 1).sum();
            last_blank = None;
        }
        i += 1;
    }
    if a <= last {
        out.push((a, last));
    }
    out
}

/// A Rust file's doc comment chunks: each `//!` block, and each `///`
/// block with the item line it documents, when the block has at least
/// [`MIN_DOC_CHARS`] of prose.
#[must_use]
pub fn chunk_rust(path: &str, text: &str) -> Vec<Chunk> {
    let lines: Vec<&str> = text.lines().collect();
    let mut chunks = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let marker = doc_marker(lines[i]);
        let Some(marker) = marker else {
            i += 1;
            continue;
        };
        let first = i;
        let mut prose: Vec<String> = Vec::new();
        while i < lines.len() && doc_marker(lines[i]) == Some(marker) {
            let body = lines[i].trim_start()[marker.len()..].to_string();
            prose.push(body.strip_prefix(' ').map_or(body.clone(), str::to_string));
            i += 1;
        }
        let mut last = i - 1;
        let title = if marker == "//!" {
            "module".to_string()
        } else {
            // The documented item: the first line after the block that is
            // not an attribute.
            let mut j = i;
            while j < lines.len() && lines[j].trim_start().starts_with("#[") {
                j += 1;
            }
            match lines.get(j) {
                Some(line) if !line.trim().is_empty() => {
                    last = j;
                    item_line(line)
                }
                _ => "item".to_string(),
            }
        };
        if prose.iter().map(|l| l.trim().len()).sum::<usize>() < MIN_DOC_CHARS {
            continue;
        }
        let body_lines: Vec<&str> = prose.iter().map(String::as_str).collect();
        for (a, b) in pieces(&body_lines, 0, body_lines.len() - 1) {
            let mut text = body_lines[a..=b].join("\n");
            let end = if b + 1 == body_lines.len() {
                if marker == "///" && last >= first + body_lines.len() {
                    text.push('\n');
                    text.push_str(lines[last].trim());
                }
                last
            } else {
                first + b
            };
            chunks.push(Chunk {
                path: path.to_string(),
                start: line_number(first + a),
                end: line_number(end),
                source: Source::Code,
                title: title.clone(),
                text,
            });
        }
    }
    chunks
}

fn doc_marker(line: &str) -> Option<&'static str> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("//!") {
        Some("//!")
    } else if trimmed.starts_with("///") && !trimmed.starts_with("////") {
        Some("///")
    } else {
        None
    }
}

/// An item line as a title: trimmed, cut before its body.
fn item_line(line: &str) -> String {
    let line = line.trim();
    let cut = line
        .find(" {")
        .or_else(|| line.find('{'))
        .unwrap_or(line.len());
    line[..cut].trim_end_matches([' ', ';']).to_string()
}

/// A vector's leading [`DIMENSIONS`] components, normalized again.
#[must_use]
pub fn reduce(vector: &[f32]) -> Vec<f32> {
    let mut kept: Vec<f32> = vector.iter().take(DIMENSIONS).copied().collect();
    let norm = kept
        .iter()
        .map(|x| f64::from(*x).powi(2))
        .sum::<f64>()
        .sqrt();
    if norm > 0.0 {
        for x in &mut kept {
            *x = (f64::from(*x) / norm) as f32;
        }
    }
    kept
}

/// One signed byte per component and the scale that restores it.
fn quantize(vector: &[f32]) -> (Vec<i8>, f32) {
    let high = vector.iter().fold(0.0_f32, |m, x| m.max(x.abs()));
    if high == 0.0 {
        return (vec![0; vector.len()], 0.0);
    }
    let scale = high / 127.0;
    let bytes = vector
        .iter()
        .map(|x| (x / scale).round().clamp(-127.0, 127.0) as i8)
        .collect();
    (bytes, scale)
}

/// The metadata an index file carries before its vectors.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Header {
    schema: String,
    repository: String,
    commit: String,
    model: String,
    dimensions: usize,
    built: String,
    chunks: Vec<Chunk>,
    digests: Vec<String>,
    scales: Vec<f32>,
}

/// An index: chunks and their quantized vectors, at one commit.
#[derive(Clone, Debug)]
pub struct Index {
    /// The repository the chunks come from, as `owner/name`.
    pub repository: String,
    /// The full commit the chunks were read at.
    pub commit: String,
    /// The embedding model, as [`Embed::model`] names it.
    pub model: String,
    /// When it was built (a UTC date).
    pub built: String,
    pub chunks: Vec<Chunk>,
    digests: Vec<String>,
    vectors: Vec<i8>,
    scales: Vec<f32>,
}

impl Index {
    /// An index of `chunks` with their full-length `vectors`, in order.
    ///
    /// # Errors
    ///
    /// The counts differ.
    pub fn new(
        repository: &str,
        commit: &str,
        model: &str,
        built: &str,
        chunks: Vec<Chunk>,
        vectors: &[Vec<f32>],
    ) -> Result<Self, String> {
        if chunks.len() != vectors.len() {
            return Err(format!(
                "{} vectors for {} chunks",
                vectors.len(),
                chunks.len()
            ));
        }
        let mut index = Index {
            repository: repository.to_string(),
            commit: commit.to_string(),
            model: model.to_string(),
            built: built.to_string(),
            digests: chunks.iter().map(Chunk::digest).collect(),
            chunks,
            vectors: Vec::with_capacity(vectors.len() * DIMENSIONS),
            scales: Vec::with_capacity(vectors.len()),
        };
        for vector in vectors {
            let reduced = reduce(vector);
            if reduced.len() != DIMENSIONS {
                return Err(format!(
                    "a vector has {} dimensions; the index keeps {DIMENSIONS}",
                    vector.len()
                ));
            }
            let (bytes, scale) = quantize(&reduced);
            index.vectors.extend(bytes);
            index.scales.push(scale);
        }
        Ok(index)
    }

    /// The short commit an answer names.
    #[must_use]
    pub fn short_commit(&self) -> &str {
        &self.commit[..self.commit.len().min(10)]
    }

    /// Chunk `i`'s vector, restored to floats.
    #[must_use]
    pub fn vector(&self, i: usize) -> Vec<f32> {
        let scale = self.scales[i];
        self.vectors[i * DIMENSIONS..(i + 1) * DIMENSIONS]
            .iter()
            .map(|b| f32::from(*b) * scale)
            .collect()
    }

    /// The `limit` chunks nearest `query` (a full-length or reduced
    /// vector from [`Index::model`]) by cosine similarity, best first, at
    /// most `per_path` from any one file.
    #[must_use]
    pub fn nearest(&self, query: &[f32], limit: usize, per_path: usize) -> Vec<(usize, f64)> {
        let query = reduce(query);
        let mut scored: Vec<(usize, f64)> = (0..self.chunks.len())
            .map(|i| (i, crate::search::cosine(&self.vector(i), &query)))
            .collect();
        scored.sort_by(|a, b| b.1.total_cmp(&a.1));
        let mut taken: HashMap<&str, usize> = HashMap::new();
        let mut out = Vec::new();
        for (i, score) in scored {
            let count = taken.entry(self.chunks[i].path.as_str()).or_default();
            if *count >= per_path.max(1) {
                continue;
            }
            *count += 1;
            out.push((i, score));
            if out.len() == limit {
                break;
            }
        }
        out
    }

    /// Writes the index, gzip-compressed; returns the bytes written.
    ///
    /// # Errors
    ///
    /// The file cannot be written.
    pub fn write(&self, path: &Path) -> Result<u64, String> {
        let header = Header {
            schema: SCHEMA.to_string(),
            repository: self.repository.clone(),
            commit: self.commit.clone(),
            model: self.model.clone(),
            dimensions: DIMENSIONS,
            built: self.built.clone(),
            chunks: self.chunks.clone(),
            digests: self.digests.clone(),
            scales: self.scales.clone(),
        };
        let json = serde_json::to_vec(&header).map_err(|e| e.to_string())?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let partial = path.with_extension("partial");
        let file =
            std::fs::File::create(&partial).map_err(|e| format!("{}: {e}", partial.display()))?;
        let mut gz = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let bytes: Vec<u8> = self.vectors.iter().map(|b| b.to_ne_bytes()[0]).collect();
        gz.write_all(MAGIC)
            .and_then(|()| gz.write_all(&(json.len() as u64).to_le_bytes()))
            .and_then(|()| gz.write_all(&json))
            .and_then(|()| gz.write_all(&bytes))
            .and_then(|()| gz.finish().map(|_| ()))
            .map_err(|e| format!("{}: {e}", partial.display()))?;
        std::fs::rename(&partial, path).map_err(|e| format!("{}: {e}", path.display()))?;
        std::fs::metadata(path)
            .map(|m| m.len())
            .map_err(|e| e.to_string())
    }

    /// Reads an index written by [`Index::write`].
    ///
    /// # Errors
    ///
    /// The file is missing, not an index, another schema, or truncated.
    pub fn read(path: &Path) -> Result<Self, String> {
        let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(file)
            .read_to_end(&mut raw)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let not_index = || format!("{} is not a codebase index", path.display());
        let rest = raw.strip_prefix(MAGIC).ok_or_else(not_index)?;
        let (length, rest) = rest.split_at_checked(8).ok_or_else(not_index)?;
        let length = usize::try_from(u64::from_le_bytes(
            length.try_into().map_err(|_| not_index())?,
        ))
        .map_err(|_| not_index())?;
        let (json, vectors) = rest.split_at_checked(length).ok_or_else(not_index)?;
        let header: Header = serde_json::from_slice(json).map_err(|e| e.to_string())?;
        if header.schema != SCHEMA {
            return Err(format!(
                "{} is {}, not {SCHEMA}",
                path.display(),
                header.schema
            ));
        }
        let n = header.chunks.len();
        if header.dimensions != DIMENSIONS
            || vectors.len() != n * DIMENSIONS
            || header.scales.len() != n
            || header.digests.len() != n
        {
            return Err(format!("{} is truncated or inconsistent", path.display()));
        }
        Ok(Index {
            repository: header.repository,
            commit: header.commit,
            model: header.model,
            built: header.built,
            chunks: header.chunks,
            digests: header.digests,
            vectors: vectors.iter().map(|b| i8::from_ne_bytes([*b])).collect(),
            scales: header.scales,
        })
    }

    /// Reduced vectors by chunk digest, for reuse by a later build.
    fn reusable(&self) -> HashMap<&str, usize> {
        self.digests
            .iter()
            .enumerate()
            .map(|(i, d)| (d.as_str(), i))
            .collect()
    }
}

/// What a build did.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Built {
    pub files: usize,
    pub chunks: usize,
    pub doc_chunks: usize,
    pub code_chunks: usize,
    /// Chunks whose vectors came from the previous index.
    pub reused: usize,
    /// Chunks embedded by this build.
    pub embedded: usize,
    /// Dollars the embeddings cost, when known.
    pub usd: Option<f64>,
}

/// Builds an index of `files` (path and text, read at `commit`), embedding
/// with `embedder` every chunk `previous` does not already hold for the
/// same model.
///
/// # Errors
///
/// An embeddings call fails or returns the wrong count.
pub async fn build<E: Embed>(
    repository: &str,
    commit: &str,
    built: &str,
    files: &[(String, String)],
    embedder: &E,
    previous: Option<&Index>,
    mut progress: impl FnMut(usize, usize),
) -> Result<(Index, Built), String> {
    let chunks: Vec<Chunk> = files
        .iter()
        .filter(|(path, _)| included(path))
        .flat_map(|(path, text)| chunk_file(path, text))
        .collect();
    let previous = previous.filter(|p| p.model == embedder.model());
    let reusable = previous.map(Index::reusable).unwrap_or_default();
    let mut vectors: Vec<Option<Vec<f32>>> = chunks
        .iter()
        .map(|chunk| {
            let digest = chunk.digest();
            reusable
                .get(digest.as_str())
                .and_then(|i| previous.map(|p| p.vector(*i)))
        })
        .collect();
    let missing: Vec<usize> = (0..chunks.len())
        .filter(|i| vectors[*i].is_none())
        .collect();
    let mut report = Built {
        files: files.iter().filter(|(p, _)| included(p)).count(),
        chunks: chunks.len(),
        doc_chunks: chunks.iter().filter(|c| c.source == Source::Doc).count(),
        code_chunks: chunks.iter().filter(|c| c.source == Source::Code).count(),
        reused: chunks.len() - missing.len(),
        embedded: 0,
        usd: Some(0.0),
    };
    for batch in missing.chunks(BATCH) {
        let inputs = batch.iter().map(|i| chunks[*i].embed_text()).collect();
        let (got, usd) = embedder
            .embed(inputs)
            .await
            .map_err(|e| format!("embedding chunks: {e}"))?;
        if got.len() != batch.len() {
            return Err(format!(
                "{} vectors came back for {} chunks",
                got.len(),
                batch.len()
            ));
        }
        for (i, vector) in batch.iter().zip(got) {
            vectors[*i] = Some(vector);
        }
        report.embedded += batch.len();
        report.usd = match (report.usd, usd) {
            (Some(a), Some(b)) => Some(a + b),
            _ => None,
        };
        progress(report.embedded, missing.len());
    }
    let vectors: Vec<Vec<f32>> = vectors.into_iter().map(Option::unwrap_or_default).collect();
    let index = Index::new(
        repository,
        commit,
        embedder.model(),
        built,
        chunks,
        &vectors,
    )?;
    Ok((index, report))
}

/// Every included file's path and text at `commit` in the Git repository
/// at `repo`, with the full commit. Reads Git objects, never the working
/// tree, so uncommitted changes are never indexed.
///
/// # Errors
///
/// Git fails or the commit does not exist.
pub fn read_commit(repo: &Path, commit: &str) -> Result<(String, Vec<(String, String)>), String> {
    let git = |args: &[&str]| -> Result<String, String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .map_err(|e| format!("git: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "git {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    };
    let full = git(&["rev-parse", "--verify", &format!("{commit}^{{commit}}")])?
        .trim()
        .to_string();
    let listing = git(&["ls-tree", "-r", "--name-only", &full])?;
    let paths: Vec<String> = listing
        .lines()
        .filter(|p| included(p))
        .map(str::to_string)
        .collect();
    let mut child = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("git cat-file: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("no stdin for git cat-file")?;
    let requests: String = paths.iter().map(|p| format!("{full}:{p}\n")).collect();
    let writer = std::thread::spawn(move || stdin.write_all(requests.as_bytes()));
    let mut reader = BufReader::new(child.stdout.take().ok_or("no stdout for git cat-file")?);
    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let mut header = String::new();
        reader
            .read_line(&mut header)
            .map_err(|e| format!("git cat-file: {e}"))?;
        let size: usize = header
            .split_whitespace()
            .nth(2)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| format!("git cat-file: {path}: {}", header.trim()))?;
        let mut body = vec![0; size + 1];
        reader
            .read_exact(&mut body)
            .map_err(|e| format!("git cat-file: {e}"))?;
        body.pop();
        if let Ok(text) = String::from_utf8(body) {
            files.push((path, text));
        }
    }
    writer
        .join()
        .map_err(|_| "the git cat-file writer panicked")?
        .map_err(|e| format!("git cat-file: {e}"))?;
    child.wait().map_err(|e| format!("git cat-file: {e}"))?;
    Ok((full, files))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_first_party_docs_and_sources_are_read() {
        assert!(included("docs/coder/design/2026-09-28-chat-router.md"));
        assert!(included("crates/coder/src/first.rs"));
        assert!(included("INVARIANTS.md"));
        for left_out in [
            "docs/transcripts/2026-01-01.md",
            "bench/terminal-bench/results/x.md",
            "nips/official/01.md",
            "knowledge/methods/x.md",
            "crates/rust-native/fixtures/shaping-corpus.md",
            "crates/verse-ruins/vendor/crates/core_units/src/lib.rs",
            "Cargo.toml",
            "scripts/build.sh",
        ] {
            assert!(!included(left_out), "{left_out}");
        }
    }

    #[test]
    fn markdown_is_chunked_by_heading_with_lines_and_titles() {
        let text = "# Title\n\nIntro paragraph that says what the document is about.\n\n\
                    ## Quota\n\nThe worker admits six jobs a minute and forty a day per key.\n\n\
                    ```sh\n# not a heading\n```\n\n### Global\n\nA global day total bounds \
                    every key together.\n";
        let chunks = chunk_markdown("docs/x.md", text);
        let found: Vec<(&str, u32, u32)> = chunks
            .iter()
            .map(|c| (c.title.as_str(), c.start, c.end))
            .collect();
        assert_eq!(
            found,
            [
                ("Title", 1, 4),
                ("Title > Quota", 5, 12),
                ("Title > Quota > Global", 13, 15)
            ]
        );
        assert!(chunks[1].text.contains("# not a heading"));
        assert_eq!(chunks[1].cite(), "docs/x.md:5-12");
    }

    #[test]
    fn a_long_section_splits_at_blank_lines_and_keeps_every_line() {
        let paragraph = "word ".repeat(100);
        let text = format!("# Long\n\n{p}\n\n{p}\n\n{p}\n\n{p}\n", p = paragraph.trim());
        let chunks = chunk_markdown("docs/long.md", &text);
        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|c| c.text.len() <= CHUNK_CHARS + 1));
        assert_eq!(chunks[0].start, 1);
        for pair in chunks.windows(2) {
            assert_eq!(pair[1].start, pair[0].end + 1);
        }
    }

    #[test]
    fn doc_comments_are_chunked_with_the_item_they_document() {
        let text = "//! The chat worker's quota: six jobs a minute and forty a day per key,\n\
                    //! with a global day total over every key, checked before any model call.\n\
                    \n\
                    /// How many jobs one key may send in a day before the worker refuses it\n\
                    /// with `rate_limited`, counted from the first job of the UTC day.\n\
                    #[derive(Clone)]\n\
                    pub const DAY: u32 = 40;\n\
                    \n\
                    /// Short.\n\
                    fn short() {}\n";
        let chunks = chunk_rust("crates/coder/src/relay/quota.rs", text);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].title, "module");
        assert_eq!((chunks[0].start, chunks[0].end), (1, 2));
        assert_eq!(chunks[1].title, "pub const DAY: u32 = 40");
        assert_eq!((chunks[1].start, chunks[1].end), (4, 7));
        assert!(chunks[1].text.starts_with("How many jobs"));
        assert!(chunks[1].text.ends_with("pub const DAY: u32 = 40;"));
    }

    #[test]
    fn reduced_vectors_are_unit_length_and_quantization_keeps_the_ranking() {
        let a: Vec<f32> = (0..1536).map(|i| ((i * 7) % 13) as f32 - 6.0).collect();
        let reduced = reduce(&a);
        assert_eq!(reduced.len(), DIMENSIONS);
        let norm: f32 = reduced.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4);
        let b: Vec<f32> = a.iter().map(|x| -x).collect();
        let chunk = |path: &str| Chunk {
            path: path.to_string(),
            start: 1,
            end: 1,
            source: Source::Doc,
            title: "t".to_string(),
            text: path.to_string(),
        };
        let index = Index::new(
            "OpenAgentsInc/openagents",
            "abc",
            "m",
            "2026-09-28",
            vec![chunk("a.md"), chunk("b.md")],
            &[a.clone(), b],
        )
        .unwrap();
        let near = index.nearest(&a, 2, 1);
        assert_eq!(near[0].0, 0);
        assert!(near[0].1 > 0.99);
        assert!(near[1].1 < -0.99);
    }

    #[test]
    fn an_index_round_trips_through_its_file() {
        let chunk = Chunk {
            path: "docs/x.md".to_string(),
            start: 3,
            end: 9,
            source: Source::Doc,
            title: "X".to_string(),
            text: "The text.".to_string(),
        };
        let vector: Vec<f32> = (0..300).map(|i| (i as f32).sin()).collect();
        let index = Index::new(
            "OpenAgentsInc/openagents",
            "0123456789abcdef",
            "openai/text-embedding-3-small",
            "2026-09-28",
            vec![chunk],
            &[vector],
        )
        .unwrap();
        let dir = std::env::temp_dir().join(format!("codebase-kb-{}", std::process::id()));
        let path = dir.join("index.kb");
        index.write(&path).unwrap();
        let read = Index::read(&path).unwrap();
        assert_eq!(read.chunks, index.chunks);
        assert_eq!(read.commit, "0123456789abcdef");
        assert_eq!(read.short_commit(), "0123456789");
        assert_eq!(read.vector(0), index.vector(0));
        std::fs::write(&path, b"not gzip").unwrap();
        assert!(Index::read(&path).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    struct Fake;

    impl Embed for Fake {
        fn model(&self) -> &str {
            "fake"
        }

        async fn embed(
            &self,
            inputs: Vec<String>,
        ) -> Result<(Vec<Vec<f32>>, Option<f64>), crate::search::EmbedError> {
            Ok((
                inputs
                    .iter()
                    .map(|t| {
                        (0..DIMENSIONS)
                            .map(|i| (t.len() * (i + 1) % 17) as f32)
                            .collect()
                    })
                    .collect(),
                Some(0.0),
            ))
        }
    }

    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(future)
    }

    #[test]
    fn a_rebuild_embeds_only_changed_chunks() {
        let files = vec![
            (
                "docs/a.md".to_string(),
                "# A\n\nThe first document says something long enough to index.\n".to_string(),
            ),
            (
                "docs/b.md".to_string(),
                "# B\n\nThe second document says something long enough to index.\n".to_string(),
            ),
        ];
        let (first, report) =
            block_on(build("r", "c1", "d", &files, &Fake, None, |_, _| {})).unwrap();
        assert_eq!((report.embedded, report.reused), (2, 0));
        let mut changed = files.clone();
        changed[1].1 = "# B\n\nThe second document now says something else entirely.\n".to_string();
        let (_, report) = block_on(build(
            "r",
            "c2",
            "d",
            &changed,
            &Fake,
            Some(&first),
            |_, _| {},
        ))
        .unwrap();
        assert_eq!((report.embedded, report.reused), (1, 1));
    }
}
