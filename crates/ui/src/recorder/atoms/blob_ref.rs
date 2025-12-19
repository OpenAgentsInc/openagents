use maud::{Markup, html};

pub fn blob_ref(sha256: &str, bytes: u64, mime: Option<&str>) -> Markup {
    let size_display = if bytes >= 1024 * 1024 {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{}B", bytes)
    };

    let sha_short = &sha256[..8.min(sha256.len())];

    html! {
        span
            title={ "Blob: " (sha256) }
            class="inline-flex items-center gap-2 border border-border bg-card text-cyan text-xs px-2 py-0.5 cursor-pointer"
        {
            "@blob sha256=" (sha_short)
            "\u{00B7}"
            (size_display)
            @if let Some(m) = mime {
                "\u{00B7}"
                (m)
            }
        }
    }
}
