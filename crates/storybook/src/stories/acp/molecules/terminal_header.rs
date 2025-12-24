//! Terminal header story.

use maud::{Markup, html};
use ui::acp::molecules::{TerminalHeader, ExitStatus};

pub fn terminal_header_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "Terminal Header" }
        p class="text-sm text-muted-foreground mb-6" {
            "Header for terminal command output with command and exit status."
        }

        div class="space-y-4" {
            div class="p-4 border border-border bg-card" {
                h3 class="text-sm font-medium mb-2" { "Running" }
                (TerminalHeader::new("cargo build --release")
                    .working_dir("/home/user/project")
                    .build())
            }
            div class="p-4 border border-border bg-card" {
                h3 class="text-sm font-medium mb-2" { "Success" }
                (TerminalHeader::new("cargo test")
                    .working_dir("/home/user/project")
                    .exit_status(ExitStatus::Success)
                    .build())
            }
            div class="p-4 border border-border bg-card" {
                h3 class="text-sm font-medium mb-2" { "Failed" }
                (TerminalHeader::new("cargo clippy")
                    .working_dir("/home/user/project")
                    .exit_status(ExitStatus::Failed(1))
                    .build())
            }
        }
    }
}
