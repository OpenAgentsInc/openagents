//! Read-only, offline issue briefings from a bounded Git snapshot.
pub mod syntax;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Instant;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const SCHEMA: &str = "openagents.briefing-lab.index.v1";
const MAX_FILE: usize = 512 * 1024;
const MAX_BYTES: usize = 64 * 1024 * 1024;
const MAX_FILES: usize = 10_000;

#[derive(Debug, Serialize, Deserialize)]
pub struct Index {
    pub schema: String,
    pub commit: String,
    pub files: Vec<Source>,
    pub known_paths: Vec<String>,
    pub history: Vec<Commit>,
    pub omissions: BTreeMap<String, usize>,
    pub index_ms: f64,
    pub scanned_bytes: usize,
    pub attempted_files: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syntax: Option<syntax::Provenance>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Source {
    pub path: String,
    pub blob: String,
    pub sha256: String,
    pub size: usize,
    pub line_count: usize,
    pub terms: BTreeSet<String>,
    pub symbols: Vec<Symbol>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syntax: Option<syntax::FileSyntax>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub line: usize,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Commit {
    pub oid: String,
    pub subject: String,
    pub terms: BTreeSet<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub title: String,
    #[serde(default, deserialize_with = "nullable_body")]
    pub body: String,
    #[serde(default)]
    pub number: Option<u64>,
    #[serde(default)]
    pub url: Option<String>,
}
fn nullable_body<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<String, D::Error> {
    Ok(Option::<String>::deserialize(d)?.unwrap_or_default())
}
#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    pub syntax: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Components {
    pub lexical: bool,
    pub symbols: bool,
    pub history: bool,
}
impl Default for Components {
    fn default() -> Self {
        Self {
            lexical: true,
            symbols: true,
            history: true,
        }
    }
}
#[derive(Debug, Serialize)]
pub struct Evidence {
    pub path: String,
    pub blob: String,
    pub file_sha256: String,
    pub excerpt_sha256: String,
    pub start_line: usize,
    pub end_line: usize,
    pub total_lines: usize,
    pub reasons: Vec<String>,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub syntax_selection: Option<syntax::Selection>,
}
#[derive(Debug, Serialize)]
pub struct Brief {
    pub schema: String,
    pub commit: String,
    pub issue: Issue,
    pub components: Components,
    pub evidence: Vec<Evidence>,
    pub history: Vec<Commit>,
    pub notes: Vec<String>,
    pub candidate_files: usize,
    pub omitted_candidates: usize,
    pub index_omissions: BTreeMap<String, usize>,
    pub timings_ms: BTreeMap<String, f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub syntax: Option<syntax::Provenance>,
}

pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn git(repo: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let out = Command::new("git")
        .arg("--literal-pathspecs")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()?;
    if !out.status.success() {
        return Err(format!(
            "Git read failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )
        .into());
    }
    Ok(out.stdout)
}
pub fn resolve(repo: &Path, revision: &str) -> Result<String> {
    if revision.is_empty() || revision.starts_with('-') || revision.chars().any(char::is_control) {
        return Err("The revision must be a Git commit or ref, without command options.".into());
    }
    let revision = format!("{revision}^{{commit}}");
    Ok(
        String::from_utf8(git(repo, &["rev-parse", "--verify", &revision])?)?
            .trim()
            .into(),
    )
}
/// Require an output directory outside the inspected Git working tree.
pub fn check_output(repo: &Path, output: &Path) -> Result<()> {
    let top = String::from_utf8(git(repo, &["rev-parse", "--show-toplevel"])?)?;
    let top = fs::canonicalize(top.trim())?;
    if output
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("Output paths must not contain parent-directory traversal.".into());
    }
    let mut ancestor = if output.is_absolute() {
        output.to_path_buf()
    } else {
        std::env::current_dir()?.join(output)
    };
    while fs::symlink_metadata(&ancestor).is_err() {
        ancestor = ancestor
            .parent()
            .ok_or("Cannot resolve output directory.")?
            .to_path_buf();
    }
    if fs::canonicalize(ancestor)?.starts_with(top) {
        return Err("Write artifacts outside the inspected repository.".into());
    }
    Ok(())
}
fn safe_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.chars().any(char::is_control)
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != ".." && part != ".")
}
fn allowed(path: &str) -> bool {
    if path.split('/').any(|p| {
        matches!(
            p,
            "vendor"
                | "node_modules"
                | "target"
                | "traces"
                | "transcripts"
                | "published"
                | "fixtures"
        )
    }) || path.ends_with(".lock")
        || path.ends_with(".generated.rs")
    {
        return false;
    }
    matches!(
        Path::new(path).extension().and_then(|v| v.to_str()),
        Some("rs" | "toml" | "md" | "sh" | "py" | "nix")
    )
}
pub fn terms(text: &str) -> BTreeSet<String> {
    let mut separated = String::new();
    let mut lower = false;
    for c in text.chars() {
        if c.is_uppercase() && lower {
            separated.push(' ');
        }
        separated.extend(c.to_lowercase());
        lower = c.is_lowercase();
    }
    separated
        .split(|c: char| !c.is_alphanumeric())
        .filter(|v| {
            v.len() >= 3
                && !matches!(
                    *v,
                    "the"
                        | "and"
                        | "for"
                        | "with"
                        | "from"
                        | "this"
                        | "that"
                        | "should"
                        | "when"
                        | "have"
                        | "into"
                        | "pub"
                        | "let"
                        | "self"
                        | "use"
                        | "not"
                        | "are"
                        | "issue"
                        | "then"
                        | "can"
                        | "all"
                        | "will"
                        | "but"
                        | "does"
                        | "after"
                        | "before"
                )
        })
        .map(str::to_owned)
        .collect()
}
fn symbols(text: &str) -> Vec<Symbol> {
    text.lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let words: Vec<_> = line.trim().split_whitespace().collect();
            for (j, word) in words.iter().enumerate() {
                if matches!(
                    *word,
                    "fn" | "struct" | "enum" | "trait" | "mod" | "type" | "const" | "static"
                ) {
                    let name = words
                        .get(j + 1)?
                        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                        .next()?;
                    if !name.is_empty() {
                        return Some(Symbol {
                            name: name.into(),
                            line: i + 1,
                        });
                    }
                }
            }
            None
        })
        .take(512)
        .collect()
}

pub fn build_index(repo: &Path, revision: &str) -> Result<Index> {
    build_index_with_options(repo, revision, Options::default())
}

pub fn build_index_with_options(repo: &Path, revision: &str, options: Options) -> Result<Index> {
    let start = Instant::now();
    let commit = resolve(repo, revision)?;
    let listing = git(repo, &["ls-tree", "-r", "-l", "-z", &commit])?;
    let mut candidates = Vec::new();
    let mut known_paths = Vec::new();
    let mut omissions = BTreeMap::<String, usize>::new();
    for record in listing.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let Some(tab) = record.iter().position(|b| *b == b'\t') else {
            return Err("Invalid Git tree entry.".into());
        };
        let Ok(path) = std::str::from_utf8(&record[tab + 1..]) else {
            *omissions.entry("non-UTF-8 paths".into()).or_default() += 1;
            continue;
        };
        if !safe_path(path) {
            *omissions.entry("unsafe paths".into()).or_default() += 1;
            continue;
        }
        known_paths.push(path.to_string());
        let fields: Vec<_> = std::str::from_utf8(&record[..tab])?
            .split_whitespace()
            .collect();
        if fields.len() != 4 {
            return Err("Invalid Git tree metadata.".into());
        }
        let size = fields[3].parse::<usize>().unwrap_or(usize::MAX);
        let reason = if !matches!(fields[0], "100644" | "100755") {
            Some("symlinks or submodules")
        } else if !allowed(path) {
            Some("excluded generated, archive, or unsupported files")
        } else if size > MAX_FILE {
            Some("files larger than 512 KiB")
        } else {
            None
        };
        if let Some(reason) = reason {
            *omissions.entry(reason.into()).or_default() += 1;
            continue;
        }
        candidates.push((path.to_string(), fields[2].to_string(), size));
    }
    // Root instructions and manifests precede source; all ties use path order.
    candidates.sort_by_key(|(p, _, _)| {
        let priority = if matches!(p.as_str(), "AGENTS.md" | "Cargo.toml" | "README.md") {
            0
        } else if p.ends_with("/AGENTS.md") || p.ends_with("/Cargo.toml") {
            1
        } else if p.ends_with(".rs") {
            2
        } else {
            3
        };
        (priority, p.clone())
    });
    let mut child = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;
    let mut input = child.stdin.take().ok_or("Git input is unavailable.")?;
    let mut output = BufReader::new(child.stdout.take().ok_or("Git output is unavailable.")?);
    let mut files = Vec::new();
    let mut bytes = 0;
    let mut attempted_files = 0;
    for (path, blob, size) in candidates {
        if attempted_files >= MAX_FILES || bytes + size > MAX_BYTES {
            *omissions
                .entry("index byte or file budget".into())
                .or_default() += 1;
            continue;
        }
        attempted_files += 1;
        bytes += size;
        writeln!(input, "{blob}")?;
        input.flush()?;
        let mut header = String::new();
        output.read_line(&mut header)?;
        if header.trim() != format!("{blob} blob {size}") {
            return Err("Git returned unexpected blob metadata.".into());
        }
        let mut raw = vec![0; size];
        output.read_exact(&mut raw)?;
        let mut delimiter = [0];
        output.read_exact(&mut delimiter)?;
        if delimiter != [b'\n'] {
            return Err("Invalid Git blob separator.".into());
        }
        let hash = sha256(&raw);
        let Ok(text) = String::from_utf8(raw) else {
            *omissions.entry("non-UTF-8 contents".into()).or_default() += 1;
            continue;
        };
        if text.is_empty() {
            *omissions.entry("empty files".into()).or_default() += 1;
            continue;
        }
        if text.contains('\0') {
            *omissions.entry("binary contents".into()).or_default() += 1;
            continue;
        }
        let syntax = if options.syntax && path.ends_with(".rs") {
            Some(syntax::extract(&text)?)
        } else {
            None
        };
        files.push(Source {
            terms: terms(&format!("{path}\n{text}")),
            symbols: if path.ends_with(".rs") {
                symbols(&text)
            } else {
                vec![]
            },
            path,
            blob,
            sha256: hash,
            size,
            line_count: text.lines().count(),
            syntax,
        });
    }
    drop(input);
    drop(output);
    if !child.wait()?.success() {
        return Err("Git blob reader failed.".into());
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let log = git(repo, &["log", "-32", "--format=%H%x09%s", &commit])?;
    let history = String::from_utf8(log)?
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .map(|(oid, subject)| Commit {
            oid: oid.into(),
            subject: subject.into(),
            terms: terms(subject),
        })
        .collect();
    Ok(Index {
        schema: SCHEMA.into(),
        commit,
        files,
        known_paths,
        history,
        omissions,
        index_ms: start.elapsed().as_secs_f64() * 1000.0,
        scanned_bytes: bytes,
        attempted_files,
        syntax: options.syntax.then(syntax::Provenance::default),
    })
}

fn references(issue: &Issue) -> BTreeSet<String> {
    format!("{}\n{}", issue.title, issue.body)
        .split(|c: char| {
            c.is_whitespace() || matches!(c, '`' | '"' | '\'' | '(' | ')' | '[' | ']' | '<' | '>')
        })
        .filter_map(|token| {
            let token = token.trim_end_matches(|c| matches!(c, ',' | ';' | '.'));
            let path = token.split([':', '#']).next().unwrap_or(token);
            let extension = Path::new(path).extension().and_then(|e| e.to_str());
            if safe_path(path)
                && !token.contains("://")
                && matches!(
                    extension,
                    Some("rs" | "toml" | "md" | "json" | "sh" | "py" | "nix" | "lock")
                )
            {
                Some(path.to_owned())
            } else {
                None
            }
        })
        .collect()
}

fn selected_text(repo: &Path, commit: &str, sources: &[&Source]) -> Result<Vec<String>> {
    if sources.is_empty() {
        return Ok(vec![]);
    }
    let mut args = vec!["ls-tree", "-r", "-z", commit, "--"];
    args.extend(sources.iter().map(|s| s.path.as_str()));
    let tree = git(repo, &args)?;
    let mut bindings = BTreeMap::new();
    for entry in tree.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let tab = entry
            .iter()
            .position(|b| *b == b'\t')
            .ok_or("Invalid selected Git path.")?;
        let fields: Vec<_> = std::str::from_utf8(&entry[..tab])?
            .split_whitespace()
            .collect();
        if fields.len() != 3 || fields[1] != "blob" || !matches!(fields[0], "100644" | "100755") {
            return Err("Selected Git entry is not a regular file.".into());
        }
        bindings.insert(
            std::str::from_utf8(&entry[tab + 1..])?.to_owned(),
            fields[2].to_owned(),
        );
    }
    for source in sources {
        if bindings.get(&source.path) != Some(&source.blob) {
            return Err(
                "Cached path or blob does not match the pinned Git commit; rebuild the index."
                    .into(),
            );
        }
    }
    let mut child = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;
    let mut input = child.stdin.take().ok_or("Git input is unavailable.")?;
    let mut output = BufReader::new(child.stdout.take().ok_or("Git output is unavailable.")?);
    let mut texts = Vec::new();
    for source in sources {
        writeln!(input, "{}", source.blob)?;
        input.flush()?;
        let mut header = String::new();
        output.read_line(&mut header)?;
        if header.trim() != format!("{} blob {}", source.blob, source.size) {
            return Err("Cached Git blob size is invalid; rebuild the index.".into());
        }
        let mut bytes = vec![0; source.size];
        output.read_exact(&mut bytes)?;
        let mut separator = [0];
        output.read_exact(&mut separator)?;
        if separator != [b'\n'] || sha256(&bytes) != source.sha256 {
            return Err("Selected source digest differs from the index; rebuild it.".into());
        }
        let text = String::from_utf8(bytes)?;
        if text.lines().count() != source.line_count {
            return Err("Cached source line count is invalid; rebuild the index.".into());
        }
        texts.push(text);
    }
    drop(input);
    drop(output);
    if !child.wait()?.success() {
        return Err("Git blob reader failed.".into());
    }
    Ok(texts)
}

pub fn assemble(
    repo: &Path,
    index: &Index,
    expected_commit: &str,
    issue: Issue,
    components: Components,
) -> Result<Brief> {
    assemble_with_options(
        repo,
        index,
        expected_commit,
        issue,
        components,
        Options::default(),
    )
}

pub fn assemble_with_options(
    repo: &Path,
    index: &Index,
    expected_commit: &str,
    issue: Issue,
    components: Components,
    options: Options,
) -> Result<Brief> {
    let start = Instant::now();
    if index.schema != SCHEMA {
        return Err("Unsupported index schema; rebuild the index.".into());
    }
    if index.commit != expected_commit {
        return Err("The index is stale for the requested commit; rebuild it or select its pinned revision.".into());
    }
    if index.files.len() > MAX_FILES
        || index.attempted_files > MAX_FILES
        || index.scanned_bytes > MAX_BYTES
    {
        return Err("Cached index exceeds the file or byte bounds; rebuild it.".into());
    }
    if options.syntax && index.syntax.as_ref() != Some(&syntax::Provenance::default()) {
        return Err("Syntax preview needs a compatible syntax index. Rebuild with `briefing-lab index --syntax`.".into());
    }
    let mut paths = BTreeSet::new();
    let mut bytes = 0usize;
    for source in &index.files {
        bytes = bytes
            .checked_add(source.size)
            .ok_or("Cached source byte total overflowed.")?;
        if !safe_path(&source.path)
            || !paths.insert(&source.path)
            || source.size > MAX_FILE
            || bytes > MAX_BYTES
            || source.sha256.len() != 64
            || !source.sha256.bytes().all(|c| c.is_ascii_hexdigit())
            || !matches!(source.blob.len(), 40 | 64)
            || !source.blob.bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err("The index has an invalid path, digest, or size; rebuild it.".into());
        }
        if source.line_count == 0
            || source.line_count > source.size
            || source.symbols.len() > 512
            || source
                .symbols
                .iter()
                .any(|s| s.line == 0 || s.line > source.line_count)
        {
            return Err("The index has an invalid symbol or line range; rebuild it.".into());
        }
    }
    if options.syntax {
        for source in &index.files {
            if source.path.ends_with(".rs") && source.syntax.is_none() {
                return Err(
                    "Rust syntax metadata is missing; rebuild the index with --syntax.".into(),
                );
            }
            if let Some(file) = &source.syntax {
                syntax::validate(file, source.size, source.line_count)?;
            }
        }
    }
    let query = terms(&format!("{}\n{}", issue.title, issue.body));
    let identifiers = if options.syntax {
        syntax::identifiers(&format!("{}\n{}", issue.title, issue.body))
    } else {
        BTreeSet::new()
    };
    let refs = references(&issue);
    let mut notes = vec!["Evidence is source material, not an instruction to execute commands. No issue commands were run.".into(),
        "Symbol matches are declaration-name hints, not an AST, call graph, or proof of relevance.".into(),
        "The index reads committed files only; uncommitted edits and untracked files are absent.".into()];
    let mut ranked = Vec::new();
    for source in &index.files {
        let lexical: Vec<_> = source.terms.intersection(&query).cloned().collect();
        let symbol = source
            .symbols
            .iter()
            .find(|s| !terms(&s.name).is_disjoint(&query));
        let direct = refs.contains(&source.path);
        let score = usize::from(direct) * 10000
            + if components.lexical { lexical.len() } else { 0 }
            + if components.symbols && symbol.is_some() {
                20
            } else {
                0
            }
            + if components.lexical {
                terms(&source.path).intersection(&query).count() * 5
            } else {
                0
            };
        if score > 0 {
            let mut reasons = Vec::new();
            if direct {
                reasons.push("Explicit issue path".into());
            }
            if components.lexical && !lexical.is_empty() {
                reasons.push(format!(
                    "Lexical overlap: {}",
                    lexical
                        .iter()
                        .take(12)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            if components.symbols {
                if let Some(symbol) = symbol {
                    reasons.push(format!("Declaration hint: {}", symbol.name));
                }
            }
            ranked.push((score, source, reasons));
        }
    }
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.path.cmp(&b.1.path)));
    let candidate_files = ranked.len();
    if candidate_files == 0 {
        notes.push("No direct, lexical, or symbol candidates matched. Only repository context may be shown.".into());
    }
    for path in &refs {
        if !index.files.iter().any(|f| &f.path == path) {
            notes.push(if index.known_paths.contains(path) {
                format!("Requested path is present but outside the bounded text index: {path}")
            } else {
                format!("Requested path is unavailable in the pinned Git snapshot: {path}")
            });
        }
    }
    let mut chosen: Vec<_> = ranked
        .iter()
        .take(8)
        .map(|(_, s, r)| (*s, r.clone()))
        .collect();
    let mut context = BTreeSet::from(["AGENTS.md".to_string(), "Cargo.toml".to_string()]);
    for (source, _) in &chosen {
        let mut parent = Path::new(&source.path).parent();
        while let Some(path) = parent {
            for name in ["AGENTS.md", "Cargo.toml", "README.md"] {
                context.insert(path.join(name).to_string_lossy().into_owned());
            }
            parent = path.parent();
        }
    }
    for path in context {
        if chosen.len() >= 14 {
            break;
        }
        if let Some(source) = index.files.iter().find(|f| f.path == path) {
            if !chosen.iter().any(|(s, _)| s.path == path) {
                chosen.push((
                    source,
                    vec!["Ancestor instructions, manifest, or crate guide".into()],
                ));
            }
        }
    }
    let read_start = Instant::now();
    let texts = selected_text(
        repo,
        &index.commit,
        &chosen.iter().map(|(s, _)| *s).collect::<Vec<_>>(),
    )?;
    let selected_read_ms = read_start.elapsed().as_secs_f64() * 1000.0;
    if options.syntax {
        for ((source, _), text) in chosen.iter().zip(&texts) {
            if let Some(file) = &source.syntax {
                syntax::validate_text(file, text)?;
            }
        }
    }
    let evidence = chosen
        .into_iter()
        .zip(texts)
        .map(|((source, mut reasons), source_text)| {
            let lines: Vec<_> = source_text.split_inclusive('\n').collect();
            let hit = if components.symbols {
                source
                    .symbols
                    .iter()
                    .find(|s| !terms(&s.name).is_disjoint(&query))
                    .map(|s| s.line - 1)
            } else {
                None
            }
            .or_else(|| {
                if components.lexical {
                    lines
                        .iter()
                        .position(|line| !terms(line).is_disjoint(&query))
                } else {
                    None
                }
            })
            .unwrap_or(0);
            let selection = if options.syntax { source.syntax.as_ref().and_then(|file| syntax::choose(file, &identifiers)) } else { None };
            let (start, end) = if let Some((declaration, match_kind, matches)) = selection {
                reasons.push(format!("Tree-sitter {match_kind}: {} ({})", declaration.qualified_name, declaration.kind));
                if matches > 1 { reasons.push(format!("Ambiguous name: {matches} equally ranked declarations; source order breaks the tie.")); }
                let start = declaration.declaration.start_line - 1;
                (start, (start + 64).min(declaration.declaration.end_line).min(lines.len()))
            } else {
                if options.syntax { reasons.push("No complete matching Rust declaration; baseline excerpt selection used.".into()); }
                let start = hit.saturating_sub(8);
                (start, (start + 64).min(lines.len()))
            };
            if options.syntax && source.syntax.as_ref().is_some_and(|file| file.parse_has_error) {
                reasons.push("The source has parse errors; declarations containing recovered errors are not selected structurally.".into());
            }
            let byte_start: usize = lines[..start].iter().map(|line| line.len()).sum();
            let byte_end = byte_start
                + lines[start..end]
                    .iter()
                    .map(|line| line.len())
                    .sum::<usize>();
            let text = source_text[byte_start..byte_end].to_owned();
            Evidence {
                path: source.path.clone(),
                blob: source.blob.clone(),
                file_sha256: source.sha256.clone(),
                excerpt_sha256: sha256(text.as_bytes()),
                start_line: start + 1,
                end_line: end,
                total_lines: lines.len(),
                reasons,
                text,
                syntax_selection: selection.map(|(declaration, match_kind, matches)| syntax::Selection {
                    partial: byte_start > declaration.declaration.start_byte || byte_end < declaration.declaration.end_byte,
                    declaration: declaration.clone(), match_kind: match_kind.into(), equally_ranked_matches: matches,
                }),
            }
        })
        .collect();
    let history = if components.history {
        let mut hits: Vec<_> = index
            .history
            .iter()
            .filter(|c| !c.terms.is_disjoint(&query))
            .collect();
        hits.sort_by(|a, b| {
            b.terms
                .intersection(&query)
                .count()
                .cmp(&a.terms.intersection(&query).count())
                .then_with(|| a.oid.cmp(&b.oid))
        });
        hits.into_iter()
            .take(4)
            .map(|c| Commit {
                oid: c.oid.clone(),
                subject: c.subject.clone(),
                terms: c.terms.clone(),
            })
            .collect()
    } else {
        vec![]
    };
    if components.history {
        notes.push("History considers subjects from at most 32 recent commits; it does not infer fixes or dependency relationships.".into());
    }
    notes.push("Excerpts show at most 64 lines per file. Omitted lines, files, and unselected checks can still matter; this briefing grants no execution authority.".into());
    if options.syntax {
        notes.push("Tree-sitter changes excerpt selection only; the baseline file ranking and 64-line maximum remain the same. Syntax names are case-sensitive lexical scopes, without macro expansion or compiler name resolution.".into());
        let limited = index
            .files
            .iter()
            .filter_map(|f| f.syntax.as_ref())
            .filter(|f| f.parse_has_error || f.traversal_limited || f.omitted_declarations > 0)
            .count();
        notes.push(format!("Syntax parsing reports errors or bounded extraction in {limited} indexed Rust files; see each file's cached limitations."));
    }
    Ok(Brief {
        syntax: options.syntax.then(syntax::Provenance::default),
        schema: "openagents.briefing-lab.preview.v1".into(),
        commit: index.commit.clone(),
        issue,
        components,
        evidence,
        history,
        notes,
        candidate_files,
        omitted_candidates: candidate_files.saturating_sub(8),
        index_omissions: index.omissions.clone(),
        timings_ms: BTreeMap::from([
            ("selected_git_validation_and_read".into(), selected_read_ms),
            ("assembly".into(), start.elapsed().as_secs_f64() * 1000.0),
        ]),
    })
}

fn fence(text: &str) -> String {
    let n = text
        .split(|c| c != '`')
        .map(str::len)
        .max()
        .unwrap_or(0)
        .max(2)
        + 1;
    "`".repeat(n)
}
pub fn markdown(brief: &Brief) -> String {
    let mut out = format!(
        "# Issue briefing preview\n\nSource commit: `{}`\n\n## Original issue\n\n",
        brief.commit
    );
    let issue_text = format!("{}\n\n{}", brief.issue.title, brief.issue.body);
    let mark = fence(&issue_text);
    out.push_str(&format!(
        "{mark}text\n{issue_text}\n{mark}\n\n## Selected evidence\n\n"
    ));
    for e in &brief.evidence {
        if let Some(selection) = &e.syntax_selection {
            out.push_str(&format!(
                "Syntax selection: `{}`; declaration {}.\n\n",
                selection.declaration.qualified_name,
                if selection.partial {
                    "partially shown under the 64-line limit"
                } else {
                    "fully shown"
                }
            ));
        }
        let mark = fence(&e.text);
        out.push_str(&format!("### `{}`: {}–{} of {} lines\n\n{}\n\nFile SHA-256: `{}`. Git blob: `{}`.\n\n{mark}text\n{}\n{mark}\n\n", e.path, e.start_line, e.end_line, e.total_lines, e.reasons.join("; "), e.file_sha256, e.blob, e.text));
    }
    out.push_str("## Recent history candidates\n\n");
    if brief.history.is_empty() {
        out.push_str("No history candidates selected.\n\n");
    }
    for c in &brief.history {
        out.push_str(&format!("- `{}`: {}\n", c.oid, c.subject));
    }
    out.push_str(&format!("\n## Coverage and omissions\n\nSelected {} evidence excerpts; {} ranked candidates omitted.\n\n", brief.evidence.len(), brief.omitted_candidates));
    for note in &brief.notes {
        out.push_str(&format!("- {note}\n"));
    }
    for (reason, count) in &brief.index_omissions {
        out.push_str(&format!("- Index omitted {count} entries: {reason}.\n"));
    }
    out.push_str("\n## Timings\n\n");
    for (name, ms) in &brief.timings_ms {
        out.push_str(&format!("- {name}: {ms:.3} ms\n"));
    }
    out
}
