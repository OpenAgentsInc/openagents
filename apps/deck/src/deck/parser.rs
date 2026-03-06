use serde::Deserialize;
use wgpui::markdown::{MarkdownBlock, MarkdownConfig, MarkdownDocument, MarkdownParser};
use wgpui::theme;

use crate::deck::model::{
    Deck, DeckMetadata, DeckTheme, MarkdownSlide, Slide, SlideDiagram, SlideKind, SlideLayout,
    SlideTransition,
};

pub struct DeckParser {
    markdown_parser: MarkdownParser,
}

impl DeckParser {
    pub fn new() -> Self {
        Self {
            markdown_parser: MarkdownParser::with_config(presentation_markdown_config()),
        }
    }

    pub fn parse(&self, source: &str) -> Result<Deck, String> {
        let normalized = strip_bom(source).trim_start_matches(['\n', '\r']);
        let (frontmatter, deck_body) = extract_frontmatter(normalized)?;
        let raw_metadata = match frontmatter {
            Some(metadata) => parse_toml::<RawDeckFrontmatter>(metadata, "deck frontmatter")?,
            None => RawDeckFrontmatter::default(),
        };

        let sections = split_slide_sections(deck_body);
        if sections.is_empty() {
            return Err("deck source did not contain any slides".to_string());
        }

        let deck_theme = raw_metadata
            .theme
            .as_deref()
            .map(DeckTheme::parse)
            .unwrap_or(DeckTheme::Hud);
        let metadata = DeckMetadata {
            title: raw_metadata
                .title
                .unwrap_or_else(|| "Untitled Deck".to_string()),
            slug: raw_metadata.slug,
            theme: deck_theme.clone(),
        };

        let mut slides = Vec::with_capacity(sections.len());
        for (index, section) in sections.iter().enumerate() {
            slides.push(self.parse_slide(section, index, &deck_theme)?);
        }

        Ok(Deck { metadata, slides })
    }

    fn parse_slide(
        &self,
        section: &str,
        index: usize,
        deck_theme: &DeckTheme,
    ) -> Result<Slide, String> {
        let (frontmatter, markdown_body) = extract_frontmatter(section)?;
        let raw_slide = match frontmatter {
            Some(metadata) => parse_toml::<RawSlideFrontmatter>(
                metadata,
                &format!("slide {} frontmatter", index + 1),
            )?,
            None => RawSlideFrontmatter::default(),
        };

        let markdown = markdown_body.trim();
        if markdown.is_empty() {
            return Err(format!("slide {} has no markdown body", index + 1));
        }

        let document = self.markdown_parser.parse(markdown);
        let inferred_title = infer_markdown_title(&document);
        let title = raw_slide
            .title
            .or(inferred_title)
            .unwrap_or_else(|| format!("Slide {}", index + 1));
        let id = raw_slide.id.unwrap_or_else(|| slugify(&title, index + 1));
        let layout = match raw_slide.layout {
            Some(layout) => SlideLayout::parse(&layout)?,
            None => infer_layout(&document),
        };
        let transition = match raw_slide.transition {
            Some(transition) => SlideTransition::parse(&transition)?,
            None => SlideTransition::None,
        };
        let theme = raw_slide
            .theme
            .as_deref()
            .map(DeckTheme::parse)
            .unwrap_or_else(|| deck_theme.clone());
        let diagram = match raw_slide.diagram {
            Some(diagram) => Some(SlideDiagram::parse(&diagram)?),
            None => None,
        };
        let notes = raw_slide.notes.and_then(normalize_optional_string);

        Ok(Slide {
            id,
            title,
            eyebrow: raw_slide.eyebrow.and_then(normalize_optional_string),
            summary: raw_slide.summary.and_then(normalize_optional_string),
            footer: raw_slide.footer.and_then(normalize_optional_string),
            sources: normalize_string_list(raw_slide.sources),
            theme,
            layout,
            diagram,
            notes,
            transition,
            kind: SlideKind::Markdown(MarkdownSlide {
                markdown: markdown.to_string(),
                document,
            }),
        })
    }
}

impl Default for DeckParser {
    fn default() -> Self {
        Self::new()
    }
}

pub fn presentation_markdown_config() -> MarkdownConfig {
    MarkdownConfig {
        base_font_size: 18.0,
        header_sizes: [2.3, 1.7, 1.35, 1.15, 1.05, 1.0],
        text_color: theme::text::PRIMARY,
        code_background: theme::bg::CODE.with_alpha(0.96),
        inline_code_background: theme::bg::MUTED.with_alpha(0.9),
        link_color: theme::accent::PRIMARY,
        header_color: theme::text::PRIMARY,
        blockquote_color: theme::accent::PRIMARY,
        max_width: None,
    }
}

#[derive(Debug, Default, Deserialize)]
struct RawDeckFrontmatter {
    title: Option<String>,
    slug: Option<String>,
    theme: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RawSlideFrontmatter {
    id: Option<String>,
    title: Option<String>,
    eyebrow: Option<String>,
    summary: Option<String>,
    footer: Option<String>,
    sources: Option<Vec<String>>,
    layout: Option<String>,
    theme: Option<String>,
    diagram: Option<String>,
    notes: Option<String>,
    transition: Option<String>,
}

fn parse_toml<T>(payload: &str, context: &str) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    toml::from_str(payload).map_err(|error| format!("invalid {context}: {error}"))
}

fn strip_bom(source: &str) -> &str {
    source.strip_prefix('\u{feff}').unwrap_or(source)
}

fn extract_frontmatter(source: &str) -> Result<(Option<&str>, &str), String> {
    let trimmed = source.trim_start_matches(['\n', '\r']);
    let Some((first_line, mut offset)) = read_line(trimmed, 0) else {
        return Ok((None, trimmed));
    };
    if first_line.trim() != "+++" {
        return Ok((None, trimmed));
    }

    let metadata_start = offset;
    loop {
        let Some((line, next_offset)) = read_line(trimmed, offset) else {
            return Err("metadata fence is missing a closing '+++' line".to_string());
        };

        if line.trim() == "+++" {
            let metadata = trimmed[metadata_start..offset].trim_end_matches(['\n', '\r']);
            let remainder = trimmed[next_offset..].trim_start_matches(['\n', '\r']);
            return Ok((Some(metadata), remainder));
        }

        offset = next_offset;
    }
}

fn read_line(source: &str, start: usize) -> Option<(&str, usize)> {
    if start >= source.len() {
        return None;
    }

    let remainder = &source[start..];
    match remainder.find('\n') {
        Some(line_end) => {
            let end = start + line_end;
            let line = source[start..end].trim_end_matches('\r');
            Some((line, end + 1))
        }
        None => Some((remainder.trim_end_matches('\r'), source.len())),
    }
}

fn split_slide_sections(source: &str) -> Vec<String> {
    let mut sections = Vec::new();
    let mut current = String::new();
    let mut in_code_fence = false;

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_code_fence = !in_code_fence;
        }

        if !in_code_fence && trimmed == "---" {
            push_slide_section(&mut sections, &mut current);
            continue;
        }

        current.push_str(line);
        current.push('\n');
    }

    push_slide_section(&mut sections, &mut current);
    sections
}

fn push_slide_section(sections: &mut Vec<String>, current: &mut String) {
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        sections.push(trimmed.to_string());
    }
    current.clear();
}

fn infer_markdown_title(document: &MarkdownDocument) -> Option<String> {
    for block in &document.blocks {
        if let MarkdownBlock::Header { lines, .. } = block {
            let mut title = String::new();
            for line in lines {
                for span in &line.spans {
                    title.push_str(&span.text);
                }
            }

            let normalized = normalize_optional_string(title);
            if normalized.is_some() {
                return normalized;
            }
        }
    }

    None
}

fn infer_layout(document: &MarkdownDocument) -> SlideLayout {
    if document
        .blocks
        .iter()
        .any(|block| matches!(block, MarkdownBlock::CodeBlock { .. }))
    {
        return SlideLayout::Code;
    }

    match document.blocks.first() {
        Some(MarkdownBlock::Header { level: 1, .. }) if document.blocks.len() <= 3 => {
            SlideLayout::Title
        }
        _ => SlideLayout::Body,
    }
}

fn normalize_optional_string(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn normalize_string_list(values: Option<Vec<String>>) -> Vec<String> {
    values
        .unwrap_or_default()
        .into_iter()
        .filter_map(normalize_optional_string)
        .collect()
}

fn slugify(title: &str, index: usize) -> String {
    let mut slug = String::new();
    let mut previous_was_dash = false;

    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_was_dash = false;
        } else if (ch.is_whitespace() || ch == '-' || ch == '_') && !previous_was_dash {
            slug.push('-');
            previous_was_dash = true;
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        format!("slide-{}", index + 1)
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::DeckParser;
    use crate::deck::model::{DeckTheme, SlideDiagram, SlideLayout};

    #[test]
    fn parses_toml_frontmatter_and_slide_metadata() {
        let source = r#"
+++
title = "Example Deck"
theme = "code"
+++

---
+++
id = "intro"
title = "Intro"
eyebrow = "01 / Intro"
summary = "Deck summary"
footer = "Footer copy"
sources = ["README.md", "docs/kernel/diagram.md"]
layout = "title"
diagram = "market-map"
+++
# Hello

Welcome.

---
+++
title = "Code"
+++
```text
---
this stays inside the slide body
```
"#;

        let parsed = DeckParser::new().parse(source);
        assert!(parsed.is_ok(), "deck should parse: {parsed:?}");
        let Ok(deck) = parsed else {
            return;
        };

        assert_eq!(deck.metadata.title, "Example Deck");
        assert_eq!(deck.metadata.theme, DeckTheme::Code);
        assert_eq!(deck.slides.len(), 2);
        assert_eq!(deck.slides[0].id, "intro");
        assert_eq!(deck.slides[0].layout, SlideLayout::Title);
        assert_eq!(deck.slides[0].eyebrow.as_deref(), Some("01 / Intro"));
        assert_eq!(deck.slides[0].summary.as_deref(), Some("Deck summary"));
        assert_eq!(deck.slides[0].footer.as_deref(), Some("Footer copy"));
        assert_eq!(
            deck.slides[0].sources,
            vec![
                "README.md".to_string(),
                "docs/kernel/diagram.md".to_string()
            ]
        );
        assert_eq!(deck.slides[0].diagram, Some(SlideDiagram::MarketMap));
        assert_eq!(deck.slides[1].title, "Code");
    }

    #[test]
    fn rejects_unclosed_frontmatter() {
        let parsed = DeckParser::new().parse(
            r#"
+++
title = "Broken"

---
# Slide
"#,
        );

        assert!(parsed.is_err());
    }
}
