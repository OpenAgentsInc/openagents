use maud::{Markup, html};

pub fn phase_line(phase_name: &str) -> Markup {
    let color = match phase_name.to_lowercase().as_str() {
        "explore" => "text-cyan",
        "design" => "text-blue",
        "review" => "text-yellow",
        "final" => "text-green",
        "exit" => "text-muted-foreground",
        _ => "text-muted-foreground",
    };

    html! {
        div class="flex items-center gap-2 py-4 my-2" {
            span class=(color) { "\u{25D0}" }
            span class="flex-1 border-b border-border" {}
            span class={ "text-xs font-semibold tracking-widest uppercase " (color) } {
                (phase_name)
            }
            span class="flex-1 border-b border-border" {}
        }
    }
}
