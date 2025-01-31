use serde_json::Value;

pub fn escape_json_string(s: &str) -> String {
    s.replace('\"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

pub fn is_valid_json_string(s: &str) -> bool {
    if let Ok(Value::String(_)) = serde_json::from_str(&format!("\"{}\"", s)) {
        true
    } else {
        false
    }
}

pub fn fix_common_json_issues(json: &str) -> String {
    // Fix unescaped newlines in strings
    let mut fixed = json.replace("\"\n", "\"\\n");

    // Fix unescaped quotes in strings
    fixed = fixed
        .replace("\\\"", "TEMP_QUOTE")
        .replace("\"", "\\\"")
        .replace("TEMP_QUOTE", "\\\"");

    // Fix missing commas between objects
    fixed = fixed.replace("}\n{", "},\n{");

    fixed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_json_string() {
        assert_eq!(escape_json_string("hello\n"), "hello\\n");
        assert_eq!(escape_json_string("\"quote\""), "\\\"quote\\\"");
        assert_eq!(escape_json_string("tab\there"), "tab\\there");
    }

    #[test]
    fn test_is_valid_json_string() {
        assert!(is_valid_json_string("hello"));
        assert!(is_valid_json_string("hello\\n"));
        assert!(!is_valid_json_string("hello\""));
    }

    #[test]
    fn test_fix_common_json_issues() {
        let input = "{\n\"key\": \"value\"\n}\n{\"key2\": \"value2\"}";
        let expected = "{\n\"key\": \"value\"\n},\n{\"key2\": \"value2\"}";
        assert_eq!(fix_common_json_issues(input), expected);
    }
}
