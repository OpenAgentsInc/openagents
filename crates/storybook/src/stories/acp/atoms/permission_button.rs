//! Permission button story.

use maud::{Markup, html};
use ui::acp::atoms::{permission_button, PermissionKind};

fn section_title(title: &str) -> Markup {
    html! { h2 class="text-base font-medium text-muted-foreground mt-8 mb-4" { (title) } }
}

fn section(content: Markup) -> Markup {
    html! { div class="p-4 border border-border bg-card mb-4" { (content) } }
}

fn row(content: Markup) -> Markup {
    html! { div class="flex gap-4 items-center flex-wrap" { (content) } }
}

fn item(label: &str, content: Markup) -> Markup {
    html! {
        div class="flex flex-col gap-2" {
            span class="text-xs text-muted-foreground" { (label) }
            (content)
        }
    }
}

fn code_block(code: &str) -> Markup {
    html! {
        pre class="text-xs bg-secondary border border-border p-4 overflow-x-auto text-muted-foreground" {
            code { (code) }
        }
    }
}

pub fn permission_button_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" {
            "Permission Button"
        }
        p class="text-sm text-muted-foreground mb-6" {
            "Action buttons for tool authorization requests."
        }

        (section_title("Button Variants"))
        (section(row(html! {
            (item("Allow Once (y)", permission_button(PermissionKind::AllowOnce, None)))
            (item("Always Allow (Y)", permission_button(PermissionKind::AllowAlways, None)))
            (item("Reject Once (n)", permission_button(PermissionKind::RejectOnce, None)))
            (item("Always Reject (N)", permission_button(PermissionKind::RejectAlways, None)))
        })))

        (section_title("With Option IDs"))
        (section(row(html! {
            (item("With ID", permission_button(PermissionKind::AllowOnce, Some("opt-123"))))
        })))

        (section_title("Usage"))
        (code_block(r#"use ui::acp::atoms::{permission_button, PermissionKind};

// Basic permission buttons
permission_button(PermissionKind::AllowOnce, None)
permission_button(PermissionKind::AllowAlways, None)
permission_button(PermissionKind::RejectOnce, None)
permission_button(PermissionKind::RejectAlways, None)

// With option ID for form handling
permission_button(PermissionKind::AllowOnce, Some("option-id-123"))"#))
    }
}
