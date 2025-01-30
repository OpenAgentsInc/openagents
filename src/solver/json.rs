use anyhow::Result;

/// Escapes special characters in JSON strings
pub fn escape_json_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
        .replace('\u{0000}', "\\u0000")
        .replace('\u{001F}', "\\u001F")
}

/// Checks if a string is valid JSON
pub fn is_valid_json_string(s: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(s).is_ok()
}

/// Fixes common JSON formatting issues
pub fn fix_common_json_issues(json_str: &str) -> String {
    json_str
        .replace("\\\\n", "\\n") // Fix double-escaped newlines
        .replace("\\\\\"", "\\\"") // Fix double-escaped quotes
        .replace("\\\\\\\\", "\\\\") // Fix double-escaped backslashes
        .replace("format!=", "format!") // Fix common format macro typo
        .replace("\n", "\\n") // Escape actual newlines
        .replace("\"", "\\\"") // Escape actual quotes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_json_string() {
        let input = "line1\nline2\t\"quoted\"";
        let escaped = escape_json_string(input);
        assert!(escaped.contains("\\n"));
        assert!(escaped.contains("\\t"));
        assert!(escaped.contains("\\\""));
    }

    #[test]
    fn test_is_valid_json_string() {
        assert!(is_valid_json_string("{\"key\": \"value\"}"));
        assert!(!is_valid_json_string("invalid json"));
    }

    #[test]
    fn test_fix_common_json_issues() {
        let input = "{\n\"key\": \"value\"\n},\n{\"key2\": \"value2\"}";
        let fixed = fix_common_json_issues(input);
        assert_eq!(
            fixed,
            "{\\n\\\"key\\\": \\\"value\\\"\\n},\\n{\\\"key2\\\": \\\"value2\\\"}"
        );
    }
}