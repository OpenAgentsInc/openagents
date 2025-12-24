//! Content type icons for tool call output.

use maud::{Markup, html};

/// Type of tool call content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    /// Text or markdown content block
    ContentBlock,
    /// File diff
    Diff,
    /// Terminal output
    Terminal,
}

impl ContentType {
    /// Icon for the content type.
    fn icon(&self) -> &'static str {
        match self {
            ContentType::ContentBlock => "[#]",
            ContentType::Diff => "[~]",
            ContentType::Terminal => "[>]",
        }
    }

    /// CSS class for styling.
    fn class(&self) -> &'static str {
        match self {
            ContentType::ContentBlock => "text-muted-foreground",
            ContentType::Diff => "text-yellow",
            ContentType::Terminal => "text-green",
        }
    }

    /// Human-readable label.
    fn label(&self) -> &'static str {
        match self {
            ContentType::ContentBlock => "Content",
            ContentType::Diff => "Diff",
            ContentType::Terminal => "Terminal",
        }
    }
}

/// Render a content type icon.
pub fn content_type_icon(content_type: ContentType) -> Markup {
    html! {
        span
            title=(content_type.label())
            class={ "text-xs " (content_type.class()) }
        {
            (content_type.icon())
        }
    }
}
