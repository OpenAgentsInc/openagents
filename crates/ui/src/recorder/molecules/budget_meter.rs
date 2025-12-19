use maud::{Markup, html};

pub fn budget_meter(spent: f64, total: f64) -> Markup {
    let remaining = total - spent;
    let pct = (spent / total * 100.0).min(100.0);
    let filled_blocks = (pct / 10.0).round() as usize;
    let empty_blocks = 10 - filled_blocks;

    let color = if pct < 50.0 {
        "text-green"
    } else if pct < 80.0 {
        "text-yellow"
    } else {
        "text-red"
    };

    html! {
        div class="inline-flex items-center gap-3 text-xs" {
            span class="text-muted-foreground" { "Budget:" }
            span class=(color) {
                @for _ in 0..filled_blocks { "\u{2588}" }
                @for _ in 0..empty_blocks { "\u{2591}" }
            }
            span class="text-muted-foreground" {
                "$" (format!("{:.2}", remaining)) " / $" (format!("{:.2}", total))
            }
        }
    }
}
