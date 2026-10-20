//! The tree-sitter grammars and highlight queries (`highlight` feature).

use std::sync::atomic::AtomicUsize;

use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

use super::*;

pub(crate) fn highlight_detected(
    request: HighlightRequest<'_>,
    language: LanguageId,
    limits: HighlightLimits,
    cancellation_flag: Option<&AtomicUsize>,
) -> Result<HighlightedDocument, HighlightError> {
    let primary_configuration = cached_configuration(language)?;
    let injected = injected_languages(language);
    let markdown_inline = if language == LanguageId::Markdown {
        Some(cached_markdown_inline_configuration()?)
    } else {
        None
    };
    let mut highlighter = Highlighter::new();
    let events = highlighter
        .highlight(
            primary_configuration,
            request.source.as_bytes(),
            cancellation_flag,
            |name| {
                if name == "markdown_inline" {
                    return markdown_inline;
                }
                let language = language_for_alias(name)?;
                if !injected.contains(&language) {
                    return None;
                }
                cached_configuration(language).ok()
            },
        )
        .map_err(|error| HighlightError::Parser(error.to_string()))?;

    let mut active = Vec::new();
    let mut spans = Vec::new();
    for event in events {
        match event.map_err(|error| HighlightError::Parser(error.to_string()))? {
            HighlightEvent::HighlightStart(highlight) => active.push(CAPTURE_KINDS[highlight.0]),
            HighlightEvent::HighlightEnd => {
                active.pop();
            }
            HighlightEvent::Source { start, end } => {
                if let Some(kind) = active.iter().copied().max_by_key(|kind| kind.precedence()) {
                    spans.push(HighlightSpan {
                        range: start..end,
                        kind,
                    });
                    if spans.len() > limits.max_spans {
                        return Err(HighlightError::TooManySpans);
                    }
                }
            }
        }
    }
    HighlightedDocument::from_absolute_spans(language, request.source, spans)
}

pub(crate) fn injected_languages(parent: LanguageId) -> Vec<LanguageId> {
    use LanguageId::*;
    match parent {
        Html => vec![JavaScript, Css, Json],
        Markdown => vec![
            Rust, JavaScript, Jsx, TypeScript, Tsx, Python, Go, Json, Jsonc, Bash, Toml, Html, Css,
            Yaml, C, Cpp, CSharp, Java, Kotlin, Swift, Ruby, Php, Sql, Lua, Dockerfile, Nix, Make,
        ],
        Dockerfile => vec![Bash, Json, Yaml, Toml],
        _ => Vec::new(),
    }
}

/// Compiled queries contain no document/theme state and are immutable after
/// capture configuration. Keep one per used grammar instead of recompiling
/// for every fence or eagerly compiling all 27 Markdown injection targets.
/// Each grammar has its own cell: concurrent requests compile it only once,
/// while unrelated languages never wait on a global compilation lock.
pub(crate) fn cached_configuration(
    language: LanguageId,
) -> Result<&'static HighlightConfiguration, HighlightError> {
    macro_rules! registry {
        ($($variant:ident),+ $(,)?) => {
            match language {
                $(LanguageId::$variant => {
                    static CONFIG: std::sync::OnceLock<Result<HighlightConfiguration, HighlightError>> =
                        std::sync::OnceLock::new();
                    CONFIG.get_or_init(|| {
                        let mut config = configuration(language)?;
                        config.configure(CAPTURE_NAMES);
                        Ok(config)
                    }).as_ref().map_err(Clone::clone)
                }),+
            }
        };
    }
    registry!(
        Rust, JavaScript, Jsx, TypeScript, Tsx, Python, Go, Json, Jsonc, Bash, Toml, Markdown,
        Html, Css, Yaml, C, Cpp, CSharp, Java, Kotlin, Swift, Ruby, Php, Sql, Lua, Dockerfile, Nix,
        Make,
    )
}

pub(crate) fn cached_markdown_inline_configuration()
-> Result<&'static HighlightConfiguration, HighlightError> {
    static CONFIG: std::sync::OnceLock<Result<HighlightConfiguration, HighlightError>> =
        std::sync::OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let mut config = markdown_inline_configuration()?;
            config.configure(CAPTURE_NAMES);
            Ok(config)
        })
        .as_ref()
        .map_err(Clone::clone)
}

pub(crate) fn rust_configuration() -> Result<HighlightConfiguration, HighlightError> {
    // The upstream Rust query groups numbers and booleans as
    // `constant.builtin`. OpenAgents preserves those structural roles separately.
    let highlights = tree_sitter_rust::HIGHLIGHTS_QUERY
        .replace(
            "(boolean_literal) @constant.builtin",
            "(boolean_literal) @boolean",
        )
        .replace(
            "(integer_literal) @constant.builtin",
            "(integer_literal) @number",
        )
        .replace(
            "(float_literal) @constant.builtin",
            "(float_literal) @number",
        );
    HighlightConfiguration::new(
        tree_sitter_rust::LANGUAGE.into(),
        "rust",
        &highlights,
        tree_sitter_rust::INJECTIONS_QUERY,
        "",
    )
    .map_err(|error| HighlightError::Parser(error.to_string()))
}

pub(crate) fn markdown_configuration() -> Result<HighlightConfiguration, HighlightError> {
    // tree-sitter-highlight excludes child ranges from injections by default.
    // The Markdown block grammar's `inline` node owns anonymous children that
    // cover its source, so the upstream query otherwise injects an empty range.
    let injections = tree_sitter_md::INJECTION_QUERY_BLOCK.replace(
        "((inline) @injection.content\n  (#set! injection.language \"markdown_inline\"))",
        "((inline) @injection.content\n  (#set! injection.language \"markdown_inline\")\n  (#set! injection.include-children))",
    );
    make_configuration(
        tree_sitter_md::LANGUAGE.into(),
        "markdown",
        tree_sitter_md::HIGHLIGHT_QUERY_BLOCK,
        &injections,
        "",
    )
}

pub(crate) fn markdown_inline_configuration() -> Result<HighlightConfiguration, HighlightError> {
    make_configuration(
        tree_sitter_md::INLINE_LANGUAGE.into(),
        "markdown_inline",
        tree_sitter_md::HIGHLIGHT_QUERY_INLINE,
        tree_sitter_md::INJECTION_QUERY_INLINE,
        "",
    )
}

pub(crate) fn make_configuration(
    language: tree_sitter::Language,
    name: &str,
    highlights: &str,
    injections: &str,
    locals: &str,
) -> Result<HighlightConfiguration, HighlightError> {
    HighlightConfiguration::new(language, name, highlights, injections, locals)
        .map_err(|error| HighlightError::Parser(error.to_string()))
}

pub(crate) fn javascript_family_highlights(language: LanguageId) -> String {
    use LanguageId::*;

    let queries = match language {
        JavaScript => &[tree_sitter_javascript::HIGHLIGHT_QUERY][..],
        Jsx => &[
            tree_sitter_javascript::HIGHLIGHT_QUERY,
            tree_sitter_javascript::JSX_HIGHLIGHT_QUERY,
        ][..],
        TypeScript => &[
            tree_sitter_javascript::HIGHLIGHT_QUERY,
            tree_sitter_typescript::HIGHLIGHTS_QUERY,
        ][..],
        Tsx => &[
            tree_sitter_javascript::HIGHLIGHT_QUERY,
            tree_sitter_javascript::JSX_HIGHLIGHT_QUERY,
            tree_sitter_typescript::HIGHLIGHTS_QUERY,
        ][..],
        _ => unreachable!("JavaScript query composition requires a JavaScript-family language"),
    };
    queries.join("\n")
}

pub(crate) fn javascript_family_configuration(
    language: LanguageId,
) -> Result<HighlightConfiguration, HighlightError> {
    use LanguageId::*;

    let highlights = javascript_family_highlights(language);
    let (grammar, name, injections, locals) = match language {
        JavaScript => (
            tree_sitter_javascript::LANGUAGE.into(),
            "javascript",
            tree_sitter_javascript::INJECTIONS_QUERY,
            tree_sitter_javascript::LOCALS_QUERY,
        ),
        Jsx => (
            tree_sitter_javascript::LANGUAGE.into(),
            "jsx",
            tree_sitter_javascript::INJECTIONS_QUERY,
            tree_sitter_javascript::LOCALS_QUERY,
        ),
        TypeScript => (
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            "typescript",
            "",
            tree_sitter_typescript::LOCALS_QUERY,
        ),
        Tsx => (
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            "tsx",
            "",
            tree_sitter_typescript::LOCALS_QUERY,
        ),
        _ => unreachable!("JavaScript configuration requires a JavaScript-family language"),
    };
    make_configuration(grammar, name, &highlights, injections, locals)
}

pub(crate) fn configuration(
    language: LanguageId,
) -> Result<HighlightConfiguration, HighlightError> {
    use LanguageId::*;
    match language {
        Rust => rust_configuration(),
        JavaScript | Jsx | TypeScript | Tsx => javascript_family_configuration(language),
        Python => make_configuration(
            tree_sitter_python::LANGUAGE.into(),
            "python",
            tree_sitter_python::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Go => make_configuration(
            tree_sitter_go::LANGUAGE.into(),
            "go",
            tree_sitter_go::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Json | Jsonc => make_configuration(
            tree_sitter_json::LANGUAGE.into(),
            "json",
            tree_sitter_json::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Bash => make_configuration(
            tree_sitter_bash::LANGUAGE.into(),
            "bash",
            tree_sitter_bash::HIGHLIGHT_QUERY,
            "",
            "",
        ),
        Toml => make_configuration(
            tree_sitter_toml_ng::LANGUAGE.into(),
            "toml",
            tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Markdown => markdown_configuration(),
        Html => make_configuration(
            tree_sitter_html::LANGUAGE.into(),
            "html",
            tree_sitter_html::HIGHLIGHTS_QUERY,
            tree_sitter_html::INJECTIONS_QUERY,
            "",
        ),
        Css => make_configuration(
            tree_sitter_css::LANGUAGE.into(),
            "css",
            tree_sitter_css::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Yaml => make_configuration(
            tree_sitter_yaml::LANGUAGE.into(),
            "yaml",
            tree_sitter_yaml::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        C => make_configuration(
            tree_sitter_c::LANGUAGE.into(),
            "c",
            tree_sitter_c::HIGHLIGHT_QUERY,
            "",
            "",
        ),
        Cpp => make_configuration(
            tree_sitter_cpp::LANGUAGE.into(),
            "cpp",
            &format!(
                "{}\n{}",
                tree_sitter_c::HIGHLIGHT_QUERY,
                tree_sitter_cpp::HIGHLIGHT_QUERY
            ),
            "",
            "",
        ),
        CSharp => make_configuration(
            tree_sitter_c_sharp::LANGUAGE.into(),
            "csharp",
            tree_sitter_c_sharp::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Java => make_configuration(
            tree_sitter_java::LANGUAGE.into(),
            "java",
            tree_sitter_java::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Kotlin => make_configuration(
            tree_sitter_kotlin_ng::LANGUAGE.into(),
            "kotlin",
            include_str!("../queries/kotlin/highlights.scm"),
            "",
            "",
        ),
        Swift => make_configuration(
            tree_sitter_swift::LANGUAGE.into(),
            "swift",
            tree_sitter_swift::HIGHLIGHTS_QUERY,
            "",
            tree_sitter_swift::LOCALS_QUERY,
        ),
        Ruby => make_configuration(
            tree_sitter_ruby::LANGUAGE.into(),
            "ruby",
            tree_sitter_ruby::HIGHLIGHTS_QUERY,
            "",
            tree_sitter_ruby::LOCALS_QUERY,
        ),
        Php => make_configuration(
            tree_sitter_php::LANGUAGE_PHP.into(),
            "php",
            tree_sitter_php::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Sql => make_configuration(
            tree_sitter_sequel::LANGUAGE.into(),
            "sql",
            tree_sitter_sequel::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Lua => make_configuration(
            tree_sitter_lua::LANGUAGE.into(),
            "lua",
            tree_sitter_lua::HIGHLIGHTS_QUERY,
            "",
            tree_sitter_lua::LOCALS_QUERY,
        ),
        Nix => make_configuration(
            tree_sitter_nix::LANGUAGE.into(),
            "nix",
            tree_sitter_nix::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Make => make_configuration(
            tree_sitter_make::LANGUAGE.into(),
            "make",
            tree_sitter_make::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Dockerfile => make_configuration(
            tree_sitter_containerfile::LANGUAGE.into(),
            "dockerfile",
            tree_sitter_containerfile::HIGHLIGHTS_QUERY,
            tree_sitter_containerfile::INJECTIONS_QUERY,
            "",
        ),
    }
}

// Ordered from generic to specific. `HighlightConfiguration::configure`
// resolves dotted captures to the best recognized name in this table.
pub(crate) const CAPTURE_NAMES: &[&str] = &[
    "comment",
    "keyword",
    "string",
    "string.special",
    "string.escape",
    "number",
    "boolean",
    "type",
    "type.builtin",
    "constructor",
    "function",
    "function.builtin",
    "function.macro",
    "property",
    "constant",
    "variable",
    "variable.builtin",
    "variable.parameter",
    "operator",
    "punctuation",
    "tag",
    "attribute",
    "label",
    "text.title",
    "text.literal",
    "text.uri",
    "text.reference",
    "text.emphasis",
    "text.strong",
    "embedded",
    "error",
];

pub(crate) const CAPTURE_KINDS: &[HighlightKind] = &[
    HighlightKind::Comment,
    HighlightKind::Keyword,
    HighlightKind::String,
    HighlightKind::StringSpecial,
    HighlightKind::Escape,
    HighlightKind::Number,
    HighlightKind::Boolean,
    HighlightKind::Type,
    HighlightKind::TypeBuiltin,
    HighlightKind::Constructor,
    HighlightKind::Function,
    HighlightKind::FunctionBuiltin,
    HighlightKind::Macro,
    HighlightKind::Property,
    HighlightKind::Constant,
    HighlightKind::Variable,
    HighlightKind::VariableSpecial,
    HighlightKind::Parameter,
    HighlightKind::Operator,
    HighlightKind::Punctuation,
    HighlightKind::Tag,
    HighlightKind::Attribute,
    HighlightKind::Label,
    HighlightKind::MarkupHeading,
    HighlightKind::MarkupRaw,
    HighlightKind::MarkupLink,
    HighlightKind::MarkupReference,
    HighlightKind::MarkupEmphasis,
    HighlightKind::MarkupStrong,
    HighlightKind::Embedded,
    HighlightKind::Invalid,
];
