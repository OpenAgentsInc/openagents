use crate::buffer::TextBuffer;
use wgpui::{Hsla, theme};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyntaxLanguage {
    Rust,
}

impl SyntaxLanguage {
    pub fn from_path(path: &str) -> Option<Self> {
        let ext = path.rsplit('.').next().unwrap_or("");
        if ext.eq_ignore_ascii_case("rs") {
            Some(Self::Rust)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct HighlightSpan {
    pub start_col: usize,
    pub end_col: usize,
    pub color: Hsla,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HighlightKind {
    Comment,
    String,
    Number,
    Keyword,
    Type,
    Function,
    Macro,
    Boolean,
}

impl HighlightKind {
    fn color(self) -> Hsla {
        match self {
            HighlightKind::Comment => theme::text::MUTED.with_alpha(0.8),
            HighlightKind::String => theme::accent::GREEN,
            HighlightKind::Number => theme::accent::BLUE,
            HighlightKind::Keyword => theme::accent::PRIMARY,
            HighlightKind::Type => theme::accent::PURPLE,
            HighlightKind::Function => theme::accent::SECONDARY,
            HighlightKind::Macro => theme::accent::RED,
            HighlightKind::Boolean => theme::accent::BLUE,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct RawSpan {
    start: usize,
    end: usize,
    kind: HighlightKind,
}

fn push_span_by_line(
    spans_by_line: &mut [Vec<HighlightSpan>],
    buffer: &TextBuffer,
    span: RawSpan,
) {
    if span.start >= span.end {
        return;
    }
    let start = buffer.byte_to_position(span.start);
    let end = buffer.byte_to_position(span.end);
    if start.line >= spans_by_line.len() {
        return;
    }

    let color = span.kind.color();
    if start.line == end.line {
        if end.column > start.column {
            spans_by_line[start.line].push(HighlightSpan {
                start_col: start.column,
                end_col: end.column,
                color,
            });
        }
        return;
    }

    let start_line_len = buffer.line_len(start.line);
    if start.column < start_line_len {
        spans_by_line[start.line].push(HighlightSpan {
            start_col: start.column,
            end_col: start_line_len,
            color,
        });
    }

    let last_line = end.line.min(spans_by_line.len().saturating_sub(1));
    for line in (start.line + 1)..last_line {
        let line_len = buffer.line_len(line);
        if line_len > 0 {
            spans_by_line[line].push(HighlightSpan {
                start_col: 0,
                end_col: line_len,
                color,
            });
        }
    }

    if end.line < spans_by_line.len() && end.column > 0 {
        spans_by_line[end.line].push(HighlightSpan {
            start_col: 0,
            end_col: end.column,
            color,
        });
    }
}

fn normalize_spans(spans: &mut Vec<HighlightSpan>) {
    spans.sort_by(|a, b| {
        match a.start_col.cmp(&b.start_col) {
            std::cmp::Ordering::Equal => b.end_col.cmp(&a.end_col),
            other => other,
        }
    });

    let mut normalized: Vec<HighlightSpan> = Vec::with_capacity(spans.len());
    for span in spans.drain(..) {
        if span.start_col >= span.end_col {
            continue;
        }
        if let Some(last) = normalized.last_mut() {
            if span.start_col < last.end_col {
                if span.end_col <= last.end_col {
                    continue;
                }
                let trimmed = HighlightSpan {
                    start_col: last.end_col,
                    end_col: span.end_col,
                    color: span.color,
                };
                if trimmed.start_col < trimmed.end_col {
                    normalized.push(trimmed);
                }
                continue;
            }
        }
        normalized.push(span);
    }

    *spans = normalized;
}

fn scan_rust_simple(source: &str) -> Vec<RawSpan> {
    const KEYWORDS: &[&str] = &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut",
        "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait", "type",
        "unsafe", "use", "where", "while", "yield",
    ];
    const PRIMITIVES: &[&str] = &[
        "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
        "f32", "f64", "bool", "char", "str",
    ];

    let bytes = source.as_bytes();
    let mut spans = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'/' && i + 1 < bytes.len() {
            let next = bytes[i + 1];
            if next == b'/' {
                let start = i;
                i += 2;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                spans.push(RawSpan {
                    start,
                    end: i,
                    kind: HighlightKind::Comment,
                });
                continue;
            }
            if next == b'*' {
                let start = i;
                i += 2;
                while i + 1 < bytes.len() {
                    if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
                spans.push(RawSpan {
                    start,
                    end: i,
                    kind: HighlightKind::Comment,
                });
                continue;
            }
        }

        if let Some((start, end)) = scan_raw_or_byte_string(bytes, i) {
            spans.push(RawSpan {
                start,
                end,
                kind: HighlightKind::String,
            });
            i = end;
            continue;
        }

        if b == b'"' {
            let start = i;
            let end = scan_quoted(bytes, i, b'"', false);
            spans.push(RawSpan {
                start,
                end,
                kind: HighlightKind::String,
            });
            i = end;
            continue;
        }

        if b == b'\'' {
            let start = i;
            let end = scan_quoted(bytes, i, b'\'', false);
            spans.push(RawSpan {
                start,
                end,
                kind: HighlightKind::String,
            });
            i = end;
            continue;
        }

        if b.is_ascii_digit() {
            let start = i;
            i += 1;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'.')
            {
                i += 1;
            }
            spans.push(RawSpan {
                start,
                end: i,
                kind: HighlightKind::Number,
            });
            continue;
        }

        if is_ident_start(b) {
            let start = i;
            i += 1;
            while i < bytes.len() && is_ident_continue(bytes[i]) {
                i += 1;
            }
            let ident = &source[start..i];
            let kind = if i < bytes.len() && bytes[i] == b'!' {
                Some(HighlightKind::Macro)
            } else if ident == "true" || ident == "false" {
                Some(HighlightKind::Boolean)
            } else if KEYWORDS.contains(&ident) {
                Some(HighlightKind::Keyword)
            } else if is_followed_by_call(bytes, i) {
                Some(HighlightKind::Function)
            } else if PRIMITIVES.contains(&ident) || ident.as_bytes()[0].is_ascii_uppercase() {
                Some(HighlightKind::Type)
            } else {
                None
            };
            if let Some(kind) = kind {
                spans.push(RawSpan {
                    start,
                    end: i,
                    kind,
                });
            }
            continue;
        }

        i += 1;
    }

    spans
}

fn scan_raw_or_byte_string(bytes: &[u8], start: usize) -> Option<(usize, usize)> {
    let len = bytes.len();
    if start >= len {
        return None;
    }
    if bytes[start] == b'r' {
        return scan_raw_string(bytes, start, 1);
    }
    if bytes[start] == b'b' && start + 1 < len {
        if bytes[start + 1] == b'"' {
            let end = scan_quoted(bytes, start + 1, b'"', false);
            return Some((start, end));
        }
        if bytes[start + 1] == b'\'' {
            let end = scan_quoted(bytes, start + 1, b'\'', false);
            return Some((start, end));
        }
        if bytes[start + 1] == b'r' {
            return scan_raw_string(bytes, start, 2);
        }
    }
    None
}

fn scan_raw_string(bytes: &[u8], start: usize, prefix_len: usize) -> Option<(usize, usize)> {
    let len = bytes.len();
    if start + prefix_len > len {
        return None;
    }
    if bytes[start + prefix_len - 1] != b'r' {
        return None;
    }
    let mut i = start + prefix_len;
    let mut hash_count = 0usize;
    while i < len && bytes[i] == b'#' {
        hash_count += 1;
        i += 1;
    }
    if i >= len || bytes[i] != b'"' {
        return None;
    }
    i += 1;
    let mut j = i;
    while j < len {
        if bytes[j] == b'"' {
            let mut k = j + 1;
            let mut matched = 0usize;
            while matched < hash_count && k < len && bytes[k] == b'#' {
                matched += 1;
                k += 1;
            }
            if matched == hash_count {
                return Some((start, k));
            }
        }
        j += 1;
    }
    Some((start, len))
}

fn scan_quoted(bytes: &[u8], start: usize, quote: u8, allow_newline: bool) -> usize {
    let len = bytes.len();
    let mut i = start + 1;
    while i < len {
        if bytes[i] == b'\\' {
            i = (i + 2).min(len);
            continue;
        }
        if !allow_newline && bytes[i] == b'\n' {
            break;
        }
        if bytes[i] == quote {
            i += 1;
            break;
        }
        i += 1;
    }
    i.min(len)
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn is_followed_by_call(bytes: &[u8], mut i: usize) -> bool {
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    i < bytes.len() && bytes[i] == b'('
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::{HighlightKind, HighlightSpan, RawSpan, SyntaxLanguage, normalize_spans, push_span_by_line, scan_rust_simple};
    use crate::buffer::TextBuffer;
    use tree_sitter::{Node, Parser, Tree};

    pub struct SyntaxHighlighter {
        parser: Parser,
        tree: Option<Tree>,
        last_revision: u64,
        spans_by_line: Vec<Vec<HighlightSpan>>,
    }

    impl SyntaxHighlighter {
        pub fn new(language: SyntaxLanguage) -> Option<Self> {
            let mut parser = Parser::new();
            parser.set_language(&language.language()).ok()?;
            Some(Self {
                parser,
                tree: None,
                last_revision: u64::MAX,
                spans_by_line: Vec::new(),
            })
        }

        pub fn update(&mut self, buffer: &TextBuffer, revision: u64) {
            if revision == self.last_revision {
                return;
            }
            self.last_revision = revision;

            let source = buffer.text();
            self.tree = self.parser.parse(&source, self.tree.as_ref());
            self.spans_by_line.clear();
            self.spans_by_line.resize_with(buffer.line_count(), Vec::new);

            let mut spans = scan_rust_simple(&source);
            if let Some(tree) = &self.tree {
                collect_tree_spans(tree.root_node(), &mut spans);
            }

            for span in spans {
                push_span_by_line(&mut self.spans_by_line, buffer, span);
            }
            for line in &mut self.spans_by_line {
                normalize_spans(line);
            }
        }

        pub fn spans_for_line(&self, line: usize) -> Option<&[HighlightSpan]> {
            self.spans_by_line.get(line).map(|line| line.as_slice())
        }
    }

    impl SyntaxLanguage {
        fn language(self) -> tree_sitter::Language {
            match self {
                SyntaxLanguage::Rust => tree_sitter_rust::LANGUAGE.into(),
            }
        }
    }

    fn collect_tree_spans(node: Node, spans: &mut Vec<RawSpan>) {
        let kind = node.kind();
        if let Some(kind) = kind_for_node(kind) {
            spans.push(RawSpan {
                start: node.start_byte(),
                end: node.end_byte(),
                kind,
            });
        }

        if kind == "function_item" {
            if let Some(name) = node.child_by_field_name("name") {
                spans.push(RawSpan {
                    start: name.start_byte(),
                    end: name.end_byte(),
                    kind: HighlightKind::Function,
                });
            }
        }

        if kind == "macro_invocation" {
            if let Some(name) = node.child_by_field_name("macro") {
                spans.push(RawSpan {
                    start: name.start_byte(),
                    end: name.end_byte(),
                    kind: HighlightKind::Macro,
                });
            }
        }

        if kind == "macro_definition" {
            if let Some(name) = node.child_by_field_name("name") {
                spans.push(RawSpan {
                    start: name.start_byte(),
                    end: name.end_byte(),
                    kind: HighlightKind::Macro,
                });
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            collect_tree_spans(child, spans);
        }
    }

    fn kind_for_node(kind: &str) -> Option<HighlightKind> {
        match kind {
            "line_comment"
            | "block_comment"
            | "inner_line_doc_comment"
            | "outer_line_doc_comment"
            | "inner_block_doc_comment"
            | "outer_block_doc_comment" => Some(HighlightKind::Comment),
            "string_literal"
            | "raw_string_literal"
            | "char_literal"
            | "byte_string_literal"
            | "raw_byte_string_literal"
            | "byte_literal" => Some(HighlightKind::String),
            "float_literal" | "integer_literal" => Some(HighlightKind::Number),
            "type_identifier" | "primitive_type" => Some(HighlightKind::Type),
            _ => None,
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use super::{HighlightSpan, SyntaxLanguage, normalize_spans, push_span_by_line, scan_rust_simple};
    use crate::buffer::TextBuffer;

    pub struct SyntaxHighlighter {
        language: SyntaxLanguage,
        last_revision: u64,
        spans_by_line: Vec<Vec<HighlightSpan>>,
    }

    impl SyntaxHighlighter {
        pub fn new(language: SyntaxLanguage) -> Option<Self> {
            Some(Self {
                language,
                last_revision: u64::MAX,
                spans_by_line: Vec::new(),
            })
        }

        pub fn update(&mut self, buffer: &TextBuffer, revision: u64) {
            if revision == self.last_revision {
                return;
            }
            self.last_revision = revision;

            let source = buffer.text();
            let spans = match self.language {
                SyntaxLanguage::Rust => scan_rust_simple(&source),
            };

            self.spans_by_line.clear();
            self.spans_by_line.resize_with(buffer.line_count(), Vec::new);
            for span in spans {
                push_span_by_line(&mut self.spans_by_line, buffer, span);
            }
            for line in &mut self.spans_by_line {
                normalize_spans(line);
            }
        }

        pub fn spans_for_line(&self, line: usize) -> Option<&[HighlightSpan]> {
            self.spans_by_line.get(line).map(|line| line.as_slice())
        }
    }
}

pub use imp::SyntaxHighlighter;
