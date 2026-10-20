//! Syntax-highlighting contracts shared by OpenAgents's desktop surfaces.
//!
//! This crate intentionally has no UI, RPC, or engine dependencies. Public
//! ranges are byte offsets relative to one UTF-8 source line.

use std::{collections::BTreeSet, ops::Range, path::Path, sync::atomic::AtomicUsize};

#[cfg(feature = "highlight")]
mod engine;
#[cfg(not(feature = "highlight"))]
mod engine {
    use super::*;

    pub(crate) fn highlight_detected(
        _request: HighlightRequest<'_>,
        language: LanguageId,
        _limits: HighlightLimits,
        _cancellation_flag: Option<&AtomicUsize>,
    ) -> Result<HighlightedDocument, HighlightError> {
        Err(HighlightError::GrammarUnavailable(language))
    }
}

pub const DEFAULT_MAX_SOURCE_BYTES: usize = 1024 * 1024;
pub const DEFAULT_MAX_SPANS: usize = 200_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HighlightLimits {
    pub max_source_bytes: usize,
    pub max_spans: usize,
}

impl Default for HighlightLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: DEFAULT_MAX_SOURCE_BYTES,
            max_spans: DEFAULT_MAX_SPANS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LanguageId {
    Rust,
    JavaScript,
    Jsx,
    TypeScript,
    Tsx,
    Python,
    Go,
    Json,
    Jsonc,
    Bash,
    Toml,
    Markdown,
    Html,
    Css,
    Yaml,
    C,
    Cpp,
    CSharp,
    Java,
    Kotlin,
    Swift,
    Ruby,
    Php,
    Sql,
    Lua,
    Dockerfile,
    Nix,
    Make,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HighlightKind {
    Comment,
    Keyword,
    String,
    StringSpecial,
    Escape,
    Number,
    Boolean,
    Type,
    TypeBuiltin,
    Constructor,
    Function,
    FunctionBuiltin,
    Macro,
    Property,
    Constant,
    Variable,
    VariableSpecial,
    Parameter,
    Operator,
    Punctuation,
    Tag,
    Attribute,
    Label,
    MarkupHeading,
    MarkupRaw,
    MarkupLink,
    MarkupReference,
    MarkupEmphasis,
    MarkupStrong,
    Embedded,
    Invalid,
}

impl HighlightKind {
    /// Stable precedence used to resolve overlapping parser captures.
    pub const fn precedence(self) -> u8 {
        match self {
            Self::Invalid => 100,
            Self::Escape => 95,
            Self::Macro => 90,
            Self::Property | Self::Attribute => 85,
            Self::FunctionBuiltin | Self::TypeBuiltin | Self::VariableSpecial => 80,
            Self::StringSpecial | Self::Constructor | Self::Parameter => 75,
            Self::Function | Self::Type | Self::Constant | Self::Tag | Self::Label => 70,
            Self::Comment | Self::Keyword | Self::String | Self::Number | Self::Boolean => 60,
            Self::Variable | Self::Operator => 50,
            Self::Punctuation | Self::Embedded => 40,
            // Markdown block captures can wrap more specific inline captures,
            // and fenced-code captures can wrap an injected language. Keep
            // markup below programming-language tokens while preserving the
            // nesting order among Markdown roles.
            Self::MarkupHeading => 30,
            Self::MarkupEmphasis => 31,
            Self::MarkupStrong => 32,
            Self::MarkupLink | Self::MarkupReference => 33,
            Self::MarkupRaw => 34,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightSpan {
    pub range: Range<usize>,
    pub kind: HighlightKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightedDocument {
    pub language: LanguageId,
    pub lines: Vec<Vec<HighlightSpan>>,
}

#[derive(Debug, Clone, Copy)]
pub struct HighlightRequest<'a> {
    pub source: &'a str,
    pub path: Option<&'a str>,
    pub fence_tag: Option<&'a str>,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum HighlightError {
    #[error("the source language is not registered")]
    UnknownLanguage,
    #[error("highlight range {start}..{end} is invalid for a {len}-byte source")]
    InvalidRange {
        start: usize,
        end: usize,
        len: usize,
    },
    #[error("highlight range {start}..{end} is not on UTF-8 boundaries")]
    InvalidUtf8Boundary { start: usize, end: usize },
    #[error("source exceeds the configured highlighting limit")]
    SourceTooLarge,
    #[error("highlight output exceeds the configured span limit")]
    TooManySpans,
    #[error("parser failed: {0}")]
    Parser(String),
    #[error("the {0:?} grammar is not bundled")]
    GrammarUnavailable(LanguageId),
}

impl HighlightedDocument {
    /// Validate, split, and normalize absolute source spans into line-relative spans.
    pub fn from_absolute_spans(
        language: LanguageId,
        source: &str,
        spans: impl IntoIterator<Item = HighlightSpan>,
    ) -> Result<Self, HighlightError> {
        let starts = line_starts(source);
        let mut lines = vec![Vec::new(); starts.len()];
        for span in spans {
            validate_span(source, &span.range)?;
            if span.range.is_empty() {
                continue;
            }
            let first_line = starts.partition_point(|&start| start <= span.range.start) - 1;
            for (line_ix, &start) in starts.iter().enumerate().skip(first_line) {
                let raw_end = starts.get(line_ix + 1).copied().unwrap_or(source.len());
                let mut end = raw_end;
                if source.as_bytes().get(end.wrapping_sub(1)) == Some(&b'\n') {
                    end -= 1;
                    if source.as_bytes().get(end.wrapping_sub(1)) == Some(&b'\r') {
                        end -= 1;
                    }
                }
                let segment_start = span.range.start.max(start);
                let segment_end = span.range.end.min(end);
                if segment_start < segment_end {
                    lines[line_ix].push(HighlightSpan {
                        range: segment_start - start..segment_end - start,
                        kind: span.kind,
                    });
                }
                if raw_end >= span.range.end {
                    break;
                }
            }
        }
        for line in &mut lines {
            *line = normalize_line(std::mem::take(line));
        }
        Ok(Self { language, lines })
    }
}

fn validate_span(source: &str, range: &Range<usize>) -> Result<(), HighlightError> {
    if range.start > range.end || range.end > source.len() {
        return Err(HighlightError::InvalidRange {
            start: range.start,
            end: range.end,
            len: source.len(),
        });
    }
    if !source.is_char_boundary(range.start) || !source.is_char_boundary(range.end) {
        return Err(HighlightError::InvalidUtf8Boundary {
            start: range.start,
            end: range.end,
        });
    }
    Ok(())
}

fn normalize_line(spans: Vec<HighlightSpan>) -> Vec<HighlightSpan> {
    #[derive(Clone, Copy)]
    enum Edge {
        Start(usize),
        End(usize),
    }

    let mut edges = spans
        .iter()
        .enumerate()
        .flat_map(|(index, span)| {
            [
                (span.range.start, Edge::Start(index)),
                (span.range.end, Edge::End(index)),
            ]
        })
        .collect::<Vec<_>>();
    edges.sort_unstable_by_key(|(offset, _)| *offset);

    // The span index is the tie-breaker so equal-precedence overlaps retain
    // the old `Iterator::max_by_key` behavior (the later span wins).
    let mut active = BTreeSet::new();
    let mut normalized: Vec<HighlightSpan> = Vec::new();
    let mut cursor = 0;
    while cursor < edges.len() {
        let offset = edges[cursor].0;
        let group_start = cursor;
        while cursor < edges.len() && edges[cursor].0 == offset {
            if let Edge::End(index) = edges[cursor].1 {
                active.remove(&(spans[index].kind.precedence(), index));
            }
            cursor += 1;
        }
        for (_, edge) in &edges[group_start..cursor] {
            if let Edge::Start(index) = *edge {
                active.insert((spans[index].kind.precedence(), index));
            }
        }

        let Some(next_offset) = edges.get(cursor).map(|(next, _)| *next) else {
            break;
        };
        if offset == next_offset {
            continue;
        }
        if let Some((_, index)) = active.last().copied() {
            let kind = spans[index].kind;
            if let Some(previous) = normalized.last_mut()
                && previous.kind == kind
                && previous.range.end == offset
            {
                previous.range.end = next_offset;
            } else {
                normalized.push(HighlightSpan {
                    range: offset..next_offset,
                    kind,
                });
            }
        }
    }
    normalized
}

fn line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    starts.extend(
        source
            .match_indices('\n')
            .map(|(index, _)| index + 1)
            .filter(|start| *start < source.len()),
    );
    starts
}

/// Whether this build contains a parser and compatible highlight queries.
/// Builds without the `highlight` feature (the web) bundle no grammar.
pub const fn supports_language(language: LanguageId) -> bool {
    let _ = language;
    cfg!(feature = "highlight")
}

/// Highlight a complete document with the default resource limits.
pub fn highlight(request: HighlightRequest<'_>) -> Result<HighlightedDocument, HighlightError> {
    highlight_with_limits(request, HighlightLimits::default(), None)
}

/// Highlight a complete document with explicit limits and cooperative cancellation.
pub fn highlight_with_limits(
    request: HighlightRequest<'_>,
    limits: HighlightLimits,
    cancellation_flag: Option<&AtomicUsize>,
) -> Result<HighlightedDocument, HighlightError> {
    if request.source.len() > limits.max_source_bytes {
        return Err(HighlightError::SourceTooLarge);
    }
    let language = detect_language(
        request.path,
        request.fence_tag,
        request.source.lines().next(),
    )
    .ok_or(HighlightError::UnknownLanguage)?;
    if !supports_language(language) {
        return Err(HighlightError::GrammarUnavailable(language));
    }

    engine::highlight_detected(request, language, limits, cancellation_flag)
}

pub fn detect_language(
    path: Option<&str>,
    fence_tag: Option<&str>,
    first_line: Option<&str>,
) -> Option<LanguageId> {
    fence_tag
        .and_then(language_for_alias)
        .or_else(|| path.and_then(language_for_path))
        .or_else(|| first_line.and_then(language_for_shebang))
}

pub fn language_for_alias(alias: &str) -> Option<LanguageId> {
    let alias = alias
        .trim()
        .split_ascii_whitespace()
        .next()?
        .to_ascii_lowercase();
    Some(match alias.as_str() {
        "rust" | "rs" => LanguageId::Rust,
        "javascript" | "js" | "mjs" | "cjs" => LanguageId::JavaScript,
        "jsx" => LanguageId::Jsx,
        "typescript" | "ts" | "mts" | "cts" => LanguageId::TypeScript,
        "tsx" => LanguageId::Tsx,
        "python" | "py" | "python3" => LanguageId::Python,
        "go" | "golang" => LanguageId::Go,
        "json" => LanguageId::Json,
        "jsonc" => LanguageId::Jsonc,
        "bash" | "sh" | "shell" | "zsh" | "console" => LanguageId::Bash,
        "toml" => LanguageId::Toml,
        "markdown" | "md" => LanguageId::Markdown,
        "html" | "htm" => LanguageId::Html,
        "css" => LanguageId::Css,
        "yaml" | "yml" => LanguageId::Yaml,
        "c" => LanguageId::C,
        "cpp" | "c++" | "cc" | "cxx" | "hpp" => LanguageId::Cpp,
        "csharp" | "c#" | "cs" => LanguageId::CSharp,
        "java" => LanguageId::Java,
        "kotlin" | "kt" | "kts" => LanguageId::Kotlin,
        "swift" => LanguageId::Swift,
        "ruby" | "rb" => LanguageId::Ruby,
        "php" => LanguageId::Php,
        "sql" => LanguageId::Sql,
        "lua" => LanguageId::Lua,
        "dockerfile" | "docker" => LanguageId::Dockerfile,
        "nix" => LanguageId::Nix,
        "make" | "makefile" => LanguageId::Make,
        _ => return None,
    })
}

pub fn language_for_path(path: &str) -> Option<LanguageId> {
    let path = Path::new(path);
    let name = path.file_name()?.to_str()?;
    match name.to_ascii_lowercase().as_str() {
        "dockerfile" | "containerfile" => return Some(LanguageId::Dockerfile),
        "makefile" | "gnumakefile" => return Some(LanguageId::Make),
        "cargo.lock" | "cargo.toml" | "pyproject.toml" => return Some(LanguageId::Toml),
        _ => {}
    }
    language_for_alias(path.extension()?.to_str()?)
}

fn language_for_shebang(line: &str) -> Option<LanguageId> {
    let line = line.strip_prefix("#!")?.to_ascii_lowercase();
    if line.contains("python") {
        Some(LanguageId::Python)
    } else if line.contains("node") {
        Some(LanguageId::JavaScript)
    } else if line.contains("ruby") {
        Some(LanguageId::Ruby)
    } else if ["bash", "zsh", "/sh", " sh"]
        .iter()
        .any(|name| line.contains(name))
    {
        Some(LanguageId::Bash)
    } else {
        None
    }
}

#[cfg(all(test, feature = "highlight"))]
use engine::*;

#[cfg(all(test, feature = "highlight"))]
mod tests {
    #[test]
    fn compiled_queries_are_shared_across_concurrent_documents() {
        let config = super::cached_configuration(super::LanguageId::Rust).unwrap();
        std::thread::scope(|scope| {
            for i in 0..8 {
                scope.spawn(move || {
                    let shared = super::cached_configuration(super::LanguageId::Rust).unwrap();
                    assert!(std::ptr::eq(config, shared));
                    let source = format!("fn example_{i}() {{ let value = {i}; }}");
                    let document = super::highlight(super::HighlightRequest {
                        source: &source,
                        path: None,
                        fence_tag: Some("rust"),
                    })
                    .unwrap();
                    assert_eq!(document.language, super::LanguageId::Rust);
                    assert!(document.lines.iter().flatten().next().is_some());
                });
            }
        });
    }

    use super::*;

    #[test]
    fn aliases_keep_language_variants_distinct() {
        let cases = [
            ("js", LanguageId::JavaScript),
            ("jsx", LanguageId::Jsx),
            ("ts", LanguageId::TypeScript),
            ("tsx", LanguageId::Tsx),
            ("RS", LanguageId::Rust),
            ("shell", LanguageId::Bash),
        ];
        for (alias, expected) in cases {
            assert_eq!(language_for_alias(alias), Some(expected), "{alias}");
        }
        assert_eq!(language_for_alias("unknown-lang"), None);
    }

    #[test]
    fn paths_and_exact_names_are_table_driven() {
        let cases = [
            ("src/main.rs", LanguageId::Rust),
            ("web/app.tsx", LanguageId::Tsx),
            ("Cargo.toml", LanguageId::Toml),
            ("Dockerfile", LanguageId::Dockerfile),
            ("GNUmakefile", LanguageId::Make),
            ("config.jsonc", LanguageId::Jsonc),
        ];
        for (path, expected) in cases {
            assert_eq!(language_for_path(path), Some(expected), "{path}");
        }
        assert_eq!(language_for_path("README"), None);
        assert_eq!(language_for_path("image.png"), None);
    }

    #[test]
    fn shebang_is_only_used_after_explicit_hints() {
        assert_eq!(
            detect_language(None, None, Some("#!/usr/bin/env python3")),
            Some(LanguageId::Python)
        );
        assert_eq!(detect_language(None, None, Some("let x = 1")), None);
    }

    #[test]
    fn spans_are_valid_sorted_non_overlapping_and_line_relative() {
        let source = "let café = \"x\";\nnext";
        let document = HighlightedDocument::from_absolute_spans(
            LanguageId::Rust,
            source,
            [
                HighlightSpan {
                    range: 0..9,
                    kind: HighlightKind::Variable,
                },
                HighlightSpan {
                    range: 0..3,
                    kind: HighlightKind::Keyword,
                },
                HighlightSpan {
                    range: 12..15,
                    kind: HighlightKind::String,
                },
                HighlightSpan {
                    range: 17..21,
                    kind: HighlightKind::Function,
                },
            ],
        )
        .unwrap();
        assert_eq!(document.lines.len(), 2);
        assert_eq!(
            document.lines[0][0],
            HighlightSpan {
                range: 0..3,
                kind: HighlightKind::Keyword
            }
        );
        for line in document.lines {
            assert!(
                line.windows(2)
                    .all(|pair| pair[0].range.end <= pair[1].range.start)
            );
        }
        assert_eq!(
            HighlightedDocument::from_absolute_spans(
                LanguageId::Rust,
                source,
                [HighlightSpan {
                    range: 8..9,
                    kind: HighlightKind::Type
                }]
            ),
            Err(HighlightError::InvalidUtf8Boundary { start: 8, end: 9 })
        );
    }

    #[test]
    fn normalization_preserves_overlap_precedence_and_tie_order() {
        let normalized = normalize_line(vec![
            HighlightSpan {
                range: 0..10,
                kind: HighlightKind::Variable,
            },
            HighlightSpan {
                range: 2..8,
                kind: HighlightKind::Keyword,
            },
            HighlightSpan {
                range: 4..6,
                kind: HighlightKind::String,
            },
        ]);
        assert_eq!(
            normalized,
            vec![
                HighlightSpan {
                    range: 0..2,
                    kind: HighlightKind::Variable,
                },
                HighlightSpan {
                    range: 2..4,
                    kind: HighlightKind::Keyword,
                },
                HighlightSpan {
                    range: 4..6,
                    kind: HighlightKind::String,
                },
                HighlightSpan {
                    range: 6..8,
                    kind: HighlightKind::Keyword,
                },
                HighlightSpan {
                    range: 8..10,
                    kind: HighlightKind::Variable,
                },
            ]
        );
    }

    #[test]
    fn minified_lines_normalize_without_quadratic_rescans() {
        let source = "let value=1;".repeat(8_000);
        let document = highlight(HighlightRequest {
            source: &source,
            path: Some("bundle.js"),
            fence_tag: None,
        })
        .unwrap();
        assert!(document.lines[0].len() > 20_000);
    }

    fn highlighted_fragments(source: &str) -> Vec<(&str, HighlightKind)> {
        let document = highlight(HighlightRequest {
            source,
            path: Some("src/lib.rs"),
            fence_tag: None,
        })
        .unwrap();
        source
            .lines()
            .zip(document.lines)
            .flat_map(|(line, spans)| {
                spans
                    .into_iter()
                    .map(move |span| (&line[span.range], span.kind))
            })
            .collect()
    }

    #[test]
    fn rust_highlighting_distinguishes_structural_categories() {
        let source = r#"pub struct Widget { field: usize }
fn build(value: usize) -> Widget {
    let name = format!("item-{value}");
    Widget { field: 42 }
}"#;
        let fragments = highlighted_fragments(source);
        for (text, expected) in [
            ("pub", HighlightKind::Keyword),
            ("Widget", HighlightKind::Type),
            ("build", HighlightKind::Function),
            ("format!", HighlightKind::Macro),
            ("42", HighlightKind::Number),
        ] {
            assert!(
                fragments.contains(&(text, expected)),
                "missing {text:?} as {expected:?}: {fragments:?}"
            );
        }
    }

    #[test]
    fn rust_multiline_raw_unicode_and_incomplete_code_remain_valid() {
        let source = "/* café\ncomment */\nlet raw = r#\"héllo\nworld\"#;\nlet before = 7;\nfn incomplete( {";
        let document = highlight(HighlightRequest {
            source,
            path: None,
            fence_tag: Some("rust"),
        })
        .unwrap();
        assert!(
            document.lines[1]
                .iter()
                .any(|span| span.kind == HighlightKind::Comment)
        );
        assert!(
            document.lines[3]
                .iter()
                .any(|span| span.kind == HighlightKind::String)
        );
        assert!(
            document
                .lines
                .iter()
                .flatten()
                .any(|span| span.kind == HighlightKind::Number)
        );
        for (line, spans) in source.lines().zip(&document.lines) {
            for span in spans {
                assert!(line.is_char_boundary(span.range.start));
                assert!(line.is_char_boundary(span.range.end));
            }
        }
    }

    #[test]
    fn limits_and_unbundled_languages_degrade_with_typed_errors() {
        assert_eq!(
            highlight_with_limits(
                HighlightRequest {
                    source: "fn main() {}",
                    path: Some("main.rs"),
                    fence_tag: None,
                },
                HighlightLimits {
                    max_source_bytes: 2,
                    max_spans: 10
                },
                None,
            ),
            Err(HighlightError::SourceTooLarge)
        );
        assert_eq!(
            highlight(HighlightRequest {
                source: "plain",
                path: Some("unknown.extension"),
                fence_tag: None,
            }),
            Err(HighlightError::UnknownLanguage)
        );
    }

    #[test]
    fn rust_queries_load_for_the_bundled_abi() {
        assert!(rust_configuration().is_ok());
        let language_version = std::hint::black_box(tree_sitter::LANGUAGE_VERSION);
        assert!(language_version >= tree_sitter::MIN_COMPATIBLE_LANGUAGE_VERSION);
    }

    #[test]
    fn every_registered_grammar_and_query_loads_for_the_bundled_abi() {
        let fixtures = [
            (LanguageId::JavaScript, "app.js", "const value = call(42);"),
            (
                LanguageId::Jsx,
                "app.jsx",
                "const view = <main id=\"x\" />;",
            ),
            (
                LanguageId::TypeScript,
                "app.ts",
                "const value: number = 42;",
            ),
            (
                LanguageId::Tsx,
                "app.tsx",
                "const view: JSX.Element = <main />;",
            ),
            (
                LanguageId::Python,
                "app.py",
                "def call(value):\n    return value",
            ),
            (LanguageId::Go, "main.go", "package main\nfunc main() {}"),
            (LanguageId::Json, "a.json", "{\"value\": 42}"),
            (LanguageId::Jsonc, "a.jsonc", "{\"value\": 42}"),
            (LanguageId::Bash, "run.sh", "echo \"hello\""),
            (LanguageId::Toml, "Cargo.toml", "name = \"openagents\""),
            (LanguageId::Markdown, "README.md", "# Heading\n\n`code`"),
            (LanguageId::Html, "index.html", "<main id=\"app\"></main>"),
            (LanguageId::Css, "app.css", ".app { color: red; }"),
            (LanguageId::Yaml, "app.yml", "name: openagents"),
            (LanguageId::C, "main.c", "int main(void) { return 0; }"),
            (LanguageId::Cpp, "main.cpp", "int main() { return 0; }"),
            (LanguageId::CSharp, "App.cs", "class App { int Value = 1; }"),
            (LanguageId::Java, "App.java", "class App { int value = 1; }"),
            (LanguageId::Kotlin, "App.kt", "val value = 1"),
            (LanguageId::Swift, "App.swift", "let value: Int = 1"),
            (LanguageId::Ruby, "app.rb", "def call(value)\n value\nend"),
            (
                LanguageId::Php,
                "app.php",
                "<?php function call() { return 1; }",
            ),
            (LanguageId::Sql, "query.sql", "SELECT name FROM users;"),
            (LanguageId::Lua, "app.lua", "local value = 1"),
            (LanguageId::Nix, "flake.nix", "{ pkgs }: pkgs.hello"),
            (LanguageId::Make, "Makefile", "all:\n\techo hello"),
            (
                LanguageId::Dockerfile,
                "Dockerfile",
                "FROM alpine\nRUN echo hello",
            ),
        ];
        for (language, path, source) in fixtures {
            let config =
                configuration(language).unwrap_or_else(|error| panic!("{language:?}: {error}"));
            assert!(
                !config.names().is_empty(),
                "{language:?} query has no captures"
            );
            let document = highlight(HighlightRequest {
                source,
                path: Some(path),
                fence_tag: None,
            })
            .unwrap_or_else(|error| panic!("{language:?}: {error}"));
            assert_eq!(document.language, language);
            assert!(
                document.lines.iter().flatten().next().is_some(),
                "{language:?} fixture has no structural spans"
            );
        }
    }

    #[test]
    fn affected_composed_and_project_queries_load_for_pinned_grammars() {
        for language in [
            LanguageId::TypeScript,
            LanguageId::Tsx,
            LanguageId::Kotlin,
            LanguageId::Dockerfile,
        ] {
            let configuration = configuration(language)
                .unwrap_or_else(|error| panic!("{language:?} query failed to load: {error}"));
            assert!(
                !configuration.names().is_empty(),
                "{language:?} query has no captures"
            );
        }
    }

    #[test]
    fn html_injects_javascript_and_css_with_a_bounded_registry() {
        let source = r#"<main id="app">
<style>.item { color: red; }</style>
<script>const answer = call(42);</script>
</main>"#;
        let document = highlight(HighlightRequest {
            source,
            path: Some("index.html"),
            fence_tag: None,
        })
        .unwrap();
        let kinds = document
            .lines
            .iter()
            .flatten()
            .map(|span| span.kind)
            .collect::<Vec<_>>();
        assert!(kinds.contains(&HighlightKind::Tag));
        assert!(kinds.contains(&HighlightKind::Attribute));
        assert!(kinds.contains(&HighlightKind::Keyword));
        assert!(kinds.contains(&HighlightKind::Number));
    }

    #[test]
    fn jsonc_accepts_and_highlights_comments() {
        let source = "{\n  // explanation\n  \"enabled\": true\n}\n";
        let document = highlight(HighlightRequest {
            source,
            path: Some("settings.jsonc"),
            fence_tag: None,
        })
        .unwrap();
        assert_eq!(document.language, LanguageId::Jsonc);
        assert!(
            document.lines[1]
                .iter()
                .any(|span| span.kind == HighlightKind::Comment)
        );
    }

    #[test]
    fn unknown_markdown_fence_does_not_break_parent_highlighting() {
        let source = "# Title\n\n```unknown-language\nopaque\n```\n";
        let document = highlight(HighlightRequest {
            source,
            path: Some("README.md"),
            fence_tag: None,
        })
        .unwrap();
        assert_eq!(document.language, LanguageId::Markdown);
    }

    #[test]
    fn markdown_highlights_block_and_inline_semantics() {
        let source = "# Heading with *emphasis* and **strong**\n\nUse `inline code` and [reference](https://example.com).\n";
        let document = highlight(HighlightRequest {
            source,
            path: Some("README.md"),
            fence_tag: None,
        })
        .unwrap();

        let assert_kind = |line_index: usize, needle: &str, expected: HighlightKind| {
            let line = source.lines().nth(line_index).unwrap();
            let start = line.find(needle).unwrap();
            let end = start + needle.len();
            assert!(
                document.lines[line_index].iter().any(|span| {
                    span.kind == expected && span.range.start <= start && span.range.end >= end
                }),
                "missing {needle:?} as {expected:?}: {:?}",
                document.lines[line_index]
            );
        };

        assert_kind(0, "Heading", HighlightKind::MarkupHeading);
        assert_kind(0, "emphasis", HighlightKind::MarkupEmphasis);
        assert_kind(0, "strong", HighlightKind::MarkupStrong);
        assert_kind(2, "inline code", HighlightKind::MarkupRaw);
        assert_kind(2, "reference", HighlightKind::MarkupReference);
        assert_kind(2, "https://example.com", HighlightKind::MarkupLink);
    }

    #[test]
    fn markdown_fences_use_all_bundled_child_grammars() {
        let source = "```rust\nfn main() { let value = 42; }\n```\n\n```yaml\nenabled: true\n```\n";
        let document = highlight(HighlightRequest {
            source,
            path: Some("README.md"),
            fence_tag: None,
        })
        .unwrap();
        assert!(
            document.lines[1]
                .iter()
                .any(|span| span.kind == HighlightKind::Keyword),
            "Rust fence was not injected: {:?}",
            document.lines[1]
        );
        assert!(
            document.lines[5].iter().any(|span| matches!(
                span.kind,
                HighlightKind::Property | HighlightKind::Boolean | HighlightKind::String
            )),
            "YAML fence was not injected: {:?}",
            document.lines[5]
        );
    }
}
