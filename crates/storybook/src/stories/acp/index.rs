//! ACP components overview page.

use maud::{Markup, html};

pub fn acp_index_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" {
            "ACP Components"
        }
        p class="text-sm text-muted-foreground mb-6" {
            "Agent Client Protocol UI components for rendering Claude Code conversations."
        }

        // Overview
        div class="grid gap-6 md:grid-cols-2" {
            // Atoms
            div class="p-4 border border-border bg-card" {
                h2 class="text-lg font-medium mb-2" { "Atoms" }
                p class="text-sm text-muted-foreground mb-3" {
                    "12 simple, single-purpose UI elements"
                }
                ul class="text-sm space-y-1" {
                    li { a href="/stories/acp/atoms/tool-icon" class="text-cyan hover:underline" { "Tool Icon" } }
                    li { a href="/stories/acp/atoms/tool-status-badge" class="text-cyan hover:underline" { "Tool Status Badge" } }
                    li { a href="/stories/acp/atoms/permission-button" class="text-cyan hover:underline" { "Permission Button" } }
                    li { a href="/stories/acp/atoms/mode-badge" class="text-cyan hover:underline" { "Mode Badge" } }
                    li { a href="/stories/acp/atoms/model-badge" class="text-cyan hover:underline" { "Model Badge" } }
                    li { a href="/stories/acp/atoms/thinking-toggle" class="text-cyan hover:underline" { "Thinking Toggle" } }
                    li { a href="/stories/acp/atoms/checkpoint-badge" class="text-cyan hover:underline" { "Checkpoint Badge" } }
                    li { a href="/stories/acp/atoms/feedback-button" class="text-cyan hover:underline" { "Feedback Button" } }
                    li { a href="/stories/acp/atoms/content-type-icon" class="text-cyan hover:underline" { "Content Type Icon" } }
                    li { a href="/stories/acp/atoms/entry-marker" class="text-cyan hover:underline" { "Entry Marker" } }
                    li { a href="/stories/acp/atoms/keybinding-hint" class="text-cyan hover:underline" { "Keybinding Hint" } }
                    li { a href="/stories/acp/atoms/streaming-indicator" class="text-cyan hover:underline" { "Streaming Indicator" } }
                }
            }

            // Molecules
            div class="p-4 border border-border bg-card" {
                h2 class="text-lg font-medium mb-2" { "Molecules" }
                p class="text-sm text-muted-foreground mb-3" {
                    "10 compositions of atoms"
                }
                ul class="text-sm space-y-1" {
                    li { a href="/stories/acp/molecules/tool-header" class="text-cyan hover:underline" { "Tool Header" } }
                    li { a href="/stories/acp/molecules/permission-bar" class="text-cyan hover:underline" { "Permission Bar" } }
                    li { a href="/stories/acp/molecules/mode-selector" class="text-cyan hover:underline" { "Mode Selector" } }
                    li { a href="/stories/acp/molecules/model-selector" class="text-cyan hover:underline" { "Model Selector" } }
                    li { a href="/stories/acp/molecules/message-header" class="text-cyan hover:underline" { "Message Header" } }
                    li { a href="/stories/acp/molecules/thinking-block" class="text-cyan hover:underline" { "Thinking Block" } }
                    li { a href="/stories/acp/molecules/diff-header" class="text-cyan hover:underline" { "Diff Header" } }
                    li { a href="/stories/acp/molecules/terminal-header" class="text-cyan hover:underline" { "Terminal Header" } }
                    li { a href="/stories/acp/molecules/checkpoint-restore" class="text-cyan hover:underline" { "Checkpoint Restore" } }
                    li { a href="/stories/acp/molecules/entry-actions" class="text-cyan hover:underline" { "Entry Actions" } }
                }
            }

            // Organisms
            div class="p-4 border border-border bg-card" {
                h2 class="text-lg font-medium mb-2" { "Organisms" }
                p class="text-sm text-muted-foreground mb-3" {
                    "9 complex UI components"
                }
                ul class="text-sm space-y-1" {
                    li { a href="/stories/acp/organisms/user-message" class="text-cyan hover:underline" { "User Message" } }
                    li { a href="/stories/acp/organisms/assistant-message" class="text-cyan hover:underline" { "Assistant Message" } }
                    li { a href="/stories/acp/organisms/tool-call-card" class="text-cyan hover:underline" { "Tool Call Card" } }
                    li { a href="/stories/acp/organisms/terminal-tool-call" class="text-cyan hover:underline" { "Terminal Tool Call" } }
                    li { a href="/stories/acp/organisms/diff-tool-call" class="text-cyan hover:underline" { "Diff Tool Call" } }
                    li { a href="/stories/acp/organisms/search-tool-call" class="text-cyan hover:underline" { "Search Tool Call" } }
                    li { a href="/stories/acp/organisms/thread-controls" class="text-cyan hover:underline" { "Thread Controls" } }
                    li { a href="/stories/acp/organisms/permission-dialog" class="text-cyan hover:underline" { "Permission Dialog" } }
                    li { a href="/stories/acp/organisms/thread-entry" class="text-cyan hover:underline" { "Thread Entry" } }
                }
            }

            // Sections
            div class="p-4 border border-border bg-card" {
                h2 class="text-lg font-medium mb-2" { "Sections" }
                p class="text-sm text-muted-foreground mb-3" {
                    "4 page-level layouts"
                }
                ul class="text-sm space-y-1" {
                    li { a href="/stories/acp/sections/thread-header" class="text-cyan hover:underline" { "Thread Header" } }
                    li { a href="/stories/acp/sections/thread-feedback" class="text-cyan hover:underline" { "Thread Feedback" } }
                    li { a href="/stories/acp/sections/message-editor" class="text-cyan hover:underline" { "Message Editor" } }
                    li { a href="/stories/acp/sections/thread-view" class="text-cyan hover:underline" { "Thread View" } }
                }
            }
        }

        // Demo link
        div class="mt-8 p-4 border border-border bg-secondary" {
            h2 class="text-lg font-medium mb-2" { "Full Demo" }
            p class="text-sm text-muted-foreground mb-3" {
                "See all components working together in a complete thread view."
            }
            a
                href="/stories/acp/demo"
                class="inline-block px-4 py-2 bg-primary text-primary-foreground text-sm"
            {
                "View Demo"
            }
        }
    }
}
