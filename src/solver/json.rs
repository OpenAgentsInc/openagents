use anyhow::Result;

/// Escapes special characters in a string for JSON
pub fn escape_json_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Checks if a string is valid JSON
pub fn is_valid_json_string(s: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(s).is_ok()
}

/// Attempts to fix common JSON formatting issues
pub fn fix_common_json_issues(s: &str) -> Result<String> {
    let mut result = s.to_string();

    // Replace single quotes with double quotes
    result = result.replace('\'', "\"");

    // Add quotes around unquoted keys
    let re = regex::Regex::new(r#"(\{|\,)\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*:"#)?;
    result = re.replace_all(&result, "$1\"$2\":").to_string();

    // Fix trailing commas
    let re = regex::Regex::new(r",\s*([}\]])")?;
    result = re.replace_all(&result, "$1").to_string();

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_json_string() {
        assert_eq!(escape_json_string(r#"test"test"#), r#"test\"test"#);
        assert_eq!(escape_json_string("test\ntest"), r#"test\ntest"#);
        assert_eq!(escape_json_string(r#"test\test"#), r#"test\\test"#);
    }

    #[test]
    fn test_is_valid_json_string() {
        assert!(is_valid_json_string(r#"{"test": "value"}"#));
        assert!(is_valid_json_string(r#"[1, 2, 3]"#));
        assert!(!is_valid_json_string(r#"{"test": value}"#));
        assert!(!is_valid_json_string("invalid"));
    }

    #[test]
    fn test_fix_common_json_issues() {
        // Test fixing unquoted keys
        let input = r#"{test: "value"}"#;
        let expected = r#"{"test":"value"}"#;
        assert_eq!(fix_common_json_issues(input).unwrap(), expected);

        // Test fixing single quotes
        let input = r#"{'test': 'value'}"#;
        let expected = r#"{"test": "value"}"#;
        assert_eq!(fix_common_json_issues(input).unwrap(), expected);

        // Test fixing trailing commas
        let input = r#"{"test": "value",}"#;
        let expected = r#"{"test": "value"}"#;
        assert_eq!(fix_common_json_issues(input).unwrap(), expected);
    }
}