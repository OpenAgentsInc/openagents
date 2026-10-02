//! Syntax classes for code blocks, from tree-sitter grammars.
//!
//! [`Grammars`] compiles each language's highlight query once and splits a
//! block into byte ranges, each with the [`Kind`] tree-sitter gave it (or
//! none, for plain text). How a kind looks is the caller's choice: the
//! native renderer paints colors, the terminal paints intensities.
//!
//! Unknown languages, inputs over [`MAX_BYTES`], and lines over
//! [`MAX_LINE`] bytes give no classes. No content leaves this process.

use std::collections::BTreeMap;
use std::sync::{LazyLock, Mutex};

use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

/// The largest block that is classified.
pub const MAX_BYTES: usize = 64 * 1024;
/// The longest line in a block that is classified.
pub const MAX_LINE: usize = 8192;

/// What a stretch of code is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Kind {
    Comment,
    String,
    StringSpecial,
    Escape,
    Keyword,
    Function,
    FunctionBuiltin,
    Macro,
    Type,
    TypeBuiltin,
    Constructor,
    Number,
    Boolean,
    Constant,
    Operator,
    Property,
    VariableSpecial,
    Tag,
    Attribute,
    Label,
    MarkupHeading,
    MarkupRaw,
    MarkupLink,
    MarkupReference,
    MarkupEmphasis,
    MarkupStrong,
    Invalid,
}

/// How many kinds there are.
pub const KINDS: usize = 27;

/// A byte range of a block and its kind; `None` is plain text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Class {
    pub start: usize,
    pub end: usize,
    pub kind: Option<Kind>,
}

const CAPTURES: [(&str, Kind); 30] = [
    ("comment", Kind::Comment),
    ("string", Kind::String),
    ("string.special", Kind::StringSpecial),
    ("escape", Kind::Escape),
    ("keyword", Kind::Keyword),
    ("function", Kind::Function),
    ("function.builtin", Kind::FunctionBuiltin),
    ("function.macro", Kind::Macro),
    ("type", Kind::Type),
    ("type.builtin", Kind::TypeBuiltin),
    ("constructor", Kind::Constructor),
    ("number", Kind::Number),
    ("boolean", Kind::Boolean),
    ("constant", Kind::Constant),
    ("operator", Kind::Operator),
    ("property", Kind::Property),
    ("variable.special", Kind::VariableSpecial),
    ("tag", Kind::Tag),
    ("attribute", Kind::Attribute),
    ("label", Kind::Label),
    ("markup.heading", Kind::MarkupHeading),
    ("markup.raw", Kind::MarkupRaw),
    ("markup.link", Kind::MarkupLink),
    ("markup.link.label", Kind::MarkupReference),
    ("markup.italic", Kind::MarkupEmphasis),
    ("markup.bold", Kind::MarkupStrong),
    ("error", Kind::Invalid),
    ("string.escape", Kind::Escape),
    ("markup.emphasis", Kind::MarkupEmphasis),
    ("markup.strong", Kind::MarkupStrong),
];

/// The compiled grammars. Building them takes a moment; keep one.
pub struct Grammars {
    configs: BTreeMap<&'static str, HighlightConfiguration>,
}

impl Default for Grammars {
    fn default() -> Self {
        let mut configs = BTreeMap::new();
        let names = CAPTURES.map(|(name, _)| name);
        // The upstream Rust query groups literals with constants. Preserve
        // their semantic roles so callers can draw them apart.
        let rust = tree_sitter_rust::HIGHLIGHTS_QUERY
            .replace(
                "(integer_literal) @constant.builtin",
                "(integer_literal) @number",
            )
            .replace(
                "(float_literal) @constant.builtin",
                "(float_literal) @number",
            )
            .replace(
                "(boolean_literal) @constant.builtin",
                "(boolean_literal) @boolean",
            );
        let cpp = format!(
            "{}\n{}",
            tree_sitter_c::HIGHLIGHT_QUERY,
            tree_sitter_cpp::HIGHLIGHT_QUERY
        );
        let typescript = format!(
            "{}\n{}",
            tree_sitter_javascript::HIGHLIGHT_QUERY,
            tree_sitter_typescript::HIGHLIGHTS_QUERY
        );
        for (name, language, query) in [
            ("rust", tree_sitter_rust::LANGUAGE, rust.as_str()),
            (
                "python",
                tree_sitter_python::LANGUAGE,
                tree_sitter_python::HIGHLIGHTS_QUERY,
            ),
            (
                "json",
                tree_sitter_json::LANGUAGE,
                tree_sitter_json::HIGHLIGHTS_QUERY,
            ),
            (
                "bash",
                tree_sitter_bash::LANGUAGE,
                tree_sitter_bash::HIGHLIGHT_QUERY,
            ),
            (
                "javascript",
                tree_sitter_javascript::LANGUAGE,
                tree_sitter_javascript::HIGHLIGHT_QUERY,
            ),
            (
                "typescript",
                tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
                typescript.as_str(),
            ),
            (
                "go",
                tree_sitter_go::LANGUAGE,
                tree_sitter_go::HIGHLIGHTS_QUERY,
            ),
            ("cpp", tree_sitter_cpp::LANGUAGE, cpp.as_str()),
            ("c", tree_sitter_c::LANGUAGE, tree_sitter_c::HIGHLIGHT_QUERY),
            (
                "html",
                tree_sitter_html::LANGUAGE,
                tree_sitter_html::HIGHLIGHTS_QUERY,
            ),
            (
                "css",
                tree_sitter_css::LANGUAGE,
                tree_sitter_css::HIGHLIGHTS_QUERY,
            ),
        ] {
            if let Ok(mut config) =
                HighlightConfiguration::new(language.into(), name, query, "", "")
            {
                config.configure(&names);
                configs.insert(name, config);
            }
        }
        Self { configs }
    }
}

impl Grammars {
    /// Whether `language` (a fence's first word, such as `rs` or `python`)
    /// has a grammar here.
    pub fn knows(&self, language: &str) -> bool {
        self.configs.contains_key(canonical(language))
    }

    /// `text` split into ranges covering it in order, each with its kind;
    /// adjacent ranges of one kind are joined. Empty when the language is
    /// unknown, the text is over the bounds, or the grammar fails.
    pub fn classes(&self, language: &str, text: &str) -> Vec<Class> {
        if text.len() > MAX_BYTES || text.lines().any(|line| line.len() > MAX_LINE) {
            return vec![];
        }
        let Some(config) = self.configs.get(canonical(language)) else {
            return vec![];
        };
        let mut highlighter = Highlighter::new();
        let Ok(events) = highlighter.highlight(config, text.as_bytes(), None, None, |_| None)
        else {
            return vec![];
        };
        let mut out: Vec<Class> = vec![];
        let mut stack = vec![];
        for event in events {
            match event {
                Ok(HighlightEvent::HighlightStart(value)) => stack.push(value.0),
                Ok(HighlightEvent::HighlightEnd) => {
                    stack.pop();
                }
                Ok(HighlightEvent::Source { start, end }) => {
                    let kind = stack
                        .last()
                        .and_then(|index| CAPTURES.get(*index))
                        .map(|(_, kind)| *kind);
                    if let Some(last) = out.last_mut()
                        && last.end == start
                        && last.kind == kind
                    {
                        last.end = end;
                    } else {
                        out.push(Class { start, end, kind });
                    }
                }
                Err(_) => return vec![],
            }
        }
        out
    }
}

/// [`Grammars::classes`] with one process-wide set of grammars, compiled
/// on first use.
pub fn classes(language: &str, text: &str) -> Vec<Class> {
    static SHARED: LazyLock<Mutex<Grammars>> = LazyLock::new(|| Mutex::new(Grammars::default()));
    if text.len() > MAX_BYTES || language.is_empty() {
        return vec![];
    }
    match SHARED.lock() {
        Ok(grammars) => grammars.classes(language, text),
        Err(_) => vec![],
    }
}

/// A fence's language name as the grammars know it.
fn canonical(language: &str) -> &str {
    match language {
        "rs" => "rust",
        "py" => "python",
        "js" | "jsx" => "javascript",
        "ts" => "typescript",
        "shell" | "sh" => "bash",
        "c++" | "cc" | "cxx" => "cpp",
        value => value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_cover_the_text_in_order() {
        let grammars = Grammars::default();
        let text = "// café 🦀\nfn main() { let s = \"héllo\"; }\n";
        let classes = grammars.classes("rs", text);
        let mut next = 0;
        for class in &classes {
            assert_eq!(class.start, next);
            next = class.end;
        }
        assert_eq!(next, text.len());
        let of = |kind| {
            classes
                .iter()
                .filter(|class| class.kind == Some(kind))
                .map(|class| &text[class.start..class.end])
                .collect::<Vec<_>>()
        };
        assert_eq!(of(Kind::Comment), ["// café 🦀"]);
        assert!(of(Kind::Keyword).contains(&"fn"));
        assert!(of(Kind::String).contains(&"\"héllo\""));
    }

    #[test]
    fn unknown_or_oversized_input_has_no_classes() {
        assert!(classes("unknown", "x").is_empty());
        assert!(classes("", "x").is_empty());
        assert!(classes("rust", &"a".repeat(MAX_BYTES + 1)).is_empty());
        assert!(classes("rust", &"a".repeat(MAX_LINE + 1)).is_empty());
        assert!(Grammars::default().knows("py"));
    }
}
