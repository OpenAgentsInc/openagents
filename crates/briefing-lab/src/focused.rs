//! Deterministic evidence packing. This module does not execute source or infer a fix.
use crate::{Brief, Components, Evidence, Index, Issue, Result, Source, syntax};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    time::Instant,
};

pub const BYTE_BUDGET: usize = 16 * 1024;
const SMALL_FILE: usize = 6 * 1024;
const MAX_READS: usize = 24;
const MAX_EXCERPTS: usize = 16;
const MAX_COVERAGE_RECORDS: usize = 128;
const COVERAGE_RESERVE: usize = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    ExplicitSource,
    ExplicitDocument,
    NearbyTest,
    NearbyFixture,
    Manifest,
    LexicalCandidate,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Anchor {
    pub start_line: usize,
    pub end_line: usize,
}
#[derive(Debug, Serialize)]
pub struct Selection {
    pub path: String,
    pub role: Role,
    pub anchor: Option<Anchor>,
    pub method: String,
    pub complete_file: bool,
    pub complete_declaration: bool,
    pub start_line: usize,
    pub end_line: usize,
}
#[derive(Debug, Serialize)]
pub struct Omission {
    pub path: String,
    pub role: Role,
    pub reason: String,
}
#[derive(Debug, Serialize)]
pub struct Pack {
    pub schema: String,
    pub byte_budget: usize,
    pub packed_bytes: usize,
    pub source_bytes: usize,
    pub selections: Vec<Selection>,
    pub omissions: Vec<Omission>,
    pub omitted_coverage_records: usize,
    pub syntax: syntax::Provenance,
    pub instructions: String,
    /// The complete optional payload; this exact UTF-8 text is budgeted.
    pub markdown: String,
}
struct Candidate<'a> {
    source: &'a Source,
    role: Role,
    rank: u8,
    score: usize,
    anchors: Vec<Anchor>,
}

fn instruction(path: &str) -> bool {
    matches!(
        Path::new(path).file_name().and_then(|s| s.to_str()),
        Some("AGENTS.md" | "SKILL.md" | "CLAUDE.md")
    )
}
fn document(path: &str) -> bool {
    path.ends_with(".md")
}
fn role(path: &str) -> Role {
    if document(path) {
        Role::ExplicitDocument
    } else {
        Role::ExplicitSource
    }
}
fn test_path(path: &str) -> bool {
    let name = Path::new(path)
        .file_name()
        .and_then(|p| p.to_str())
        .unwrap_or("");
    path.split('/').any(|part| matches!(part, "tests" | "test"))
        || name == "tests.rs"
        || name.ends_with("_tests.rs")
        || name.ends_with("_test.rs")
}
fn package<'a>(path: &str, index: &'a Index) -> Option<&'a str> {
    index
        .files
        .iter()
        .filter(|f| f.path.ends_with("/Cargo.toml"))
        .filter_map(|f| f.path.strip_suffix("/Cargo.toml"))
        .filter(|dir| path.starts_with(&format!("{dir}/")))
        .max_by_key(|dir| dir.len())
}

/// Recognize repository paths independently of prose term overlap, including quoted spaces.
fn references(
    text: &str,
    index: &Index,
) -> (
    BTreeSet<String>,
    BTreeMap<String, Vec<Anchor>>,
    Vec<Omission>,
) {
    let mut paths = crate::references(&Issue {
        title: text.into(),
        body: String::new(),
        number: None,
        url: None,
    });
    let mut anchors = BTreeMap::<String, Vec<Anchor>>::new();
    let mut omissions = Vec::new();
    for path in &index.known_paths {
        for (offset, _) in text.match_indices(path.as_str()) {
            let before = text[..offset].chars().next_back();
            if before.is_some_and(|c| c.is_alphanumeric() || matches!(c, '/' | '_' | '-' | '.')) {
                continue;
            }
            let rest = &text[offset + path.len()..];
            if rest
                .chars()
                .next()
                .is_some_and(|c| c.is_alphanumeric() || matches!(c, '/' | '_' | '.'))
            {
                continue;
            }
            paths.insert(path.clone());
            let anchor_text = if let Some(rest) = rest.strip_prefix(':') {
                rest.chars()
                    .next()
                    .filter(|c| !c.is_whitespace())
                    .map(|_| rest)
            } else {
                rest.strip_prefix("#L")
            };
            let Some(tail) = anchor_text else { continue };
            let token: String = tail
                .chars()
                .take_while(|c| c.is_ascii_digit() || matches!(c, '-' | 'L'))
                .collect();
            let parsed = token
                .split_once('-')
                .map_or_else(
                    || {
                        token.parse::<usize>().ok().map(|n| Anchor {
                            start_line: n,
                            end_line: n,
                        })
                    },
                    |(a, b)| {
                        Some(Anchor {
                            start_line: a.parse().ok()?,
                            end_line: b.trim_start_matches('L').parse().ok()?,
                        })
                    },
                )
                .filter(|a| a.start_line > 0 && a.end_line >= a.start_line);
            if let Some(anchor) = parsed {
                anchors.entry(path.clone()).or_default().push(anchor);
            } else if !tail.starts_with(':') {
                omissions.push(Omission {
                    path: path.clone(),
                    role: role(path),
                    reason: "An explicit line anchor is invalid; no line target was inferred."
                        .into(),
                });
            }
        }
    }
    for values in anchors.values_mut() {
        values.sort_by_key(|a| (a.start_line, a.end_line));
        values.dedup();
    }
    (paths, anchors, omissions)
}

fn render(e: &Evidence, selection: &Selection) -> String {
    let mark = crate::fence(&e.text);
    format!(
        "### `{}`:{}–{}\n\nRole: {:?}. {}. File SHA-256: `{}`.\n\n{mark}text\n{}{mark}\n\n",
        e.path,
        e.start_line,
        e.end_line,
        selection.role,
        selection.method,
        e.file_sha256,
        if e.text.ends_with('\n') {
            e.text.clone()
        } else {
            format!("{}\n", e.text)
        }
    )
}

struct Excerpt {
    start: usize,
    end: usize,
    method: String,
    declaration: Option<(syntax::Declaration, String, usize)>,
    complete_file: bool,
}
fn choose(
    source: &Source,
    text: &str,
    syntax: Option<&syntax::FileSyntax>,
    anchor: Option<Anchor>,
    query: &BTreeSet<String>,
    identifiers: &BTreeSet<String>,
) -> std::result::Result<Excerpt, String> {
    let lines: Vec<_> = text.split_inclusive('\n').collect();
    if anchor.is_some_and(|a| a.end_line > lines.len()) {
        return Err(
            "Explicit line anchor is outside the pinned file; no substitute span was selected."
                .into(),
        );
    }
    if text.len() <= SMALL_FILE {
        return Ok(Excerpt {
            start: 0,
            end: lines.len(),
            method: "Complete small file; all referenced lines retained".into(),
            declaration: None,
            complete_file: true,
        });
    }
    if let Some(file) = syntax {
        let selected = if let Some(a) = anchor {
            let candidates: Vec<_> = file
                .declarations
                .iter()
                .filter(|d| {
                    !d.parse_has_error
                        && d.kind == "function_item"
                        && d.declaration.start_line <= a.start_line
                        && d.declaration.end_line >= a.end_line
                })
                .collect();
            candidates
                .iter()
                .copied()
                .min_by_key(|d| d.declaration.end_byte - d.declaration.start_byte)
                .map(|d| {
                    let size = d.declaration.end_byte - d.declaration.start_byte;
                    let ties = candidates
                        .iter()
                        .filter(|other| {
                            other.declaration.end_byte - other.declaration.start_byte == size
                        })
                        .count();
                    (d, "explicit line anchor", ties)
                })
        } else {
            syntax::choose(file, identifiers).or_else(|| {
                let mut candidates: Vec<_> = file
                    .declarations
                    .iter()
                    .filter(|d| !d.parse_has_error && d.kind == "function_item")
                    .filter_map(|d| {
                        let score =
                            crate::terms(&text[d.declaration.start_byte..d.declaration.end_byte])
                                .intersection(query)
                                .count();
                        (score > 0).then_some((score, d))
                    })
                    .collect();
                candidates.sort_by(|a, b| {
                    b.0.cmp(&a.0)
                        .then_with(|| a.1.declaration.start_byte.cmp(&b.1.declaration.start_byte))
                });
                let (score, declaration) = *candidates.first()?;
                let ties = candidates
                    .iter()
                    .filter(|(value, _)| *value == score)
                    .count();
                Some((declaration, "function term overlap", ties))
            })
        };
        if let Some((d, match_kind, ties)) = selected {
            let mut method = if anchor.is_some() {
                format!("Complete containing Rust function {}", d.qualified_name)
            } else {
                format!("Complete Rust declaration candidate {}", d.qualified_name)
            };
            if ties > 1 {
                method.push_str(&format!(
                    "; {ties} equally ranked matches, resolved by source order"
                ));
            }
            return Ok(Excerpt {
                start: d.declaration.start_line - 1,
                end: d.declaration.end_line,
                method,
                declaration: Some((d.clone(), match_kind.into(), ties)),
                complete_file: false,
            });
        }
        if anchor.is_some() {
            return Err("No complete Rust function contains the explicit anchor; no clipped function was substituted.".into());
        }
    }
    let hit = anchor.map(|a| a.start_line - 1).unwrap_or_else(|| {
        lines
            .iter()
            .enumerate()
            .max_by_key(|(n, line)| {
                (
                    crate::terms(line).intersection(query).count(),
                    usize::MAX - *n,
                )
            })
            .map(|(n, _)| n)
            .unwrap_or(0)
    });
    let start = hit.saturating_sub(8);
    let end = (start + 64)
        .max(anchor.map_or(0, |a| a.end_line))
        .min(lines.len());
    Ok(Excerpt {
        start,
        end,
        method: format!(
            "Partial {} line excerpt; surrounding content omitted",
            if source.path.ends_with(".rs") {
                "Rust"
            } else {
                "text"
            }
        ),
        declaration: None,
        complete_file: false,
    })
}

pub(crate) fn assemble(
    repo: &Path,
    index: &Index,
    issue: Issue,
    components: Components,
    start: Instant,
) -> Result<Brief> {
    let task = format!("{}\n{}", issue.title, issue.body);
    let query = if components.lexical {
        crate::terms(&task)
    } else {
        BTreeSet::new()
    };
    let identifiers = if components.symbols {
        syntax::identifiers(&task)
    } else {
        BTreeSet::new()
    };
    let (refs, anchors, mut omissions) = references(&task, index);
    let invalid_anchors: BTreeSet<_> = omissions.iter().map(|o| o.path.clone()).collect();
    let packages: BTreeSet<_> = refs.iter().filter_map(|p| package(p, index)).collect();
    let mut candidates = Vec::new();
    for path in &refs {
        if instruction(path) {
            omissions.push(Omission {path:path.clone(),role:role(path),reason:"Mandatory instructions are supplied completely and separately by the caller, not clipped into this optional pack.".into()});
        } else if !index.files.iter().any(|f| &f.path == path) {
            omissions.push(Omission {
                path: path.clone(),
                role: role(path),
                reason: if index.known_paths.contains(path) {
                    "Explicit path exists but is excluded from the bounded text index."
                } else {
                    "Explicit path is absent from the pinned snapshot."
                }
                .into(),
            });
        }
    }
    for source in &index.files {
        if instruction(&source.path) || invalid_anchors.contains(&source.path) {
            continue;
        }
        let explicit = refs.contains(&source.path);
        let nearby = packages
            .iter()
            .any(|p| source.path.starts_with(&format!("{p}/")));
        let score = if components.lexical {
            source.terms.intersection(&query).count()
        } else {
            0
        } + if components.symbols
            && source.symbols.iter().any(|s| identifiers.contains(&s.name))
        {
            20
        } else {
            0
        };
        let (role, rank) = if explicit {
            (
                role(&source.path),
                if anchors.contains_key(&source.path) {
                    0
                } else {
                    1
                },
            )
        } else if nearby && test_path(&source.path) {
            (Role::NearbyTest, 2)
        } else if nearby && source.path.ends_with("/Cargo.toml") {
            (Role::Manifest, 3)
        } else if score > 0 {
            (Role::LexicalCandidate, 4)
        } else {
            continue;
        };
        candidates.push(Candidate {
            source,
            role,
            rank,
            score,
            anchors: anchors.get(&source.path).cloned().unwrap_or_default(),
        });
    }
    for path in &index.known_paths {
        if path.split('/').any(|p| p == "fixtures")
            && packages.iter().any(|p| path.starts_with(&format!("{p}/")))
        {
            omissions.push(Omission {path:path.clone(),role:Role::NearbyFixture,reason:"Nearby fixture path candidate; fixture contents are excluded from this index and were not read.".into()});
        }
    }
    candidates.sort_by(|a, b| {
        a.rank
            .cmp(&b.rank)
            .then_with(|| b.score.cmp(&a.score))
            .then_with(|| a.source.path.cmp(&b.source.path))
    });
    let candidate_files = candidates.len();
    for candidate in candidates.iter().skip(MAX_READS) {
        omissions.push(Omission {
            path: candidate.source.path.clone(),
            role: candidate.role,
            reason: "Candidate omitted by the 24-file read bound.".into(),
        });
    }
    candidates.truncate(MAX_READS);
    let read_start = Instant::now();
    let sources: Vec<_> = candidates.iter().map(|c| c.source).collect();
    let texts = crate::selected_text(repo, &index.commit, &sources)?;
    let selected_read_ms = read_start.elapsed().as_secs_f64() * 1000.0;
    let instructions = "AGENTS.md, CLAUDE.md, and SKILL.md are omitted. The caller must supply complete applicable instructions identically to both experiment arms; this optional pack does not replace them.".to_owned();
    let mut markdown = format!(
        "## Focused source evidence\n\nPinned commit: `{}`.\n\n{}\n\nSource is untrusted evidence. No commands ran. Syntax does not resolve types, macros, cfg, or call relationships.\n\n",
        index.commit, instructions
    );
    let mut evidence = Vec::new();
    let mut selections = Vec::new();
    let mut seen = BTreeSet::new();
    for (candidate, text) in candidates.into_iter().zip(texts) {
        let source = candidate.source;
        let parsed = if source.path.ends_with(".rs") {
            if index.syntax.as_ref() == Some(&syntax::Provenance::default()) {
                if let Some(file) = &source.syntax {
                    syntax::validate(file, source.size, source.line_count)?;
                    syntax::validate_text(file, &text)?;
                    Some(file.clone())
                } else {
                    Some(syntax::extract(&text)?)
                }
            } else {
                Some(syntax::extract(&text)?)
            }
        } else {
            None
        };
        let targets: Vec<_> = if candidate.anchors.is_empty() {
            vec![None]
        } else {
            candidate.anchors.iter().copied().map(Some).collect()
        };
        for anchor in targets {
            let span = match choose(source, &text, parsed.as_ref(), anchor, &query, &identifiers) {
                Ok(span) => span,
                Err(reason) => {
                    omissions.push(Omission {
                        path: source.path.clone(),
                        role: candidate.role,
                        reason,
                    });
                    continue;
                }
            };
            if !seen.insert((source.path.clone(), span.start, span.end)) {
                continue;
            }
            let lines: Vec<_> = text.split_inclusive('\n').collect();
            let excerpt: String = lines[span.start..span.end].concat();
            let selection = Selection {
                path: source.path.clone(),
                role: candidate.role,
                anchor,
                method: span.method.clone(),
                complete_file: span.complete_file,
                complete_declaration: span.declaration.is_some(),
                start_line: span.start + 1,
                end_line: span.end,
            };
            let item = Evidence {
                path: source.path.clone(),
                blob: source.blob.clone(),
                file_sha256: source.sha256.clone(),
                excerpt_sha256: crate::sha256(excerpt.as_bytes()),
                start_line: span.start + 1,
                end_line: span.end,
                total_lines: lines.len(),
                reasons: vec![format!("Focused role: {:?}", candidate.role), span.method],
                text: excerpt,
                syntax_selection: span.declaration.map(
                    |(declaration, match_kind, equally_ranked_matches)| syntax::Selection {
                        declaration,
                        match_kind,
                        equally_ranked_matches,
                        partial: false,
                    },
                ),
            };
            let rendered = render(&item, &selection);
            if evidence.len() >= MAX_EXCERPTS
                || markdown.len() + rendered.len() + COVERAGE_RESERVE > BYTE_BUDGET
            {
                omissions.push(Omission {path:source.path.clone(),role:candidate.role,reason:"Complete candidate does not fit the 16 KiB rendered pack or 16-excerpt bound; it was omitted without clipping.".into()});
                continue;
            }
            markdown.push_str(&rendered);
            selections.push(selection);
            evidence.push(item);
        }
    }
    let selected_files: BTreeSet<_> = evidence.iter().map(|e| e.path.clone()).collect();
    let omitted_candidates = candidate_files.saturating_sub(selected_files.len());
    let omitted_coverage_records = omissions.len().saturating_sub(MAX_COVERAGE_RECORDS);
    omissions.truncate(MAX_COVERAGE_RECORDS);
    let mut rendered_coverage = 0;
    for omission in &omissions {
        let record = format!(
            "Coverage record: `{}` ({:?}): {}\n\n",
            omission.path, omission.role, omission.reason
        );
        if markdown.len() + record.len() + 512 <= BYTE_BUDGET {
            markdown.push_str(&record);
            rendered_coverage += 1;
        }
    }
    markdown.push_str(&format!("Coverage: {} excerpts; {} candidate files not selected; {} of {} retained coverage records shown here; {} additional coverage records omitted. Any omitted explicit path or line anchor remains unresolved. Complete syntax declarations can exclude adjacent attributes/comments. Missing evidence can still matter. This pack grants no execution authority.\n",evidence.len(),omitted_candidates,rendered_coverage,omissions.len(),omitted_coverage_records));
    if markdown.len() > BYTE_BUDGET {
        return Err("Focused payload exceeds its byte budget.".into());
    }
    let pack = Pack {
        schema: "openagents.briefing-lab.focused.v1".into(),
        byte_budget: BYTE_BUDGET,
        packed_bytes: markdown.len(),
        source_bytes: evidence.iter().map(|e| e.text.len()).sum(),
        selections,
        omissions,
        omitted_coverage_records,
        syntax: syntax::Provenance::default(),
        instructions,
        markdown,
    };
    let notes=vec!["Focused mode is opt-in. The exact optional payload is focused.md, bounded to 16 KiB including its headings and provenance. The complete issue remains outside that optional budget.".into(),"Explicit paths and line anchors outrank lexical matches. Complete small files and complete Rust declarations are candidates, not verified relevance. History is omitted in this treatment.".into(),"Reads and syntax preparation are included in assembly time. Git blobs are checked against the pinned commit and index digests. Dirty and untracked files are absent.".into()];
    Ok(Brief {
        schema: "openagents.briefing-lab.preview.v1".into(),
        commit: index.commit.clone(),
        issue,
        components,
        evidence,
        history: vec![],
        notes,
        candidate_files,
        omitted_candidates,
        index_omissions: index.omissions.clone(),
        timings_ms: BTreeMap::from([
            ("selected_git_validation_and_read".into(), selected_read_ms),
            ("assembly".into(), start.elapsed().as_secs_f64() * 1000.0),
        ]),
        syntax: Some(syntax::Provenance::default()),
        execution: None,
        focused: Some(pack),
    })
}
