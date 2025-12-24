//! Message header story.

use maud::{Markup, html};
use ui::acp::atoms::EntryKind;
use ui::acp::molecules::MessageHeader;

pub fn message_header_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "Message Header" }
        p class="text-sm text-muted-foreground mb-6" {
            "Header for user/assistant messages with entry marker and actions."
        }

        div class="space-y-4" {
            div class="p-4 border border-border bg-card" {
                h3 class="text-sm font-medium mb-2" { "User Message" }
                (MessageHeader::new(EntryKind::User, "entry-1")
                    .timestamp("2:30 PM")
                    .editable()
                    .build())
            }
            div class="p-4 border border-border bg-card" {
                h3 class="text-sm font-medium mb-2" { "Assistant Message" }
                (MessageHeader::new(EntryKind::Assistant, "entry-2")
                    .timestamp("2:31 PM")
                    .build())
            }
            div class="p-4 border border-border bg-card" {
                h3 class="text-sm font-medium mb-2" { "Tool Message" }
                (MessageHeader::new(EntryKind::Tool, "entry-3")
                    .build())
            }
        }
    }
}
