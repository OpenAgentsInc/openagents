//! Optional Rust syntax metadata. This does not resolve names or expand macros.
use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use tree_sitter::{Node, Parser};

pub const MAX_DECLARATIONS: usize = 1024;
const MAX_NODES: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub parser: String,
    pub parser_version: String,
    pub grammar: String,
    pub grammar_version: String,
    pub extractor_version: String,
}
impl Default for Provenance {
    fn default() -> Self {
        Self {
            parser: "tree-sitter".into(),
            parser_version: "0.27.0".into(),
            grammar: "tree-sitter-rust".into(),
            grammar_version: "0.24.2".into(),
            extractor_version: "briefing-lab-rust-v1".into(),
        }
    }
}
/// Bytes are zero-based and half-open. Lines are one-based and inclusive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub end_line: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Declaration {
    pub kind: String,
    pub name: String,
    pub qualified_name: String,
    pub declaration: Span,
    pub signature: Span,
    pub body: Option<Span>,
    pub parse_has_error: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSyntax {
    pub declarations: Vec<Declaration>,
    pub parse_has_error: bool,
    pub error_nodes: usize,
    pub missing_nodes: usize,
    pub omitted_declarations: usize,
    pub traversal_limited: bool,
    pub limitations: Vec<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Selection {
    pub declaration: Declaration,
    pub match_kind: String,
    pub equally_ranked_matches: usize,
    pub partial: bool,
}

fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(
            text.bytes()
                .enumerate()
                .filter_map(|(i, b)| (b == b'\n').then_some(i + 1)),
        )
        .collect()
}
fn span(starts: &[usize], start: usize, end: usize) -> Span {
    Span {
        start_byte: start,
        end_byte: end,
        start_line: starts.partition_point(|n| *n <= start),
        end_line: starts.partition_point(|n| *n <= end.saturating_sub(1).max(start)),
    }
}
fn type_name(node: Node<'_>, text: &str) -> String {
    fn lexical(node: Node<'_>, text: &str, depth: usize) -> String {
        if depth < 32 {
            if node.kind() == "generic_type" {
                if let Some(base) = node.child_by_field_name("type") {
                    return lexical(base, text, depth + 1);
                }
            }
            if matches!(node.kind(), "scoped_type_identifier" | "scoped_identifier") {
                if let Some(name) = node.child_by_field_name("name") {
                    let name = lexical(name, text, depth + 1);
                    return if let Some(path) = node.child_by_field_name("path") {
                        format!("{}::{name}", lexical(path, text, depth + 1))
                    } else {
                        name
                    };
                }
            }
        }
        text.get(node.byte_range())
            .unwrap_or("")
            .split_whitespace()
            .collect()
    }
    lexical(node, text, 0)
}

pub fn extract(text: &str) -> Result<FileSyntax> {
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_rust::LANGUAGE.into())?;
    let tree = parser
        .parse(text, None)
        .ok_or("Tree-sitter did not return a Rust syntax tree.")?;
    let starts = line_starts(text);
    let mut result = FileSyntax {
        declarations: Vec::new(), parse_has_error: tree.root_node().has_error(), error_nodes: 0, missing_nodes: 0,
        omitted_declarations: 0, traversal_limited: false,
        limitations: vec!["Syntax only: no macro expansion, cfg evaluation, imports, type resolution, or call graph.".into(),
            "Names are lexical scopes, not compiler-resolved identities. Separate attributes and comments are outside declaration spans.".into()],
    };
    let mut stack = vec![(tree.root_node(), Vec::<String>::new())];
    let mut visited = 0;
    while let Some((node, scope)) = stack.pop() {
        visited += 1;
        if visited > MAX_NODES {
            result.traversal_limited = true;
            break;
        }
        result.error_nodes += usize::from(node.is_error());
        result.missing_nodes += usize::from(node.is_missing());
        let kind = node.kind();
        let is_declaration = matches!(
            kind,
            "function_item"
                | "function_signature_item"
                | "struct_item"
                | "enum_item"
                | "trait_item"
                | "impl_item"
                | "mod_item"
                | "type_item"
                | "const_item"
                | "static_item"
                | "macro_definition"
        );
        let name = if kind == "impl_item" {
            node.child_by_field_name("type").map(|n| type_name(n, text))
        } else if is_declaration {
            node.child_by_field_name("name")
                .and_then(|n| text.get(n.byte_range()))
                .map(str::to_owned)
        } else {
            None
        };
        let mut nested_scope = scope.clone();
        if let Some(name) = name.filter(|n| !n.is_empty()) {
            let mut qualified = scope;
            qualified.push(name.clone());
            let body = node.child_by_field_name("body");
            if result.declarations.len() < MAX_DECLARATIONS {
                result.declarations.push(Declaration {
                    kind: kind.into(),
                    name: name.clone(),
                    qualified_name: qualified.join("::"),
                    declaration: span(&starts, node.start_byte(), node.end_byte()),
                    signature: span(
                        &starts,
                        node.start_byte(),
                        body.map_or(node.end_byte(), |b| b.start_byte()),
                    ),
                    body: body.map(|b| span(&starts, b.start_byte(), b.end_byte())),
                    parse_has_error: node.has_error() || node.is_missing(),
                });
            } else {
                result.omitted_declarations += 1;
            }
            if matches!(
                kind,
                "impl_item" | "mod_item" | "trait_item" | "function_item"
            ) {
                nested_scope = qualified;
            }
        }
        // Visit concrete children to count missing punctuation, but comments cannot become items.
        let mut cursor = node.walk();
        let children: Vec<_> = node.children(&mut cursor).collect();
        for child in children.into_iter().rev() {
            stack.push((child, nested_scope.clone()));
        }
    }
    if result.parse_has_error {
        result.limitations.push("The parser recovered from incomplete or invalid syntax. Declarations containing errors are excluded from structural selection.".into());
    }
    if result.traversal_limited || result.omitted_declarations > 0 {
        result
            .limitations
            .push("The declaration or traversal bound omitted syntax metadata.".into());
    }
    Ok(result)
}

pub fn identifiers(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | ':' | '#')))
        .map(|s| s.trim_matches(':'))
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

pub fn choose<'a>(
    file: &'a FileSyntax,
    identifiers: &BTreeSet<String>,
) -> Option<(&'a Declaration, &'static str, usize)> {
    let mut candidates: Vec<_> = file
        .declarations
        .iter()
        .filter(|d| !d.parse_has_error)
        .filter_map(|d| {
            if d.qualified_name.contains("::") && identifiers.contains(&d.qualified_name) {
                Some((2, d, "exact qualified name"))
            } else if identifiers.contains(&d.name) {
                Some((1, d, "exact identifier"))
            } else {
                None
            }
        })
        .collect();
    candidates.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.declaration.start_byte.cmp(&b.1.declaration.start_byte))
            .then_with(|| a.1.kind.cmp(&b.1.kind))
    });
    let (rank, declaration, kind) = *candidates.first()?;
    let ties = candidates.iter().filter(|(r, _, _)| *r == rank).count();
    Some((declaration, kind, ties))
}

fn valid_span(s: &Span, size: usize, lines: usize) -> bool {
    s.start_byte <= s.end_byte
        && s.end_byte <= size
        && s.start_line > 0
        && s.end_line >= s.start_line
        && s.end_line <= lines + usize::from(s.start_byte == size && s.end_byte == size)
}
pub fn validate(file: &FileSyntax, size: usize, lines: usize) -> Result<()> {
    if file.declarations.len() > MAX_DECLARATIONS {
        return Err("Cached syntax declaration limit exceeded; rebuild with --syntax.".into());
    }
    for d in &file.declarations {
        let parent = &d.declaration;
        if d.name.is_empty()
            || d.qualified_name.is_empty()
            || parent.start_byte == parent.end_byte
            || !valid_span(parent, size, lines)
            || !valid_span(&d.signature, size, lines)
            || d.signature.start_byte != parent.start_byte
            || d.signature.end_byte > parent.end_byte
            || d.body.as_ref().is_some_and(|body| {
                !valid_span(body, size, lines)
                    || body.start_byte < parent.start_byte
                    || body.end_byte > parent.end_byte
            })
        {
            return Err(
                "Cached syntax has an invalid declaration range; rebuild with --syntax.".into(),
            );
        }
    }
    Ok(())
}
/// Check cached byte boundaries and line coordinates against the selected Git source.
pub fn validate_text(file: &FileSyntax, text: &str) -> Result<()> {
    let starts = line_starts(text);
    for d in &file.declarations {
        for s in std::iter::once(&d.declaration)
            .chain(std::iter::once(&d.signature))
            .chain(d.body.iter())
        {
            if !text.is_char_boundary(s.start_byte)
                || !text.is_char_boundary(s.end_byte)
                || *s != span(&starts, s.start_byte, s.end_byte)
            {
                return Err(
                    "Cached syntax coordinates do not match source bytes; rebuild with --syntax."
                        .into(),
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiline_visibility_generics_and_impl_methods_have_structural_spans() {
        let text = "pub struct Store<T> { value: T }\nimpl<T> Store<T> {\n    pub(crate)\n    async unsafe fn\n    fetch_value<'a>(\n        &'a self,\n    ) -> &'a T\n    where T: Sync\n    {\n        &self.value\n    }\n}\n";
        let file = extract(text).unwrap();
        assert!(!file.parse_has_error);
        let method = file
            .declarations
            .iter()
            .find(|d| d.name == "fetch_value")
            .unwrap();
        assert_eq!(method.qualified_name, "Store::fetch_value");
        assert_eq!(method.declaration.start_line, 3);
        assert_eq!(method.declaration.end_line, 11);
        let signature = &text[method.signature.start_byte..method.signature.end_byte];
        assert!(signature.starts_with("pub(crate)\n"));
        assert!(signature.contains("async unsafe fn\n"));
        assert!(signature.contains("where T: Sync"));
        let body = method.body.as_ref().unwrap();
        assert_eq!(
            &text[body.start_byte..body.end_byte],
            "{\n        &self.value\n    }"
        );
        validate(&file, text.len(), text.lines().count()).unwrap();
        validate_text(&file, text).unwrap();
    }

    #[test]
    fn comments_strings_and_macro_tokens_do_not_become_declarations() {
        let text = "// pub fn phantom() {}\n/* struct Phantom {} */\nconst TEXT: &str = \"fn fake() {}\";\nmacro_rules! generate { () => { fn generated() {} }; }\nfn actual() {}\n";
        let file = extract(text).unwrap();
        assert!(!file.parse_has_error);
        let names: Vec<_> = file.declarations.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"actual"));
        for absent in ["phantom", "Phantom", "fake", "generated"] {
            assert!(!names.contains(&absent), "{absent}");
        }
    }

    #[test]
    fn qualified_names_win_without_splitting_into_generic_words() {
        let text = "struct First; struct Second;\nimpl First { fn repair_cache() {} }\nimpl Second { fn repair_cache() {} }\n";
        let file = extract(text).unwrap();
        let (selected, kind, ties) =
            choose(&file, &identifiers("Fix `Second::repair_cache` now")).unwrap();
        assert_eq!(selected.qualified_name, "Second::repair_cache");
        assert_eq!(kind, "exact qualified name");
        assert_eq!(ties, 1);
        assert!(choose(&file, &identifiers("repair cache behavior")).is_none());
        let (selected, _, ties) = choose(&file, &identifiers("repair_cache")).unwrap();
        assert_eq!(selected.qualified_name, "First::repair_cache");
        assert_eq!(ties, 2);
        assert!(choose(&file, &identifiers("second::repair_cache")).is_none());
    }

    #[test]
    fn incomplete_parse_is_reported_and_good_declarations_remain_available() {
        let text = "fn complete() {}\nfn incomplete( {\n";
        let file = extract(text).unwrap();
        assert!(file.parse_has_error);
        assert!(file.error_nodes + file.missing_nodes > 0);
        assert!(file.limitations.iter().any(|s| s.contains("recovered")));
        assert!(choose(&file, &identifiers("complete")).is_some());
        assert!(choose(&file, &identifiers("incomplete")).is_none());
    }

    #[test]
    fn trait_signatures_and_nested_functions_have_scope_and_optional_bodies() {
        let file =
            extract("mod worker { trait Runner { fn run(&self); } fn outer() { fn inner() {} } }")
                .unwrap();
        let signature = file.declarations.iter().find(|d| d.name == "run").unwrap();
        assert_eq!(signature.qualified_name, "worker::Runner::run");
        assert_eq!(signature.kind, "function_signature_item");
        assert!(signature.body.is_none());
        let inner = file
            .declarations
            .iter()
            .find(|d| d.name == "inner")
            .unwrap();
        assert_eq!(inner.qualified_name, "worker::outer::inner");
    }
}

#[cfg(test)]
mod qualified_impl_tests {
    use super::*;

    #[test]
    fn qualified_generic_impl_preserves_written_type_namespace() {
        let file =
            extract("impl<T> a::Store<T> { fn open() {} }\nimpl<T> b::Store<T> { fn open() {} }\n")
                .unwrap();
        assert!(!file.parse_has_error);
        let (declaration, kind, ties) = choose(&file, &identifiers("a::Store::open")).unwrap();
        assert_eq!(declaration.qualified_name, "a::Store::open");
        assert_eq!(kind, "exact qualified name");
        assert_eq!(ties, 1);
        assert!(choose(&file, &identifiers("Store::open")).is_none());
    }

    #[test]
    fn trait_impls_with_same_lexical_type_and_method_remain_ambiguous() {
        let file = extract("impl first::Named for a::Store { fn open() {} }\nimpl second::Named for a::Store { fn open() {} }\n").unwrap();
        assert!(!file.parse_has_error);
        let (declaration, _, ties) = choose(&file, &identifiers("a::Store::open")).unwrap();
        assert_eq!(declaration.qualified_name, "a::Store::open");
        assert_eq!(ties, 2);
    }
}
