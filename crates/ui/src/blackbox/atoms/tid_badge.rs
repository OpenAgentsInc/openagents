use maud::{Markup, html};

fn tid_class(tid: u8) -> &'static str {
    match tid {
        1 => "text-muted-foreground",
        2 => "text-blue",
        3 => "text-green",
        4 => "text-magenta",
        5 => "text-cyan",
        _ => "text-yellow",
    }
}

pub fn tid_badge(tid: u8) -> Markup {
    html! {
        span
            title={ "Thread ID: " (tid) }
            class={ "text-xs cursor-pointer " (tid_class(tid)) }
        {
            "tid:" (tid)
        }
    }
}
