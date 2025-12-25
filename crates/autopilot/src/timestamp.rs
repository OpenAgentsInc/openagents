//! Central US time utilities for file naming

use chrono::{Datelike, Timelike, Utc};
use chrono_tz::America::Chicago;

/// Get current time in Central US timezone
pub fn now_central() -> chrono::DateTime<chrono_tz::Tz> {
    Utc::now().with_timezone(&Chicago)
}

/// Format date as YYYYMMDD for directory
pub fn date_dir() -> String {
    let ct = now_central();
    format!("{:04}{:02}{:02}", ct.year(), ct.month(), ct.day())
}

/// Format time as HHMMSS for filename prefix (includes seconds to prevent duplicates)
pub fn time_prefix() -> String {
    let ct = now_central();
    format!("{:02}{:02}{:02}", ct.hour(), ct.minute(), ct.second())
}

/// Generate full filename: HHMMSS-slug.ext
pub fn filename(slug: &str, ext: &str) -> String {
    format!("{}-{}.{}", time_prefix(), slug, ext)
}

/// Generate a slug from prompt text (first 4 words, slugified)
pub fn generate_slug(prompt: &str) -> String {
    let words: Vec<&str> = prompt
        .split_whitespace()
        .filter(|w| w.len() > 1) // Skip single chars
        .take(4)
        .collect();

    let slug = words
        .join("-")
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>();

    // Remove consecutive dashes and trim
    let mut result = String::new();
    let mut prev_dash = false;
    for c in slug.chars() {
        if c == '-' {
            if !prev_dash && !result.is_empty() {
                result.push(c);
                prev_dash = true;
            }
        } else {
            result.push(c);
            prev_dash = false;
        }
    }

    // Trim trailing dash
    result.trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_slug() {
        assert_eq!(
            generate_slug("Say hello and list files"),
            "say-hello-and-list"
        );
        assert_eq!(generate_slug("Fix the bug!"), "fix-the-bug");
        assert_eq!(generate_slug("a b c d e f"), ""); // Skips all single chars, returns empty
        assert_eq!(generate_slug("Read README.md"), "read-readme-md");
    }

    #[test]
    fn test_filename() {
        let f = filename("test-slug", "rlog");
        assert!(f.ends_with("-test-slug.rlog"));
        assert_eq!(f.len(), "HHMMSS-test-slug.rlog".len());
    }
}
