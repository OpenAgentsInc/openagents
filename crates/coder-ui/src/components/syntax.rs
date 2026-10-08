//! Portable syntax runs from public Grok Night theme and two-face grammars.

use crate::{components::run, source_theme as t};
use rust_native::{style::Color, view::RichRun};
use std::{io::Cursor, sync::OnceLock};
use syntect::{
    easy::HighlightLines,
    highlighting::{Theme, ThemeSet},
    parsing::{SyntaxDefinition, SyntaxReference, SyntaxSet},
};

fn syntax() -> &'static (SyntaxSet, Theme) {
    static SYNTAX: OnceLock<(SyntaxSet, Theme)> = OnceLock::new();
    SYNTAX.get_or_init(|| {
        let theme = ThemeSet::load_from_reader(&mut Cursor::new(include_bytes!(
            "../../../code-highlight/assets/grok-build/grok-night.tmTheme"
        )))
        .expect("the bundled public theme is valid");
        let swift = SyntaxDefinition::load_from_str(
            include_str!("../../../code-highlight/assets/grok-build/Swift.sublime-syntax"),
            true,
            None,
        )
        .expect("the bundled public Swift grammar is valid");
        let mut builder = two_face::syntax::extra_newlines().into_builder();
        builder.add(swift);
        (builder.build(), theme)
    })
}

pub fn lines(source: &str, language: &str) -> Vec<Vec<RichRun>> {
    let (syntaxes, theme) = syntax();
    let Some(grammar) = grammar(syntaxes, language) else {
        return source
            .split('\n')
            .map(|line| vec![run(line, t::TEXT_SECONDARY)])
            .collect();
    };
    let mut highlighter = HighlightLines::new(grammar, theme);
    source
        .split_inclusive('\n')
        .map(|line| {
            highlighter
                .highlight_line(line, syntaxes)
                .map(|pieces| {
                    pieces
                        .into_iter()
                        .map(|(style, text)| {
                            let mut piece = run(
                                text.trim_end_matches(['\n', '\r']),
                                t::remap(Color::rgb(
                                    style.foreground.r,
                                    style.foreground.g,
                                    style.foreground.b,
                                )),
                            );
                            piece.bold = style
                                .font_style
                                .contains(syntect::highlighting::FontStyle::BOLD);
                            piece.italic = style
                                .font_style
                                .contains(syntect::highlighting::FontStyle::ITALIC);
                            piece.underline = style
                                .font_style
                                .contains(syntect::highlighting::FontStyle::UNDERLINE);
                            piece
                        })
                        .collect()
                })
                .unwrap_or_else(|_| vec![run(line, t::TEXT_SECONDARY)])
        })
        .collect()
}

pub fn known(language: &str) -> bool {
    let (syntaxes, _) = syntax();
    grammar(syntaxes, language).is_some()
}

fn grammar<'a>(syntaxes: &'a SyntaxSet, language: &str) -> Option<&'a SyntaxReference> {
    let mut citation = language.splitn(3, ':');
    let first = citation.next().unwrap_or_default();
    let second = citation.next();
    let path = citation.next();
    let token = match (second, path) {
        (Some(second), Some(path))
            if !first.is_empty()
                && first.bytes().all(|b| b.is_ascii_digit())
                && !second.is_empty()
                && second.bytes().all(|b| b.is_ascii_digit()) =>
        {
            path
        }
        _ => language,
    };
    let extension = token.rsplit('.').next().unwrap_or(token);
    if token.eq_ignore_ascii_case("swift") || extension.eq_ignore_ascii_case("swift") {
        return syntaxes.syntaxes().iter().rev().find(|syntax| {
            syntax
                .file_extensions
                .iter()
                .any(|ext| ext.eq_ignore_ascii_case("swift"))
        });
    }
    syntaxes
        .find_syntax_by_token(token)
        .or_else(|| syntaxes.find_syntax_by_extension(extension))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_grammar_and_citation_keep_source_spans() {
        for language in ["rust", "1:3:src/main.rs", "swift"] {
            assert!(known(language));
            let source = if language == "swift" {
                "let value: String = \"hello\""
            } else {
                "let value = \"hello\";"
            };
            let rows = lines(source, language);
            assert_eq!(
                rows.iter()
                    .flatten()
                    .map(|r| r.text.as_str())
                    .collect::<String>(),
                source
            );
            assert!(
                rows.iter()
                    .flatten()
                    .any(|r| r.foreground != Some(t::TEXT_SECONDARY))
            );
        }
    }
}
