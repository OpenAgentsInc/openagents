use maud::{Markup, html};

fn cost_class(cost: f64) -> &'static str {
    if cost < 0.01 {
        "text-green"
    } else if cost < 0.10 {
        "text-yellow"
    } else {
        "text-red"
    }
}

pub fn cost_badge(cost: f64) -> Markup {
    html! {
        span
            title={ "Cost: $" (format!("{:.4}", cost)) }
            class={ "text-xs tabular-nums " (cost_class(cost)) }
        {
            "$" (format!("{:.4}", cost))
        }
    }
}
