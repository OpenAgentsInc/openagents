use maud::{Markup, html};

fn latency_class(ms: u32) -> &'static str {
    if ms < 1000 {
        "text-green"
    } else if ms < 5000 {
        "text-yellow"
    } else {
        "text-red"
    }
}

pub fn latency_badge(ms: u32) -> Markup {
    let display = if ms >= 1000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        format!("{}ms", ms)
    };

    html! {
        span
            title={ "Latency: " (ms) "ms" }
            class={ "text-xs tabular-nums " (latency_class(ms)) }
        {
            (display)
        }
    }
}
