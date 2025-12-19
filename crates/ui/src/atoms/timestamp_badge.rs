use maud::{Markup, html};

pub fn timestamp_badge_elapsed(hours: u8, minutes: u8, seconds: u8) -> Markup {
    html! {
        span
            title="Elapsed time"
            class="text-xs text-muted-foreground opacity-60 cursor-pointer tabular-nums"
        {
            (format!("{:02}:{:02}:{:02}", hours, minutes, seconds))
        }
    }
}

pub fn timestamp_badge_wall(iso_short: &str) -> Markup {
    html! {
        span
            title="Wall clock time"
            class="text-xs text-muted-foreground opacity-60 cursor-pointer tabular-nums"
        {
            (iso_short)
        }
    }
}
