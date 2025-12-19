//! Call ID badge story.

use maud::{Markup, html};
use ui::{CallType, call_id_badge};

use super::shared::{code_block, item, row, section, section_title};

pub fn call_id_badge_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "Call ID Badge" }
        p class="text-sm text-muted-foreground mb-6" { "Colored identifier for tool, MCP, and subagent calls." }

        (section_title("Variants"))
        (section(row(html! {
            (item("Tool call", call_id_badge("call_47", CallType::Tool)))
            (item("MCP call", call_id_badge("call_13", CallType::Mcp)))
            (item("Subagent", call_id_badge("sub_1", CallType::Subagent)))
        })))

        (section_title("Usage"))
        (code_block(r#"use ui::{CallType, call_id_badge};

call_id_badge("call_47", CallType::Tool)
call_id_badge("call_13", CallType::Mcp)
call_id_badge("sub_1", CallType::Subagent)"#))
    }
}
