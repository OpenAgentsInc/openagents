use serde_json::Value;

pub fn escape_json_string(s: &str) -> String {
    s.replace('\\', "\\\\")
     .replace('\"', "\\\"")
     .replace('\n', "\\n")
     .replace('\r', "\\r")
     .replace('\t', "\\t")
     .replace('\u{0000}', "\\u0000")
     .replace('\u{001F}', "\\u001F")
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
    let mut fixed = json.to_string();
    
    // Fix missing commas between objects
    fixed = fixed.replace("}\n{", "},\n{");
    
    // Fix format!= typo
    fixed = fixed.replace("format!=", "format!");
    
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