use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};

use super::types::*;
use crate::color::Hsla;
use crate::theme;

pub const SUPPORTED_LANGUAGES: &[&str] = &[
    "rust",
    "javascript",
    "typescript",
    "python",
    "json",
    "yaml",
    "markdown",
    "bash",
    "html",
    "css",
    "sql",
    "go",
    "c",
    "cpp",
    "java",
    "ruby",
    "php",
    "swift",
    "kotlin",
    "toml",
    "xml",
];

pub struct SyntaxHighlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

impl SyntaxHighlighter {
    pub fn new() -> Result<Self, String> {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();

        Ok(Self {
            syntax_set,
            theme_set,
        })
    }

    fn get_syntax(&self, language: &str) -> Option<&SyntaxReference> {
        let lang_lower = language.to_lowercase();
        let (ext, token) = match lang_lower.as_str() {
            "js" | "javascript" => ("js", "JavaScript"),
            "ts" | "typescript" => ("ts", "TypeScript"),
            "py" | "python" => ("py", "Python"),
            "rb" | "ruby" => ("rb", "Ruby"),
            "rs" | "rust" => ("rs", "Rust"),
            "sh" | "bash" | "zsh" | "shell" => ("sh", "Bourne Again Shell (bash)"),
            "yml" | "yaml" => ("yml", "YAML"),
            "md" | "markdown" => ("md", "Markdown"),
            "c++" | "cpp" => ("cpp", "C++"),
            "dockerfile" => ("dockerfile", "Dockerfile"),
            other => (other, other),
        };

        let primary = self
            .syntax_set
            .find_syntax_by_extension(ext)
            .or_else(|| self.syntax_set.find_syntax_by_token(token));

        if primary.is_some() {
            return primary;
        }

        match lang_lower.as_str() {
            "ts" | "typescript" => self
                .syntax_set
                .find_syntax_by_extension("js")
                .or_else(|| self.syntax_set.find_syntax_by_token("JavaScript")),
            "js" | "javascript" => self
                .syntax_set
                .find_syntax_by_extension("js")
                .or_else(|| self.syntax_set.find_syntax_by_token("JavaScript")),
            _ => None,
        }
    }

    pub fn highlight(
        &self,
        code: &str,
        language: &str,
        config: &MarkdownConfig,
    ) -> Vec<StyledLine> {
        let syntax = match self.get_syntax(language) {
            Some(s) => s,
            None => return self.plain_lines(code, config),
        };

        let theme = self.theme_set.themes.get("base16-ocean.dark").or_else(|| {
            let mut keys: Vec<_> = self.theme_set.themes.keys().collect();
            keys.sort();
            keys.first().and_then(|k| self.theme_set.themes.get(*k))
        });

        let Some(theme) = theme else {
            return self.plain_lines(code, config);
        };

        let mut highlighter = HighlightLines::new(syntax, theme);

        code.lines()
            .map(|line| {
                let highlighted = highlighter
                    .highlight_line(line, &self.syntax_set)
                    .unwrap_or_default();

                let spans: Vec<StyledSpan> = highlighted
                    .into_iter()
                    .map(|(style, text)| {
                        StyledSpan::new(
                            text.to_string(),
                            TextStyle {
                                color: syntect_style_to_hsla(&style),
                                font_size: config.base_font_size,
                                monospace: true,
                                ..Default::default()
                            },
                        )
                    })
                    .collect();

                let spans = if spans.is_empty() {
                    vec![StyledSpan::new(
                        " ".to_string(),
                        TextStyle {
                            color: config.text_color,
                            font_size: config.base_font_size,
                            monospace: true,
                            ..Default::default()
                        },
                    )]
                } else {
                    spans
                };

                StyledLine {
                    spans,
                    line_height: theme::line_height::NORMAL,
                    margin_top: 0.0,
                    indent: 0,
                }
            })
            .collect()
    }

    fn plain_lines(&self, code: &str, config: &MarkdownConfig) -> Vec<StyledLine> {
        code.lines()
            .map(|line| {
                let text = if line.is_empty() { " " } else { line };
                StyledLine::from_span(StyledSpan::new(
                    text.to_string(),
                    TextStyle {
                        color: config.text_color,
                        font_size: config.base_font_size,
                        monospace: true,
                        ..Default::default()
                    },
                ))
                .with_line_height(theme::line_height::NORMAL)
            })
            .collect()
    }

    pub fn is_supported(&self, language: &str) -> bool {
        self.get_syntax(language).is_some()
    }
}

fn syntect_style_to_hsla(style: &Style) -> Hsla {
    let fg = style.foreground;
    Hsla::from_rgb(
        fg.r as f32 / 255.0,
        fg.g as f32 / 255.0,
        fg.b as f32 / 255.0,
    )
    .with_alpha(fg.a as f32 / 255.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlighter_creation() {
        let highlighter = SyntaxHighlighter::new();
        assert!(highlighter.is_ok());
    }

    #[test]
    fn test_rust_highlighting() {
        let highlighter = SyntaxHighlighter::new().unwrap();
        let config = MarkdownConfig::default();
        let code = "fn main() {\n    println!(\"Hello\");\n}";
        let lines = highlighter.highlight(code, "rust", &config);

        assert_eq!(lines.len(), 3);
        assert!(!lines[0].spans.is_empty());
    }

    #[test]
    fn test_language_aliases() {
        let highlighter = SyntaxHighlighter::new().unwrap();

        assert!(highlighter.is_supported("js"));
        assert!(highlighter.is_supported("javascript"));
        assert!(highlighter.is_supported("py"));
        assert!(highlighter.is_supported("rs"));
        assert!(highlighter.is_supported("sh"));
    }

    #[test]
    fn test_unknown_language() {
        let highlighter = SyntaxHighlighter::new().unwrap();
        let config = MarkdownConfig::default();
        let code = "some random text";
        let lines = highlighter.highlight(code, "unknown_lang_xyz", &config);

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].spans.len(), 1);
    }
}
