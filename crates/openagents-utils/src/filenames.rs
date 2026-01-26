//! Filename sanitization helpers.

const INVALID_CHARS: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|', '\0'];
const DEFAULT_MAX_LEN: usize = 100;

/// Sanitize a title string into a valid filename.
pub fn sanitize_filename(title: &str) -> Option<String> {
    let sanitized: String = title
        .chars()
        .map(|c| if INVALID_CHARS.contains(&c) { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    let trimmed = if sanitized.len() > DEFAULT_MAX_LEN {
        sanitized[..DEFAULT_MAX_LEN].trim_end().to_string()
    } else {
        sanitized
    };

    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Sanitize identifiers used as filename stems, preserving simple tokens.
pub fn sanitize_filename_simple(name: &str) -> String {
    name.replace("::", "_").replace(':', "_").replace('/', "_")
}
