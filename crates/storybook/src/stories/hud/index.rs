//! HUD components overview page.

use maud::{Markup, html};

pub fn hud_index_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "HUD Components" }
        p class="text-sm text-muted-foreground mb-6" {
            "WGPUI HUD components for overlays, menus, notifications, and status surfaces."
        }

        div class="grid gap-6 md:grid-cols-2" {
            div class="p-4 border border-border bg-card" {
                h2 class="text-lg font-medium mb-2" { "Overlays" }
                p class="text-sm text-muted-foreground mb-3" { "Command and tooltip overlays." }
                ul class="text-sm space-y-1" {
                    li { a href="/stories/hud/command-palette" class="text-cyan hover:underline" { "Command Palette" } }
                    li { a href="/stories/hud/tooltip" class="text-cyan hover:underline" { "Tooltip" } }
                }
            }
            div class="p-4 border border-border bg-card" {
                h2 class="text-lg font-medium mb-2" { "Menus" }
                p class="text-sm text-muted-foreground mb-3" { "Context menu surfaces." }
                ul class="text-sm space-y-1" {
                    li { a href="/stories/hud/context-menu" class="text-cyan hover:underline" { "Context Menu" } }
                }
            }
            div class="p-4 border border-border bg-card" {
                h2 class="text-lg font-medium mb-2" { "Feedback" }
                p class="text-sm text-muted-foreground mb-3" { "Toast notifications and alerts." }
                ul class="text-sm space-y-1" {
                    li { a href="/stories/hud/notifications" class="text-cyan hover:underline" { "Notifications" } }
                }
            }
            div class="p-4 border border-border bg-card" {
                h2 class="text-lg font-medium mb-2" { "Shell" }
                p class="text-sm text-muted-foreground mb-3" { "Persistent system chrome." }
                ul class="text-sm space-y-1" {
                    li { a href="/stories/hud/status-bar" class="text-cyan hover:underline" { "Status Bar" } }
                }
            }
        }
    }
}
