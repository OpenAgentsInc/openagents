//! Output formatting utilities

use crate::OutputFormat;
use colored::Colorize;
use serde::Serialize;

/// Format and print output based on format type
pub fn print_output<T: Serialize + std::fmt::Display>(value: &T, format: OutputFormat) {
    match format {
        OutputFormat::Text => println!("{}", value),
        OutputFormat::Json => {
            if let Ok(json) = serde_json::to_string_pretty(value) {
                println!("{}", json);
            }
        }
    }
}

/// Print a success message
pub fn print_success(msg: &str) {
    println!("{} {}", "✓".green(), msg);
}

/// Print an error message
pub fn print_error(msg: &str) {
    eprintln!("{} {}", "✗".red(), msg);
}

/// Print a warning message
pub fn print_warning(msg: &str) {
    println!("{} {}", "!".yellow(), msg);
}

/// Print an info message
pub fn print_info(msg: &str) {
    println!("{} {}", "ℹ".blue(), msg);
}

/// Format a task status with color
pub fn format_status(status: &str) -> String {
    match status {
        "ready" => status.green().to_string(),
        "in_progress" => status.yellow().to_string(),
        "blocked" => status.red().to_string(),
        "done" => status.cyan().to_string(),
        _ => status.to_string(),
    }
}

/// Format a priority with color
pub fn format_priority(priority: &str) -> String {
    match priority {
        "P0" => priority.red().bold().to_string(),
        "P1" => priority.yellow().to_string(),
        "P2" => priority.blue().to_string(),
        _ => priority.normal().to_string(),
    }
}

/// Format duration in human-readable form
pub fn format_duration(seconds: u64) -> String {
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    }
}

/// Format a timestamp
pub fn format_timestamp(ts: &chrono::DateTime<chrono::Utc>) -> String {
    ts.format("%Y-%m-%d %H:%M").to_string()
}

/// Truncate string with ellipsis
pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len <= 3 {
        s[..max_len].to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(30), "30s");
        assert_eq!(format_duration(90), "1m 30s");
        assert_eq!(format_duration(3661), "1h 1m");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 8), "hello...");
        assert_eq!(truncate("hi", 2), "hi");
    }
}
