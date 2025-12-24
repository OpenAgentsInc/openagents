//! Thread entry type markers.

use maud::{Markup, html};

/// Kind of thread entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// User message
    User,
    /// Assistant message
    Assistant,
    /// Tool call
    Tool,
}

impl EntryKind {
    /// Icon for the entry type.
    fn icon(&self) -> &'static str {
        match self {
            EntryKind::User => ">",
            EntryKind::Assistant => "<",
            EntryKind::Tool => "*",
        }
    }

    /// CSS class for styling.
    fn class(&self) -> &'static str {
        match self {
            EntryKind::User => "text-cyan",
            EntryKind::Assistant => "text-magenta",
            EntryKind::Tool => "text-yellow",
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            EntryKind::User => "You",
            EntryKind::Assistant => "Claude",
            EntryKind::Tool => "Tool",
        }
    }
}

/// Render an entry type marker.
pub fn entry_marker(kind: EntryKind) -> Markup {
    html! {
        span
            title=(kind.label())
            class={ "text-sm leading-none " (kind.class()) }
        {
            (kind.icon())
        }
    }
}
