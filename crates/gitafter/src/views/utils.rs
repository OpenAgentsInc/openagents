/// Helper function to extract tag value from event
pub fn get_tag_value(event: &Event, tag_name: &str) -> Option<String> {
    event
        .tags
        .iter()
        .find(|tag| tag.first().map(|t| t == tag_name).unwrap_or(false))
        .and_then(|tag| tag.get(1).cloned())
}

/// Helper function to extract all values for a tag name
fn get_all_tag_values(event: &Event, tag_name: &str) -> Vec<String> {
    event
        .tags
        .iter()
        .filter(|tag| tag.first().map(|t| t == tag_name).unwrap_or(false))
        .filter_map(|tag| tag.get(1).cloned())
        .collect()
}

/// Format a Unix timestamp as relative time (e.g., "2 hours ago", "yesterday")
fn format_relative_time(timestamp: u64) -> String {
    let dt = DateTime::from_timestamp(timestamp as i64, 0).unwrap_or_else(|| Utc::now());
    let now = Utc::now();
    let duration = now.signed_duration_since(dt);

    if duration.num_seconds() < 60 {
        "just now".to_string()
    } else if duration.num_minutes() < 60 {
        let mins = duration.num_minutes();
        format!("{} minute{} ago", mins, if mins == 1 { "" } else { "s" })
    } else if duration.num_hours() < 24 {
        let hours = duration.num_hours();
        format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
    } else if duration.num_days() < 7 {
        let days = duration.num_days();
        if days == 1 {
            "yesterday".to_string()
        } else {
            format!("{} days ago", days)
        }
    } else if duration.num_weeks() < 4 {
        let weeks = duration.num_weeks();
        format!("{} week{} ago", weeks, if weeks == 1 { "" } else { "s" })
    } else {
        // For older dates, show the actual date
        dt.format("%b %d, %Y").to_string()
    }
}

/// Render a single repository card
#[allow(dead_code)]
fn repository_card(event: &Event) -> Markup {
    repository_card_with_bounty_count(event, &std::collections::HashMap::new())
}

fn repository_card_with_bounty_count(
    event: &Event,
    bounty_counts: &std::collections::HashMap<String, usize>,
) -> Markup {
    let name = get_tag_value(event, "name").unwrap_or_else(|| "Unnamed Repository".to_string());
    let description = get_tag_value(event, "description").unwrap_or_default();
    let identifier = get_tag_value(event, "d").unwrap_or_default();
    let has_clone_url = get_tag_value(event, "clone").is_some();
    let has_web_url = get_tag_value(event, "web").is_some();
    let bounty_count = bounty_counts.get(&identifier).copied().unwrap_or(0);

    // Truncate pubkey for display
    let short_pubkey = if event.pubkey.len() > 16 {
        format!(
            "{}...{}",
            &event.pubkey[..8],
            &event.pubkey[event.pubkey.len() - 8..]
        )
    } else {
        event.pubkey.clone()
    };

    html! {
        a.repo-card href={"/repo/" (identifier)} {
            div.repo-header {
                h3.repo-name { (name) }
                span.repo-id { "d:" (identifier) }
            }
            @if !description.is_empty() {
                p.repo-description { (description) }
            }
            div.repo-meta {
                span.repo-author { "by " (short_pubkey) }
                @if has_clone_url {
                    span.repo-clone { "Clone" }
                }
                @if has_web_url {
                    span.repo-web { "View" }
                }
                @if bounty_count > 0 {
                    span style="color: #fbbf24; margin-left: 0.5rem;" { "⚡ " (bounty_count) " bounties" }
                }
            }
        }
    }
}

