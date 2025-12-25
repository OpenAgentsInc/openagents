//! Context menu HUD story.

use maud::{Markup, html};

use super::shared::{code_block, section, section_title, story_header};

struct MenuRow {
    label: &'static str,
    shortcut: Option<&'static str>,
    disabled: bool,
    checked: Option<bool>,
    submenu: bool,
    separator: bool,
    selected: bool,
}

fn menu_preview(items: &[MenuRow]) -> Markup {
    html! {
        div class="border border-border bg-card w-64" {
            @for item in items {
                @if item.separator {
                    div class="border-t border-border my-1" {}
                } @else {
                    div class=(if item.selected { "px-3 py-2 bg-secondary" } else { "px-3 py-2" }) {
                        div class="flex items-center justify-between gap-3" {
                            div class="flex items-center gap-2" {
                                @if let Some(checked) = item.checked {
                                    div class="text-xs text-muted-foreground" {
                                        @if checked { "[x]" } @else { "[ ]" }
                                    }
                                }
                                div class=(if item.disabled { "text-xs text-muted-foreground" } else { "text-xs text-foreground" }) {
                                    (item.label)
                                }
                            }
                            div class="flex items-center gap-2" {
                                @if let Some(shortcut) = item.shortcut {
                                    div class="text-xs text-muted-foreground" { (shortcut) }
                                }
                                @if item.submenu {
                                    div class="text-xs text-muted-foreground" { ">" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn context_menu_story() -> Markup {
    let basic = vec![
        MenuRow {
            label: "Copy",
            shortcut: Some("Ctrl+C"),
            disabled: false,
            checked: None,
            submenu: false,
            separator: false,
            selected: true,
        },
        MenuRow {
            label: "Paste",
            shortcut: Some("Ctrl+V"),
            disabled: false,
            checked: None,
            submenu: false,
            separator: false,
            selected: false,
        },
        MenuRow {
            label: "Rename",
            shortcut: Some("F2"),
            disabled: false,
            checked: None,
            submenu: false,
            separator: false,
            selected: false,
        },
    ];

    let with_states = vec![
        MenuRow {
            label: "Show Hidden Files",
            shortcut: None,
            disabled: false,
            checked: Some(true),
            submenu: false,
            separator: false,
            selected: true,
        },
        MenuRow {
            label: "Auto Save",
            shortcut: None,
            disabled: false,
            checked: Some(false),
            submenu: false,
            separator: false,
            selected: false,
        },
        MenuRow {
            label: "Disabled Action",
            shortcut: Some("Ctrl+D"),
            disabled: true,
            checked: None,
            submenu: false,
            separator: false,
            selected: false,
        },
        MenuRow {
            label: "Separator",
            shortcut: None,
            disabled: false,
            checked: None,
            submenu: false,
            separator: true,
            selected: false,
        },
        MenuRow {
            label: "Preferences",
            shortcut: None,
            disabled: false,
            checked: None,
            submenu: false,
            separator: false,
            selected: false,
        },
    ];

    let with_submenu = vec![
        MenuRow {
            label: "New",
            shortcut: None,
            disabled: false,
            checked: None,
            submenu: true,
            separator: false,
            selected: true,
        },
        MenuRow {
            label: "Open",
            shortcut: Some("Ctrl+O"),
            disabled: false,
            checked: None,
            submenu: false,
            separator: false,
            selected: false,
        },
        MenuRow {
            label: "Close",
            shortcut: Some("Ctrl+W"),
            disabled: false,
            checked: None,
            submenu: false,
            separator: false,
            selected: false,
        },
    ];

    let submenu_items = vec![
        MenuRow {
            label: "File",
            shortcut: None,
            disabled: false,
            checked: None,
            submenu: false,
            separator: false,
            selected: true,
        },
        MenuRow {
            label: "Folder",
            shortcut: None,
            disabled: false,
            checked: None,
            submenu: false,
            separator: false,
            selected: false,
        },
        MenuRow {
            label: "Workspace",
            shortcut: None,
            disabled: false,
            checked: None,
            submenu: false,
            separator: false,
            selected: false,
        },
    ];

    html! {
        (story_header(
            "Context Menu",
            "Right click menu with shortcuts, separators, and submenus."
        ))

        (section_title("Basic menu"))
        (section(menu_preview(&basic)))

        (section_title("Checked, disabled, and separators"))
        (section(menu_preview(&with_states)))

        (section_title("Submenu open"))
        (section(html! {
            div class="flex gap-4 flex-wrap" {
                (menu_preview(&with_submenu))
                (menu_preview(&submenu_items))
            }
        }))

        (section_title("Usage"))
        (code_block(r#"use wgpui::components::hud::{ContextMenu, MenuItem};

let menu = ContextMenu::new().items(vec![
    MenuItem::new("copy", "Copy").shortcut("Ctrl+C"),
    MenuItem::separator(),
    MenuItem::new("paste", "Paste").shortcut("Ctrl+V"),
    MenuItem::new("prefs", "Preferences").disabled(false),
]);
"#))
    }
}
