//! Tool kind icons for ACP tool calls.

use maud::{Markup, html};

/// The kind of tool being invoked, matching ACP protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    /// File read operation
    Read,
    /// File edit/write operation
    Edit,
    /// File deletion
    Delete,
    /// Command execution (terminal)
    Execute,
    /// File/content search
    Search,
    /// Agent thinking/reasoning
    Think,
    /// Web fetch operation
    Fetch,
    /// Mode switch request
    SwitchMode,
    /// Generic/other tool
    Other,
}

impl ToolKind {
    /// ASCII icon character for each tool kind.
    pub fn icon(&self) -> &'static str {
        match self {
            ToolKind::Read => "[R]",
            ToolKind::Edit => "[E]",
            ToolKind::Delete => "[D]",
            ToolKind::Execute => "[>]",
            ToolKind::Search => "[?]",
            ToolKind::Think => "[~]",
            ToolKind::Fetch => "[W]",
            ToolKind::SwitchMode => "[<>]",
            ToolKind::Other => "[*]",
        }
    }

    /// CSS class for styling the icon.
    pub fn class(&self) -> &'static str {
        match self {
            ToolKind::Read => "text-cyan",
            ToolKind::Edit => "text-yellow",
            ToolKind::Delete => "text-red",
            ToolKind::Execute => "text-green",
            ToolKind::Search => "text-cyan",
            ToolKind::Think => "text-magenta",
            ToolKind::Fetch => "text-blue",
            ToolKind::SwitchMode => "text-orange",
            ToolKind::Other => "text-muted-foreground",
        }
    }

    /// Human-readable label for the tool kind.
    pub fn label(&self) -> &'static str {
        match self {
            ToolKind::Read => "Read",
            ToolKind::Edit => "Edit",
            ToolKind::Delete => "Delete",
            ToolKind::Execute => "Execute",
            ToolKind::Search => "Search",
            ToolKind::Think => "Think",
            ToolKind::Fetch => "Fetch",
            ToolKind::SwitchMode => "Switch Mode",
            ToolKind::Other => "Tool",
        }
    }
}

/// Render a tool kind icon.
pub fn tool_icon(kind: ToolKind) -> Markup {
    html! {
        span
            title=(kind.label())
            class={ "text-sm leading-none " (kind.class()) }
        {
            (kind.icon())
        }
    }
}
