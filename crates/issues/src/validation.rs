//! Input validation for issue data
//!
//! Validates user-provided data to prevent invalid states and ensure data quality.

use thiserror::Error;

/// Maximum length for issue title
pub const MAX_TITLE_LENGTH: usize = 200;

/// Maximum length for issue description
pub const MAX_DESCRIPTION_LENGTH: usize = 10_000;

/// Validation error types
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("Title cannot be empty")]
    TitleEmpty,

    #[error("Title cannot exceed {MAX_TITLE_LENGTH} characters (got {0})")]
    TitleTooLong(usize),

    #[error("Title cannot have leading or trailing whitespace")]
    TitleHasWhitespace,

    #[error("Description cannot exceed {MAX_DESCRIPTION_LENGTH} characters (got {0})")]
    DescriptionTooLong(usize),

    #[error("Agent name must be 'claude' or 'codex' (got '{0}')")]
    InvalidAgent(String),

    #[error("Directive ID '{0}' does not exist")]
    DirectiveNotFound(String),
}

/// Validate issue title
///
/// Requirements:
/// - Non-empty
/// - Max 200 characters
/// - No leading/trailing whitespace
pub fn validate_title(title: &str) -> Result<String, ValidationError> {
    let trimmed = title.trim();

    if trimmed.is_empty() {
        return Err(ValidationError::TitleEmpty);
    }

    if trimmed.len() > MAX_TITLE_LENGTH {
        return Err(ValidationError::TitleTooLong(trimmed.len()));
    }

    if trimmed != title {
        return Err(ValidationError::TitleHasWhitespace);
    }

    Ok(trimmed.to_string())
}

/// Validate issue description
///
/// Requirements:
/// - Max 10,000 characters
pub fn validate_description(description: Option<&str>) -> Result<Option<String>, ValidationError> {
    if let Some(desc) = description {
        if desc.len() > MAX_DESCRIPTION_LENGTH {
            return Err(ValidationError::DescriptionTooLong(desc.len()));
        }
        Ok(Some(desc.to_string()))
    } else {
        Ok(None)
    }
}

/// Validate agent name
///
/// Requirements:
/// - Must be "claude" or "codex"
pub fn validate_agent(agent: &str) -> Result<String, ValidationError> {
    match agent {
        "claude" | "codex" => Ok(agent.to_string()),
        other => Err(ValidationError::InvalidAgent(other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // Title validation tests
    #[test]
    fn test_valid_title() {
        let result = validate_title("Fix authentication bug");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Fix authentication bug");
    }

    #[test]
    fn test_empty_title() {
        let result = validate_title("");
        assert_eq!(result, Err(ValidationError::TitleEmpty));
    }

    #[test]
    fn test_whitespace_only_title() {
        let result = validate_title("   ");
        assert_eq!(result, Err(ValidationError::TitleEmpty));
    }

    #[test]
    fn test_title_too_long() {
        let long_title = "a".repeat(201);
        let result = validate_title(&long_title);
        assert!(matches!(result, Err(ValidationError::TitleTooLong(201))));
    }

    #[test]
    fn test_title_max_length() {
        let title = "a".repeat(200);
        let result = validate_title(&title);
        assert!(result.is_ok());
    }

    #[test]
    fn test_title_leading_whitespace() {
        let result = validate_title("  Fix bug");
        assert_eq!(result, Err(ValidationError::TitleHasWhitespace));
    }

    #[test]
    fn test_title_trailing_whitespace() {
        let result = validate_title("Fix bug  ");
        assert_eq!(result, Err(ValidationError::TitleHasWhitespace));
    }

    #[test]
    fn test_title_with_unicode() {
        let result = validate_title("修复认证错误");
        assert!(result.is_ok());
    }

    // Description validation tests
    #[test]
    fn test_valid_description() {
        let result = validate_description(Some("This is a test description"));
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            Some("This is a test description".to_string())
        );
    }

    #[test]
    fn test_none_description() {
        let result = validate_description(None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_description_too_long() {
        let long_desc = "a".repeat(10_001);
        let result = validate_description(Some(&long_desc));
        assert!(matches!(
            result,
            Err(ValidationError::DescriptionTooLong(10_001))
        ));
    }

    #[test]
    fn test_description_max_length() {
        let desc = "a".repeat(10_000);
        let result = validate_description(Some(&desc));
        assert!(result.is_ok());
    }

    #[test]
    fn test_description_with_newlines() {
        let result = validate_description(Some("Line 1\nLine 2\nLine 3"));
        assert!(result.is_ok());
    }

    // Agent validation tests
    #[test]
    fn test_valid_agent_claude() {
        let result = validate_agent("claude");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "claude");
    }

    #[test]
    fn test_valid_agent_codex() {
        let result = validate_agent("codex");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "codex");
    }

    #[test]
    fn test_invalid_agent() {
        let result = validate_agent("gpt4");
        assert_eq!(
            result,
            Err(ValidationError::InvalidAgent("gpt4".to_string()))
        );
    }

    #[test]
    fn test_invalid_agent_empty() {
        let result = validate_agent("");
        assert_eq!(
            result,
            Err(ValidationError::InvalidAgent("".to_string()))
        );
    }

    #[test]
    fn test_invalid_agent_case_sensitive() {
        // Agent names are case-sensitive
        let result = validate_agent("Claude");
        assert_eq!(
            result,
            Err(ValidationError::InvalidAgent("Claude".to_string()))
        );
    }

    // Property-based tests
    proptest! {
        #[test]
        fn prop_title_rejects_empty_and_whitespace(s in "\\s*") {
            let result = validate_title(&s);
            prop_assert!(result.is_err());
            prop_assert!(matches!(result, Err(ValidationError::TitleEmpty)));
        }

        #[test]
        fn prop_title_rejects_too_long(s in "[a-z]{201,300}") {
            let result = validate_title(&s);
            prop_assert!(result.is_err());
            prop_assert!(matches!(result, Err(ValidationError::TitleTooLong(_))));
        }

        #[test]
        fn prop_title_accepts_valid_length(s in "[a-zA-Z0-9 ]{1,200}") {
            let trimmed = s.trim();
            if !trimmed.is_empty() && trimmed == s.as_str() {
                let result = validate_title(&s);
                prop_assert!(result.is_ok());
                prop_assert_eq!(result.unwrap(), s);
            }
        }

        #[test]
        fn prop_title_rejects_leading_whitespace(s in "\\s+[a-z]+") {
            let result = validate_title(&s);
            prop_assert!(result.is_err());
            prop_assert!(matches!(result, Err(ValidationError::TitleHasWhitespace)));
        }

        #[test]
        fn prop_title_rejects_trailing_whitespace(s in "[a-z]+\\s+") {
            let result = validate_title(&s);
            prop_assert!(result.is_err());
            prop_assert!(matches!(result, Err(ValidationError::TitleHasWhitespace)));
        }

        #[test]
        fn prop_description_accepts_valid_length(s in "[a-zA-Z0-9 \\n]{0,10000}") {
            let result = validate_description(Some(&s));
            prop_assert!(result.is_ok());
            prop_assert_eq!(result.unwrap(), Some(s));
        }

        #[test]
        fn prop_description_rejects_too_long(s in "[a-z]{10001,10100}") {
            let result = validate_description(Some(&s));
            prop_assert!(result.is_err());
            prop_assert!(matches!(result, Err(ValidationError::DescriptionTooLong(_))));
        }

        #[test]
        fn prop_agent_accepts_only_valid_names(s in "[a-z]{1,20}") {
            let result = validate_agent(&s);
            if s == "claude" || s == "codex" {
                prop_assert!(result.is_ok());
                prop_assert_eq!(result.unwrap(), s);
            } else {
                prop_assert!(result.is_err());
                prop_assert!(matches!(result, Err(ValidationError::InvalidAgent(_))));
            }
        }
    }

    #[test]
    fn test_description_none_handling() {
        let result = validate_description(None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }
}
