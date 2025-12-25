//! Command palette HUD story.

use maud::{Markup, html};

use super::shared::{code_block, section, section_title, story_header};

struct PaletteItem {
    label: &'static str,
    description: Option<&'static str>,
    keys: Option<&'static str>,
    category: Option<&'static str>,
    selected: bool,
}

fn palette_preview(query: &str, items: &[PaletteItem], empty_label: Option<&'static str>, footer: Option<&'static str>) -> Markup {
    html! {
        div class="border border-border bg-secondary p-6" {
            div class="mx-auto w-full max-w-xl border border-border bg-card" {
                div class="border-b border-border px-3 py-2 text-xs text-muted-foreground" {
                    "Command Palette"
                }
                div class="border-b border-border px-3 py-2 text-xs text-muted-foreground" {
                    "Query: " (query)
                }
                @if items.is_empty() {
                    div class="px-3 py-6 text-sm text-muted-foreground" {
                        (empty_label.unwrap_or("No matching commands"))
                    }
                } @else {
                    @for item in items {
                        div class=(if item.selected { "px-3 py-2 bg-secondary" } else { "px-3 py-2" }) {
                            div class="flex items-start justify-between gap-4" {
                                div {
                                    div class="text-sm text-foreground" { (item.label) }
                                    @if let Some(desc) = item.description {
                                        div class="text-xs text-muted-foreground mt-1" { (desc) }
                                    }
                                    @if let Some(category) = item.category {
                                        div class="text-xs text-muted-foreground mt-1" { "Category: " (category) }
                                    }
                                }
                                @if let Some(keys) = item.keys {
                                    div class="text-xs text-muted-foreground" { (keys) }
                                }
                            }
                        }
                    }
                }
                @if let Some(label) = footer {
                    div class="border-t border-border px-3 py-2 text-xs text-muted-foreground" { (label) }
                }
            }
        }
    }
}

pub fn command_palette_story() -> Markup {
    let default_items = vec![
        PaletteItem {
            label: "Open File",
            description: Some("Open from disk"),
            keys: Some("Ctrl+O"),
            category: Some("File"),
            selected: true,
        },
        PaletteItem {
            label: "Open Recent",
            description: Some("Recent files and folders"),
            keys: None,
            category: Some("File"),
            selected: false,
        },
        PaletteItem {
            label: "Search in Project",
            description: Some("Find in workspace"),
            keys: Some("Ctrl+Shift+F"),
            category: Some("Search"),
            selected: false,
        },
    ];

    let minimal_items = vec![
        PaletteItem {
            label: "Toggle Sidebar",
            description: None,
            keys: Some("Ctrl+B"),
            category: None,
            selected: true,
        },
        PaletteItem {
            label: "Toggle Panel",
            description: None,
            keys: Some("Ctrl+J"),
            category: None,
            selected: false,
        },
    ];

    let filtered_items = vec![
        PaletteItem {
            label: "Build Project",
            description: Some("Compile and bundle"),
            keys: Some("Ctrl+Shift+B"),
            category: Some("Build"),
            selected: true,
        },
        PaletteItem {
            label: "Build and Run",
            description: Some("Build, then run"),
            keys: None,
            category: Some("Build"),
            selected: false,
        },
    ];

    let long_items = vec![
        PaletteItem {
            label: "Switch Theme",
            description: Some("Cycle themes"),
            keys: None,
            category: Some("View"),
            selected: true,
        },
        PaletteItem {
            label: "Toggle Minimap",
            description: None,
            keys: None,
            category: Some("View"),
            selected: false,
        },
        PaletteItem {
            label: "Show Command Log",
            description: None,
            keys: None,
            category: Some("View"),
            selected: false,
        },
        PaletteItem {
            label: "Open Settings",
            description: Some("User settings"),
            keys: Some("Ctrl+,"),
            category: Some("System"),
            selected: false,
        },
        PaletteItem {
            label: "Open Keymap",
            description: None,
            keys: None,
            category: Some("System"),
            selected: false,
        },
        PaletteItem {
            label: "Toggle Terminal",
            description: None,
            keys: Some("Ctrl+`"),
            category: Some("View"),
            selected: false,
        },
        PaletteItem {
            label: "Toggle Explorer",
            description: None,
            keys: Some("Ctrl+Shift+E"),
            category: Some("View"),
            selected: false,
        },
    ];

    html! {
        (story_header(
            "Command Palette",
            "Searchable command list with keyboard navigation and filtering."
        ))

        (section_title("Default"))
        (section(palette_preview("open", &default_items, None, None)))

        (section_title("Minimal rows"))
        (section(palette_preview("toggle", &minimal_items, None, None)))

        (section_title("Filtered"))
        (section(palette_preview("build", &filtered_items, None, None)))

        (section_title("Empty state"))
        (section(palette_preview("xyz", &[], Some("No commands for query"), None)))

        (section_title("Long list and max visible"))
        (section(palette_preview(
            "view",
            &long_items,
            None,
            Some("More results below")
        )))

        (section_title("Usage"))
        (code_block(r#"use wgpui::components::hud::{Command, CommandPalette};

let mut palette = CommandPalette::new()
    .commands(vec![
        Command::new("file.open", "Open File")
            .description("Open from disk")
            .keybinding("Ctrl+O")
            .category("File"),
        Command::new("search.project", "Search in Project")
            .description("Find in workspace")
            .keybinding("Ctrl+Shift+F")
            .category("Search"),
    ])
    .max_visible_items(6);

palette.open();
"#))
    }
}
