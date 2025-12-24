//! Mode selector story.

use maud::{Markup, html};
use ui::acp::atoms::AgentMode;
use ui::acp::molecules::ModeSelector;

pub fn mode_selector_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "Mode Selector" }
        p class="text-sm text-muted-foreground mb-6" {
            "Dropdown for selecting agent operating mode."
        }

        div class="space-y-4" {
            div class="p-4 border border-border bg-card" {
                h3 class="text-sm font-medium mb-2" { "Code Mode" }
                (ModeSelector::new(AgentMode::Code, "session-1").build())
            }
            div class="p-4 border border-border bg-card" {
                h3 class="text-sm font-medium mb-2" { "Plan Mode" }
                (ModeSelector::new(AgentMode::Plan, "session-2").build())
            }
            div class="p-4 border border-border bg-card" {
                h3 class="text-sm font-medium mb-2" { "Ask Mode" }
                (ModeSelector::new(AgentMode::Ask, "session-3").build())
            }
        }
    }
}
