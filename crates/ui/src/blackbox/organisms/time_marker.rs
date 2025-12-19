use maud::{Markup, html};

pub fn time_marker(hours: u8, minutes: u8, seconds: u8) -> Markup {
    html! {
        div class="flex items-center gap-2 py-3 my-2 cursor-pointer" title="Click to copy" {
            span class="flex-1 border-b border-dashed border-border" {}
            span class="text-xs text-muted-foreground" {
                (format!("{:02}:{:02}:{:02}", hours, minutes, seconds))
            }
            span class="flex-1 border-b border-dashed border-border" {}
        }
    }
}
