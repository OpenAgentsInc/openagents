use maud::{Markup, html};

use super::super::atoms::{blob_ref, result_arrow};

/// Result type for display.
pub enum ResultType {
    Ok,
    Count {
        count: u32,
        unit: String,
    },
    Error(String),
    Blob {
        sha256: String,
        bytes: u64,
        mime: Option<String>,
    },
    Pending,
}

/// Renders result portion after arrow.
pub fn result_display(result: ResultType) -> Markup {
    html! {
        span class="inline-flex items-center gap-1" {
            (result_arrow())
            @match result {
                ResultType::Ok => {
                    span class="text-green" { "[ok]" }
                }
                ResultType::Count { count, unit } => {
                    span class="text-muted-foreground" { "[" (count) " " (unit) "]" }
                }
                ResultType::Error(msg) => {
                    span class="text-red" { "[err: " (msg) "]" }
                }
                ResultType::Blob { sha256, bytes, mime } => {
                    (blob_ref(&sha256, bytes, mime.as_deref()))
                }
                ResultType::Pending => {
                    span class="text-yellow" { "[pending]" }
                }
            }
        }
    }
}
